//! Incoming payments to watched accounts, the rows reconciliation works on.

use std::sync::Mutex;

use async_trait::async_trait;
use sqlx::{PgPool, Row};

use crate::IngestError;

/// One payment into a watched account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payment {
    /// RPC event id. Recording the same event twice is a no-op.
    pub event_id: String,
    pub tx_hash: String,
    pub ledger: u32,
    /// Ledger close time (RFC3339); `None` falls back to now on write.
    pub closed_at: Option<String>,
    /// The watched account that received the payment.
    pub account: String,
    pub from_address: String,
    /// SEP-11 asset (`native`, `CODE:ISSUER`) or the token contract id.
    pub asset: String,
    pub asset_contract: String,
    /// Raw integer units of the asset.
    pub amount: i128,
    /// `id`, `text` or `hash`, when the payment carried a muxed ID or memo.
    pub reference_type: Option<String>,
    pub reference: Option<String>,
}

#[async_trait]
pub trait PaymentStore: Send + Sync {
    /// Record `payment`. Returns `false` if its event was already recorded.
    async fn record(&self, payment: &Payment) -> Result<bool, IngestError>;
}

/// In-memory store for tests and database-free runs.
#[derive(Default)]
pub struct InMemoryPaymentStore {
    payments: Mutex<Vec<Payment>>,
}

impl InMemoryPaymentStore {
    pub fn payments(&self) -> Vec<Payment> {
        self.payments.lock().unwrap().clone()
    }
}

#[async_trait]
impl PaymentStore for InMemoryPaymentStore {
    async fn record(&self, payment: &Payment) -> Result<bool, IngestError> {
        let mut payments = self.payments.lock().unwrap();
        if payments.iter().any(|p| p.event_id == payment.event_id) {
            return Ok(false);
        }
        payments.push(payment.clone());
        Ok(true)
    }
}

/// A [`PaymentStore`] backed by the `payments` table.
pub struct PostgresPaymentStore {
    pool: PgPool,
}

impl PostgresPaymentStore {
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Most recent payments, newest first, optionally for one account.
    pub async fn recent(
        &self,
        account: Option<&str>,
        limit: i64,
    ) -> Result<Vec<Payment>, IngestError> {
        let rows = sqlx::query(
            "select event_id, tx_hash, ledger,
                    to_char(closed_at at time zone 'utc',
                            'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') as closed_at,
                    account, from_address, asset, asset_contract, amount::text as amount,
                    reference_type, reference
             from payments
             where $1::text is null or account = $1
             order by payments.closed_at desc, id desc
             limit $2",
        )
        .bind(account)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter()
            .map(|row| {
                let amount: String = row.get("amount");
                Ok(Payment {
                    event_id: row.get("event_id"),
                    tx_hash: row.get("tx_hash"),
                    ledger: row.get::<i32, _>("ledger") as u32,
                    closed_at: row.get("closed_at"),
                    account: row.get("account"),
                    from_address: row.get("from_address"),
                    asset: row.get("asset"),
                    asset_contract: row.get("asset_contract"),
                    amount: amount
                        .parse()
                        .map_err(|_| IngestError::Store(format!("bad amount {amount}")))?,
                    reference_type: row.get("reference_type"),
                    reference: row.get("reference"),
                })
            })
            .collect()
    }
}

#[async_trait]
impl PaymentStore for PostgresPaymentStore {
    async fn record(&self, payment: &Payment) -> Result<bool, IngestError> {
        // sqlx has no i128 support, so the amount travels as text.
        let inserted = sqlx::query(
            "insert into payments (event_id, tx_hash, ledger, closed_at, account, from_address,
                                   asset, asset_contract, amount, reference_type, reference)
             values ($1, $2, $3, coalesce($4::timestamptz, now()), $5, $6, $7, $8,
                     $9::numeric, $10, $11)
             on conflict (event_id) do nothing",
        )
        .bind(&payment.event_id)
        .bind(&payment.tx_hash)
        .bind(payment.ledger as i32)
        .bind(&payment.closed_at)
        .bind(&payment.account)
        .bind(&payment.from_address)
        .bind(&payment.asset)
        .bind(&payment.asset_contract)
        .bind(payment.amount.to_string())
        .bind(&payment.reference_type)
        .bind(&payment.reference)
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(inserted > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCOUNT: &str = "GBTF2Z62VJD4B54NGIS6JTGNPVH2O5HQNQF4S75NHVZIBP4JONQMRP7K";

    fn payment(event_id: &str) -> Payment {
        Payment {
            event_id: event_id.into(),
            tx_hash: "fa08b760".into(),
            ledger: 4_710_943,
            closed_at: Some("2026-09-16T17:00:00Z".into()),
            account: ACCOUNT.into(),
            from_address: "GPAYER".into(),
            asset: "native".into(),
            asset_contract: "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC".into(),
            amount: 170_141_183_460_469_231_731_687_303_715_884_105_727,
            reference_type: Some("id".into()),
            reference: Some("100042".into()),
        }
    }

    #[tokio::test]
    async fn records_each_event_once() {
        let store = InMemoryPaymentStore::default();
        assert!(store.record(&payment("e1")).await.unwrap());
        assert!(!store.record(&payment("e1")).await.unwrap());
        assert!(store.record(&payment("e2")).await.unwrap());
        assert_eq!(store.payments().len(), 2);
    }

    /// Needs a live DB with migrations 0001 to 0005:
    /// DATABASE_URL=... cargo test -p stardex-core -- --ignored
    #[tokio::test]
    #[ignore = "requires DATABASE_URL to a Postgres with the payments table"]
    async fn postgres_records_idempotently_and_keeps_i128() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL set for this test");
        let pool = crate::connect_pool(&url).await.unwrap();
        sqlx::query("insert into accounts (address) values ($1) on conflict do nothing")
            .bind(ACCOUNT)
            .execute(&pool)
            .await
            .unwrap();
        let store = PostgresPaymentStore::from_pool(pool.clone());

        let event_id = format!("test-{}", std::process::id());
        let p = payment(&event_id);
        assert!(store.record(&p).await.unwrap());
        assert!(!store.record(&p).await.unwrap());

        let back = store
            .recent(Some(ACCOUNT), 50)
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.event_id == event_id)
            .expect("recorded payment is listed");
        assert_eq!(back.amount, p.amount);
        assert_eq!(back.reference.as_deref(), Some("100042"));

        sqlx::query("delete from payments where event_id = $1")
            .bind(&event_id)
            .execute(&pool)
            .await
            .unwrap();
    }
}
