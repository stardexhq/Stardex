//! `stardex` — the command-line tool.
//!
//! For now this hand-parses a couple of subcommands so the skeleton runs with
//! zero dependencies. TODO(#28/#29): replace with `clap` and wire real
//! ingestion + a `new decoder` scaffolder.

use std::env;

use stardex_core::Ingestor;
use stardex_decoders::default_registry;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("index") => {
            let Some(contract) = args.get(1) else {
                eprintln!("usage: stardex index <contract_id>");
                std::process::exit(2);
            };
            let mut ingestor = Ingestor::new(default_rpc());
            println!("indexing {contract} via {} ...", ingestor.rpc_url());
            if let Err(e) = ingestor.index_contract(contract) {
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
