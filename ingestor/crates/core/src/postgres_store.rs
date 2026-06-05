//! Postgres-backed [`CursorStore`] — the real persistence so ingestion resumes
//! after a restart.
//!
//! It reads and writes the `cursors` table defined in
//! `db/migrations/0001_init.sql`:
//!
//! ```sql
//! create table cursors (
//!     stream         text primary key,   -- a contract_id, or "global"
//!     last_ledger    integer not null default 0,
//!     last_event_id  text,
//!     updated_at     timestamptz not null default now()
//! );
//! ```
//!
//! The `cursors` table must already exist (run the migration first).

use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};

use crate::{Cursor, CursorStore, IngestError};

/// A [`CursorStore`] backed by a Postgres connection pool.
pub struct PostgresCursorStore {
    pool: PgPool,
}

impl PostgresCursorStore {
    /// Connect to Postgres at `database_url` (e.g. the `DATABASE_URL` env var)
    /// and build a small connection pool.
    pub async fn connect(database_url: &str) -> Result<Self, IngestError> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    /// Build a store from an existing pool (useful when the pool is shared).
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CursorStore for PostgresCursorStore {
    async fn load(&self, stream: &str) -> Result<Option<Cursor>, IngestError> {
        let row = sqlx::query("select last_ledger, last_event_id from cursors where stream = $1")
            .bind(stream)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|row| {
            // `last_ledger` is a Postgres `integer` (i32); ledger numbers stay
            // well within i32 range for the foreseeable future.
            let last_ledger: i32 = row.get("last_ledger");
            let last_event_id: Option<String> = row.get("last_event_id");
            Cursor {
                last_ledger: last_ledger as u32,
                last_event_id,
            }
        }))
    }

    async fn save(&self, stream: &str, cursor: &Cursor) -> Result<(), IngestError> {
        sqlx::query(
            "insert into cursors (stream, last_ledger, last_event_id, updated_at)
             values ($1, $2, $3, now())
             on conflict (stream) do update
               set last_ledger   = excluded.last_ledger,
                   last_event_id = excluded.last_event_id,
                   updated_at    = now()",
        )
        .bind(stream)
        .bind(cursor.last_ledger as i32)
        .bind(&cursor.last_event_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trips a cursor through a real Postgres. Ignored by default because
    /// it needs a live database; run with:
    ///   DATABASE_URL=postgres://... cargo test -p stardex-core -- --ignored
    /// against a DB that has the `cursors` table (migration 0001).
    #[tokio::test]
    #[ignore = "requires DATABASE_URL to a Postgres with the cursors table"]
    async fn postgres_round_trip() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL set for this test");
        let store = PostgresCursorStore::connect(&url).await.unwrap();

        let stream = "test-stream-stardex";
        let cursor = Cursor {
            last_ledger: 777,
            last_event_id: Some("0000000777-0000000003".into()),
        };
        store.save(stream, &cursor).await.unwrap();

        let loaded = store.load(stream).await.unwrap().expect("saved cursor");
        assert_eq!(loaded.last_ledger, 777);
        assert_eq!(
            loaded.last_event_id.as_deref(),
            Some("0000000777-0000000003")
        );
    }
}
