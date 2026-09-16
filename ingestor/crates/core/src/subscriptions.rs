//! Who wants which events pushed to them, and where. Streams is Postgres-only:
//! the matching between events and subscriptions is done in SQL, so there is no
//! in-memory implementation to keep in step.

use rand::Rng;
use sqlx::{PgPool, Row};

use crate::IngestError;

/// A webhook subscription. A `None` filter means "any": a subscription with no
/// contract and no kind receives every indexed event.
#[derive(Debug, Clone)]
pub struct Subscription {
    pub id: i64,
    pub url: String,
    pub contract_id: Option<String>,
    pub kind: Option<String>,
    pub active: bool,
}

/// Create, list, and retire subscriptions.
pub struct Subscriptions {
    pool: PgPool,
}

impl Subscriptions {
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Register `url` to receive events matching the filters. Returns the new
    /// id and its signing secret; the secret is shown once, at creation.
    pub async fn create(
        &self,
        url: &str,
        contract_id: Option<&str>,
        kind: Option<&str>,
    ) -> Result<(i64, String), IngestError> {
        let secret = new_secret();
        let row = sqlx::query(
            "insert into subscriptions (url, contract_id, kind, secret)
             values ($1, $2, $3, $4)
             returning id",
        )
        .bind(url)
        .bind(contract_id)
        .bind(kind)
        .bind(&secret)
        .fetch_one(&self.pool)
        .await?;
        Ok((row.get("id"), secret))
    }

    /// Every subscription, retired ones included, oldest first.
    pub async fn list(&self) -> Result<Vec<Subscription>, IngestError> {
        let rows =
            sqlx::query("select id, url, contract_id, kind, active from subscriptions order by id")
                .fetch_all(&self.pool)
                .await?;

        Ok(rows
            .iter()
            .map(|row| Subscription {
                id: row.get("id"),
                url: row.get("url"),
                contract_id: row.get("contract_id"),
                kind: row.get("kind"),
                active: row.get("active"),
            })
            .collect())
    }

    /// Stop delivering to a subscription. Its history stays for the record.
    /// Returns false if no such subscription exists.
    pub async fn deactivate(&self, id: i64) -> Result<bool, IngestError> {
        let done = sqlx::query("update subscriptions set active = false where id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() > 0)
    }
}

/// A 32-byte random secret, hex encoded.
fn new_secret() -> String {
    let bytes: [u8; 32] = rand::thread_rng().gen();
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_long_and_unique() {
        let a = new_secret();
        let b = new_secret();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }
}
