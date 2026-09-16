//! The Streams dispatcher: tails the `events` table, works out who wants each
//! event, and posts it to their webhook with retries.
//!
//! It runs apart from ingestion on purpose. Delivery waits on other people's
//! servers, and nothing about reading the chain should ever wait on those.

use std::time::Duration;

use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;
use sqlx::{PgPool, Row};
use tokio::task::JoinSet;

use crate::IngestError;

const CURSOR_NAME: &str = "streams";
/// How many events to match, and deliveries to send, per pass.
const BATCH: i64 = 100;
/// Give up on a delivery once it has failed this many times.
const MAX_ATTEMPTS: i32 = 8;
/// How long a claimed delivery is held before another pass may retry it, so a
/// dispatcher that dies mid-send does not strand the row forever.
const CLAIM_LEASE_SECS: f64 = 300.0;
const POLL_INTERVAL: Duration = Duration::from_secs(2);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Matches indexed events to subscriptions and delivers them.
pub struct Dispatcher {
    pool: PgPool,
    http: reqwest::Client,
}

/// One delivery that is due to be sent.
struct Due {
    id: i64,
    attempts: i32,
    url: String,
    secret: String,
    payload: Value,
}

impl Dispatcher {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("HTTP client builds with a timeout"),
        }
    }

    /// Queue new events and send what is due, forever. A failing pass logs and
    /// is retried on the next tick rather than stopping the dispatcher.
    pub async fn run(&self) {
        println!("stardex: streams dispatcher started");
        loop {
            self.tick().await;
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    /// Queue and send everything that is currently due, then return. For
    /// scheduled jobs (e.g. a cron worker that wakes, delivers, and exits) where
    /// there is no always-on process. Deliveries that fail are left for a later
    /// run with their backoff, not retried in a tight loop here.
    pub async fn run_once(&self) {
        println!("stardex: streams dispatcher catching up");
        loop {
            let (queued, attempted) = self.tick().await;
            if queued == 0 && attempted == 0 {
                break;
            }
        }
        println!("stardex: streams caught up");
    }

    /// One enqueue pass and one delivery pass. Returns how many were queued and
    /// attempted, so `run_once` can loop until there is nothing left to do. Pass
    /// errors are logged, not fatal.
    async fn tick(&self) -> (u64, u64) {
        let queued = self.enqueue_pass().await.unwrap_or_else(|e| {
            eprintln!("stardex: could not queue deliveries: {e}");
            0
        });
        if queued > 0 {
            println!("stardex: queued {queued} delivery(s)");
        }
        let attempted = self.deliver_pass().await.unwrap_or_else(|e| {
            eprintln!("stardex: delivery pass failed: {e}");
            0
        });
        (queued, attempted)
    }

    /// Read the next slice of events and write one delivery row per matching
    /// subscription. The cursor only moves once those rows are committed, so an
    /// event can never be skipped.
    async fn enqueue_pass(&self) -> Result<u64, IngestError> {
        let mut tx = self.pool.begin().await?;

        // Upsert-and-return locks the cursor row for this transaction, so a
        // second dispatcher cannot claim the same slice of events. On the very
        // first run it starts at the newest event: subscribers get what happens
        // from now on, not a replay of everything already indexed.
        let last: i64 = sqlx::query_scalar(
            "insert into stream_cursors (name, last_event_id)
             values ($1, coalesce((select max(id) from events), 0))
             on conflict (name) do update set name = excluded.name
             returning last_event_id",
        )
        .bind(CURSOR_NAME)
        .fetch_one(&mut *tx)
        .await?;

        let upper: Option<i64> = sqlx::query_scalar(
            "select max(id) from (
                 select id from events where id > $1 order by id limit $2
             ) slice",
        )
        .bind(last)
        .bind(BATCH)
        .fetch_one(&mut *tx)
        .await?;

        // Nothing new since last pass.
        let Some(upper) = upper else {
            tx.commit().await?;
            return Ok(0);
        };

        // Matching happens in SQL: every active subscription whose filters the
        // event satisfies gets a row. A null filter means "any".
        let queued = sqlx::query(
            "insert into deliveries (subscription_id, event_id)
             select s.id, e.id
             from events e
             join subscriptions s
               on s.active
              and (s.contract_id is null or s.contract_id = e.contract_id)
              and (s.kind is null or s.kind = e.kind)
             where e.id > $1 and e.id <= $2
             on conflict (subscription_id, event_id) do nothing",
        )
        .bind(last)
        .bind(upper)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        sqlx::query("update stream_cursors set last_event_id = $2 where name = $1")
            .bind(CURSOR_NAME)
            .bind(upper)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(queued)
    }

    /// Send everything that is due, in parallel, and record each outcome.
    /// Returns how many deliveries were attempted (claimed this pass), so a
    /// caller can tell there was work even when every send failed.
    async fn deliver_pass(&self) -> Result<u64, IngestError> {
        let due = self.claim_due().await?;
        if due.is_empty() {
            return Ok(0);
        }
        let attempted = due.len() as u64;

        // One slow or dead receiver must not hold up the rest of the batch.
        let mut tasks = JoinSet::new();
        for delivery in due {
            let pool = self.pool.clone();
            let http = self.http.clone();
            tasks.spawn(async move { deliver_one(&pool, &http, delivery).await });
        }

        let mut delivered = 0;
        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok(Ok(true)) => delivered += 1,
                Ok(Ok(false)) => {}
                Ok(Err(e)) => eprintln!("stardex: could not record a delivery: {e}"),
                Err(e) => eprintln!("stardex: delivery task panicked: {e}"),
            }
        }
        println!("stardex: delivered {delivered}/{attempted}");
        Ok(attempted)
    }

    /// Take the next due deliveries, counting the attempt up front and holding
    /// them on a lease so a concurrent pass skips them.
    async fn claim_due(&self) -> Result<Vec<Due>, IngestError> {
        let ids: Vec<i64> = sqlx::query_scalar(
            "update deliveries
             set attempts = attempts + 1,
                 next_attempt_at = now() + make_interval(secs => $2)
             where id in (
                 select id from deliveries
                 where status = 'pending' and next_attempt_at <= now()
                 order by next_attempt_at
                 limit $1
                 for update skip locked
             )
             returning id",
        )
        .bind(BATCH)
        .bind(CLAIM_LEASE_SECS)
        .fetch_all(&self.pool)
        .await?;

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        // `closed_at` is rendered here so the payload matches the ISO timestamps
        // the REST API returns, without pulling a date library into the core.
        let rows = sqlx::query(
            "select d.id, d.attempts, d.subscription_id, s.url, s.secret,
                    e.id as event_id, e.contract_id, e.ledger, e.kind, e.fields,
                    to_char(e.closed_at at time zone 'utc',
                            'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"') as closed_at
             from deliveries d
             join subscriptions s on s.id = d.subscription_id
             join events e on e.id = d.event_id
             where d.id = any($1)",
        )
        .bind(&ids)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let event_id: i64 = row.get("event_id");
                let delivery_id: i64 = row.get("id");
                let subscription_id: i64 = row.get("subscription_id");
                Due {
                    id: delivery_id,
                    attempts: row.get("attempts"),
                    url: row.get("url"),
                    secret: row.get("secret"),
                    payload: json!({
                        "deliveryId": delivery_id.to_string(),
                        "subscriptionId": subscription_id.to_string(),
                        // Mirrors the StardexEvent shape served by /events.
                        "event": {
                            "id": event_id.to_string(),
                            "contractId": row.get::<String, _>("contract_id"),
                            "ledger": row.get::<i32, _>("ledger"),
                            "kind": row.get::<String, _>("kind"),
                            "fields": row.get::<Value, _>("fields"),
                            "closedAt": row.get::<Option<String>, _>("closed_at"),
                        }
                    }),
                }
            })
            .collect())
    }
}

