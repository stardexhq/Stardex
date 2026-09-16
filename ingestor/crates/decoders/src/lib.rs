pub mod sink;

pub use sink::DecodingSink;

use stardex_core::RawEvent;
use stellar_xdr::{Limits, ReadXdr, ScMap, ScVal};

/// A decoded, typed event ready to be stored.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedEvent {
    pub kind: String,
    pub fields: Vec<(String, String)>,
}

/// Implement this to teach Stardex about one contract's events.
pub trait Decoder: Send + Sync {
    fn name(&self) -> &'static str;

    /// `Some(decoded)` if this decoder understands the event, else `None`.
    fn decode(&self, event: &RawEvent) -> Option<DecodedEvent>;
}

/// Registered decoders, dispatched against each event.
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

    pub fn names(&self) -> Vec<&'static str> {
        self.decoders.iter().map(|d| d.name()).collect()
    }

    pub fn decode(&self, event: &RawEvent) -> Vec<DecodedEvent> {
        self.decoders
            .iter()
            .filter_map(|d| d.decode(event))
            .collect()
    }
}

/// A token transfer. Covers classic payments (CAP-67 unified events) and
/// Soroban token transfers (SEP-41).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transfer {
    pub from: String,
    pub to: String,
    pub amount: i128,
    /// SEP-11 asset (`native` or `CODE:ISSUER`). Only Stellar Asset Contract
    /// events carry it; custom Soroban tokens leave it out.
    pub asset: Option<String>,
    /// The muxed ID or transaction memo that came with the payment.
    pub to_muxed_id: Option<MuxedId>,
}

/// CAP-67 folds both a muxed destination ID and a transaction memo into
/// `to_muxed_id`, with the muxed ID taking precedence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MuxedId {
    /// A muxed account ID or `MEMO_ID`.
    Id(u64),
    /// `MEMO_TEXT`.
    Text(String),
    /// `MEMO_HASH` or `MEMO_RETURN`, hex encoded.
    Hash(String),
}

impl MuxedId {
    pub fn kind(&self) -> &'static str {
        match self {
            MuxedId::Id(_) => "id",
            MuxedId::Text(_) => "text",
            MuxedId::Hash(_) => "hash",
        }
    }

    pub fn value(&self) -> String {
        match self {
            MuxedId::Id(id) => id.to_string(),
            MuxedId::Text(text) => text.clone(),
            MuxedId::Hash(hex) => hex.clone(),
        }
    }
}

/// Decode a `transfer` event:
/// `topics: [Symbol("transfer"), Address(from), Address(to), String(asset)?]`,
/// `value: i128(amount)` or `Map { amount: i128, to_muxed_id: u64 | string | bytes }`.
pub fn decode_transfer(event: &RawEvent) -> Option<Transfer> {
    let topic0 = parse_scval(event.topics.first()?)?;
    if as_symbol(&topic0).as_deref() != Some("transfer") {
        return None;
    }

    let from = as_address(&parse_scval(event.topics.get(1)?)?)?;
    let to = as_address(&parse_scval(event.topics.get(2)?)?)?;
    let asset = match event.topics.get(3) {
        Some(b64) => Some(as_string(&parse_scval(b64)?)?),
        None => None,
    };

    let (amount, to_muxed_id) = match parse_scval(&event.data)? {
        ScVal::Map(Some(map)) => {
            let amount = as_i128(map_get(&map, "amount")?)?;
            let muxed = match map_get(&map, "to_muxed_id") {
                Some(v) => Some(as_muxed_id(v)?),
                None => None,
            };
            (amount, muxed)
        }
        other => (as_i128(&other)?, None),
    };

    Some(Transfer {
        from,
        to,
        amount,
        asset,
        to_muxed_id,
    })
}

/// Decoder for token `transfer` events, stored as `kind = "transfer"`.
pub struct TokenDecoder;

