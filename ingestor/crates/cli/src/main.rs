//! `stardex` command-line tool. TODO(#28/#29): replace hand-rolled arg parsing
//! with `clap`.

use std::env;

use stardex_core::{CursorStore, InMemoryCursorStore, Ingestor, PostgresCursorStore};
use stardex_decoders::default_registry;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("index") => {
            let Some(contract) = args.get(1) else {
                eprintln!("usage: stardex index <contract_id>");
                std::process::exit(2);
            };
            let store = cursor_store().await;
            let mut ingestor = Ingestor::with_store(default_rpc(), store);
            println!("indexing {contract} via {} ...", ingestor.rpc_url());
            if let Err(e) = ingestor.index_contract(contract).await {
                eprintln!("stardex: {e}");
                std::process::exit(1);
            }
        }
        Some("decoders") if args.get(1).map(String::as_str) == Some("list") => {
            for name in default_registry().names() {
                println!("{name}");
            }
        }
        _ => {
            eprintln!("stardex — Stellar/Soroban indexer\n");
            eprintln!("usage:");
            eprintln!("  stardex index <contract_id>     index a contract's events");
            eprintln!("  stardex decoders list           list registered decoders");
            std::process::exit(2);
        }
    }
}

/// RPC endpoint, overridable via the STARDEX_RPC_URL env var.
fn default_rpc() -> String {
    env::var("STARDEX_RPC_URL")
        .unwrap_or_else(|_| "https://soroban-testnet.stellar.org".to_string())
}

/// Pick where the ingestion cursor is stored. If `DATABASE_URL` is set we
/// persist to Postgres (survives restarts); otherwise we keep it in memory.
async fn cursor_store() -> Box<dyn CursorStore> {
    match env::var("DATABASE_URL") {
        Ok(url) => match PostgresCursorStore::connect(&url).await {
            Ok(store) => {
                println!("cursor: persisting to Postgres (resumes after restart)");
                Box::new(store)
            }
            Err(e) => {
                eprintln!("stardex: could not connect to Postgres: {e}");
                std::process::exit(1);
            }
        },
        Err(_) => {
            println!("cursor: in-memory only (set DATABASE_URL to persist across restarts)");
            Box::new(InMemoryCursorStore::default())
        }
    }
}
