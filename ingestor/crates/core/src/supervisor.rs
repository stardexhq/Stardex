//! Runs many per-contract ingestors at once. Each contract gets its own task
//! and its own cursor, so one contract's failure never stalls the others.

use std::sync::Arc;
use std::time::Duration;

use crate::{CursorStore, EventSink, Ingestor};

/// Builds the collaborators for one indexing task. Called once per contract so
/// each task owns a fresh cursor store and sink over a shared backend (e.g. a
/// connection pool). Kept decoder-agnostic: the CLI supplies the real wiring.
pub trait IngestorFactory: Send + Sync + 'static {
    fn cursor_store(&self) -> Box<dyn CursorStore>;
    fn sink(&self) -> Box<dyn EventSink>;
}

/// How long to wait before restarting a contract's task after it errors.
const DEFAULT_RESTART_BACKOFF: Duration = Duration::from_secs(5);

/// Supervises one indexing task per contract, restarting any that fail.
pub struct Supervisor<F: IngestorFactory> {
    rpc_url: String,
    factory: Arc<F>,
    restart_backoff: Duration,
}

impl<F: IngestorFactory> Supervisor<F> {
    pub fn new(rpc_url: impl Into<String>, factory: F) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            factory: Arc::new(factory),
            restart_backoff: DEFAULT_RESTART_BACKOFF,
        }
    }

    /// Spawn an indexing task per contract and run until all exit. Each task
    /// streams forever; if one errors it logs and restarts after a backoff,
    /// leaving the other contracts untouched.
    pub async fn run(&self, contracts: Vec<String>) {
        let mut handles = Vec::with_capacity(contracts.len());
        for contract in contracts {
            let rpc_url = self.rpc_url.clone();
            let factory = Arc::clone(&self.factory);
            let backoff = self.restart_backoff;
            handles.push(tokio::spawn(async move {
                loop {
                    let mut ingestor =
                        Ingestor::with_store(rpc_url.clone(), factory.cursor_store())
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
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }
    }
}
