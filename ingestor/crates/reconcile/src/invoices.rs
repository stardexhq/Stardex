//! Creating and listing invoices. Postgres only: invoice status comes from the
//! `recalc_invoice` SQL function, so there is no in-memory version to keep in
//! step with it.

use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use stardex_core::IngestError;

use crate::rules::InvoiceStatus;
use crate::stellar::validate_asset;

/// Fields for a new invoice. `amount` is in raw units of `asset`.
#[derive(Debug, Clone, Default)]
pub struct NewInvoice {
    pub account: String,
    pub asset: String,
    pub amount: i128,
    /// Defaults to `INV-<reference>`.
    pub number: Option<String>,
    pub customer_name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invoice {
    pub id: i64,
    pub number: String,
    pub account: String,
    pub customer_name: Option<String>,
    pub description: Option<String>,
    pub asset: String,
    pub amount: i128,
    pub amount_received: i128,
    /// The muxed ID / MEMO_ID customers pay with.
    pub reference: i64,
    pub status: InvoiceStatus,
    pub issued_at: String,
    pub paid_at: Option<String>,
}

#[derive(Debug)]
pub enum CreateError {
    Invalid(String),
    Db(IngestError),
}

impl std::fmt::Display for CreateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CreateError::Invalid(msg) => f.write_str(msg),
            CreateError::Db(e) => write!(f, "{e}"),
        }
    }
}

impl From<sqlx::Error> for CreateError {
    fn from(e: sqlx::Error) -> Self {
        CreateError::Db(e.into())
    }
}

const COLUMNS: &str = "id, number, account, customer_name, description, asset,
    amount::text as amount, amount_received::text as amount_received, reference, status,
    to_char(issued_at at time zone 'utc', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') as issued_at,
    to_char(paid_at at time zone 'utc', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') as paid_at";

pub struct Invoices {
    pool: PgPool,
}

impl Invoices {
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, new: &NewInvoice) -> Result<Invoice, CreateError> {
        validate_asset(&new.asset).map_err(CreateError::Invalid)?;
        if new.amount <= 0 {
            return Err(CreateError::Invalid(
                "amount must be greater than zero".into(),
            ));
        }
        let watched: Option<bool> =
            sqlx::query_scalar("select active from accounts where address = $1")
                .bind(&new.account)
                .fetch_optional(&self.pool)
                .await?;
        if watched != Some(true) {
            return Err(CreateError::Invalid(format!(
                "{} is not a watched account; add it with `stardex accounts add`",
                new.account
            )));
        }

        let sql = format!(
            "with r as (select nextval('invoice_reference_seq') as reference)
             insert into invoices (reference, number, account, customer_name, description,
                                   asset, amount)
             select r.reference, coalesce($1, 'INV-' || r.reference), $2, $3, $4, $5,
                    $6::numeric
             from r
             returning {COLUMNS}"
        );
        let row = sqlx::query(&sql)
            .bind(&new.number)
            .bind(&new.account)
            .bind(&new.customer_name)
            .bind(&new.description)
            .bind(&new.asset)
            .bind(new.amount.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(|e| match &e {
                sqlx::Error::Database(db) if db.is_unique_violation() => {
                    CreateError::Invalid(format!("invoice number {:?} is already used", new.number))
                }
                _ => e.into(),
            })?;
        to_invoice(&row).map_err(CreateError::Db)
    }

    /// Newest first, optionally filtered by account and status.
    pub async fn list(
        &self,
        account: Option<&str>,
        status: Option<InvoiceStatus>,
        limit: i64,
    ) -> Result<Vec<Invoice>, IngestError> {
        let sql = format!(
            "select {COLUMNS} from invoices
             where ($1::text is null or account = $1)
               and ($2::text is null or status = $2)
             order by id desc
             limit $3"
        );
        let rows = sqlx::query(&sql)
            .bind(account)
            .bind(status.map(|s| s.as_str()))
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(to_invoice).collect()
    }
}

fn to_invoice(row: &PgRow) -> Result<Invoice, IngestError> {
    let status: String = row.get("status");
    Ok(Invoice {
        id: row.get("id"),
        number: row.get("number"),
        account: row.get("account"),
        customer_name: row.get("customer_name"),
        description: row.get("description"),
        asset: row.get("asset"),
        amount: parse_units(row.get("amount"))?,
        amount_received: parse_units(row.get("amount_received"))?,
        reference: row.get("reference"),
        status: InvoiceStatus::parse(&status)
            .ok_or_else(|| IngestError::Store(format!("unknown invoice status {status}")))?,
        issued_at: row.get("issued_at"),
        paid_at: row.get("paid_at"),
    })
}

pub(crate) fn parse_units(text: String) -> Result<i128, IngestError> {
    text.parse()
        .map_err(|_| IngestError::Store(format!("bad amount {text}")))
}
