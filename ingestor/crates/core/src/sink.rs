//! The seam between raw ingestion and what happens to each event.

use async_trait::async_trait;

use crate::{IngestError, RawEvent};

/// Receives each [`RawEvent`] the ingestor streams. Implementations decode and
/// persist; the default just prints.
#[async_trait]
pub trait EventSink: Send + Sync {
    async fn handle(&self, event: RawEvent) -> Result<(), IngestError>;
}

/// Hands each event to several sinks in order, e.g. store the decoded event and
/// also record it as a payment. Stops at the first error so the cursor does not
/// move past an event that was only partly handled.
pub struct TeeSink {
    sinks: Vec<Box<dyn EventSink>>,
}

impl TeeSink {
    pub fn new(sinks: Vec<Box<dyn EventSink>>) -> Self {
        Self { sinks }
    }
}

#[async_trait]
impl EventSink for TeeSink {
    async fn handle(&self, event: RawEvent) -> Result<(), IngestError> {
        for sink in &self.sinks {
            sink.handle(event.clone()).await?;
        }
        Ok(())
    }
}

/// Default sink: prints a one-line summary. Used when nothing else is wired in.
pub struct PrintSink;

#[async_trait]
impl EventSink for PrintSink {
    async fn handle(&self, event: RawEvent) -> Result<(), IngestError> {
        println!(
            "event @ ledger {} from {} — topics={:?}",
            event.ledger, event.contract_id, event.topics
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    struct Recording {
        name: &'static str,
        log: Arc<Mutex<Vec<String>>>,
        fail: bool,
    }

    #[async_trait]
    impl EventSink for Recording {
        async fn handle(&self, event: RawEvent) -> Result<(), IngestError> {
            self.log
                .lock()
                .unwrap()
                .push(format!("{}:{}", self.name, event.event_id));
            if self.fail {
                return Err(IngestError::Store("boom".into()));
            }
            Ok(())
        }
    }

    fn sink(name: &'static str, log: &Arc<Mutex<Vec<String>>>, fail: bool) -> Box<dyn EventSink> {
        Box::new(Recording {
            name,
            log: Arc::clone(log),
            fail,
        })
    }

    #[tokio::test]
    async fn tee_runs_every_sink_in_order() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let tee = TeeSink::new(vec![sink("a", &log, false), sink("b", &log, false)]);
        let event = RawEvent {
            event_id: "e1".into(),
            ..Default::default()
        };

        tee.handle(event).await.unwrap();
        assert_eq!(*log.lock().unwrap(), vec!["a:e1", "b:e1"]);
    }

    #[tokio::test]
    async fn tee_stops_at_the_first_error() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let tee = TeeSink::new(vec![sink("a", &log, true), sink("b", &log, false)]);

        assert!(tee.handle(RawEvent::default()).await.is_err());
        assert_eq!(log.lock().unwrap().len(), 1);
    }
}
