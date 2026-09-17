//! Matching rules. Pure functions with no database, so each rule can be read
//! and tested on its own.

/// The parts of a payment the rules look at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentView {
    pub id: i64,
    pub account: String,
    pub asset: String,
    pub amount: i128,
    /// `id`, `text` or `hash`.
    pub reference_type: Option<String>,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvoiceStatus {
    Open,
    Partial,
    Paid,
    Overpaid,
    Cancelled,
}

impl InvoiceStatus {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "open" => Some(Self::Open),
            "partial" => Some(Self::Partial),
            "paid" => Some(Self::Paid),
            "overpaid" => Some(Self::Overpaid),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Partial => "partial",
            Self::Paid => "paid",
            Self::Overpaid => "overpaid",
            Self::Cancelled => "cancelled",
        }
    }
}

/// The parts of an invoice the rules look at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceView {
    pub id: i64,
    pub account: String,
    pub number: String,
    pub reference: i64,
    pub asset: String,
    pub status: InvoiceStatus,
}

/// What to look an invoice up by, derived from a payment's reference.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LookupKey {
    /// Matches `invoices.reference`.
    pub reference: Option<i64>,
    /// Matches `invoices.number`, case insensitive.
    pub number: Option<String>,
}

/// How to find the invoice a payment is for, or `None` if it carries nothing
/// usable. A muxed ID or memo ID is the invoice reference. A text memo can be
/// the invoice number ("INV-100001") or the reference typed as text.
/// Hash memos are not used for matching.
pub fn lookup_key(payment: &PaymentView) -> Option<LookupKey> {
    let value = payment.reference.as_deref()?.trim();
    match payment.reference_type.as_deref()? {
        "id" => Some(LookupKey {
            reference: value.parse().ok(),
            number: None,
        })
        .filter(|k| k.reference.is_some()),
        "text" if !value.is_empty() => Some(LookupKey {
            reference: value.parse().ok(),
            number: Some(value.to_string()),
        }),
        _ => None,
    }
}

/// Why a payment was left unmatched. Stored in `payments.unmatched_reason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// No memo or muxed ID, or one that cannot identify an invoice.
    NoReference,
    /// A reference was given but no invoice on this account has it.
    NoInvoice,
    /// The invoice asks for a different asset.
    AssetMismatch,
    /// The invoice was cancelled.
    InvoiceCancelled,
}

impl Reason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoReference => "no_reference",
            Self::NoInvoice => "no_invoice",
            Self::AssetMismatch => "asset_mismatch",
            Self::InvoiceCancelled => "invoice_cancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Allocate `amount` of the payment to the invoice.
    Allocate {
        invoice_id: i64,
        amount: i128,
    },
    Unmatched(Reason),
}

/// Decide what to do with `payment`, given the invoice found by its
/// [`lookup_key`] (if any). The whole payment goes to the invoice; whether that
/// leaves it partly paid, paid or overpaid is worked out by `recalc_invoice`.
pub fn decide(payment: &PaymentView, candidate: Option<&InvoiceView>) -> Decision {
    if lookup_key(payment).is_none() {
        return Decision::Unmatched(Reason::NoReference);
    }
    let Some(invoice) = candidate.filter(|i| i.account == payment.account) else {
        return Decision::Unmatched(Reason::NoInvoice);
    };
    if invoice.status == InvoiceStatus::Cancelled {
        return Decision::Unmatched(Reason::InvoiceCancelled);
    }
    if invoice.asset != payment.asset {
        return Decision::Unmatched(Reason::AssetMismatch);
    }
    Decision::Allocate {
        invoice_id: invoice.id,
        amount: payment.amount,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payment(reference_type: Option<&str>, reference: Option<&str>) -> PaymentView {
        PaymentView {
            id: 7,
            account: "GBIZ".into(),
            asset: "native".into(),
            amount: 50_000_000,
            reference_type: reference_type.map(Into::into),
            reference: reference.map(Into::into),
        }
    }

    fn invoice() -> InvoiceView {
        InvoiceView {
            id: 1,
            account: "GBIZ".into(),
            number: "INV-100042".into(),
            reference: 100_042,
            asset: "native".into(),
            status: InvoiceStatus::Open,
        }
    }

    #[test]
    fn memo_id_looks_up_by_reference() {
        let key = lookup_key(&payment(Some("id"), Some("100042"))).unwrap();
        assert_eq!(key.reference, Some(100_042));
        assert_eq!(key.number, None);
    }

    #[test]
    fn text_memo_looks_up_by_number_and_numeric_text() {
        let key = lookup_key(&payment(Some("text"), Some("  INV-100042 "))).unwrap();
        assert_eq!(key.number.as_deref(), Some("INV-100042"));
        assert_eq!(key.reference, None);

        let key = lookup_key(&payment(Some("text"), Some("100042"))).unwrap();
        assert_eq!(key.reference, Some(100_042));
    }

    #[test]
    fn hash_empty_and_missing_references_are_unusable() {
        assert_eq!(lookup_key(&payment(Some("hash"), Some("abcd"))), None);
        assert_eq!(lookup_key(&payment(Some("text"), Some("   "))), None);
        assert_eq!(lookup_key(&payment(None, None)), None);
        // A u64 muxed ID too large for an invoice reference.
        assert_eq!(
            lookup_key(&payment(Some("id"), Some("18446744073709551615"))),
            None
        );
    }

    #[test]
    fn allocates_the_whole_payment_to_a_matching_invoice() {
        let decision = decide(&payment(Some("id"), Some("100042")), Some(&invoice()));
        assert_eq!(
            decision,
            Decision::Allocate {
                invoice_id: 1,
                amount: 50_000_000
            }
        );
    }

    #[test]
    fn paid_invoices_still_take_extra_payments() {
        let mut paid = invoice();
        paid.status = InvoiceStatus::Paid;
        assert!(matches!(
            decide(&payment(Some("id"), Some("100042")), Some(&paid)),
            Decision::Allocate { .. }
        ));
    }

    #[test]
    fn no_reference() {
        assert_eq!(
            decide(&payment(None, None), Some(&invoice())),
            Decision::Unmatched(Reason::NoReference)
        );
    }

    #[test]
    fn no_invoice_found_or_invoice_on_another_account() {
        let p = payment(Some("id"), Some("100042"));
        assert_eq!(decide(&p, None), Decision::Unmatched(Reason::NoInvoice));

        let mut elsewhere = invoice();
        elsewhere.account = "GOTHER".into();
        assert_eq!(
            decide(&p, Some(&elsewhere)),
            Decision::Unmatched(Reason::NoInvoice)
        );
    }

    #[test]
    fn cancelled_invoice() {
        let mut cancelled = invoice();
        cancelled.status = InvoiceStatus::Cancelled;
        assert_eq!(
            decide(&payment(Some("id"), Some("100042")), Some(&cancelled)),
            Decision::Unmatched(Reason::InvoiceCancelled)
        );
    }

    #[test]
    fn asset_mismatch() {
        let mut usdc = invoice();
        usdc.asset = "USDC:GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN".into();
        assert_eq!(
            decide(&payment(Some("id"), Some("100042")), Some(&usdc)),
            Decision::Unmatched(Reason::AssetMismatch)
        );
    }

    #[test]
    fn status_round_trips() {
        for s in ["open", "partial", "paid", "overpaid", "cancelled"] {
            assert_eq!(InvoiceStatus::parse(s).unwrap().as_str(), s);
        }
        assert_eq!(InvoiceStatus::parse("nope"), None);
    }
}
