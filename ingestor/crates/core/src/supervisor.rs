//! Runs many streams at once. Each registered contract or watched account gets
//! its own task and its own cursor, so one stream's failure never stalls the
//! others.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::task::{JoinHandle, JoinSet};

use crate::{
    AccountStore, ContractStore, CursorStore, EventSink, IngestError, Ingestor, StreamSpec,
};

/// Builds the collaborators for one indexing task. Called once per stream so
/// each task owns a fresh cursor store and sink over a shared backend (e.g. a
/// connection pool). Kept decoder-agnostic: the CLI supplies the real wiring.
pub trait IngestorFactory: Send + Sync + 'static {
    fn cursor_store(&self) -> Box<dyn CursorStore>;
    /// The sink for `spec`, so contract and account streams can store
    /// events differently.
    fn sink(&self, spec: &StreamSpec) -> Box<dyn EventSink>;
}

/// The set of streams that should be running right now.
#[async_trait]
pub trait StreamRegistry: Send + Sync {
    async fn streams(&self) -> Result<Vec<StreamSpec>, IngestError>;
}

/// Streams for every registered contract followed by every watched account.
pub struct CombinedRegistry {
    contracts: Box<dyn ContractStore>,
    accounts: Box<dyn AccountStore>,
}

impl CombinedRegistry {
    pub fn new(contracts: Box<dyn ContractStore>, accounts: Box<dyn AccountStore>) -> Self {
        Self {
            contracts,
            accounts,
        }
    }
}

#[async_trait]
impl StreamRegistry for CombinedRegistry {
    async fn streams(&self) -> Result<Vec<StreamSpec>, IngestError> {
        let mut specs: Vec<StreamSpec> = self
            .contracts
            .list()
            .await?
            .iter()
            .map(|id| StreamSpec::contract(id))
            .collect();

        for account in self.accounts.list().await? {
            match StreamSpec::account(&account.address) {
                Ok(spec) => specs.push(spec.with_start_ledger(account.first_ledger)),
                // Addresses are validated on add, so this only catches rows
                // edited by hand. Skip them rather than stop every stream.
                Err(e) => eprintln!("stardex: skipping watched account: {e}"),
            }
        }
        Ok(specs)
    }
}

/// How long to wait before restarting a stream's task after it errors.
const DEFAULT_RESTART_BACKOFF: Duration = Duration::from_secs(5);

/// How often to re-read the registry to notice added or removed streams.
const DEFAULT_RELOAD_INTERVAL: Duration = Duration::from_secs(10);

/// Supervises one indexing task per stream, restarting any that fail and
/// following the registry as it changes.
pub struct Supervisor<F: IngestorFactory> {
    rpc_url: String,
    factory: Arc<F>,
    restart_backoff: Duration,
    reload_interval: Duration,
}

