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
use stellar_xdr::curr::{Limits, PublicKey, ReadXdr, ScAddress, ScVal};

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

/// Decoder for Stellar Asset Contract (SAC) / token `transfer` events.
///
/// A transfer event is shaped like:
///   topics: [ Symbol("transfer"), Address(from), Address(to), <asset?> ]
///   value:  i128(amount)
/// (SAC adds a 4th topic with the asset string; plain tokens omit it.)
pub struct TokenDecoder;

impl Decoder for TokenDecoder {
    fn name(&self) -> &'static str {
        "token"
    }

    fn decode(&self, event: &RawEvent) -> Option<DecodedEvent> {
        // topic[0] must decode to the symbol "transfer".
        let topic0 = parse_scval(event.topics.first()?)?;
        if as_symbol(&topic0).as_deref() != Some("transfer") {
            return None;
        }

        // topic[1] = from address, topic[2] = to address.
        let from = as_address(&parse_scval(event.topics.get(1)?)?)?;
        let to = as_address(&parse_scval(event.topics.get(2)?)?)?;
        // value = the transferred amount (i128).
        let amount = as_i128(&parse_scval(&event.data)?)?;

        Some(DecodedEvent {
            kind: "transfer".into(),
            fields: vec![
                ("from".into(), from),
                ("to".into(), to),
                ("amount".into(), amount.to_string()),
            ],
        })
    }
}

/// Parse one base64-XDR `ScVal` (a single topic or the event value).
fn parse_scval(b64: &str) -> Option<ScVal> {
    ScVal::from_xdr_base64(b64, Limits::none()).ok()
}

/// The string of a `ScVal::Symbol`, else `None`.
fn as_symbol(v: &ScVal) -> Option<String> {
    match v {
        ScVal::Symbol(s) => Some(s.to_string()),
        _ => None,
    }
}

/// The strkey (`G…` account / `C…` contract) of a `ScVal::Address`, else `None`.
fn as_address(v: &ScVal) -> Option<String> {
    let ScVal::Address(addr) = v else {
        return None;
    };
    match addr {
        ScAddress::Account(account_id) => {
            let PublicKey::PublicKeyTypeEd25519(key) = &account_id.0;
            Some(stellar_strkey::ed25519::PublicKey(key.0).to_string())
        }
        ScAddress::Contract(hash) => Some(stellar_strkey::Contract(hash.0).to_string()),
    }
}

/// The value of a `ScVal::I128`, reassembled from its hi/lo parts, else `None`.
fn as_i128(v: &ScVal) -> Option<i128> {
    match v {
        ScVal::I128(p) => Some(((p.hi as i128) << 64) | (p.lo as i128)),
        _ => None,
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
    use stellar_xdr::curr::{AccountId, Int128Parts, ScSymbol, ScVal, Uint256, WriteXdr};

    /// Encode an `ScVal` to base64 XDR, exactly as it appears in a real event.
    fn b64(v: &ScVal) -> String {
        v.to_xdr_base64(Limits::none()).unwrap()
    }

    fn symbol(s: &str) -> ScVal {
        ScVal::Symbol(ScSymbol(s.try_into().unwrap()))
    }

    /// An account (`G…`) address ScVal from 32 raw bytes.
    fn account(seed: u8) -> ScVal {
        let key = PublicKey::PublicKeyTypeEd25519(Uint256([seed; 32]));
        ScVal::Address(ScAddress::Account(AccountId(key)))
    }

    fn i128_val(n: i128) -> ScVal {
        ScVal::I128(Int128Parts {
            hi: (n >> 64) as i64,
            lo: n as u64,
        })
    }

    /// Build a transfer event the way RPC would deliver it (base64 XDR).
    fn transfer_event(amount: i128) -> RawEvent {
        RawEvent {
            ledger: 42,
            contract_id: "CABC".into(),
            topics: vec![b64(&symbol("transfer")), b64(&account(1)), b64(&account(2))],
            data: b64(&i128_val(amount)),
        }
    }

    #[test]
    fn decodes_transfer_into_from_to_amount() {
        let decoded = TokenDecoder.decode(&transfer_event(1000)).expect("decodes");
        assert_eq!(decoded.kind, "transfer");

        let get = |k: &str| {
            decoded
                .fields
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.as_str())
        };
        assert!(get("from").unwrap().starts_with('G'), "from is a G-strkey");
        assert!(get("to").unwrap().starts_with('G'), "to is a G-strkey");
        assert_ne!(get("from"), get("to"));
        assert_eq!(get("amount"), Some("1000"));
    }

    #[test]
    fn decodes_large_amount_without_overflow() {
        let big = 170_141_183_460_469_231_731i128; // > u64::MAX, exercises hi/lo
        let decoded = TokenDecoder.decode(&transfer_event(big)).unwrap();
        let amount = decoded.fields.iter().find(|(k, _)| k == "amount").unwrap();
        assert_eq!(amount.1, big.to_string());
    }

    #[test]
    fn ignores_non_transfer_events() {
        let mut ev = transfer_event(1);
        ev.topics[0] = b64(&symbol("mint")); // same shape, different symbol
        assert!(TokenDecoder.decode(&ev).is_none());
    }

    #[test]
    fn ignores_garbage_topics() {
        let ev = RawEvent {
            ledger: 1,
            contract_id: "CABC".into(),
            topics: vec!["not-valid-xdr".into()],
            data: String::new(),
        };
        assert!(TokenDecoder.decode(&ev).is_none());
    }

    #[test]
    fn default_registry_has_token() {
        assert!(default_registry().names().contains(&"token"));
    }
}
