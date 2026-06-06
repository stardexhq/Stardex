use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::{EventStore, IngestError, StoredEvent};

/// An [`EventStore`] backed by a Postgres connection pool.
pub struct PostgresEventStore {
    pool: PgPool,
}

impl PostgresEventStore {
    pub async fn connect(database_url: &str) -> Result<Self, IngestError> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventStore for PostgresEventStore {
    async fn store(&self, event: &StoredEvent) -> Result<(), IngestError> {
        // The events FK requires the contract row to exist first.
        sqlx::query(
            "insert into contracts (contract_id, first_seen_ledger)
             values ($1, $2)
             on conflict (contract_id) do nothing",
        )
        .bind(&event.contract_id)
        .bind(event.ledger as i32)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "insert into events (contract_id, ledger, kind, fields, closed_at)
             values ($1, $2, $3, $4, coalesce($5::timestamptz, now()))",
        )
        .bind(&event.contract_id)
        .bind(event.ledger as i32)
        .bind(&event.kind)
        .bind(&event.fields)
        .bind(&event.closed_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Needs a live DB: DATABASE_URL=... cargo test -p stardex-core -- --ignored
    #[tokio::test]
    #[ignore = "requires DATABASE_URL to a Postgres with the events table"]
    async fn stores_an_event() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL set for this test");
        let store = PostgresEventStore::connect(&url).await.unwrap();

        store
            .store(&StoredEvent {
                contract_id: "CTEST_STARDEX".into(),
                ledger: 123,
                kind: "transfer".into(),
                fields: json!({"from": "G...", "to": "G...", "amount": "42"}),
                closed_at: Some("2026-06-05T00:00:00Z".into()),
            })
            .await
            .unwrap();
    }
}