impl<F: IngestorFactory> Supervisor<F> {
    pub fn new(rpc_url: impl Into<String>, factory: F) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            factory: Arc::new(factory),
            restart_backoff: DEFAULT_RESTART_BACKOFF,
            reload_interval: DEFAULT_RELOAD_INTERVAL,
        }
    }

    /// Keep one indexing task per stream, re-reading `registry` every
    /// [`DEFAULT_RELOAD_INTERVAL`] so streams added or removed while running
    /// are picked up without a restart. Runs until cancelled.
    pub async fn watch(&self, registry: Box<dyn StreamRegistry>) {
        let mut running: HashMap<String, JoinHandle<()>> = HashMap::new();
        loop {
            match registry.streams().await {
                Ok(specs) => self.reconcile(&mut running, specs),
                // A blip reading the registry shouldn't stop what's already
                // indexing; keep the current set and try again next tick.
                Err(e) => eprintln!("stardex: could not read the stream registry: {e}; retrying"),
            }
            tokio::time::sleep(self.reload_interval).await;
        }
    }

    /// Catch every stream in `specs` up to the chain tip at the same time, then
    /// return. For scheduled jobs with no always-on worker. Returns the streams
    /// that failed, so the caller can report them; the others still finish.
    pub async fn catch_up(&self, specs: Vec<StreamSpec>) -> Vec<(String, IngestError)> {
        let mut tasks = JoinSet::new();
        for spec in specs {
            let rpc_url = self.rpc_url.clone();
            let factory = Arc::clone(&self.factory);
            tasks.spawn(async move {
                let mut ingestor = Ingestor::with_store(rpc_url, factory.cursor_store())
                    .with_event_sink(factory.sink(&spec));
                let result = ingestor.catch_up_stream(&spec).await;
                (spec.key, result)
            });
        }

        let mut failures = Vec::new();
        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok((key, Ok(()))) => println!("stardex: {key} caught up"),
                Ok((key, Err(e))) => failures.push((key, e)),
                Err(e) => failures.push(("task".into(), IngestError::Store(e.to_string()))),
            }
        }
        failures
    }

    /// Start a task for every new stream and stop the ones no longer wanted.
    fn reconcile(&self, running: &mut HashMap<String, JoinHandle<()>>, specs: Vec<StreamSpec>) {
        let wanted: HashSet<String> = specs.iter().map(|s| s.key.clone()).collect();

        for spec in specs {
            running.entry(spec.key.clone()).or_insert_with(|| {
                println!("stardex: started indexing {}", spec.key);
                self.spawn(spec)
            });
        }

        running.retain(|key, handle| {
            if wanted.contains(key) {
                return true;
            }
            println!("stardex: stopped indexing {key}");
            handle.abort();
            false
        });
    }

    /// One stream's task: run forever, restarting after a backoff if it errors.
    fn spawn(&self, spec: StreamSpec) -> JoinHandle<()> {
        let rpc_url = self.rpc_url.clone();
        let factory = Arc::clone(&self.factory);
        let backoff = self.restart_backoff;
        tokio::spawn(async move {
            loop {
                let mut ingestor = Ingestor::with_store(rpc_url.clone(), factory.cursor_store())
                    .with_event_sink(factory.sink(&spec));
                match ingestor.index_stream(&spec).await {
                    // The stream only ends on error; a clean return is a no-op.
                    Ok(()) => break,
                    Err(e) => {
                        eprintln!(
                            "stardex: indexing {} failed: {e}; restarting in {backoff:?}",
                            spec.key
                        );
                        tokio::time::sleep(backoff).await;
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InMemoryAccountStore, InMemoryContractStore, InMemoryCursorStore, PrintSink};

    const ACCOUNT: &str = "GBTF2Z62VJD4B54NGIS6JTGNPVH2O5HQNQF4S75NHVZIBP4JONQMRP7K";

    struct NoopFactory;

    impl IngestorFactory for NoopFactory {
        fn cursor_store(&self) -> Box<dyn CursorStore> {
            Box::new(InMemoryCursorStore::default())
        }
        fn sink(&self, _spec: &StreamSpec) -> Box<dyn EventSink> {
            Box::new(PrintSink)
        }
    }

    #[tokio::test]
    async fn combined_registry_lists_contracts_then_accounts() {
        let contracts = InMemoryContractStore::default();
        contracts.register("CABC", 1).await.unwrap();
        let accounts = InMemoryAccountStore::default();
        accounts
            .add(ACCOUNT, Some("Acme"), Some(4_700_000))
            .await
            .unwrap();

        let registry = CombinedRegistry::new(Box::new(contracts), Box::new(accounts));
        let keys: Vec<String> = registry
            .streams()
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.key)
            .collect();

        assert_eq!(keys, vec!["CABC".to_string(), format!("account:{ACCOUNT}")]);
    }

    #[tokio::test]
    async fn account_streams_start_at_the_ledger_they_were_added() {
        let accounts = InMemoryAccountStore::default();
        accounts.add(ACCOUNT, None, Some(4_700_000)).await.unwrap();
        let registry = CombinedRegistry::new(
            Box::new(InMemoryContractStore::default()),
            Box::new(accounts),
        );
        let specs = registry.streams().await.unwrap();
        assert_eq!(specs[0].start_ledger, Some(4_700_000));
    }

    #[tokio::test]
    async fn catch_up_reports_streams_that_fail() {
        let supervisor = Supervisor::new("http://127.0.0.1:1", NoopFactory);
        let specs = vec![StreamSpec::contract("CABC")];
        // Unroutable RPC: the stream fails fast on the first request.
        let failures = tokio::time::timeout(Duration::from_secs(60), supervisor.catch_up(specs))
            .await
            .expect("catch_up returns");
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].0, "CABC");
    }

    #[tokio::test]
    async fn combined_registry_skips_invalid_accounts() {
        let accounts = InMemoryAccountStore::default();
        accounts.add("not-an-address", None, None).await.unwrap();
        accounts.add(ACCOUNT, None, None).await.unwrap();

        let registry = CombinedRegistry::new(
            Box::new(InMemoryContractStore::default()),
            Box::new(accounts),
        );
        assert_eq!(registry.streams().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn reconcile_starts_and_stops_mixed_streams() {
        // An unroutable RPC URL: tasks just fail and back off, which is fine
        // since only the set of running keys is checked.
        let supervisor = Supervisor::new("http://127.0.0.1:1", NoopFactory);
        let mut running = HashMap::new();

        let contract = StreamSpec::contract("CABC");
        let account = StreamSpec::account(ACCOUNT).unwrap();

        supervisor.reconcile(&mut running, vec![contract.clone(), account.clone()]);
        let mut keys: Vec<_> = running.keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, vec!["CABC".to_string(), account.key.clone()]);

        supervisor.reconcile(&mut running, vec![account.clone()]);
        assert_eq!(
            running.keys().cloned().collect::<Vec<_>>(),
            vec![account.key]
        );

        for (_, handle) in running.drain() {
            handle.abort();
        }
    }
}
