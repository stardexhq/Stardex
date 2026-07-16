//! One shared Postgres pool, cloned to every store so the connection count
//! stays bounded no matter how many contracts are indexed at once.

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::IngestError;

/// Connect to Postgres at `database_url` and build a shared pool.
pub async fn connect_pool(database_url: &str) -> Result<PgPool, IngestError> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    Ok(pool)
}
