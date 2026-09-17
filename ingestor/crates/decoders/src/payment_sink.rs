//! Turns transfer events on an account stream into payment rows.

use async_trait::async_trait;
use stardex_core::{EventSink, IngestError, Payment, PaymentStore, RawEvent};

use crate::decode_transfer;

/// Records every transfer paid into `account` as a [`Payment`]. Anything else
/// on the stream (outgoing transfers, payments to itself, events that are not
/// transfers) is ignored.
pub struct PaymentSink {
    account: String,
    store: Box<dyn PaymentStore>,
}

impl PaymentSink {
    pub fn new(account: impl Into<String>, store: Box<dyn PaymentStore>) -> Self {
        Self {
            account: account.into(),
            store,
        }
    }
}

#[async_trait]
impl EventSink for PaymentSink {
    async fn handle(&self, event: RawEvent) -> Result<(), IngestError> {
        let Some(transfer) = decode_transfer(&event) else {
            return Ok(());
        };
        if transfer.to != self.account || transfer.from == self.account {
            return Ok(());
        }

        let (reference_type, reference) = match &transfer.to_muxed_id {
            Some(id) => (Some(id.kind().to_string()), Some(id.value())),
            None => (None, None),
        };

        self.store
            .record(&Payment {
                event_id: event.event_id.clone(),
                tx_hash: event.tx_hash.clone(),
                ledger: event.ledger,
                closed_at: (!event.closed_at.is_empty()).then(|| event.closed_at.clone()),
                account: transfer.to,
                from_address: transfer.from,
                // Custom Soroban tokens carry no SEP-11 asset topic; the
                // contract id is their identity.
                asset: transfer.asset.unwrap_or_else(|| event.contract_id.clone()),
                asset_contract: event.contract_id.clone(),
                amount: transfer.amount,
                reference_type,
                reference,
            })
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stardex_core::InMemoryPaymentStore;

    use super::*;

    // Real testnet transfer: 5 XLM to BUSINESS with MEMO_ID 100042.
    const TRANSFER: &str = "AAAADwAAAAh0cmFuc2Zlcg==";
    const NATIVE: &str = "AAAADgAAAAZuYXRpdmUAAA==";
    const PAYER: &str = "AAAAEgAAAAAAAAAA/t89CuPlx7YW2JEo5Qg8LjgHksEH82BA/4wDoLb1LNw=";
    const BUSINESS: &str = "AAAAEgAAAAAAAAAAZl1n2qpHwPeNMiXkzM19T6d08GwLyX+tPXKAv4lzYMg=";
    const BUSINESS_G: &str = "GBTF2Z62VJD4B54NGIS6JTGNPVH2O5HQNQF4S75NHVZIBP4JONQMRP7K";
    const MEMO_ID_VALUE: &str = "AAAAEQAAAAEAAAACAAAADwAAAAZhbW91bnQAAAAAAAoAAAAAAAAAAAAAAAAC+vCAAAAADwAAAAt0b19tdXhlZF9pZAAAAAAFAAAAAAABhso=";
    const XLM_SAC: &str = "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC";

    struct Shared(Arc<InMemoryPaymentStore>);

    #[async_trait]
    impl PaymentStore for Shared {
        async fn record(&self, payment: &Payment) -> Result<bool, IngestError> {
            self.0.record(payment).await
        }
    }

    fn event(event_id: &str, from: &str, to: &str) -> RawEvent {
        RawEvent {
            event_id: event_id.into(),
            tx_hash: "fa08b760645256e8f9211a1dc6927e52b711078772c1172b391bd7948cbc4550".into(),
            ledger: 4_710_943,
            contract_id: XLM_SAC.into(),
            topics: vec![TRANSFER.into(), from.into(), to.into(), NATIVE.into()],
            data: MEMO_ID_VALUE.into(),
            closed_at: "2026-09-16T17:00:00Z".into(),
        }
    }

    fn sink() -> (PaymentSink, Arc<InMemoryPaymentStore>) {
        let store = Arc::new(InMemoryPaymentStore::default());
        let sink = PaymentSink::new(BUSINESS_G, Box::new(Shared(Arc::clone(&store))));
        (sink, store)
    }

    #[tokio::test]
    async fn records_an_incoming_payment_with_its_reference() {
        let (sink, store) = sink();
        sink.handle(event("e1", PAYER, BUSINESS)).await.unwrap();

        let payments = store.payments();
        assert_eq!(payments.len(), 1);
        let p = &payments[0];
        assert_eq!(p.event_id, "e1");
        assert_eq!(p.account, BUSINESS_G);
        assert!(p.from_address.starts_with('G'));
        assert_eq!(p.asset, "native");
        assert_eq!(p.asset_contract, XLM_SAC);
        assert_eq!(p.amount, 50_000_000);
        assert_eq!(p.reference_type.as_deref(), Some("id"));
        assert_eq!(p.reference.as_deref(), Some("100042"));
        assert_eq!(p.closed_at.as_deref(), Some("2026-09-16T17:00:00Z"));
    }

    #[tokio::test]
    async fn ignores_outgoing_and_self_payments() {
        let (sink, store) = sink();
        sink.handle(event("out", BUSINESS, PAYER)).await.unwrap();
        sink.handle(event("self", BUSINESS, BUSINESS))
            .await
            .unwrap();
        assert!(store.payments().is_empty());
    }

    #[tokio::test]
    async fn ignores_events_that_are_not_transfers() {
        let (sink, store) = sink();
        let mut ev = event("fee", PAYER, BUSINESS);
        ev.topics = vec!["AAAADwAAAANmZWUA".into(), PAYER.into()];
        sink.handle(ev).await.unwrap();
        assert!(store.payments().is_empty());
    }

    #[tokio::test]
    async fn replaying_an_event_does_not_duplicate_the_payment() {
        let (sink, store) = sink();
        sink.handle(event("e1", PAYER, BUSINESS)).await.unwrap();
        sink.handle(event("e1", PAYER, BUSINESS)).await.unwrap();
        assert_eq!(store.payments().len(), 1);
    }

    #[tokio::test]
    async fn custom_tokens_use_the_contract_as_asset() {
        let (sink, store) = sink();
        let mut ev = event("tok", PAYER, BUSINESS);
        ev.topics.truncate(3);
        ev.contract_id = "CTOKEN".into();
        sink.handle(ev).await.unwrap();
        assert_eq!(store.payments()[0].asset, "CTOKEN");
    }
}
