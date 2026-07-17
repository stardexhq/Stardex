//! The set of contracts Stardex indexes. The supervisor reads this to know
//! which streams to run; `stardex add` writes to it.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use sqlx::{PgPool, Row};

use crate::IngestError;

/// Records which contracts to index and lists them back. Registration is
/// idempotent: adding the same contract twice is a no-op, and re-adding one
/// that was removed puts it back into indexing.
#[async_trait]
pub trait ContractStore: Send + Sync {
    /// Register `contract_id`, noting the ledger at which tracking began.
    async fn register(&self, contract_id: &str, first_seen_ledger: u32) -> Result<(), IngestError>;
    /// Stop indexing `contract_id`, keeping everything it already indexed.
    /// Unknown contracts are ignored.
    async fn unregister(&self, contract_id: &str) -> Result<(), IngestError>;
    /// The contract ids currently being indexed, in the order they were added.
    async fn list(&self) -> Result<Vec<String>, IngestError>;
}

/// In-memory registry: the default when no database is configured (does not
/// survive a restart). Preserves insertion order for stable listing.
#[derive(Default)]
pub struct InMemoryContractStore {
    // contract_id -> (insertion index, still indexing?), so `list` can return
    // add-order and removal can keep the slot without dropping the entry.
    seen: Mutex<HashMap<String, (usize, bool)>>,
}

#[async_trait]
impl ContractStore for InMemoryContractStore {
    async fn register(
        &self,
        contract_id: &str,
        _first_seen_ledger: u32,
    ) -> Result<(), IngestError> {
        let mut seen = self.seen.lock().unwrap();
        let next = seen.len();
        seen.entry(contract_id.to_string())
            .and_modify(|(_, active)| *active = true)
            .or_insert((next, true));
        Ok(())
    }

    async fn unregister(&self, contract_id: &str) -> Result<(), IngestError> {
        if let Some((_, active)) = self.seen.lock().unwrap().get_mut(contract_id) {
            *active = false;
        }
        Ok(())
    }

    async fn list(&self) -> Result<Vec<String>, IngestError> {
        let seen = self.seen.lock().unwrap();
        let mut ordered: Vec<_> = seen.iter().filter(|(_, (_, active))| *active).collect();
        ordered.sort_by_key(|(_, (idx, _))| *idx);
        Ok(ordered.into_iter().map(|(id, _)| id.clone()).collect())
    }
}

/// A [`ContractStore`] backed by the `contracts` table.
pub struct PostgresContractStore {
    pool: PgPool,
}

impl PostgresContractStore {
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ContractStore for PostgresContractStore {
    async fn register(&self, contract_id: &str, first_seen_ledger: u32) -> Result<(), IngestError> {
        // Re-adding a removed contract resumes it; first_seen_ledger is kept
        // from the original registration.
        sqlx::query(
            "insert into contracts (contract_id, first_seen_ledger)
             values ($1, $2)
             on conflict (contract_id) do update set active = true",
        )
        .bind(contract_id)
        .bind(first_seen_ledger as i32)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn unregister(&self, contract_id: &str) -> Result<(), IngestError> {
        sqlx::query("update contracts set active = false where contract_id = $1")
            .bind(contract_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn list(&self) -> Result<Vec<String>, IngestError> {
        let rows = sqlx::query(
            "select contract_id from contracts where active order by added_at, contract_id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|row| row.get("contract_id")).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn register_is_idempotent_and_keeps_add_order() {
        let store = InMemoryContractStore::default();
        store.register("C_ONE", 10).await.unwrap();
        store.register("C_TWO", 20).await.unwrap();
        store.register("C_ONE", 99).await.unwrap(); // duplicate: ignored

        assert_eq!(store.list().await.unwrap(), vec!["C_ONE", "C_TWO"]);
    }

    #[tokio::test]
    async fn unregister_drops_a_contract_from_the_list() {
        let store = InMemoryContractStore::default();
        store.register("C_ONE", 10).await.unwrap();
        store.register("C_TWO", 20).await.unwrap();

        store.unregister("C_ONE").await.unwrap();
        assert_eq!(store.list().await.unwrap(), vec!["C_TWO"]);

        // Re-adding resumes it, back in its original position.
        store.register("C_ONE", 30).await.unwrap();
        assert_eq!(store.list().await.unwrap(), vec!["C_ONE", "C_TWO"]);
    }

    #[tokio::test]
    async fn unregister_ignores_unknown_contracts() {
        let store = InMemoryContractStore::default();
        store.unregister("C_NEVER_ADDED").await.unwrap();
        assert!(store.list().await.unwrap().is_empty());
    }
}
