//! The seam between raw ingestion and what happens to each event.

use async_trait::async_trait;

use crate::{IngestError, RawEvent};

/// Receives each [`RawEvent`] the ingestor streams. Implementations decode and
/// persist; the default just prints.
#[async_trait]
pub trait EventSink: Send + Sync {
    async fn handle(&self, event: RawEvent) -> Result<(), IngestError>;
}

/// Default sink: prints a one-line summary. Used when nothing else is wired in.
pub struct PrintSink;

#[async_trait]
impl EventSink for PrintSink {
    async fn handle(&self, event: RawEvent) -> Result<(), IngestError> {
        println!(
            "event @ ledger {} from {} — topics={:?}",
            event.ledger, event.contract_id, event.topics
        );
        Ok(())
    }
}
