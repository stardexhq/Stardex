pub mod contract_store;
pub mod cursor_store;
pub mod event_store;
pub mod pool;
pub mod postgres_event_store;
pub mod postgres_store;
pub mod rpc;
pub mod rpc_client;
pub mod sink;
pub mod supervisor;

pub use contract_store::{ContractStore, InMemoryContractStore, PostgresContractStore};
pub use cursor_store::{CursorStore, InMemoryCursorStore};
pub use event_store::{EventStore, InMemoryEventStore, StoredEvent};
pub use pool::connect_pool;
pub use postgres_event_store::PostgresEventStore;
pub use postgres_store::PostgresCursorStore;
pub use sink::{EventSink, PrintSink};
pub use supervisor::{IngestorFactory, Supervisor};

pub use sqlx::PgPool;

/// A raw contract event from RPC, before decoding. Topics/data are base64 XDR.
#[derive(Debug, Clone)]
pub struct RawEvent {
    pub ledger: u32,
    pub contract_id: String,
    pub topics: Vec<String>,
    pub data: String,
    /// Ledger close time as RFC3339, from RPC's `ledgerClosedAt`.
    pub closed_at: String,
}

impl From<rpc::RpcEvent> for RawEvent {
    fn from(e: rpc::RpcEvent) -> Self {
        // `id` is dropped: it's the stream cursor, tracked by the ingestor.
        RawEvent {
            ledger: e.ledger,
            contract_id: e.contract_id,
            topics: e.topic,
            data: e.value,
            closed_at: e.ledger_closed_at,
        }
    }
}

/// Position in the event stream, so ingestion can resume after a restart.
#[derive(Debug, Clone, Default)]
pub struct Cursor {
    pub last_ledger: u32,
    pub last_event_id: Option<String>,
}

const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
const INITIAL_BACKOFF: std::time::Duration = std::time::Duration::from_secs(1);
const MAX_BACKOFF: std::time::Duration = std::time::Duration::from_secs(30);

/// The ingestion engine.
pub struct Ingestor {
    rpc_url: String,
    cursor: Cursor,
    store: Box<dyn CursorStore>,
    sink: Box<dyn EventSink>,
}

impl Ingestor {
    /// Ingestor whose cursor lives only in memory (does not survive a restart).
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self::with_store(rpc_url, Box::new(InMemoryCursorStore::default()))
    }

    /// Ingestor backed by a specific [`CursorStore`] (e.g. Postgres). Events are
    /// printed until an [`EventSink`] is set via [`Ingestor::with_event_sink`].
    pub fn with_store(rpc_url: impl Into<String>, store: Box<dyn CursorStore>) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            cursor: Cursor::default(),
            store,
            sink: Box::new(PrintSink),
        }
    }

    /// Route each streamed event through `sink` (e.g. decode-and-store).
    pub fn with_event_sink(mut self, sink: Box<dyn EventSink>) -> Self {
        self.sink = sink;
        self
    }

    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    /// Load the saved cursor for `stream` so a restart resumes from it.
    pub async fn restore_cursor(&mut self, stream: &str) -> Result<(), IngestError> {
        if let Some(saved) = self.store.load(stream).await? {
            self.cursor = saved;
        }
        Ok(())
    }

    /// Connect to RPC and continuously stream events for `contract_id`, polling
    /// every [`POLL_INTERVAL`] once caught up. Runs until cancelled.
    pub async fn index_contract(&mut self, contract_id: &str) -> Result<(), IngestError> {
        self.run(contract_id, true).await
    }

    /// Stream events for `contract_id` until caught up to the tip, then return.
    /// Suitable for scheduled / one-shot jobs (e.g. a cron worker that wakes,
    /// catches up, and exits).
    pub async fn catch_up(&mut self, contract_id: &str) -> Result<(), IngestError> {
        self.run(contract_id, false).await
    }

    /// Shared streaming loop. When `continuous` is true it polls forever; when
    /// false it returns as soon as it reaches the tip (a page with no events).
    async fn run(&mut self, contract_id: &str, continuous: bool) -> Result<(), IngestError> {
        let client = rpc_client::RpcClient::new(self.rpc_url.clone());

        self.restore_cursor(contract_id).await?;

        // With no saved cursor, start from the current tip to capture new events.
        let mut start_ledger = match &self.cursor.last_event_id {
            Some(_) => None,
            None if self.cursor.last_ledger > 0 => Some(self.cursor.last_ledger),
            None => {
                let tip = client.latest_ledger().await?;
                println!("starting from current ledger {tip}");
                Some(tip)
            }
        };

        loop {
            let cursor = self.cursor.last_event_id.clone();
            let page = match self
                .fetch_page_with_retry(&client, contract_id, start_ledger, cursor)
                .await
            {
                Ok(page) => page,
                // The cursor fell behind the RPC's retention window (it prunes old
                // ledgers). Skip ahead to the oldest ledger it still serves and
                // resume; events before that are gone from this RPC, see backfill.
                Err(err) => match retention_floor(&err) {
                    Some(floor) => {
                        eprintln!(
                            "stardex: {contract_id} cursor is behind the RPC retention \
                             window; skipping ahead to ledger {floor} (earlier events are \
                             no longer served by this RPC)"
                        );
                        self.cursor.last_ledger = floor;
                        self.cursor.last_event_id = None;
                        self.store.save(contract_id, &self.cursor).await?;
                        start_ledger = Some(floor);
                        continue;
                    }
                    None => return Err(err),
                },
            };
            // RPC rejects sending both start_ledger and a cursor; the cursor wins.
            start_ledger = None;

            let received = page.events.len();
            let mut last_event_id = None;
            for event in page.events {
                last_event_id = Some(event.id.clone());
                let raw: RawEvent = event.into();
                self.cursor.last_ledger = raw.ledger;
                self.sink.handle(raw).await?;
            }

            // Fall back to the last event id if the page omits a top-level cursor.
            if let Some(next) = page.cursor.or(last_event_id) {
                self.cursor.last_event_id = Some(next);
            }

            self.store.save(contract_id, &self.cursor).await?;

            if received == 0 {
                if !continuous {
                    return Ok(());
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        }
    }

    /// Fetch one page, retrying transient transport errors with capped
    /// exponential backoff; other errors propagate.
    async fn fetch_page_with_retry(
        &self,
        client: &rpc_client::RpcClient,
        contract_id: &str,
        start_ledger: Option<u32>,
        cursor: Option<String>,
    ) -> Result<rpc::GetEventsResult, IngestError> {
        let mut backoff = INITIAL_BACKOFF;
        loop {
            match client
                .get_events(contract_id, start_ledger, cursor.clone())
                .await
            {
                Ok(page) => return Ok(page),
                Err(IngestError::Http(e)) => {
                    eprintln!("stardex: transient RPC error ({e}); retrying in {backoff:?}");
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(MAX_BACKOFF);
                }
                Err(fatal) => return Err(fatal),
            }
        }
    }
}

/// If `err` is the RPC "startLedger must be within the ledger range" rejection,
/// return the oldest ledger the RPC still serves (the window floor) so ingestion
/// can skip its stale cursor forward to it.
fn retention_floor(err: &IngestError) -> Option<u32> {
    let IngestError::Rpc { message, .. } = err else {
        return None;
    };
    parse_retention_range(message).map(|(floor, _tip)| floor)
}

/// Pull `(min, max)` out of a "... ledger range: <min> - <max>" message.
fn parse_retention_range(message: &str) -> Option<(u32, u32)> {
    let range = message.split_once("ledger range:")?.1;
    let (min, max) = range.split_once('-')?;
    Some((min.trim().parse().ok()?, max.trim().parse().ok()?))
}

#[derive(Debug)]
pub enum IngestError {
    Http(reqwest::Error),
    Rpc { code: i64, message: String },
    EmptyResponse,
    Store(String),
}

impl std::fmt::Display for IngestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IngestError::Http(e) => write!(f, "rpc transport error: {e}"),
            IngestError::Rpc { code, message } => write!(f, "rpc error {code}: {message}"),
            IngestError::EmptyResponse => write!(f, "rpc returned neither result nor error"),
            IngestError::Store(msg) => write!(f, "cursor store error: {msg}"),
        }
    }
}