/// Post one delivery and record whether it landed.
async fn deliver_one(pool: &PgPool, http: &reqwest::Client, due: Due) -> Result<bool, IngestError> {
    let body = due.payload.to_string();
    let signature = sign(&due.secret, &body);

    let sent = http
        .post(&due.url)
        .header("content-type", "application/json")
        .header("x-stardex-signature", format!("sha256={signature}"))
        .header("x-stardex-delivery", due.id.to_string())
        .body(body)
        .send()
        .await;

    match sent {
        Ok(res) if res.status().is_success() => {
            sqlx::query(
                "update deliveries
                 set status = 'delivered', delivered_at = now(), last_error = null
                 where id = $1",
            )
            .bind(due.id)
            .execute(pool)
            .await?;
            Ok(true)
        }
        Ok(res) => {
            record_failure(pool, &due, &format!("receiver returned {}", res.status())).await?;
            Ok(false)
        }
        Err(e) => {
            record_failure(pool, &due, &e.to_string()).await?;
            Ok(false)
        }
    }
}

/// Schedule the next attempt, or give up once it has failed too many times.
async fn record_failure(pool: &PgPool, due: &Due, error: &str) -> Result<(), IngestError> {
    if due.attempts >= MAX_ATTEMPTS {
        sqlx::query("update deliveries set status = 'dead', last_error = $2 where id = $1")
            .bind(due.id)
            .bind(error)
            .execute(pool)
            .await?;
        eprintln!(
            "stardex: delivery {} gave up after {} attempts: {error}",
            due.id, due.attempts
        );
        return Ok(());
    }

    sqlx::query(
        "update deliveries
         set next_attempt_at = now() + make_interval(secs => $2), last_error = $3
         where id = $1",
    )
    .bind(due.id)
    .bind(backoff_secs(due.attempts))
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

/// Exponential backoff, capped, so a struggling receiver is not hammered.
fn backoff_secs(attempts: i32) -> f64 {
    const CAP: f64 = 3600.0;
    let exponent = attempts.clamp(0, 16) as u32;
    (5.0 * 2f64.powi(exponent as i32)).min(CAP)
}

/// `sha256=<hex>` HMAC over the exact body bytes, so a receiver can prove the
/// request came from this Stardex and not from someone imitating it.
fn sign(secret: &str, body: &str) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(body.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_is_stable_and_secret_dependent() {
        let body = r#"{"event":"transfer"}"#;
        assert_eq!(sign("topsecret", body), sign("topsecret", body));
        assert_ne!(sign("topsecret", body), sign("other", body));
        assert_ne!(sign("topsecret", body), sign("topsecret", "tampered"));
        assert_eq!(sign("topsecret", body).len(), 64);
    }

    #[test]
    fn backoff_grows_and_is_capped() {
        assert_eq!(backoff_secs(1), 10.0);
        assert_eq!(backoff_secs(2), 20.0);
        assert!(backoff_secs(5) > backoff_secs(4));
        assert_eq!(backoff_secs(99), 3600.0);
    }
}
