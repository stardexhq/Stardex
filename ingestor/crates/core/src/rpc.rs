//! Typed shapes for the Stellar RPC **`getEvents`** JSON-RPC call.
//!
//! These structs are just the *data shapes* — what we send and what we get
//! back — so the rest of the engine can work with typed Rust instead of raw
//! JSON. The actual network call lives in [`crate::rpc_client`] (Step 3).
//!
//! Reference: <https://developers.stellar.org/docs/data/apis/rpc/api-reference/methods/getEvents>
//!
//! ## What an exchange looks like
//! Request (we send):
//! ```json
//! { "jsonrpc":"2.0","id":1,"method":"getEvents",
//!   "params":{ "startLedger":1234,
//!              "filters":[{"type":"contract","contractIds":["C..."]}],
//!              "pagination":{"limit":100} } }
//! ```
//! Response (we get):
//! ```json
//! { "jsonrpc":"2.0","id":1,
//!   "result":{ "events":[{ "ledger":1234,"contractId":"C...",
//!                          "id":"0000001234-0000000001",
//!                          "topic":["AAAA..."],"value":"AAAA..." }],
//!              "latestLedger":1240,"cursor":"0000001234-0000000001" } }
//! ```

use serde::{Deserialize, Serialize};

// ─────────────────────────── request side (we send) ───────────────────────────

/// A JSON-RPC 2.0 envelope wrapping the method + params we send.
#[derive(Debug, Serialize)]
pub struct JsonRpcRequest<P> {
    pub jsonrpc: &'static str,
    pub id: u32,
    pub method: &'static str,
    pub params: P,
}

impl<P> JsonRpcRequest<P> {
    /// Wrap any RPC `method` + `params` in the JSON-RPC 2.0 envelope.
    pub fn new(method: &'static str, params: P) -> Self {
        Self {
            jsonrpc: "2.0",
            id: 1,
            method,
            params,
        }
    }
}

/// Params for `getEvents`. You pass **either** `start_ledger` (first call)
/// **or** a `cursor` inside `pagination` (to resume) — not both.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetEventsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_ledger: Option<u32>,
    pub filters: Vec<EventFilter>,
    pub pagination: Pagination,
}

/// Narrow the query to specific contract(s). `type` is always "contract" here.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventFilter {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub contract_ids: Vec<String>,
}

impl EventFilter {
    pub fn contract(contract_id: impl Into<String>) -> Self {
        Self {
            kind: "contract",
            contract_ids: vec![contract_id.into()],
        }
    }
}

/// How big a page to ask for, and where to resume from.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    pub limit: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

// ─────────────────────────── response side (we get) ───────────────────────────

/// A JSON-RPC 2.0 reply: either a `result` or an `error`.
#[derive(Debug, Deserialize)]
pub struct JsonRpcResponse<R> {
    pub result: Option<R>,
    pub error: Option<JsonRpcError>,
}

/// The error object the server returns when a call fails.
#[derive(Debug, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

/// The `result` payload of a successful `getEvents` call.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetEventsResult {
    #[serde(default)]
    pub events: Vec<RpcEvent>,
    /// Newest ledger the server knows about — lets us tell when we've caught up.
    pub latest_ledger: u32,
    /// Bookmark to pass back next time to continue where this page ended.
    #[serde(default)]
    pub cursor: Option<String>,
}

/// One raw event exactly as RPC returns it (topics/value are base64 XDR).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcEvent {
    pub ledger: u32,
    pub contract_id: String,
    /// Unique id of this event; doubles as a paging cursor.
    pub id: String,
    #[serde(default)]
    pub topic: Vec<String>,
    #[serde(default)]
    pub value: String,
}

/// Result of `getLatestLedger` — we only need the current ledger number, used
/// to start streaming from "now".
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestLedgerResult {
    pub sequence: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RawEvent;

    /// A trimmed but realistic `getEvents` reply, like the one we streamed live
    /// from testnet. Parsing it without a network proves our types match RPC.
    const SAMPLE_GET_EVENTS: &str = r#"{
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "events": [
                {
                    "type": "contract",
                    "ledger": 2929511,
                    "ledgerClosedAt": "2026-06-05T00:00:00Z",
                    "contractId": "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC",
                    "id": "0002929511-0000000001",
                    "pagingToken": "0002929511-0000000001",
                    "topic": ["AAAADwAAAANmZWUA", "AAAAEgAAAAAAAAAAAyptMa"],
                    "value": "AAAAAAAAAAE=",
                    "inSuccessfulContractCall": true
                }
            ],
            "latestLedger": 2929520,
            "cursor": "0002929511-0000000001"
        }
    }"#;

    #[test]
    fn parses_get_events_result() {
        let resp: JsonRpcResponse<GetEventsResult> =
            serde_json::from_str(SAMPLE_GET_EVENTS).expect("valid getEvents JSON");

        assert!(resp.error.is_none());
        let result = resp.result.expect("has a result");
        assert_eq!(result.latest_ledger, 2929520);
        assert_eq!(result.cursor.as_deref(), Some("0002929511-0000000001"));
        assert_eq!(result.events.len(), 1);

        let ev = &result.events[0];
        assert_eq!(ev.ledger, 2929511);
        assert_eq!(ev.topic.len(), 2);
        assert_eq!(ev.value, "AAAAAAAAAAE=");
    }

    #[test]
    fn rpc_event_converts_to_raw_event() {
        let resp: JsonRpcResponse<GetEventsResult> =
            serde_json::from_str(SAMPLE_GET_EVENTS).unwrap();
        let ev = resp.result.unwrap().events.pop().unwrap();

        let raw: RawEvent = ev.into();
        assert_eq!(raw.ledger, 2929511);
        assert_eq!(
            raw.contract_id,
            "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"
        );
        assert_eq!(raw.topics.len(), 2); // topic -> topics
        assert_eq!(raw.data, "AAAAAAAAAAE="); // value -> data
    }

    #[test]
    fn parses_rpc_error_reply() {
        let body = r#"{"jsonrpc":"2.0","id":1,
            "error":{"code":-32602,"message":"invalid contract id"}}"#;
        let resp: JsonRpcResponse<GetEventsResult> = serde_json::from_str(body).unwrap();
        assert!(resp.result.is_none());
        let err = resp.error.expect("has an error");
        assert_eq!(err.code, -32602);
    }
}