impl std::error::Error for IngestError {}

impl From<reqwest::Error> for IngestError {
    fn from(e: reqwest::Error) -> Self {
        IngestError::Http(e)
    }
}

impl From<sqlx::Error> for IngestError {
    fn from(e: sqlx::Error) -> Self {
        IngestError::Store(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_ingestor_starts_at_genesis_cursor() {
        let ing = Ingestor::new("https://soroban-testnet.stellar.org");
        assert_eq!(ing.cursor().last_ledger, 0);
        assert_eq!(ing.rpc_url(), "https://soroban-testnet.stellar.org");
    }

    #[test]
    fn retention_floor_reads_the_window_start() {
        let err = IngestError::Rpc {
            code: -32600,
            message: "startLedger must be within the ledger range: 3524293 - 3645252".into(),
        };
        assert_eq!(retention_floor(&err), Some(3524293));
    }

    #[test]
    fn retention_floor_ignores_unrelated_errors() {
        let err = IngestError::Rpc {
            code: -32602,
            message: "filter 1 invalid: contract ID 1 invalid".into(),
        };
        assert_eq!(retention_floor(&err), None);
        assert_eq!(retention_floor(&IngestError::EmptyResponse), None);
    }

    #[tokio::test]
    async fn restores_saved_cursor_on_startup() {
        let store = Box::new(InMemoryCursorStore::default());
        store
            .save(
                "CABC",
                &Cursor {
                    last_ledger: 4242,
                    last_event_id: Some("0000004242-0000000007".into()),
                },
            )
            .await
            .unwrap();

        let mut ing = Ingestor::with_store("https://rpc.example", store);
        assert_eq!(ing.cursor().last_ledger, 0);

        ing.restore_cursor("CABC").await.unwrap();
        assert_eq!(ing.cursor().last_ledger, 4242);
        assert_eq!(
            ing.cursor().last_event_id.as_deref(),
            Some("0000004242-0000000007")
        );
    }

    #[tokio::test]
    async fn restore_is_noop_for_unknown_stream() {
        let mut ing = Ingestor::new("https://rpc.example");
        ing.restore_cursor("never-seen").await.unwrap();
        assert_eq!(ing.cursor().last_ledger, 0);
        assert!(ing.cursor().last_event_id.is_none());
    }
}
