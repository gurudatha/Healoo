//! care-worker consumers (doc 6.4):
//! * `audit-writer` — persists `access.audit` decisions into Cassandra.
//! * `media` — on `item.created`, makes image thumbnails, counts PDF pages and records sha256
//!   (doc 7.4), then emits `item.updated` so open screens refresh.
//! Both are idempotent by event_id via `processed_events` (doc 6.3).

use crate::{
    events::{topics, Envelope, EventBus, EventHandler, Publisher},
    files::{key_of, object_uri, ObjectStore},
    model::AttachmentUdt,
    store::Db,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::{io::Cursor, sync::Arc};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct AuditWriter {
    pub db: Arc<Db>,
}

#[async_trait]
impl EventHandler for AuditWriter {
    async fn handle(&self, _topic: &str, ev: Envelope) -> Result<()> {
        if self.db.already_processed("audit-writer", &ev.event_id).await? { return Ok(()); }
        let item: Uuid = ev.subject_id.parse().context("audit subject is not an item id")?;
        let user: Uuid = ev.payload.get("user_id").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(ev.actor_id);
        let access = ev.payload.get("access").and_then(|v| v.as_str()).unwrap_or("UNKNOWN");
        let reason = ev.payload.get("reason").and_then(|v| v.as_str()).unwrap_or("");
        self.db.write_audit(item, user, access, reason, &ev.event_id).await?;
        self.db.mark_processed("audit-writer", &ev.event_id).await
    }
}

pub struct MediaProcessor {
    pub db: Arc<Db>,
    pub files: Arc<dyn ObjectStore>,
    pub publisher: Publisher,
}

const THUMB_PX: u32 = 320;

impl MediaProcessor {
    async fn process(&self, a: &AttachmentUdt) -> Result<AttachmentUdt> {
        let mut out = a.clone();
        let Some(key) = a.uri.as_deref().and_then(key_of) else { return Ok(out) };
        let bytes = self.files.get(key).await.with_context(|| format!("reading {key}"))?;
        out.sha256 = Some(hex::encode(Sha256::digest(&bytes)));
        out.size = Some(bytes.len() as i64);

        match a.kind.as_deref() {
            Some("IMAGE") if a.thumb_uri.is_none() => {
                let data = bytes.clone();
                let jpeg = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
                    let img = image::load_from_memory(&data)?;
                    let thumb = img.thumbnail(THUMB_PX, THUMB_PX).to_rgb8();
                    let mut buf = Vec::new();
                    thumb.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Jpeg)?;
                    Ok(buf)
                }).await??;
                let thumb_key = format!("thumbs/{key}.jpg");
                self.files.put(&thumb_key, jpeg.into(), "image/jpeg").await?;
                out.thumb_uri = Some(object_uri(&thumb_key));
            }
            Some("PDF") if a.page_count.is_none() => {
                let data = bytes.clone();
                let pages = tokio::task::spawn_blocking(move || lopdf::Document::load_mem(&data).map(|d| d.get_pages().len() as i32)).await?;
                match pages {
                    Ok(n) => out.page_count = Some(n),
                    Err(e) => tracing::warn!(key, "could not read PDF: {e}"),
                }
                // First-page thumbnails need a PDF renderer (e.g. pdfium); the apps show a PDF icon meanwhile.
            }
            _ => {}
        }
        Ok(out)
    }
}

#[async_trait]
impl EventHandler for MediaProcessor {
    async fn handle(&self, _topic: &str, ev: Envelope) -> Result<()> {
        if ev.event_type != "item.created" { return Ok(()); }
        if self.db.already_processed("media", &ev.event_id).await? { return Ok(()); }
        let id: Uuid = ev.subject_id.parse()?;
        let Some(item) = self.db.item(id).await? else { return Ok(()) };
        if item.attachments.is_empty() { return self.db.mark_processed("media", &ev.event_id).await; }

        let mut updated = Vec::with_capacity(item.attachments.len());
        for a in &item.attachments {
            updated.push(match self.process(a).await {
                Ok(x) => x,
                Err(e) => { tracing::warn!(item = %id, "attachment processing failed: {e:#}"); a.clone() }
            });
        }
        self.db.set_attachments(id, &updated).await?;
        let done = Envelope::new("item.updated", ev.actor_id, id, ev.audience.clone(), serde_json::json!({ "item_id": id, "media": "processed" }));
        self.publisher.emit(topics::ITEMS, &item.owner_id.to_string(), done).await?;
        self.db.mark_processed("media", &ev.event_id).await
    }
}

/// Starts both consumers; used by care-worker, and inside care-api when EVENT_BUS=memory.
pub fn spawn(bus: Arc<dyn EventBus>, db: Arc<Db>, files: Arc<dyn ObjectStore>, publisher: Publisher, stop: CancellationToken) -> Vec<tokio::task::JoinHandle<()>> {
    let audit: Arc<dyn EventHandler> = Arc::new(AuditWriter { db: db.clone() });
    let media: Arc<dyn EventHandler> = Arc::new(MediaProcessor { db, files, publisher });
    let (b1, s1) = (bus.clone(), stop.clone());
    let (b2, s2) = (bus, stop);
    vec![
        tokio::spawn(async move {
            if let Err(e) = b1.consume("audit-writer", &[topics::AUDIT], true, audit, s1).await { tracing::error!("audit-writer stopped: {e:#}"); }
        }),
        tokio::spawn(async move {
            if let Err(e) = b2.consume("media", &[topics::ITEMS], true, media, s2).await { tracing::error!("media worker stopped: {e:#}"); }
        }),
    ]
}
