# How reconciliation works

This page explains how Stardex matches a payment to an invoice. It is the place to start if you want to change or add a matching rule.

## The pieces

| Piece | Where | What it does |
|---|---|---|
| `payments` table | `db/migrations/0005_payments.sql` | One row per payment into a watched account, written by `stardex run` |
| `invoices` table | `db/migrations/0006_invoices.sql` | What customers owe, each with a unique `reference` |
| `payment_allocations` table | `db/migrations/0006_invoices.sql` | Which payment went to which invoice, and how it was matched |
| `recalc_invoice()` | `db/migrations/0006_invoices.sql` | Works out an invoice's status from its allocations |
| Matching rules | `ingestor/crates/reconcile/src/rules.rs` | Pure functions: given a payment and a candidate invoice, decide |
| Engine | `ingestor/crates/reconcile/src/engine.rs` | Reads new payments, applies the rules, writes the result |

## References

Every invoice gets a numeric `reference`, starting at 100001. A customer pays in one of two ways, and both arrive the same way:

1. **To the muxed address** built from the business account and the reference (`M...`). Stellar delivers it to the base `G...` account with the reference attached.
2. **To the base account with the reference as a `MEMO_ID`.**

Since Protocol 23 (CAP-67), the payment's transfer event carries either one as `to_muxed_id`, which Stardex stores on the payment as `reference_type = id`.

A customer may also type the invoice number (for example `INV-0007`) as a text memo. That is stored as `reference_type = text` and matched against `invoices.number`, ignoring case and surrounding spaces. A text memo that is only digits is also tried as a reference. Hash memos are not used for matching.

## The rules

`decide()` in `rules.rs` runs these checks in order:

| Check | Result |
|---|---|
| The payment has no usable reference | Unmatched, `no_reference` |
| No invoice on the same account has that reference or number | Unmatched, `no_invoice` |
| The invoice is cancelled | Unmatched, `invoice_cancelled` |
| The invoice asks for a different asset | Unmatched, `asset_mismatch` |
| Otherwise | The whole payment is allocated to the invoice |

The rules never decide paid versus partial. That is always `recalc_invoice()`'s job, so the engine and the backend (for manual matching) cannot disagree.

## Invoice status

`recalc_invoice(id)` sums the invoice's allocations and sets:

| Received | Status |
|---|---|
| nothing | `open` |
| less than the amount | `partial` |
| exactly the amount | `paid` (with `paid_at`) |
| more than the amount | `overpaid` (with `paid_at`) |

A `cancelled` invoice stays cancelled whatever it receives. A paid invoice still accepts further payments with its reference, which is how it becomes overpaid.

## The engine

`stardex reconcile` runs two passes on every tick:

1. **New payments.** Reads payments after its cursor (`stream_cursors`, name `reconcile`), decides each one still `unmatched`, and moves the cursor, all in one transaction. On first run it starts from the beginning, so payments recorded before the engine existed are matched too.
2. **Recheck.** A payment can arrive before its invoice is created. Payments from the last 30 days that were left `no_invoice` are checked again once a newer invoice exists on the same account.

Payments that someone matched or ignored by hand are never touched by the engine. Allocations are unique per payment, so running the engine twice never double counts.

## Amounts

Amounts are integers in the asset's smallest unit (`i128` in Rust, `numeric(39,0)` in Postgres). For XLM and other classic assets that is 7 decimal places, so 5 XLM is `50000000`. There is no floating point anywhere in this path.

## Adding a rule

1. Add a variant to `Reason` in `rules.rs` if the rule can leave a payment unmatched.
2. Add the check to `decide()` in the right order.
3. Add a unit test in `rules.rs` next to the others.
4. If the rule needs more data about the payment or invoice, add a field to `PaymentView` or `InvoiceView` and select it in `engine.rs`.
5. Run `cargo test -p stardex-reconcile`, and the database test with `DATABASE_URL=... cargo test -p stardex-reconcile -- --ignored`.
