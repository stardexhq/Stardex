//! `stardex` command-line tool. TODO(#28/#29): replace hand-rolled arg parsing
//! with `clap`.

use std::env;

use stardex_core::rpc_client::RpcClient;
use stardex_core::{
    connect_pool, ContractStore, CursorStore, EventSink, EventStore, InMemoryCursorStore,
    InMemoryEventStore, IngestError, Ingestor, IngestorFactory, PgPool, PostgresContractStore,
    PostgresCursorStore, PostgresEventStore, Supervisor,
};
use stardex_decoders::{default_registry, DecodingSink};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("index") => cmd_index(&args).await,
        Some("add") => cmd_add(&args).await,
        Some("remove") => cmd_remove(&args).await,
        Some("run") => cmd_run().await,
        Some("contracts") if args.get(1).map(String::as_str) == Some("list") => {
            cmd_contracts_list().await
        }
        Some("decoders") if args.get(1).map(String::as_str) == Some("list") => {
            for name in default_registry().names() {
                println!("{name}");
            }
        }
        _ => {
            usage();
            std::process::exit(2);
        }
    }
}

/// `stardex index <contract> [--once]` — stream a single contract (works with or
/// without a database). `--once` catches up to the tip and exits, for scheduled
/// jobs; otherwise it runs until stopped.
async fn cmd_index(args: &[String]) {
    let once = args.iter().any(|a| a == "--once");
    let Some(contract) = args.iter().skip(1).find(|a| !a.starts_with("--")) else {
        eprintln!("usage: stardex index <contract_id> [--once]");
        std::process::exit(2);
    };
    let (cursor_store, event_store) = stores().await;
    let sink = Box::new(DecodingSink::new(default_registry(), event_store));
    let mut ingestor = Ingestor::with_store(default_rpc(), cursor_store).with_event_sink(sink);
    println!("indexing {contract} via {} ...", ingestor.rpc_url());
    let result = if once {
        ingestor.catch_up(contract).await
    } else {
        ingestor.index_contract(contract).await
    };
    if let Err(e) = result {
        eprintln!("stardex: {e}");
        std::process::exit(1);
    }
    if once {
        println!("stardex: caught up to tip");
    }
}

/// `stardex add <contract>` — register a contract so `stardex run` indexes it.
async fn cmd_add(args: &[String]) {
    let Some(contract) = args.get(1) else {
        eprintln!("usage: stardex add <contract_id>");
        std::process::exit(2);
    };
    let pool = connect_pool(&require_database_url())
        .await
        .unwrap_or_else(|e| exit_db(e));
    let store = PostgresContractStore::from_pool(pool);

    // Record the current ledger as where tracking begins, for "tracking since N".
    let tip = RpcClient::new(default_rpc())
        .latest_ledger()
        .await
        .unwrap_or_else(|e| {
            eprintln!("stardex: could not reach RPC to record first-seen ledger: {e}");
            std::process::exit(1);
        });

    store
        .register(contract, tip)
        .await
        .unwrap_or_else(|e| exit_db(e));
    println!(
        "registered {contract} (tracking from ledger {tip}); \
         a running `stardex run` picks it up within seconds"
    );
}

/// `stardex remove <contract>` — stop indexing a contract, keeping its history.
async fn cmd_remove(args: &[String]) {
    let Some(contract) = args.get(1) else {
        eprintln!("usage: stardex remove <contract_id>");
        std::process::exit(2);
    };
    let pool = connect_pool(&require_database_url())
        .await
        .unwrap_or_else(|e| exit_db(e));
    PostgresContractStore::from_pool(pool)
        .unregister(contract)
        .await
        .unwrap_or_else(|e| exit_db(e));
    println!("stopped indexing {contract}; everything it already indexed is kept");
}

/// `stardex run` — index every registered contract concurrently, following the
/// registry so `add` and `remove` take effect without a restart.
async fn cmd_run() {
    let pool = connect_pool(&require_database_url())
        .await
        .unwrap_or_else(|e| exit_db(e));
    let registered = PostgresContractStore::from_pool(pool.clone())
        .list()
        .await
        .unwrap_or_else(|e| exit_db(e));

    println!(
        "stardex: watching the contract registry via {}",
        default_rpc()
    );
    if registered.is_empty() {
        println!("no contracts registered yet — `stardex add <contract_id>` and it starts indexing automatically");
    }

    Supervisor::new(default_rpc(), PgFactory { pool: pool.clone() })
        .watch(Box::new(PostgresContractStore::from_pool(pool)))
        .await;
}

/// `stardex contracts list` — print the registered contracts.
async fn cmd_contracts_list() {
    let pool = connect_pool(&require_database_url())
        .await
        .unwrap_or_else(|e| exit_db(e));
    let contracts = PostgresContractStore::from_pool(pool)
        .list()
        .await
        .unwrap_or_else(|e| exit_db(e));

    if contracts.is_empty() {
        eprintln!("no contracts registered");
        return;
    }
    for contract in contracts {
        println!("{contract}");
    }
}

/// Builds a fresh cursor store and decoding sink per task, all over one shared
/// pool, so every contract's events run through the same decoder registry.
struct PgFactory {
    pool: PgPool,
}

impl IngestorFactory for PgFactory {
    fn cursor_store(&self) -> Box<dyn CursorStore> {
        Box::new(PostgresCursorStore::from_pool(self.pool.clone()))
    }

    fn sink(&self) -> Box<dyn EventSink> {
        let store: Box<dyn EventStore> = Box::new(PostgresEventStore::from_pool(self.pool.clone()));
        Box::new(DecodingSink::new(default_registry(), store))
    }
}

/// RPC endpoint, overridable via the STARDEX_RPC_URL env var.
fn default_rpc() -> String {
    env::var("STARDEX_RPC_URL")
        .unwrap_or_else(|_| "https://soroban-testnet.stellar.org".to_string())
}

/// Pick where a single-contract `index` run stores events and its cursor. With
/// `DATABASE_URL` set both persist to Postgres (resumes after restart);
/// otherwise both stay in memory so `stardex index` still runs with no database.
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

/// Commands that need a database require `DATABASE_URL`; bail with a hint if it
/// is missing.
fn require_database_url() -> String {
    env::var("DATABASE_URL").unwrap_or_else(|_| {
        eprintln!("stardex: DATABASE_URL must be set for this command");
        std::process::exit(2);
    })
}

fn exit_db(e: IngestError) -> ! {
    eprintln!("stardex: database error: {e}");
    std::process::exit(1);
}

fn usage() {
    eprintln!("stardex — Stellar/Soroban indexer\n");
    eprintln!("usage:");
    eprintln!("  stardex run                            index all registered contracts, following add/remove live");
    eprintln!("  stardex add <contract_id>              register a contract to index (needs DATABASE_URL)");
    eprintln!(
        "  stardex remove <contract_id>           stop indexing a contract, keeping its history"
    );
    eprintln!("  stardex index <contract_id> [--once]   index a single contract; --once catches up and exits");
    eprintln!("  stardex contracts list                 list registered contracts");
    eprintln!("  stardex decoders list                  list registered decoders");
}