impl Decoder for TokenDecoder {
    fn name(&self) -> &'static str {
        "token"
    }

    fn decode(&self, event: &RawEvent) -> Option<DecodedEvent> {
        let transfer = decode_transfer(event)?;

        let mut fields = vec![
            ("from".to_string(), transfer.from),
            ("to".to_string(), transfer.to),
            ("amount".to_string(), transfer.amount.to_string()),
        ];
        if let Some(asset) = transfer.asset {
            fields.push(("asset".into(), asset));
        }
        if let Some(muxed) = transfer.to_muxed_id {
            fields.push(("to_muxed_id".into(), muxed.value()));
            fields.push(("to_muxed_id_type".into(), muxed.kind().into()));
        }

        Some(DecodedEvent {
            kind: "transfer".into(),
            fields,
        })
    }
}

fn parse_scval(b64: &str) -> Option<ScVal> {
    ScVal::from_xdr_base64(b64, Limits::none()).ok()
}

fn as_symbol(v: &ScVal) -> Option<String> {
    match v {
        ScVal::Symbol(s) => s.0.to_utf8_string().ok(),
        _ => None,
    }
}

fn as_string(v: &ScVal) -> Option<String> {
    match v {
        ScVal::String(s) => s.0.to_utf8_string().ok(),
        _ => None,
    }
}

/// Renders any address kind as its strkey: G (account), C (contract),
/// M (muxed), B (claimable balance) or L (liquidity pool).
fn as_address(v: &ScVal) -> Option<String> {
    match v {
        ScVal::Address(addr) => Some(addr.to_string()),
        _ => None,
    }
}

/// Soroban encodes i128 as hi/lo halves.
fn as_i128(v: &ScVal) -> Option<i128> {
    match v {
        ScVal::I128(p) => Some(((p.hi as i128) << 64) | (p.lo as i128)),
        _ => None,
    }
}

fn as_muxed_id(v: &ScVal) -> Option<MuxedId> {
    match v {
        ScVal::U64(id) => Some(MuxedId::Id(*id)),
        ScVal::String(s) => Some(MuxedId::Text(s.0.to_utf8_string_lossy())),
        ScVal::Bytes(b) => Some(MuxedId::Hash(hex(b.0.as_slice()))),
        _ => None,
    }
}

fn map_get<'a>(map: &'a ScMap, key: &str) -> Option<&'a ScVal> {
    map.0
        .iter()
        .find(|entry| as_symbol(&entry.key).as_deref() == Some(key))
        .map(|entry| &entry.val)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Registry preloaded with the built-in decoders.
