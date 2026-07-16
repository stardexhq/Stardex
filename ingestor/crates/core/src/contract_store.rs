//! The set of contracts Stardex indexes. The supervisor reads this to know
//! which streams to run; `stardex add` writes to it.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use sqlx::{PgPool, Row};

use crate::IngestError;

/// Records which contracts to index and lists them back. Registration is
/// idempotent: adding the same contract twice is a no-op.
#[async_trait]
pub trait ContractStore: Send + Sync {
    /// Register `contract_id`, noting the ledger at which tracking began.
    async fn register(&self, contract_id: &str, first_seen_ledger: u32) -> Result<(), IngestError>;
    /// All registered contract ids, in the order they were added.
    async fn list(&self) -> Result<Vec<String>, IngestError>;
}

/// In-memory registry: the default when no database is configured (does not
/// survive a restart). Preserves insertion order for stable listing.
#[derive(Default)]
pub struct InMemoryContractStore {
    // contract_id -> insertion index, so `list` can return add-order.
    seen: Mutex<HashMap<String, usize>>,
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
        seen.entry(contract_id.to_string()).or_insert(next);
        Ok(())
    }

    async fn list(&self) -> Result<Vec<String>, IngestError> {
        let seen = self.seen.lock().unwrap();
        let mut ordered: Vec<(&String, &usize)> = seen.iter().collect();
        ordered.sort_by_key(|(_, idx)| **idx);
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
        sqlx::query(
            "insert into contracts (contract_id, first_seen_ledger)
             values ($1, $2)
             on conflict (contract_id) do nothing",
        )
        .bind(contract_id)
        .bind(first_seen_ledger as i32)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list(&self) -> Result<Vec<String>, IngestError> {
        let rows = sqlx::query("select contract_id from contracts order by added_at, contract_id")
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
}
