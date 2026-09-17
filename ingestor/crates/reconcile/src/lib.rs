//! Matches recorded payments to invoices.
//!
//! [`rules`] holds the matching logic as pure functions, [`engine`] applies it
//! to the database, and [`invoices`] creates and lists invoices.

pub mod engine;
pub mod invoices;
pub mod rules;
pub mod stellar;

pub use engine::Engine;
pub use invoices::{CreateError, Invoice, Invoices, NewInvoice};
pub use rules::{decide, lookup_key, Decision, InvoiceStatus, Reason};
