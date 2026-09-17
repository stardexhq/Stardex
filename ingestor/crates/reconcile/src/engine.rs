//! The reconcile engine: reads recorded payments, matches each one to an
//! invoice using [`crate::rules`], and records the result.
//!
//! It runs apart from ingestion so matching can never slow down reading the
//! chain, and it tracks its own position in `stream_cursors`.

use std::time::Duration;

use sqlx::postgres::PgRow;
use sqlx::{PgConnection, PgPool, Row};
use stardex_core::IngestError;

use crate::invoices::parse_units;
use crate::rules::{
    decide, lookup_key, Decision, InvoiceStatus, InvoiceView, LookupKey, PaymentView,
};

const CURSOR_NAME: &str = "reconcile";
/// Payments read per pass.
const BATCH: i64 = 100;
const POLL_INTERVAL: Duration = Duration::from_secs(2);

const PAYMENT_COLUMNS: &str =
    "p.id, p.account, p.asset, p.amount::text as amount, p.reference_type, p.reference, p.match_status";

pub struct Engine {
    pool: PgPool,
}

impl Engine {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Match new payments forever. A failing pass is logged and retried on the
    /// next tick.
    pub async fn run(&self) {
        println!("stardex: reconcile engine started");
        loop {
            self.tick().await;
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    /// Match everything currently waiting, then return. For scheduled jobs.
    pub async fn run_once(&self) {
        while self.tick().await > 0 {}
        println!("stardex: reconcile caught up");
    }

    /// One pass over new payments and one recheck pass. Returns how much work
    /// was done, so `run_once` knows when to stop.
    async fn tick(&self) -> u64 {
        let read = self.new_payments_pass().await.unwrap_or_else(|e| {
            eprintln!("stardex: reconcile pass failed: {e}");
            0
        });
        let rematched = self.recheck_pass().await.unwrap_or_else(|e| {
            eprintln!("stardex: reconcile recheck failed: {e}");
            0
        });
        if rematched > 0 {
            println!("stardex: matched {rematched} earlier payment(s) to new invoices");
        }
        read + rematched
    }

    /// Decide every payment recorded since the last pass. The cursor only
    /// moves in the same transaction as the decisions, so none is skipped.
    async fn new_payments_pass(&self) -> Result<u64, IngestError> {
        let mut tx = self.pool.begin().await?;

        // Upsert-and-return locks the cursor row, so two engines cannot claim
        // the same payments. Starts at 0: existing payments get matched too.
        let last: i64 = sqlx::query_scalar(
            "insert into stream_cursors (name, last_event_id) values ($1, 0)
             on conflict (name) do update set name = excluded.name
             returning last_event_id",
        )
        .bind(CURSOR_NAME)
        .fetch_one(&mut *tx)
        .await?;

        let sql = format!(
            "select {PAYMENT_COLUMNS} from payments p where p.id > $1 order by p.id limit $2"
        );
        let rows = sqlx::query(&sql)
            .bind(last)
            .bind(BATCH)
            .fetch_all(&mut *tx)
            .await?;
        let Some(upper) = rows.last().map(|r| r.get::<i64, _>("id")) else {
            tx.commit().await?;
            return Ok(0);
        };

        let mut matched = 0;
        for row in &rows {
            // Payments already matched or ignored by hand are left alone.
            if row.get::<String, _>("match_status") != "unmatched" {
                continue;
            }
            if apply(&mut tx, &payment_view(row)?).await? {
                matched += 1;
            }
        }

        sqlx::query("update stream_cursors set last_event_id = $2 where name = $1")
            .bind(CURSOR_NAME)
            .bind(upper)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        println!(
            "stardex: reconciled {} payment(s), {matched} matched",
            rows.len()
        );
        Ok(rows.len() as u64)
    }

    /// A payment can arrive before its invoice is created. Recheck recent
    /// payments that named an invoice we did not have yet, once a newer invoice
    /// exists on the same account.
    async fn recheck_pass(&self) -> Result<u64, IngestError> {
        let mut tx = self.pool.begin().await?;
        let sql = format!(
            "select {PAYMENT_COLUMNS} from payments p
             where p.match_status = 'unmatched'
               and p.unmatched_reason = 'no_invoice'
               and p.closed_at > now() - interval '30 days'
               and p.id <= coalesce(
                   (select last_event_id from stream_cursors where name = $1), 0)
               and exists (select 1 from invoices i
                           where i.account = p.account and i.issued_at >= p.recorded_at)
             order by p.id
             limit $2
             for update of p skip locked"
        );
        let rows = sqlx::query(&sql)
            .bind(CURSOR_NAME)
            .bind(BATCH)
            .fetch_all(&mut *tx)
            .await?;

        let mut matched = 0;
        for row in &rows {
            if apply(&mut tx, &payment_view(row)?).await? {
                matched += 1;
            }
        }
        tx.commit().await?;
        Ok(matched)
    }
}

/// Decide one payment and write the outcome. Returns whether it was matched.
async fn apply(conn: &mut PgConnection, payment: &PaymentView) -> Result<bool, IngestError> {
    let candidate = match lookup_key(payment) {
        Some(key) => find_invoice(conn, &payment.account, &key).await?,
        None => None,
    };

    match decide(payment, candidate.as_ref()) {
        Decision::Allocate { invoice_id, amount } => {
            sqlx::query(
                "insert into payment_allocations (payment_id, invoice_id, amount, matched_by)
                 values ($1, $2, $3::numeric, 'reference')
                 on conflict (payment_id) do nothing",
            )
            .bind(payment.id)
            .bind(invoice_id)
            .bind(amount.to_string())
            .execute(&mut *conn)
            .await?;
            sqlx::query(
                "update payments set match_status = 'matched', unmatched_reason = null
                 where id = $1",
            )
            .bind(payment.id)
            .execute(&mut *conn)
            .await?;
            sqlx::query("select recalc_invoice($1)")
                .bind(invoice_id)
                .execute(&mut *conn)
                .await?;
            Ok(true)
        }
        Decision::Unmatched(reason) => {
            sqlx::query(
                "update payments set unmatched_reason = $2
                 where id = $1 and match_status = 'unmatched'",
            )
            .bind(payment.id)
            .bind(reason.as_str())
            .execute(&mut *conn)
            .await?;
            Ok(false)
        }
    }
}

/// The invoice on `account` that `key` points to. A reference match wins over
/// a number match if a text memo happens to fit both.
async fn find_invoice(
    conn: &mut PgConnection,
    account: &str,
    key: &LookupKey,
) -> Result<Option<InvoiceView>, IngestError> {
    let row = sqlx::query(
        "select id, account, number, reference, asset, status from invoices
         where account = $1
           and (reference = $2 or lower(number) = lower($3))
         order by (reference = $2) is true desc
         limit 1",
    )
    .bind(account)
    .bind(key.reference)
    .bind(key.number.as_deref())
    .fetch_optional(&mut *conn)
    .await?;

    row.map(|row| {
        let status: String = row.get("status");
        Ok(InvoiceView {
            id: row.get("id"),
            account: row.get("account"),
            number: row.get("number"),
            reference: row.get("reference"),
            asset: row.get("asset"),
            status: InvoiceStatus::parse(&status)
                .ok_or_else(|| IngestError::Store(format!("unknown invoice status {status}")))?,
        })
    })
    .transpose()
}

fn payment_view(row: &PgRow) -> Result<PaymentView, IngestError> {
    Ok(PaymentView {
        id: row.get("id"),
        account: row.get("account"),
        asset: row.get("asset"),
        amount: parse_units(row.get("amount"))?,
        reference_type: row.get("reference_type"),
        reference: row.get("reference"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invoices::{Invoices, NewInvoice};
    use crate::stellar::parse_amount;

    struct Fixture {
        pool: PgPool,
        account: String,
        invoices: Invoices,
    }

    impl Fixture {
        async fn invoice(&self, amount: &str, asset: &str, number: Option<&str>) -> (i64, i64) {
            let inv = self
                .invoices
                .create(&NewInvoice {
                    account: self.account.clone(),
                    asset: asset.into(),
                    amount: parse_amount(amount).unwrap(),
                    number: number.map(Into::into),
                    ..Default::default()
                })
                .await
                .unwrap();
            (inv.id, inv.reference)
        }

        async fn payment(&self, tag: &str, amount: &str, reference: Option<(&str, String)>) {
            let (kind, value) = match reference {
                Some((k, v)) => (Some(k), Some(v)),
                None => (None, None),
            };
            sqlx::query(
                "insert into payments (event_id, tx_hash, ledger, closed_at, account,
                    from_address, asset, asset_contract, amount, reference_type, reference)
                 values ($1, 'tx', 1, now(), $2, 'GPAYER', 'native', 'CXLM', $3::numeric, $4, $5)",
            )
            .bind(format!("{}-{tag}", self.account))
            .bind(&self.account)
            .bind(parse_amount(amount).unwrap().to_string())
            .bind(kind)
            .bind(value)
            .execute(&self.pool)
            .await
            .unwrap();
        }

        async fn invoice_state(&self, id: i64) -> (String, String) {
            let row = sqlx::query(
                "select status, amount_received::text as received from invoices where id = $1",
            )
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .unwrap();
            (row.get("status"), row.get("received"))
        }

        async fn payment_state(&self, tag: &str) -> (String, Option<String>) {
            let row = sqlx::query(
                "select match_status, unmatched_reason from payments where event_id = $1",
            )
            .bind(format!("{}-{tag}", self.account))
            .fetch_one(&self.pool)
            .await
            .unwrap();
            (row.get("match_status"), row.get("unmatched_reason"))
        }
    }

    /// Needs a live DB with migrations 0001 to 0006:
    /// DATABASE_URL=... cargo test -p stardex-reconcile -- --ignored
    #[tokio::test]
    #[ignore = "requires DATABASE_URL to a Postgres with the invoices tables"]
    async fn matches_payments_to_invoices_end_to_end() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL set for this test");
        let pool = stardex_core::connect_pool(&url).await.unwrap();
        let account = format!("GTEST{}", std::process::id());
        sqlx::query("insert into accounts (address) values ($1)")
            .bind(&account)
            .execute(&pool)
            .await
            .unwrap();
        let f = Fixture {
            invoices: Invoices::from_pool(pool.clone()),
            pool,
            account,
        };

        let (paid, paid_ref) = f.invoice("5", "native", None).await;
        let (partial, partial_ref) = f.invoice("5", "native", None).await;
        let (over, _) = f
            .invoice("1", "native", Some(&format!("{}-OVER", f.account)))
            .await;
        let usdc_issuer = "GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN";
        let (_, usdc_ref) = f.invoice("5", &format!("USDC:{usdc_issuer}"), None).await;

        f.payment("paid", "5", Some(("id", paid_ref.to_string())))
            .await;
        f.payment("partial", "3", Some(("id", partial_ref.to_string())))
            .await;
        f.payment("over", "2", Some(("text", format!("{}-over", f.account))))
            .await;
        f.payment("noref", "2.5", None).await;
        f.payment("unknown", "1", Some(("id", "999999999".into())))
            .await;
        f.payment("asset", "5", Some(("id", usdc_ref.to_string())))
            .await;
        f.payment("late", "4", Some(("text", format!("{}-LATE", f.account))))
            .await;

        let engine = Engine::new(f.pool.clone());
        engine.run_once().await;

        assert_eq!(
            f.invoice_state(paid).await,
            ("paid".into(), "50000000".into())
        );
        assert_eq!(
            f.invoice_state(partial).await,
            ("partial".into(), "30000000".into())
        );
        assert_eq!(
            f.invoice_state(over).await,
            ("overpaid".into(), "20000000".into())
        );
        assert_eq!(f.payment_state("paid").await, ("matched".into(), None));
        assert_eq!(
            f.payment_state("noref").await,
            ("unmatched".into(), Some("no_reference".into()))
        );
        assert_eq!(
            f.payment_state("unknown").await,
            ("unmatched".into(), Some("no_invoice".into()))
        );
        assert_eq!(
            f.payment_state("asset").await,
            ("unmatched".into(), Some("asset_mismatch".into()))
        );
        assert_eq!(
            f.payment_state("late").await,
            ("unmatched".into(), Some("no_invoice".into()))
        );

        // The invoice for the early payment is created afterwards; the recheck
        // pass picks it up.
        let (late, _) = f
            .invoice("4", "native", Some(&format!("{}-LATE", f.account)))
            .await;
        engine.run_once().await;
        assert_eq!(
            f.invoice_state(late).await,
            ("paid".into(), "40000000".into())
        );
        assert_eq!(f.payment_state("late").await, ("matched".into(), None));

        // Running again changes nothing.
        engine.run_once().await;
        let allocations: i64 = sqlx::query_scalar(
            "select count(*) from payment_allocations a join payments p on p.id = a.payment_id
             where p.account = $1",
        )
        .bind(&f.account)
        .fetch_one(&f.pool)
        .await
        .unwrap();
        assert_eq!(allocations, 4);
    }
}
