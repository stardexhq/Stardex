//! Runs many per-contract ingestors at once. Each contract gets its own task
//! and its own cursor, so one contract's failure never stalls the others.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;

use crate::{ContractStore, CursorStore, EventSink, Ingestor};

/// Builds the collaborators for one indexing task. Called once per contract so
/// each task owns a fresh cursor store and sink over a shared backend (e.g. a
/// connection pool). Kept decoder-agnostic: the CLI supplies the real wiring.
pub trait IngestorFactory: Send + Sync + 'static {
    fn cursor_store(&self) -> Box<dyn CursorStore>;
    fn sink(&self) -> Box<dyn EventSink>;
}

/// How long to wait before restarting a contract's task after it errors.
const DEFAULT_RESTART_BACKOFF: Duration = Duration::from_secs(5);

/// How often to re-read the registry to notice added or removed contracts.
const DEFAULT_RELOAD_INTERVAL: Duration = Duration::from_secs(10);

/// Supervises one indexing task per registered contract, restarting any that
/// fail and following the registry as it changes.
pub struct Supervisor<F: IngestorFactory> {
    rpc_url: String,
    factory: Arc<F>,
    restart_backoff: Duration,
    reload_interval: Duration,
}

impl<F: IngestorFactory> Supervisor<F> {
    pub fn new(rpc_url: impl Into<String>, factory: F) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            factory: Arc::new(factory),
            restart_backoff: DEFAULT_RESTART_BACKOFF,
            reload_interval: DEFAULT_RELOAD_INTERVAL,
        }
    }

    /// Keep one indexing task per registered contract, re-reading `store` every
    /// [`DEFAULT_RELOAD_INTERVAL`] so contracts added or removed while running
    /// are picked up without a restart. Runs until cancelled.
    pub async fn watch(&self, store: Box<dyn ContractStore>) {
        let mut running: HashMap<String, JoinHandle<()>> = HashMap::new();
        loop {
            match store.list().await {
                Ok(registered) => self.reconcile(&mut running, registered),
                // A blip reading the registry shouldn't stop what's already
                // indexing; keep the current set and try again next tick.
                Err(e) => eprintln!("stardex: could not read the contract list: {e}; retrying"),
            }
            tokio::time::sleep(self.reload_interval).await;
        }
    }

    /// Start a task for every newly registered contract and stop the ones that
    /// are no longer registered.
    fn reconcile(&self, running: &mut HashMap<String, JoinHandle<()>>, registered: Vec<String>) {
        for contract in &registered {
            running.entry(contract.clone()).or_insert_with(|| {
                println!("stardex: started indexing {contract}");
                self.spawn(contract.clone())
            });
        }

        let registered: HashSet<String> = registered.into_iter().collect();
        running.retain(|contract, handle| {
            if registered.contains(contract) {
                return true;
            }
            println!("stardex: stopped indexing {contract}");
            handle.abort();
            false
        });
    }

    /// One contract's task: stream forever, restarting after a backoff if it
    /// errors, so a single contract's failure never stalls the others.
    fn spawn(&self, contract: String) -> JoinHandle<()> {
        let rpc_url = self.rpc_url.clone();
        let factory = Arc::clone(&self.factory);
        let backoff = self.restart_backoff;
        tokio::spawn(async move {
            loop {
                let mut ingestor = Ingestor::with_store(rpc_url.clone(), factory.cursor_store())
                    .with_event_sink(factory.sink());
                match ingestor.index_contract(&contract).await {
                    // The stream only ends on error; a clean return is a no-op.
                    Ok(()) => break,
                    Err(e) => {
                        eprintln!(
                            "stardex: indexing {contract} failed: {e}; \
                             restarting in {backoff:?}"
                        );
                        tokio::time::sleep(backoff).await;
                    }
                }
            }
        })
    }
}
