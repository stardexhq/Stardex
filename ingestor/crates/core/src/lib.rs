//! Stardex ingestion **core** — the neutral engine.
//!
//! It connects to Stellar RPC, pulls raw contract events, and remembers how
//! far it has progressed (the "cursor") so it can resume after a restart.
//! It deliberately knows *nothing* about what any event means — that's the
//! job of the decoders crate.

/// A raw contract event pulled from RPC, before it has been decoded.
#[derive(Debug, Clone)]
pub struct RawEvent {
    /// Ledger (block) sequence the event was emitted in.
    pub ledger: u32,
    /// Contract that emitted the event.
    pub contract_id: String,
    /// Event topics (XDR/base64 placeholders for now).
    pub topics: Vec<String>,
    /// Event payload (XDR/base64 placeholder for now).
    pub data: String,
}

/// Position in the event stream, so ingestion can resume where it left off.
#[derive(Debug, Clone, Default)]
pub struct Cursor {
    pub last_ledger: u32,
    pub last_event_id: Option<String>,
}

/// The ingestion engine.
///
/// TODO(M1): wire this to a real Stellar RPC client and a Postgres store.
pub struct Ingestor {
    rpc_url: String,
    cursor: Cursor,
}

impl Ingestor {
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            cursor: Cursor::default(),
        }
    }

    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    /// Connect to RPC and stream events for `contract_id`.
    ///
    /// TODO(#1): implement RPC streaming; TODO(#2): persist the cursor.
    pub fn index_contract(&mut self, contract_id: &str) -> Result<(), IngestError> {
        let _ = contract_id;
        Err(IngestError::NotImplemented("index_contract"))
    }
}

#[derive(Debug)]
pub enum IngestError {
    NotImplemented(&'static str),
}

impl std::fmt::Display for IngestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IngestError::NotImplemented(what) => write!(f, "not implemented yet: {what}"),
        }
    }
}

impl std::error::Error for IngestError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_ingestor_starts_at_genesis_cursor() {
        let ing = Ingestor::new("https://soroban-testnet.stellar.org");
        assert_eq!(ing.cursor().last_ledger, 0);
        assert_eq!(ing.rpc_url(), "https://soroban-testnet.stellar.org");
    }
}
