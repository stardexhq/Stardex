//! `stardex` command-line tool. TODO(#28/#29): replace hand-rolled arg parsing
//! with `clap`.

use std::env;

use stardex_core::{
    CursorStore, EventStore, InMemoryCursorStore, InMemoryEventStore, Ingestor,
    PostgresCursorStore, PostgresEventStore,
};
use stardex_decoders::{default_registry, DecodingSink};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("index") => {
            let Some(contract) = args.get(1) else {
                eprintln!("usage: stardex index <contract_id>");
                std::process::exit(2);
            };
            let (cursor_store, event_store) = stores().await;
            let sink = Box::new(DecodingSink::new(default_registry(), event_store));
            let mut ingestor =
                Ingestor::with_store(default_rpc(), cursor_store).with_event_sink(sink);
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

/// Pick where events and the ingestion cursor are stored. With `DATABASE_URL`
/// set both persist to Postgres (resumes after restart); otherwise both stay
/// in memory so `stardex index` still runs with no database.
async fn stores() -> (Box<dyn CursorStore>, Box<dyn EventStore>) {
    match env::var("DATABASE_URL") {
        Ok(url) => {
            let cursors = PostgresCursorStore::connect(&url)
                .await
                .unwrap_or_else(|e| exit_db(e));
            let events = PostgresEventStore::connect(&url)
                .await
                .unwrap_or_else(|e| exit_db(e));
            println!("storage: Postgres (events + resumable cursor)");
            (Box::new(cursors), Box::new(events))
        }
        Err(_) => {
            println!("storage: in-memory only (set DATABASE_URL to persist across restarts)");
            (
                Box::new(InMemoryCursorStore::default()),
                Box::new(InMemoryEventStore::default()),
            )
        }
    }
}

fn exit_db(e: stardex_core::IngestError) -> ! {
    eprintln!("stardex: could not connect to Postgres: {e}");
    std::process::exit(1);
}
