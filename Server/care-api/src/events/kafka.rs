//! Kafka implementation (doc 6.3): idempotent producer with acks=all; consumers commit after
//! the handler succeeds and send poison events to `<topic>.dlq`.

use super::{handle_with_retry, Envelope, EventBus, EventHandler};
use crate::config::BusKind;
use anyhow::{Context, Result};
use async_trait::async_trait;
use rdkafka::{
    config::ClientConfig,
    consumer::{Consumer, StreamConsumer},
    message::Message,
    producer::{FutureProducer, FutureRecord},
};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

pub struct KafkaBus {
    brokers: String,
    producer: FutureProducer,
}

impl KafkaBus {
    pub fn new(brokers: &str) -> Result<Self> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("enable.idempotence", "true")
            .set("acks", "all")
            .set("message.timeout.ms", "10000")
            .set("compression.type", "lz4")
            .create()
            .context("creating Kafka producer")?;
        Ok(Self { brokers: brokers.to_string(), producer })
    }
}

#[async_trait]
impl EventBus for KafkaBus {
    async fn publish(&self, topic: &str, key: &str, ev: &Envelope) -> Result<()> {
        let payload = serde_json::to_string(ev)?;
        self.producer
            .send(FutureRecord::to(topic).key(key).payload(&payload), Duration::from_secs(5))
            .await
            .map_err(|(e, _)| anyhow::anyhow!("kafka send to {topic}: {e}"))?;
        Ok(())
    }

    async fn consume(&self, group: &str, topics: &[&str], from_start: bool, handler: Arc<dyn EventHandler>, stop: CancellationToken) -> Result<()> {
        // Offsets are stored only after the handler succeeds and auto-committed from there,
        // which gives at-least-once delivery without holding a borrowed message across awaits.
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &self.brokers)
            .set("group.id", group)
            .set("enable.auto.commit", "true")
            .set("enable.auto.offset.store", "false")
            .set("auto.offset.reset", if from_start { "earliest" } else { "latest" })
            .set("session.timeout.ms", "10000")
            .create()
            .context("creating Kafka consumer")?;
        consumer.subscribe(topics).context("subscribing")?;
        tracing::info!(group, ?topics, "Kafka consumer started");

        loop {
            // Copy what we need out of the borrowed message inside this block, so nothing
            // borrowed from librdkafka lives across the awaits below.
            let data = {
                let received = tokio::select! {
                    _ = stop.cancelled() => return Ok(()),
                    m = consumer.recv() => m,
                };
                let copied = match &received {
                    Ok(m) => Some((
                        m.topic().to_string(),
                        m.partition(),
                        m.offset(),
                        m.key().map(|k| String::from_utf8_lossy(k).into_owned()).unwrap_or_default(),
                        m.payload_view::<str>().and_then(|r| r.ok()).map(serde_json::from_str::<Envelope>),
                    )),
                    Err(e) => {
                        tracing::warn!("Kafka receive error: {e}");
                        None
                    }
                };
                copied
            };
            let Some((topic, partition, offset, key, parsed)) = data else {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            };

            match parsed {
                Some(Ok(ev)) => {
                    if let Err(e) = handle_with_retry(&handler, &topic, &ev).await {
                        tracing::error!(%topic, event_id = %ev.event_id, "sending to DLQ after retries: {e:#}");
                        let _ = self.publish(&format!("{topic}.dlq"), &key, &ev).await;
                    }
                }
                _ => tracing::warn!(%topic, "skipping message that is not a valid envelope"),
            }
            if let Err(e) = consumer.store_offset(&topic, partition, offset) {
                tracing::warn!("storing offset failed: {e}");
            }
        }
    }

    fn kind(&self) -> BusKind { BusKind::Kafka }
}
