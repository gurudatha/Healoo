//! In-process bus (`EVENT_BUS=memory`) for a quick LAN trial without Kafka.
//! Every subscriber sees every event; consumer groups and persistence are not emulated,
//! so run the worker inside the API process (care-api does this automatically in memory mode).

use super::{handle_with_retry, Envelope, EventBus, EventHandler};
use crate::config::BusKind;
use anyhow::Result;
use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

pub struct MemoryBus {
    topics: DashMap<String, broadcast::Sender<Envelope>>,
}

impl MemoryBus {
    pub fn new() -> Self { Self { topics: DashMap::new() } }

    fn sender(&self, topic: &str) -> broadcast::Sender<Envelope> {
        self.topics.entry(topic.to_string()).or_insert_with(|| broadcast::channel(1024).0).clone()
    }
}

impl Default for MemoryBus {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl EventBus for MemoryBus {
    async fn publish(&self, topic: &str, _key: &str, ev: &Envelope) -> Result<()> {
        let _ = self.sender(topic).send(ev.clone()); // no subscribers is fine
        Ok(())
    }

    async fn consume(&self, group: &str, topics: &[&str], _from_start: bool, handler: Arc<dyn EventHandler>, stop: CancellationToken) -> Result<()> {
        let mut tasks = Vec::new();
        for t in topics {
            let mut rx = self.sender(t).subscribe();
            let (topic, h, stop) = (t.to_string(), handler.clone(), stop.clone());
            tasks.push(tokio::spawn(async move {
                loop {
                    let ev = tokio::select! {
                        _ = stop.cancelled() => return,
                        r = rx.recv() => match r {
                            Ok(ev) => ev,
                            Err(broadcast::error::RecvError::Lagged(n)) => { tracing::warn!(%topic, n, "memory bus lagged"); continue; }
                            Err(broadcast::error::RecvError::Closed) => return,
                        },
                    };
                    if let Err(e) = handle_with_retry(&h, &topic, &ev).await {
                        tracing::error!(%topic, "handler failed after retries: {e:#}");
                    }
                }
            }));
        }
        tracing::info!(group, ?topics, "memory bus consumer started");
        for t in tasks { let _ = t.await; }
        Ok(())
    }

    fn kind(&self) -> BusKind { BusKind::Memory }
}
