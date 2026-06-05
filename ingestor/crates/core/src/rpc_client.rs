//! The thin **network client** for Stellar RPC.
//!
//! One job: send a single `getEvents` call and hand back the parsed page.
//! The looping/streaming logic that *uses* this lives in the ingestor itself
//! (Step 5). Keeping the raw call separate makes it easy to test and reason
//! about.

use serde::Serialize;

use crate::rpc::{
    EventFilter, GetEventsParams, GetEventsResult, JsonRpcRequest, JsonRpcResponse,
    LatestLedgerResult, Pagination,
};
use crate::IngestError;

/// How many events to request per page. RPC allows large pages; 100 keeps each
/// round-trip small and predictable.
const DEFAULT_PAGE_LIMIT: u32 = 100;

/// A reusable HTTP client pointed at one RPC endpoint.
pub struct RpcClient {
    http: reqwest::Client,
    url: String,
}

impl RpcClient {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            url: url.into(),
        }
    }

    /// Send one JSON-RPC call and return its typed `result`, mapping transport
    /// and RPC-level failures onto [`IngestError`].
    async fn call<P, R>(&self, method: &'static str, params: P) -> Result<R, IngestError>
    where
        P: Serialize,
        R: serde::de::DeserializeOwned,
    {
        let request = JsonRpcRequest::new(method, params);

        let response: JsonRpcResponse<R> = self
            .http
            .post(&self.url)
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        // A JSON-RPC reply is either a result or an error — never silently both.
        if let Some(err) = response.error {
            return Err(IngestError::Rpc {
                code: err.code,
                message: err.message,
            });
        }
        response.result.ok_or(IngestError::EmptyResponse)
    }

    /// Current ledger number, so we can start streaming from "now".
    pub async fn latest_ledger(&self) -> Result<u32, IngestError> {
        let result: LatestLedgerResult = self.call("getLatestLedger", ()).await?;
        Ok(result.sequence)
    }

    /// Fetch one page of events for `contract_id`.
    ///
    /// Pass **either** `start_ledger` (first call — where to begin) **or**
    /// `cursor` (resume from a previous page's bookmark). RPC rejects sending
    /// both, so when a cursor is given we drop `start_ledger`.
    pub async fn get_events(
        &self,
        contract_id: &str,
        start_ledger: Option<u32>,
        cursor: Option<String>,
    ) -> Result<GetEventsResult, IngestError> {
        // A cursor already encodes the position, so it wins over start_ledger.
        let start_ledger = if cursor.is_some() { None } else { start_ledger };

        let params = GetEventsParams {
            start_ledger,
            filters: vec![EventFilter::contract(contract_id)],
            pagination: Pagination {
                limit: DEFAULT_PAGE_LIMIT,
                cursor,
            },
        };
        self.call("getEvents", params).await
    }
}
