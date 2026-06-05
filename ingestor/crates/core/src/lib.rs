//! Stardex ingestion **core** — the neutral engine.
//!
//! It connects to Stellar RPC, pulls raw contract events, and remembers how
//! far it has progressed (the "cursor") so it can resume after a restart.
//! It deliberately knows *nothing* about what any event means — that's the
//! job of the decoders crate.

pub mod rpc;
pub mod rpc_client;

/// A raw contract event pulled from RPC, before it has been decoded.
#[derive(Debug, Clone)]
pub struct RawEvent {
    /// Ledger (block) sequence the event was emitted in.
    pub ledger: u32,
    /// Contract that emitted the event.
    pub contract_id: String,
    /// Event topics (XDR/base64 placeholders for now).
    pub topics: Vec<String>,
    /// Event payload (XDR/base64 placeholder for now).
    pub data: String,
}

/// Translate an RPC-shaped event into our neutral [`RawEvent`].
///
/// This is the seam that keeps RPC's wire format from leaking into the rest of
/// Stardex. The RPC event's `id` is intentionally dropped here — it's used as
/// the stream cursor, tracked separately by the ingestor, not per-event.
impl From<rpc::RpcEvent> for RawEvent {
    fn from(e: rpc::RpcEvent) -> Self {
        RawEvent {
            ledger: e.ledger,
            contract_id: e.contract_id,
            topics: e.topic,
            data: e.value,
        }
    }
}

/// Position in the event stream, so ingestion can resume where it left off.
#[derive(Debug, Clone, Default)]
pub struct Cursor {
    pub last_ledger: u32,
    pub last_event_id: Option<String>,
}

/// How long to wait before polling again once we've caught up to the chain tip.
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

/// The ingestion engine.
///
/// TODO(#2): persist the cursor to Postgres so it resumes after a restart.
pub struct Ingestor {
    rpc_url: String,
    cursor: Cursor,
}

impl Ingestor {
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            cursor: Cursor::default(),
        }
    }

    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    /// Connect to RPC and continuously stream events for `contract_id`.
    ///
    /// This runs forever: it pages through events as fast as the chain
    /// produces them, and once caught up it polls every [`POLL_INTERVAL`] for
    /// new ones. Stop it with Ctrl-C. Each event is handed to
    /// [`Self::handle_event`] — for now that just prints it; decoding (#6) and
    /// storage (#13) will plug in there.
    pub async fn index_contract(&mut self, contract_id: &str) -> Result<(), IngestError> {
        let client = rpc_client::RpcClient::new(self.rpc_url.clone());

        // Where to begin. With no saved cursor, start from the current tip so
        // we capture *new* events going forward.
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
            let page = client.get_events(contract_id, start_ledger, cursor).await?;
            // After the first request the cursor carries our position; never
            // send start_ledger again (RPC rejects sending both).
            start_ledger = None;

            let received = page.events.len();
            let mut last_event_id = None;
            for event in page.events {
                last_event_id = Some(event.id.clone());
                let raw: RawEvent = event.into();
                self.cursor.last_ledger = raw.ledger;
                self.handle_event(raw);
            }

            // Advance our resume bookmark to the end of this page. Prefer the
            // server's page cursor; fall back to the last event's id so we never
            // lose our place if a non-empty page omits the top-level cursor.
            if let Some(next) = page.cursor.or(last_event_id) {
                self.cursor.last_event_id = Some(next);
            }

            // No events means we've reached the chain tip — wait for new ledgers.
            if received == 0 {
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        }
    }

    /// Sink for one decoded-not-yet raw event. Temporary: prints a one-line
    /// summary. TODO(#6/#13): run it through the decoder registry and persist.
    fn handle_event(&self, event: RawEvent) {
        println!(
            "event @ ledger {} from {} — topics={:?}",
            event.ledger, event.contract_id, event.topics
        );
    }
}

#[derive(Debug)]
pub enum IngestError {
    /// Network/transport failure talking to RPC (connection, timeout, bad body).
    Http(reqwest::Error),
    /// RPC accepted the call but returned a JSON-RPC error (e.g. bad contract).
    Rpc { code: i64, message: String },
    /// RPC reply had neither a `result` nor an `error` — should never happen.
    EmptyResponse,
}

impl std::fmt::Display for IngestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IngestError::Http(e) => write!(f, "rpc transport error: {e}"),
            IngestError::Rpc { code, message } => write!(f, "rpc error {code}: {message}"),
            IngestError::EmptyResponse => write!(f, "rpc returned neither result nor error"),
        }
    }
}

impl std::error::Error for IngestError {}

impl From<reqwest::Error> for IngestError {
    fn from(e: reqwest::Error) -> Self {
        IngestError::Http(e)
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
}
