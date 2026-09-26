//! Events (design doc 6). The API writes Cassandra first, then publishes through the outbox so
//! that nothing is lost if the process dies in between (doc 6.3).
//!
//! [`EventBus`] has two implementations:
//! * [`kafka::KafkaBus`] — the real backbone (both LAN docker-compose and internet deployments).
//! * [`memory::MemoryBus`] — in-process broadcast, for a quick LAN run without Kafka
//!   (`EVENT_BUS=memory`); consumers then run inside the API process.

pub mod kafka;
pub mod memory;

use crate::{
    config::{BusKind, Config},
    store::{misc::OutboxRow, misc::OUTBOX_SHARDS, Db},
};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub mod topics {
    pub const CHAT: &str = "chat.messages";
    pub const ITEMS: &str = "items.events";
    pub const ALERTS: &str = "alerts.events";
    pub const ACCESS_CHANGES: &str = "access.changes";
    pub const AUDIT: &str = "access.audit";
    pub const NOTIFY: &str = "notify.requests";
    pub const ALL: [&str; 6] = [CHAT, ITEMS, ALERTS, ACCESS_CHANGES, AUDIT, NOTIFY];
}

/// Doc 6.2 envelope.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Envelope {
    pub event_id: String,
    pub event_type: String,
    pub occurred_at: DateTime<Utc>,
    pub actor_id: Uuid,
    pub subject_id: String,
    /// "user-uuid" or "hospital:uuid"
    pub audience: Vec<String>,
    pub schema_version: u32,
    pub payload: Value,
}

impl Envelope {
    pub fn new(event_type: &str, actor: Uuid, subject: impl ToString, audience: Vec<String>, payload: Value) -> Self {
        Envelope {
            event_id: ulid::Ulid::new().to_string(),
            event_type: event_type.to_string(),
            occurred_at: Utc::now(),
            actor_id: actor,
            subject_id: subject.to_string(),
            audience,
            schema_version: 1,
            payload,
        }
    }
}

#[async_trait]
pub trait EventHandler: Send + Sync + 'static {
    async fn handle(&self, topic: &str, ev: Envelope) -> Result<()>;
}

#[async_trait]
pub trait EventBus: Send + Sync + 'static {
    async fn publish(&self, topic: &str, key: &str, ev: &Envelope) -> Result<()>;

    /// Consume `topics` as consumer group `group` until `stop`. At-least-once: offsets are
    /// committed after the handler succeeds; 3 attempts, then the event goes to `<topic>.dlq`.
    async fn consume(&self, group: &str, topics: &[&str], from_start: bool, handler: Arc<dyn EventHandler>, stop: CancellationToken) -> Result<()>;

    fn kind(&self) -> BusKind;
}

pub async fn build(cfg: &Config) -> Result<Arc<dyn EventBus>> {
    Ok(match cfg.bus {
        BusKind::Kafka => Arc::new(kafka::KafkaBus::new(&cfg.kafka_brokers)?),
        BusKind::Memory => Arc::new(memory::MemoryBus::new()),
    })
}

/// Runs a handler with the doc 6.3 retry policy. Returns Err only if all attempts failed.
pub async fn handle_with_retry(handler: &Arc<dyn EventHandler>, topic: &str, ev: &Envelope) -> Result<()> {
    let mut last = None;
    for attempt in 0..3u32 {
        match handler.handle(topic, ev.clone()).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                tracing::warn!(topic, event_id = %ev.event_id, attempt, "handler failed: {e:#}");
                last = Some(e);
                tokio::time::sleep(Duration::from_millis(200 * 2u64.pow(attempt))).await;
            }
        }
    }
    Err(last.unwrap())
}

/// Write-then-publish through the Cassandra outbox.
#[derive(Clone)]
pub struct Publisher {
    bus: Arc<dyn EventBus>,
    db: Arc<Db>,
}

impl Publisher {
    pub fn new(bus: Arc<dyn EventBus>, db: Arc<Db>) -> Self { Self { bus, db } }

    pub async fn emit(&self, topic: &str, key: &str, ev: Envelope) -> Result<()> {
        let shard = (fxhash(key) % OUTBOX_SHARDS as u64) as i32;
        let row = OutboxRow { shard, event_id: ev.event_id.clone(), topic: topic.into(), key: key.into(), payload: serde_json::to_string(&ev)? };
        self.db.outbox_put(&row).await?;
        match self.bus.publish(topic, key, &ev).await {
            Ok(()) => { let _ = self.db.outbox_delete(shard, &ev.event_id).await; }
            Err(e) => tracing::warn!(topic, "publish failed, outbox relay will retry: {e:#}"),
        }
        Ok(())
    }

    /// Publish directly, skipping the outbox — for high-volume, loss-tolerant events (audit).
    pub async fn emit_fast(&self, topic: &str, key: &str, ev: Envelope) {
        if let Err(e) = self.bus.publish(topic, key, &ev).await {
            tracing::warn!(topic, "publish failed: {e:#}");
        }
    }

    /// Relay: republish outbox rows older than 5 s (the normal path deletes them immediately).
    pub async fn run_relay(self, stop: CancellationToken) {
        let mut tick = tokio::time::interval(Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = stop.cancelled() => return,
                _ = tick.tick() => {}
            }
            for shard in 0..OUTBOX_SHARDS {
                let rows = match self.db.outbox_pending(shard, 5_000).await {
                    Ok(r) => r,
                    Err(e) => { tracing::warn!("outbox scan failed: {e:#}"); continue; }
                };
                for row in rows {
                    let Ok(ev) = serde_json::from_str::<Envelope>(&row.payload) else {
                        let _ = self.db.outbox_delete(shard, &row.event_id).await;
                        continue;
                    };
                    if self.bus.publish(&row.topic, &row.key, &ev).await.is_ok() {
                        let _ = self.db.outbox_delete(shard, &row.event_id).await;
                        tracing::info!(topic = %row.topic, event_id = %row.event_id, "outbox relay published");
                    }
                }
            }
        }
    }
}

fn fxhash(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}
