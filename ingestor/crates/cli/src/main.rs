//! `stardex` command-line tool. TODO(#28/#29): replace hand-rolled arg parsing
//! with `clap`.

use std::env;

use stardex_core::rpc_client::RpcClient;
use stardex_core::{
    connect_pool, AccountStore, CombinedRegistry, ContractStore, CursorStore, Dispatcher,
    EventSink, EventStore, InMemoryCursorStore, InMemoryEventStore, IngestError, Ingestor,
    IngestorFactory, PgPool, PostgresAccountStore, PostgresContractStore, PostgresCursorStore,
    PostgresEventStore, StreamSpec, Subscriptions, Supervisor,
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
        Some("streams") => cmd_streams(&args).await,
        Some("subscriptions") => cmd_subscriptions(&args).await,
        Some("accounts") => cmd_accounts(&args).await,
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

/// `stardex run` — index every registered contract and watched account
/// concurrently, following both registries so adds and removes take effect
/// without a restart.
async fn cmd_run() {
    let pool = connect_pool(&require_database_url())
        .await
        .unwrap_or_else(|e| exit_db(e));
    let contracts = PostgresContractStore::from_pool(pool.clone())
        .list()
        .await
        .unwrap_or_else(|e| exit_db(e));
    let accounts = PostgresAccountStore::from_pool(pool.clone())
        .list()
        .await
        .unwrap_or_else(|e| exit_db(e));

    println!(
        "stardex: following {} contract(s) and {} account(s) via {}",
        contracts.len(),
        accounts.len(),
        default_rpc()
    );
    if contracts.is_empty() && accounts.is_empty() {
        println!(
            "nothing registered yet; `stardex add <contract_id>` or \
             `stardex accounts add <address>` and it starts automatically"
        );
    }

    let registry = CombinedRegistry::new(
        Box::new(PostgresContractStore::from_pool(pool.clone())),
        Box::new(PostgresAccountStore::from_pool(pool.clone())),
    );
    Supervisor::new(default_rpc(), PgFactory { pool })
        .watch(Box::new(registry))
        .await;
}

/// `stardex accounts <add|list|remove>` — manage watched accounts.
async fn cmd_accounts(args: &[String]) {
    match args.get(1).map(String::as_str) {
        Some("add") => cmd_accounts_add(args).await,
        Some("list") => cmd_accounts_list().await,
        Some("remove") => cmd_accounts_remove(args).await,
        _ => {
            eprintln!("usage: stardex accounts <add|list|remove>");
            std::process::exit(2);
        }
    }
}

/// `stardex accounts add <address> [--label <name>]` — watch an account for
/// incoming payments in any asset.
async fn cmd_accounts_add(args: &[String]) {
    let Some(address) = positional(args, 2) else {
        eprintln!("usage: stardex accounts add <address> [--label <name>]");
        std::process::exit(2);
    };
    // Muxed (M...) addresses are rejected here: payments to them are already
    // caught by watching the base G... account, with the ID kept on each event.
    if let Err(e) = StreamSpec::account(address) {
        eprintln!("stardex: {e}");
        std::process::exit(2);
    }

    let pool = connect_pool(&require_database_url())
        .await
        .unwrap_or_else(|e| exit_db(e));
    PostgresAccountStore::from_pool(pool)
        .add(address, flag(args, "--label"))
        .await
        .unwrap_or_else(|e| exit_db(e));
    println!("watching {address}; a running `stardex run` picks it up within seconds");
}

/// `stardex accounts list` — print watched accounts.
async fn cmd_accounts_list() {
    let pool = connect_pool(&require_database_url())
        .await
        .unwrap_or_else(|e| exit_db(e));
    let accounts = PostgresAccountStore::from_pool(pool)
        .list()
        .await
        .unwrap_or_else(|e| exit_db(e));

    if accounts.is_empty() {
        eprintln!("no accounts watched; add one with `stardex accounts add <address>`");
        return;
    }
    for account in accounts {
        match account.label {
            Some(label) => println!("{} ({label})", account.address),
            None => println!("{}", account.address),
        }
    }
}

/// `stardex accounts remove <address>` — stop watching an account.
async fn cmd_accounts_remove(args: &[String]) {
    let Some(address) = positional(args, 2) else {
        eprintln!("usage: stardex accounts remove <address>");
        std::process::exit(2);
    };
    let pool = connect_pool(&require_database_url())
        .await
        .unwrap_or_else(|e| exit_db(e));
    PostgresAccountStore::from_pool(pool)
        .remove(address)
        .await
        .unwrap_or_else(|e| exit_db(e));
    println!("stopped watching {address}; what it already recorded is kept");
}

/// `stardex streams [--once]` — deliver indexed events to subscriber webhooks.
/// Runs as its own process so a slow receiver can never hold up indexing.
/// `--once` drains what is due and exits, for cron-style jobs with no worker.
async fn cmd_streams(args: &[String]) {
    let once = args.iter().any(|a| a == "--once");
    let pool = connect_pool(&require_database_url())
        .await
        .unwrap_or_else(|e| exit_db(e));
    let dispatcher = Dispatcher::new(pool);
    if once {
        dispatcher.run_once().await;
    } else {
        dispatcher.run().await;
    }
}

/// `stardex subscriptions <add|list|remove>` — manage webhook subscriptions.
async fn cmd_subscriptions(args: &[String]) {
    match args.get(1).map(String::as_str) {
        Some("add") => cmd_subscriptions_add(args).await,
        Some("list") => cmd_subscriptions_list().await,
        Some("remove") => cmd_subscriptions_remove(args).await,
        _ => {
            eprintln!("usage: stardex subscriptions <add|list|remove>");
            std::process::exit(2);
        }
    }
}

/// `stardex subscriptions add <url> [--contract <id>] [--kind <kind>]`. Leaving
/// a filter out means "any": no filters at all subscribes to every event.
async fn cmd_subscriptions_add(args: &[String]) {
    let Some(url) = positional(args, 2) else {
        eprintln!("usage: stardex subscriptions add <url> [--contract <id>] [--kind <kind>]");
        std::process::exit(2);
    };
    let pool = connect_pool(&require_database_url())
        .await
        .unwrap_or_else(|e| exit_db(e));

    let (id, secret) = Subscriptions::from_pool(pool)
        .create(url, flag(args, "--contract"), flag(args, "--kind"))
        .await
        .unwrap_or_else(|e| exit_db(e));

    println!("subscription {id} -> {url}");
    println!("  contract: {}", flag(args, "--contract").unwrap_or("any"));
    println!("  kind:     {}", flag(args, "--kind").unwrap_or("any"));
    println!("  secret:   {secret}");
    println!("save the secret now, it is not shown again; verify it against the");
    println!("x-stardex-signature header (sha256=HMAC of the exact request body)");
}

/// `stardex subscriptions list` — print every subscription, retired included.
async fn cmd_subscriptions_list() {
    let pool = connect_pool(&require_database_url())
        .await
        .unwrap_or_else(|e| exit_db(e));
    let subscriptions = Subscriptions::from_pool(pool)
        .list()
        .await
        .unwrap_or_else(|e| exit_db(e));

    if subscriptions.is_empty() {
        eprintln!("no subscriptions — add one with `stardex subscriptions add <url>`");
        return;
    }
    for s in subscriptions {
        let state = if s.active { "active" } else { "retired" };
        let contract = s.contract_id.as_deref().unwrap_or("any");
        let kind = s.kind.as_deref().unwrap_or("any");
        println!(
            "{} [{state}] contract={contract} kind={kind} -> {}",
            s.id, s.url
        );
    }
}

/// `stardex subscriptions remove <id>` — stop delivering to a subscription.
async fn cmd_subscriptions_remove(args: &[String]) {
    let Some(raw) = positional(args, 2) else {
        eprintln!("usage: stardex subscriptions remove <id>");
        std::process::exit(2);
    };
    let Ok(id) = raw.parse::<i64>() else {
        eprintln!("stardex: subscription id must be a number, got {raw}");
        std::process::exit(2);
    };

    let pool = connect_pool(&require_database_url())
        .await
        .unwrap_or_else(|e| exit_db(e));
    let existed = Subscriptions::from_pool(pool)
        .deactivate(id)
        .await
        .unwrap_or_else(|e| exit_db(e));

    if !existed {
        eprintln!("stardex: no subscription with id {id}");
        std::process::exit(1);
    }
    println!("subscription {id} retired; it will stop receiving events");
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
/// pool, so every stream's events run through the same decoder registry.
struct PgFactory {
    pool: PgPool,
}

impl IngestorFactory for PgFactory {
    fn cursor_store(&self) -> Box<dyn CursorStore> {
        Box::new(PostgresCursorStore::from_pool(self.pool.clone()))
    }

    fn sink(&self, _spec: &StreamSpec) -> Box<dyn EventSink> {
        let store: Box<dyn EventStore> = Box::new(PostgresEventStore::from_pool(self.pool.clone()));
        Box::new(DecodingSink::new(default_registry(), store))
    }
}

/// Value of a `--name value` flag, if it was passed.
fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let at = args.iter().position(|a| a == name)?;
    args.get(at + 1).map(String::as_str)
}

/// First plain argument after `skip`, stepping over any `--flag value` pairs.
fn positional(args: &[String], skip: usize) -> Option<&str> {
    let mut rest = args.iter().skip(skip);
    while let Some(arg) = rest.next() {
        if arg.starts_with("--") {
            rest.next();
            continue;
        }
        return Some(arg);
    }
    None
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
    eprintln!("  stardex run                            index all registered contracts and watched accounts, following changes live");
    eprintln!("  stardex add <contract_id>              register a contract to index (needs DATABASE_URL)");
    eprintln!(
        "  stardex remove <contract_id>           stop indexing a contract, keeping its history"
    );
    eprintln!("  stardex index <contract_id> [--once]   index a single contract; --once catches up and exits");
    eprintln!("  stardex contracts list                 list registered contracts");
    eprintln!("  stardex decoders list                  list registered decoders");
    eprintln!();
    eprintln!("accounts (watch an address for incoming payments):");
    eprintln!("  stardex accounts add <address>         watch a G... address; --label names it");
    eprintln!("  stardex accounts list                  list watched accounts");
    eprintln!(
        "  stardex accounts remove <address>      stop watching an account, keeping its history"
    );
    eprintln!();
    eprintln!("streams (push indexed events to webhooks):");
    eprintln!("  stardex streams [--once]               run the delivery dispatcher; --once drains and exits");
    eprintln!("  stardex subscriptions add <url>        subscribe a webhook; --contract and --kind filter it");
    eprintln!("  stardex subscriptions list             list subscriptions");
    eprintln!("  stardex subscriptions remove <id>      stop delivering to a subscription");
}