pub fn default_registry() -> Registry {
    let mut r = Registry::new();
    r.register(Box::new(TokenDecoder));
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellar_xdr::{
        AccountId, ClaimableBalanceId, ContractId, Hash, Int128Parts, PublicKey, ScAddress,
        ScBytes, ScMapEntry, ScString, ScSymbol, Uint256, WriteXdr,
    };

    fn b64(v: &ScVal) -> String {
        v.to_xdr_base64(Limits::none()).unwrap()
    }

    fn symbol(s: &str) -> ScVal {
        ScVal::Symbol(ScSymbol(s.try_into().unwrap()))
    }

    fn string(s: &str) -> ScVal {
        ScVal::String(ScString(s.try_into().unwrap()))
    }

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

    fn map(entries: Vec<(&str, ScVal)>) -> ScVal {
        let entries: Vec<ScMapEntry> = entries
            .into_iter()
            .map(|(k, v)| ScMapEntry {
                key: symbol(k),
                val: v,
            })
            .collect();
        ScVal::Map(Some(ScMap(entries.try_into().unwrap())))
    }

    fn event(topics: Vec<ScVal>, data: ScVal) -> RawEvent {
        RawEvent {
            ledger: 42,
            contract_id: "CABC".into(),
            topics: topics.iter().map(b64).collect(),
            data: b64(&data),
            closed_at: "2026-06-05T00:00:00Z".into(),
        }
    }

    /// A classic payment as CAP-67 emits it: 4 topics with the SEP-11 asset.
    fn classic_payment(data: ScVal) -> RawEvent {
        event(
            vec![
                symbol("transfer"),
                account(1),
                account(2),
                string("USDC:GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN"),
            ],
            data,
        )
    }

    fn transfer_event(amount: i128) -> RawEvent {
        event(
            vec![symbol("transfer"), account(1), account(2)],
            i128_val(amount),
        )
    }

    fn field<'a>(decoded: &'a DecodedEvent, key: &str) -> Option<&'a str> {
        decoded
            .fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn decodes_transfer_into_from_to_amount() {
        let decoded = TokenDecoder.decode(&transfer_event(1000)).expect("decodes");
        assert_eq!(decoded.kind, "transfer");
        assert!(field(&decoded, "from").unwrap().starts_with('G'));
        assert!(field(&decoded, "to").unwrap().starts_with('G'));
        assert_ne!(field(&decoded, "from"), field(&decoded, "to"));
        assert_eq!(field(&decoded, "amount"), Some("1000"));
        assert_eq!(field(&decoded, "asset"), None);
        assert_eq!(field(&decoded, "to_muxed_id"), None);
    }

    #[test]
    fn decodes_large_amount_without_overflow() {
        let big = 170_141_183_460_469_231_731i128;
        let decoded = TokenDecoder.decode(&transfer_event(big)).unwrap();
        assert_eq!(field(&decoded, "amount"), Some(big.to_string().as_str()));
    }

    #[test]
    fn reads_the_asset_topic() {
        let transfer = decode_transfer(&classic_payment(i128_val(5_000_000))).unwrap();
        assert_eq!(
            transfer.asset.as_deref(),
            Some("USDC:GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN")
        );
        assert_eq!(transfer.amount, 5_000_000);
        assert_eq!(transfer.to_muxed_id, None);
    }

    #[test]
    fn reads_a_memo_id_from_map_data() {
        let data = map(vec![
            ("amount", i128_val(5_000_000)),
            ("to_muxed_id", ScVal::U64(100_042)),
        ]);
        let decoded = TokenDecoder.decode(&classic_payment(data)).unwrap();
        assert_eq!(field(&decoded, "amount"), Some("5000000"));
        assert_eq!(field(&decoded, "to_muxed_id"), Some("100042"));
        assert_eq!(field(&decoded, "to_muxed_id_type"), Some("id"));
    }

    #[test]
    fn reads_a_memo_text_from_map_data() {
        let data = map(vec![
            ("amount", i128_val(10)),
            ("to_muxed_id", string("INV-0042")),
        ]);
        let transfer = decode_transfer(&classic_payment(data)).unwrap();
        assert_eq!(transfer.to_muxed_id, Some(MuxedId::Text("INV-0042".into())));
    }

    #[test]
    fn reads_a_memo_hash_as_hex() {
        let bytes = ScBytes(vec![0xab; 32].try_into().unwrap());
        let data = map(vec![
            ("amount", i128_val(10)),
            ("to_muxed_id", ScVal::Bytes(bytes)),
        ]);
        let muxed = decode_transfer(&classic_payment(data))
            .unwrap()
            .to_muxed_id
            .unwrap();
        assert_eq!(muxed.kind(), "hash");
        assert_eq!(muxed.value(), "ab".repeat(32));
    }

    #[test]
    fn map_without_to_muxed_id_still_decodes() {
        let data = map(vec![("amount", i128_val(7))]);
        let transfer = decode_transfer(&classic_payment(data)).unwrap();
        assert_eq!(transfer.amount, 7);
        assert_eq!(transfer.to_muxed_id, None);
    }

    #[test]
    fn renders_claimable_balance_and_contract_addresses() {
        let claimable = ScVal::Address(ScAddress::ClaimableBalance(
            ClaimableBalanceId::ClaimableBalanceIdTypeV0(Hash([7; 32])),
        ));
        let contract = ScVal::Address(ScAddress::Contract(ContractId(Hash([9; 32]))));
        let transfer = decode_transfer(&event(
            vec![symbol("transfer"), claimable, contract],
            i128_val(1),
        ))
        .unwrap();
        assert!(transfer.from.starts_with('B'), "got {}", transfer.from);
        assert!(transfer.to.starts_with('C'), "got {}", transfer.to);
    }

    #[test]
    fn ignores_non_transfer_events() {
        let mut ev = transfer_event(1);
        ev.topics[0] = b64(&symbol("mint"));
        assert!(TokenDecoder.decode(&ev).is_none());
    }

    #[test]
    fn ignores_garbage_topics() {
        let ev = RawEvent {
            ledger: 1,
            contract_id: "CABC".into(),
            topics: vec!["not-valid-xdr".into()],
            data: String::new(),
            closed_at: String::new(),
        };
        assert!(TokenDecoder.decode(&ev).is_none());
    }

    #[test]
    fn ignores_map_data_without_amount() {
        let data = map(vec![("to_muxed_id", ScVal::U64(1))]);
        assert!(decode_transfer(&classic_payment(data)).is_none());
    }

    #[test]
    fn default_registry_has_token() {
        assert!(default_registry().names().contains(&"token"));
    }
}

