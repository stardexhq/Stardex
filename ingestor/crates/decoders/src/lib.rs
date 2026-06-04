//! Stardex **decoders** — the translators.
//!
//! A decoder turns a [`RawEvent`] (raw blockchain gibberish) into a
//! [`DecodedEvent`] (a clean, readable record). Adding support for a new
//! contract means writing a new `Decoder` and registering it here — you
//! never touch the core engine.
//!
//! Writing a decoder is the canonical `good first issue`. See the
//! "Write your own decoder" tutorial (backlog issue #30).

use stardex_core::RawEvent;

/// A decoded, typed event, ready to be stored.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedEvent {
    /// Decoder-defined kind, e.g. "transfer", "swap", "stream_create".
    pub kind: String,
    /// Decoded key/value fields (kept simple for now).
    pub fields: Vec<(String, String)>,
}

/// Implement this to teach Stardex about one contract's events.
pub trait Decoder {
    /// Short, unique name, e.g. "token", "soroswap".
    fn name(&self) -> &'static str;

    /// Return `Some(decoded)` if this decoder understands the event,
    /// otherwise `None`.
    fn decode(&self, event: &RawEvent) -> Option<DecodedEvent>;
}

/// Holds all registered decoders and dispatches events to them.
#[derive(Default)]
pub struct Registry {
    decoders: Vec<Box<dyn Decoder>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, decoder: Box<dyn Decoder>) {
        self.decoders.push(decoder);
    }

    /// Names of all registered decoders.
    pub fn names(&self) -> Vec<&'static str> {
        self.decoders.iter().map(|d| d.name()).collect()
    }

    /// Run every decoder against an event and collect what matches.
    pub fn decode(&self, event: &RawEvent) -> Vec<DecodedEvent> {
        self.decoders
            .iter()
            .filter_map(|d| d.decode(event))
            .collect()
    }
}

/// A minimal example: a token `transfer` decoder. See backlog issue #8.
///
/// TODO(#8): properly parse SAC/token transfer topics + data from XDR
/// (from/to/amount). This stub only recognizes the event by its topic.
pub struct TokenDecoder;

impl Decoder for TokenDecoder {
    fn name(&self) -> &'static str {
        "token"
    }

    fn decode(&self, event: &RawEvent) -> Option<DecodedEvent> {
        if event
            .topics
            .first()
            .map(|t| t == "transfer")
            .unwrap_or(false)
        {
            Some(DecodedEvent {
                kind: "transfer".into(),
                fields: vec![
                    ("contract".into(), event.contract_id.clone()),
                    ("ledger".into(), event.ledger.to_string()),
                ],
            })
        } else {
            None
        }
    }
}

/// Build a registry pre-loaded with the built-in decoders.
pub fn default_registry() -> Registry {
    let mut r = Registry::new();
    r.register(Box::new(TokenDecoder));
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(topic: &str) -> RawEvent {
        RawEvent {
            ledger: 42,
            contract_id: "CABC...".into(),
            topics: vec![topic.into()],
            data: String::new(),
        }
    }

    #[test]
    fn token_decoder_matches_transfer() {
        let decoded = TokenDecoder.decode(&sample("transfer")).unwrap();
        assert_eq!(decoded.kind, "transfer");
    }

    #[test]
    fn token_decoder_ignores_other_events() {
        assert!(TokenDecoder.decode(&sample("mint")).is_none());
    }

    #[test]
    fn default_registry_has_token() {
        assert!(default_registry().names().contains(&"token"));
    }
}
