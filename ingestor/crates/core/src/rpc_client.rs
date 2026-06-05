use serde::Serialize;

use crate::rpc::{
    EventFilter, GetEventsParams, GetEventsResult, JsonRpcRequest, JsonRpcResponse,
    LatestLedgerResult, Pagination,
};
use crate::IngestError;

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

        if let Some(err) = response.error {
            return Err(IngestError::Rpc {
                code: err.code,
                message: err.message,
            });
        }
        response.result.ok_or(IngestError::EmptyResponse)
    }

    /// Current ledger number, used to start streaming from the chain tip.
    pub async fn latest_ledger(&self) -> Result<u32, IngestError> {
        let result: LatestLedgerResult = self.call("getLatestLedger", ()).await?;
        Ok(result.sequence)
    }

    /// Fetch one page of events for `contract_id`, resuming from `cursor` if set.
    pub async fn get_events(
        &self,
        contract_id: &str,
        start_ledger: Option<u32>,
        cursor: Option<String>,
    ) -> Result<GetEventsResult, IngestError> {
        // RPC rejects sending both; the cursor already encodes the position.
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
