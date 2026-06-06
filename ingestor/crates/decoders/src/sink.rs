//! Bridges decoding and storage: an [`EventSink`] that runs each event through
//! a [`Registry`] and writes the result to an [`EventStore`].

use async_trait::async_trait;
use serde_json::{Map, Value};
use stardex_core::{EventSink, EventStore, IngestError, RawEvent, StoredEvent};

use crate::Registry;

/// Decodes each event and stores it. Events no decoder recognizes are stored
/// raw (`kind = "raw"`) so nothing is lost while decoder coverage grows.
pub struct DecodingSink {
    registry: Registry,
    store: Box<dyn EventStore>,
}

impl DecodingSink {
    pub fn new(registry: Registry, store: Box<dyn EventStore>) -> Self {
        Self { registry, store }
    }
}

#[async_trait]
impl EventSink for DecodingSink {
    async fn handle(&self, event: RawEvent) -> Result<(), IngestError> {
        let decoded = self.registry.decode(&event);
        if decoded.is_empty() {
            self.store.store(&raw_row(&event)).await?;
            return Ok(());
        }
        for d in decoded {
            let fields = d
                .fields
                .into_iter()
                .map(|(k, v)| (k, Value::String(v)))
                .collect::<Map<String, Value>>();
            self.store
                .store(&StoredEvent {
                    contract_id: event.contract_id.clone(),
                    ledger: event.ledger,
                    kind: d.kind,
                    fields: Value::Object(fields),
                    closed_at: closed_at(&event),
                })
                .await?;
        }
        Ok(())
    }
}

/// Preserve an undecoded event as its raw base64 topics/data.
fn raw_row(event: &RawEvent) -> StoredEvent {
    let mut fields = Map::new();
    fields.insert(
        "topics".into(),
        Value::Array(event.topics.iter().cloned().map(Value::String).collect()),
    );
    fields.insert("data".into(), Value::String(event.data.clone()));
    StoredEvent {
        contract_id: event.contract_id.clone(),
        ledger: event.ledger,
        kind: "raw".into(),
        fields: Value::Object(fields),
        closed_at: closed_at(event),
    }
}

fn closed_at(event: &RawEvent) -> Option<String> {
    (!event.closed_at.is_empty()).then(|| event.closed_at.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::default_registry;
    use stardex_core::InMemoryEventStore;
    use std::sync::Arc;
    use stellar_xdr::curr::{
        AccountId, Int128Parts, Limits, PublicKey, ScAddress, ScSymbol, ScVal, Uint256, WriteXdr,
    };

    fn b64(v: &ScVal) -> String {
        v.to_xdr_base64(Limits::none()).unwrap()
    }

    fn account(seed: u8) -> ScVal {
        ScVal::Address(ScAddress::Account(AccountId(
            PublicKey::PublicKeyTypeEd25519(Uint256([seed; 32])),
        )))
    }

    fn transfer_event() -> RawEvent {
        RawEvent {
            ledger: 42,
            contract_id: "CABC".into(),
            topics: vec![
                b64(&ScVal::Symbol(ScSymbol("transfer".try_into().unwrap()))),
                b64(&account(1)),
                b64(&account(2)),
            ],
            data: b64(&ScVal::I128(Int128Parts { hi: 0, lo: 1000 })),
            closed_at: "2026-06-05T00:00:00Z".into(),
        }
    }

    // An InMemoryEventStore shared with the sink so the test can read it back.
    struct SharedStore(Arc<InMemoryEventStore>);

    #[async_trait]
    impl EventStore for SharedStore {
        async fn store(&self, event: &StoredEvent) -> Result<(), IngestError> {
            self.0.store(event).await
        }
    }

    #[tokio::test]
    async fn decodes_and_stores_a_transfer() {
        let inner = Arc::new(InMemoryEventStore::default());
        let sink = DecodingSink::new(default_registry(), Box::new(SharedStore(inner.clone())));

        sink.handle(transfer_event()).await.unwrap();

        let rows = inner.events();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, "transfer");
        assert_eq!(rows[0].fields["amount"], "1000");
        assert_eq!(rows[0].closed_at.as_deref(), Some("2026-06-05T00:00:00Z"));
    }

    #[tokio::test]
    async fn stores_unknown_events_raw() {
        let inner = Arc::new(InMemoryEventStore::default());
        let sink = DecodingSink::new(default_registry(), Box::new(SharedStore(inner.clone())));

        let unknown = RawEvent {
            ledger: 1,
            contract_id: "CABC".into(),
            topics: vec!["not-valid-xdr".into()],
            data: String::new(),
            closed_at: String::new(),
        };
        sink.handle(unknown).await.unwrap();

        let rows = inner.events();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, "raw");
    }
}
