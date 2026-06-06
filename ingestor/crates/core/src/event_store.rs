//! Persistence seam for decoded/raw events; keeps the core db-agnostic.

use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::Value;

use crate::IngestError;

/// A row ready to be written to the events store.
#[derive(Debug, Clone)]
pub struct StoredEvent {
    pub contract_id: String,
    pub ledger: u32,
    pub kind: String,
    pub fields: Value,
    /// Ledger close time (RFC3339); `None` falls back to now on write.
    pub closed_at: Option<String>,
}

/// Writes events produced by the ingestion pipeline.
#[async_trait]
pub trait EventStore: Send + Sync {
    async fn store(&self, event: &StoredEvent) -> Result<(), IngestError>;
}

/// In-memory store: the default when no database is configured. Retains rows
/// so tests can inspect what was written.
#[derive(Default)]
pub struct InMemoryEventStore {
    events: Mutex<Vec<StoredEvent>>,
}

impl InMemoryEventStore {
    pub fn events(&self) -> Vec<StoredEvent> {
        self.events.lock().unwrap().clone()
    }
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    async fn store(&self, event: &StoredEvent) -> Result<(), IngestError> {
        self.events.lock().unwrap().push(event.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn retains_stored_events() {
        let store = InMemoryEventStore::default();
        store
            .store(&StoredEvent {
                contract_id: "CABC".into(),
                ledger: 7,
                kind: "transfer".into(),
                fields: json!({"amount": "10"}),
                closed_at: None,
            })
            .await
            .unwrap();

        let rows = store.events();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, "transfer");
    }
}
