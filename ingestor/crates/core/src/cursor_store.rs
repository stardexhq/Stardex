//! Where the ingestion **cursor** is saved and restored from.
//!
//! The engine doesn't care *how* the cursor is stored — only that it can load
//! the last position on startup and save progress as it goes. That contract is
//! the [`CursorStore`] trait. Keeping it behind a trait means the core stays
//! database-agnostic and the resume logic is testable without a live database
//! (see [`InMemoryCursorStore`]). The real implementation
//! ([`crate::postgres_store::PostgresCursorStore`]) writes to Postgres.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::{Cursor, IngestError};

/// Loads and persists the ingestion [`Cursor`] for a stream (a `contract_id`).
#[async_trait]
pub trait CursorStore: Send + Sync {
    /// Return the saved cursor for `stream`, or `None` if we've never indexed it.
    async fn load(&self, stream: &str) -> Result<Option<Cursor>, IngestError>;

    /// Persist `cursor` as the latest position for `stream`.
    async fn save(&self, stream: &str, cursor: &Cursor) -> Result<(), IngestError>;
}

/// An in-memory store: used in tests and as the default when no database is
/// configured, so `stardex index` still runs without Postgres (it just won't
/// survive a restart).
#[derive(Default)]
pub struct InMemoryCursorStore {
    cursors: Mutex<HashMap<String, Cursor>>,
}

#[async_trait]
impl CursorStore for InMemoryCursorStore {
    async fn load(&self, stream: &str) -> Result<Option<Cursor>, IngestError> {
        Ok(self.cursors.lock().unwrap().get(stream).cloned())
    }

    async fn save(&self, stream: &str, cursor: &Cursor) -> Result<(), IngestError> {
        self.cursors
            .lock()
            .unwrap()
            .insert(stream.to_string(), cursor.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn round_trips_a_cursor() {
        let store = InMemoryCursorStore::default();
        assert!(store.load("C1").await.unwrap().is_none());

        let cursor = Cursor {
            last_ledger: 100,
            last_event_id: Some("0000000100-0000000001".into()),
        };
        store.save("C1", &cursor).await.unwrap();

        let loaded = store.load("C1").await.unwrap().expect("saved cursor");
        assert_eq!(loaded.last_ledger, 100);
        assert_eq!(
            loaded.last_event_id.as_deref(),
            Some("0000000100-0000000001")
        );
        // a different stream is still empty
        assert!(store.load("C2").await.unwrap().is_none());
    }
}