/// Real CAP-67 events captured from testnet RPC `getEvents`, so the decoder is
/// checked against what the network emits rather than only hand built values.
#[cfg(test)]
mod testnet_fixtures {
    use super::*;

    const TRANSFER: &str = "AAAADwAAAAh0cmFuc2Zlcg==";
    const NATIVE: &str = "AAAADgAAAAZuYXRpdmUAAA==";
    const PAYER: &str = "AAAAEgAAAAAAAAAA/t89CuPlx7YW2JEo5Qg8LjgHksEH82BA/4wDoLb1LNw=";
    const BUSINESS: &str = "AAAAEgAAAAAAAAAAZl1n2qpHwPeNMiXkzM19T6d08GwLyX+tPXKAv4lzYMg=";
    const BUSINESS_G: &str = "GBTF2Z62VJD4B54NGIS6JTGNPVH2O5HQNQF4S75NHVZIBP4JONQMRP7K";

    fn fixture(topics: &[&str], value: &str) -> RawEvent {
        RawEvent {
            ledger: 4_710_943,
            contract_id: "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC".into(),
            topics: topics.iter().map(|t| t.to_string()).collect(),
            data: value.into(),
            closed_at: String::new(),
        }
    }

    /// 5 XLM with `MEMO_ID` 100042 (tx fa08b760...4550).
    #[test]
    fn memo_id_payment() {
        let ev = fixture(
            &[TRANSFER, PAYER, BUSINESS, NATIVE],
            "AAAAEQAAAAEAAAACAAAADwAAAAZhbW91bnQAAAAAAAoAAAAAAAAAAAAAAAAC+vCAAAAADwAAAAt0b19tdXhlZF9pZAAAAAAFAAAAAAABhso=",
        );
        let t = decode_transfer(&ev).unwrap();
        assert_eq!(t.to, BUSINESS_G);
        assert_eq!(t.amount, 50_000_000);
        assert_eq!(t.asset.as_deref(), Some("native"));
        assert_eq!(t.to_muxed_id, Some(MuxedId::Id(100_042)));
    }

    /// 5 XLM sent to the muxed address M...(id 100043) with no memo
    /// (tx 06ab2516...a90e). `to` is still the base G address.
    #[test]
    fn muxed_destination_payment() {
        let ev = fixture(
            &[TRANSFER, PAYER, BUSINESS, NATIVE],
            "AAAAEQAAAAEAAAACAAAADwAAAAZhbW91bnQAAAAAAAoAAAAAAAAAAAAAAAAC+vCAAAAADwAAAAt0b19tdXhlZF9pZAAAAAAFAAAAAAABhss=",
        );
        let t = decode_transfer(&ev).unwrap();
        assert_eq!(t.to, BUSINESS_G);
        assert_eq!(t.to_muxed_id, Some(MuxedId::Id(100_043)));
    }

