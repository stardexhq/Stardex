//! The Stellar accounts whose incoming payments Stardex watches. The supervisor
//! runs one stream per account; `stardex accounts add` writes to it.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use sqlx::{PgPool, Row};

use crate::IngestError;

/// An account being watched for incoming payments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchedAccount {
    pub address: String,
    /// Optional human name, e.g. the business it belongs to.
    pub label: Option<String>,
    /// Ledger current when the account was first added. Its stream starts
    /// here, so payments made before the watcher first runs are not missed.
    pub first_ledger: Option<u32>,
}

/// Records which accounts to watch. Adding is idempotent, and re-adding a
/// removed account resumes watching it.
#[async_trait]
pub trait AccountStore: Send + Sync {
    /// Start watching `address`. A `None` label keeps any existing label, and
    /// `first_ledger` is only recorded the first time.
    async fn add(
        &self,
        address: &str,
        label: Option<&str>,
        first_ledger: Option<u32>,
    ) -> Result<(), IngestError>;
    /// Stop watching `address`, keeping what was already recorded. Unknown
    /// accounts are ignored.
    async fn remove(&self, address: &str) -> Result<(), IngestError>;
    /// Accounts currently watched, in the order they were added.
    async fn list(&self) -> Result<Vec<WatchedAccount>, IngestError>;
}

struct Entry {
    order: usize,
    label: Option<String>,
    first_ledger: Option<u32>,
    active: bool,
}

/// In-memory store for tests and database-free runs.
#[derive(Default)]
pub struct InMemoryAccountStore {
    accounts: Mutex<HashMap<String, Entry>>,
}

#[async_trait]
impl AccountStore for InMemoryAccountStore {
    async fn add(
        &self,
        address: &str,
        label: Option<&str>,
        first_ledger: Option<u32>,
    ) -> Result<(), IngestError> {
        let mut accounts = self.accounts.lock().unwrap();
        let next = accounts.len();
        let entry = accounts.entry(address.to_string()).or_insert(Entry {
            order: next,
            label: None,
            first_ledger,
            active: true,
        });
        entry.active = true;
        if let Some(label) = label {
            entry.label = Some(label.to_string());
        }
        Ok(())
    }

    async fn remove(&self, address: &str) -> Result<(), IngestError> {
        if let Some(entry) = self.accounts.lock().unwrap().get_mut(address) {
            entry.active = false;
        }
        Ok(())
    }

    async fn list(&self) -> Result<Vec<WatchedAccount>, IngestError> {
        let accounts = self.accounts.lock().unwrap();
        let mut active: Vec<_> = accounts.iter().filter(|(_, e)| e.active).collect();
        active.sort_by_key(|(_, e)| e.order);
        Ok(active
            .into_iter()
            .map(|(address, e)| WatchedAccount {
                address: address.clone(),
                label: e.label.clone(),
                first_ledger: e.first_ledger,
            })
            .collect())
    }
}

/// An [`AccountStore`] backed by the `accounts` table.
pub struct PostgresAccountStore {
    pool: PgPool,
}

impl PostgresAccountStore {
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AccountStore for PostgresAccountStore {
    async fn add(
        &self,
        address: &str,
        label: Option<&str>,
        first_ledger: Option<u32>,
    ) -> Result<(), IngestError> {
        sqlx::query(
            "insert into accounts (address, label, first_ledger)
             values ($1, $2, $3)
             on conflict (address) do update
               set active = true,
                   label = coalesce(excluded.label, accounts.label),
                   first_ledger = coalesce(accounts.first_ledger, excluded.first_ledger)",
        )
        .bind(address)
        .bind(label)
        .bind(first_ledger.map(|l| l as i32))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn remove(&self, address: &str) -> Result<(), IngestError> {
        sqlx::query("update accounts set active = false where address = $1")
            .bind(address)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn list(&self) -> Result<Vec<WatchedAccount>, IngestError> {
        let rows = sqlx::query(
            "select address, label, first_ledger from accounts
             where active order by added_at, address",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|row| WatchedAccount {
                address: row.get("address"),
                label: row.get("label"),
                first_ledger: row.get::<Option<i32>, _>("first_ledger").map(|l| l as u32),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addresses(list: Vec<WatchedAccount>) -> Vec<String> {
        list.into_iter().map(|a| a.address).collect()
    }

    #[tokio::test]
    async fn add_is_idempotent_and_keeps_add_order() {
        let store = InMemoryAccountStore::default();
        store.add("G_ONE", Some("Acme"), Some(100)).await.unwrap();
        store.add("G_TWO", None, None).await.unwrap();
        store.add("G_ONE", None, Some(999)).await.unwrap();

        let list = store.list().await.unwrap();
        assert_eq!(addresses(list.clone()), vec!["G_ONE", "G_TWO"]);
        assert_eq!(list[0].label.as_deref(), Some("Acme"));
        // The first ledger is kept from the original add.
        assert_eq!(list[0].first_ledger, Some(100));
    }

    #[tokio::test]
    async fn remove_then_add_resumes_watching() {
        let store = InMemoryAccountStore::default();
        store.add("G_ONE", None, Some(999)).await.unwrap();
        store.add("G_TWO", None, None).await.unwrap();

        store.remove("G_ONE").await.unwrap();
        assert_eq!(addresses(store.list().await.unwrap()), vec!["G_TWO"]);

        store
            .add("G_ONE", Some("Renamed"), Some(500))
            .await
            .unwrap();
        let list = store.list().await.unwrap();
        assert_eq!(addresses(list.clone()), vec!["G_ONE", "G_TWO"]);
        assert_eq!(list[0].label.as_deref(), Some("Renamed"));
    }

    #[tokio::test]
    async fn remove_ignores_unknown_accounts() {
        let store = InMemoryAccountStore::default();
        store.remove("G_NEVER_ADDED").await.unwrap();
        assert!(store.list().await.unwrap().is_empty());
    }
}