    /// 10 XLM with `MEMO_TEXT` "note 1" (tx 1a72a496...c599).
    #[test]
    fn memo_text_payment() {
        let ev = fixture(
            &[
                TRANSFER,
                "AAAAEgAAAAAAAAAAxbdFVp63HQ+jzdm3eC7kQa8U0hLJXlNU+V6ebmv1kMU=",
                "AAAAEgAAAAAAAAAAuLGgztvHi6z1oXFAcd21NWIuQCYm8KIDlBMJ4S1d3QY=",
                NATIVE,
            ],
            "AAAAEQAAAAEAAAACAAAADwAAAAZhbW91bnQAAAAAAAoAAAAAAAAAAAAAAAAF9eEAAAAADwAAAAt0b19tdXhlZF9pZAAAAAAOAAAABm5vdGUgMQAA",
        );
        let t = decode_transfer(&ev).unwrap();
        assert_eq!(t.amount, 100_000_000);
        assert_eq!(t.to_muxed_id, Some(MuxedId::Text("note 1".into())));
    }

    /// 1 stroop with `MEMO_HASH` (tx 031fbc79...f379).
    #[test]
    fn memo_hash_payment() {
        let account = "AAAAEgAAAAAAAAAA/cvqzp62XaGzPtF38DzBiPM+3smvaEjTK2D6CNFv7WA=";
        let ev = fixture(
            &[TRANSFER, account, account, NATIVE],
            "AAAAEQAAAAEAAAACAAAADwAAAAZhbW91bnQAAAAAAAoAAAAAAAAAAAAAAAAAAAABAAAADwAAAAt0b19tdXhlZF9pZAAAAAANAAAAII+7BAK+I7aXYcc8//NYiLcfCQjYm+GNaTU6jdjqnb1R",
        );
        let t = decode_transfer(&ev).unwrap();
        assert_eq!(t.amount, 1);
        assert_eq!(
            t.to_muxed_id,
            Some(MuxedId::Hash(
                "8fbb0402be23b69761c73cfff35888b71f0908d89be18d69353a8dd8ea9dbd51".into()
            ))
        );
    }

    /// A non native asset: PYUSD with a text memo (tx 51c0dadf...a348).
    #[test]
    fn issued_asset_payment() {
        let ev = fixture(
            &[
                TRANSFER,
                "AAAAEgAAAAAAAAAABb7SuG6zHwPPzkD2GAq1nCR53zmsfEuHNElET9K3Vao=",
                "AAAAEgAAAAAAAAAAf0+4aD25Arvr2pjvSSn2e+fD+LCaYRwEhnfDDZAQSi0=",
                "AAAADgAAAD5QWVVTRDpHQlQyS0pES1VaWVpUUVBDU1I1N1ZaVDVOSkhJNEg3Rk9CNUxUNUZQUldTUjdJNUI0RlMzVVU3RwAA",
            ],
            "AAAAEQAAAAEAAAACAAAADwAAAAZhbW91bnQAAAAAAAoAAAAAAAAAAAAAAAAAAAABAAAADwAAAAt0b19tdXhlZF9pZAAAAAAOAAAAG1hMTSBlMmUgbW9uaXRvciB0cmFuc2FjdGlvbgA=",
        );
        let t = decode_transfer(&ev).unwrap();
        assert_eq!(
            t.asset.as_deref(),
            Some("PYUSD:GBT2KJDKUZYZTQPCSR57VZT5NJHI4H7FOB5LT5FPRWSR7I5B4FS3UU7G")
        );
        assert_eq!(
            t.to_muxed_id,
            Some(MuxedId::Text("XLM e2e monitor transaction".into()))
        );
    }
}
