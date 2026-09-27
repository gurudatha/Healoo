//! care-worker consumers (doc 6.4):
//! * `audit-writer` — persists `access.audit` decisions into Cassandra.
//! * `media` — on `item.attachments_added`, makes image thumbnails, counts PDF pages and records
//!   sha256 (doc 7.4), then emits `item.attachment` so open screens refresh.
//! * alert scheduler — fires due alerts every minute (DataItem v2).
//! All are idempotent via `processed_events` (doc 6.3).

use crate::{
    events::{topics, Envelope, EventBus, EventHandler, Publisher},
    files::{key_of, object_uri, ObjectStore},
    model::ItemStatus,
    store::{
        parts::{local_millis, today_in, AlertRow, AttachmentRow},
        Db,
    },
};
use chrono::Utc;
use std::time::Duration;
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

/// Result of processing one file.
pub struct Media { pub sha256: String, pub size: i64, pub page_count: Option<i32>, pub thumb_uri: Option<String> }

/// Hash and size for any file; a 320 px JPEG thumbnail for images and for the first page of PDFs;
/// the page count for PDFs. Fields already set on the row are kept. Shared by the media consumer
/// and the seed job (`backfill_media`).
pub async fn process_media(files: &dyn ObjectStore, a: &AttachmentRow) -> Result<Option<Media>> {
    let Some(key) = a.uri.as_deref().and_then(key_of) else { return Ok(None) };
    let bytes = files.get(key).await.with_context(|| format!("reading {key}"))?;
    let mut m = Media { sha256: hex::encode(Sha256::digest(&bytes)), size: bytes.len() as i64, page_count: a.page_count, thumb_uri: a.thumb_uri.clone() };
    let thumb = match a.kind.as_deref() {
        Some("IMAGE") if a.thumb_uri.is_none() => {
            let data = bytes.clone();
            Some(tokio::task::spawn_blocking(move || image_thumbnail(&data)).await??)
        }
        Some("PDF") => {
            if a.page_count.is_none() {
                let data = bytes.clone();
                match tokio::task::spawn_blocking(move || lopdf::Document::load_mem(&data).map(|d| d.get_pages().len() as i32)).await? {
                    Ok(n) => m.page_count = Some(n),
                    Err(e) => tracing::warn!(key, "could not read PDF: {e}"),
                }
            }
            if a.thumb_uri.is_none() {
                let data = bytes.clone();
                match tokio::task::spawn_blocking(move || pdf_thumbnail(&data)).await? {
                    Ok(t) => t,
                    Err(e) => { tracing::warn!(key, "could not render PDF thumbnail: {e:#}"); None }
                }
            } else { None }
        }
        _ => None,
    };
    if let Some(jpeg) = thumb {
        let thumb_key = format!("thumbs/{key}.jpg");
        files.put(&thumb_key, jpeg.into(), "image/jpeg").await?;
        m.thumb_uri = Some(object_uri(&thumb_key));
    }
    Ok(Some(m))
}

fn image_thumbnail(data: &[u8]) -> Result<Vec<u8>> {
    let img = image::load_from_memory(data)?;
    let thumb = img.thumbnail(THUMB_PX, THUMB_PX).to_rgb8();
    let mut buf = Vec::new();
    thumb.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Jpeg)?;
    Ok(buf)
}

/// Renders page 1 with poppler's `pdftoppm` (package poppler-utils, installed in the Docker image).
/// Returns None when the tool isn't installed (e.g. running natively without poppler): the apps
/// then show a PDF icon.
pub(crate) fn pdf_thumbnail(data: &[u8]) -> Result<Option<Vec<u8>>> {
    let dir = std::env::temp_dir().join(format!("healoo-pdf-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir)?;
    let result = (|| -> Result<Option<Vec<u8>>> {
        let input = dir.join("in.pdf");
        std::fs::write(&input, data)?;
        let out = dir.join("page");
        let status = match std::process::Command::new("pdftoppm")
            .args(["-jpeg", "-f", "1", "-l", "1", "-singlefile", "-scale-to", &THUMB_PX.to_string()])
            .arg(&input).arg(&out)
            .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped())
            .output()
        {
            Ok(o) => o,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::debug!("pdftoppm not installed; skipping PDF thumbnail");
                return Ok(None);
            }
            Err(e) => return Err(e.into()),
        };
        anyhow::ensure!(status.status.success(), "pdftoppm failed: {}", String::from_utf8_lossy(&status.stderr).trim());
        Ok(Some(std::fs::read(dir.join("page.jpg"))?))
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Makes the missing thumbnails, page counts and hashes for every attachment of `items`
/// (the seed job uses it, because seeded files never pass through the media consumer).
/// Returns how many attachments were updated.
pub async fn backfill_media(db: &Db, files: &dyn ObjectStore, items: &[Uuid]) -> Result<usize> {
    let mut n = 0;
    for &id in items {
        for a in db.attachments(id).await? {
            let pdf = a.kind.as_deref() == Some("PDF");
            if a.thumb_uri.is_some() && (!pdf || a.page_count.is_some()) { continue; }
            match process_media(files, &a).await {
                Ok(Some(m)) if m.thumb_uri != a.thumb_uri || m.page_count != a.page_count => {
                    db.set_attachment_media(id, a.position, m.page_count, m.thumb_uri.as_deref(), &m.sha256, m.size).await?;
                    n += 1;
                }
                Ok(_) => {}
                Err(e) => tracing::warn!(item = %id, "attachment processing failed: {e:#}"),
            }
        }
    }
    Ok(n)
}

impl MediaProcessor {
    async fn process(&self, a: &AttachmentRow) -> Result<Option<Media>> {
        process_media(self.files.as_ref(), a).await
    }
}

#[async_trait]
impl EventHandler for MediaProcessor {
    async fn handle(&self, _topic: &str, ev: Envelope) -> Result<()> {
        if ev.event_type != "item.attachments_added" { return Ok(()); }
        if self.db.already_processed("media", &ev.event_id).await? { return Ok(()); }
        let id: Uuid = ev.subject_id.parse()?;
        let wanted: Vec<Uuid> = ev.payload.get("attachment_ids").and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str()?.parse().ok()).collect()).unwrap_or_default();
        let Some(item) = self.db.item(id).await? else { return Ok(()) };
        for a in self.db.attachments(id).await?.iter().filter(|a| wanted.contains(&a.attachment_id)) {
            match self.process(a).await {
                Ok(Some(m)) => self.db.set_attachment_media(id, a.position, m.page_count, m.thumb_uri.as_deref(), &m.sha256, m.size).await?,
                Ok(None) => {}
                Err(e) => tracing::warn!(item = %id, "attachment processing failed: {e:#}"),
            }
        }
        let done = Envelope::new("item.attachment", ev.actor_id, id, item.audience(), serde_json::json!({ "item_id": id, "change": "attachment" }));
        self.publisher.emit(topics::ITEMS, &item.owner_id.to_string(), done).await?;
        self.db.mark_processed("media", &ev.event_id).await
    }
}

/// Alert scheduler (DataItem_Design.md 3.5/3.7): every minute, fires queued alerts, queues the
/// next firing of recurring ones, and skips appointment reminders whose visit was cancelled.
pub async fn run_alert_scheduler(db: Arc<Db>, publisher: Publisher, stop: CancellationToken) {
    let mut next_minute = Utc::now().timestamp_millis() / 60_000 * 60_000 - 5 * 60_000; // catch up 5 min after restart
    let mut tick = tokio::time::interval(Duration::from_secs(20));
    loop {
        tokio::select! { _ = stop.cancelled() => return, _ = tick.tick() => {} }
        let now = Utc::now().timestamp_millis();
        while next_minute <= now {
            if let Err(e) = fire_minute(&db, &publisher, next_minute).await {
                tracing::warn!("alert scheduler: {e:#}");
                break; // retry this minute on the next tick
            }
            next_minute += 60_000;
        }
    }
}

async fn fire_minute(db: &Db, publisher: &Publisher, minute: i64) -> Result<()> {
    for (alert_id, item_id, user) in db.alerts_due_at(minute).await? {
        let key = format!("{alert_id}:{minute}");
        if db.already_processed("alert-scheduler", &key).await? { db.remove_due(minute, alert_id).await?; continue; }
        let (Some(item), Some(alert)) = (db.item(item_id).await?, db.alert(item_id, alert_id).await?) else {
            db.remove_due(minute, alert_id).await?;
            continue;
        };
        if alert.active.unwrap_or(true) && item.status == ItemStatus::Open && reminder_still_valid(db, item_id, &alert, minute).await? {
            let ev = Envelope::new("alert.fired", item.owner_id, item_id, vec![user.to_string()], serde_json::json!({
                "alert_id": alert_id, "type": alert.kind, "text": alert.text, "item_id": item_id,
            }));
            publisher.emit(topics::ALERTS, &user.to_string(), ev).await?;
        }
        match alert.next_after(minute + 59_999) {
            Some(next) if alert.active.unwrap_or(true) => db.queue_alert(next, alert_id, item_id, user).await?,
            _ => db.set_alert_active(item_id, alert_id, false).await?,
        }
        db.mark_processed("alert-scheduler", &key).await?;
        db.remove_due(minute, alert_id).await?;
    }
    Ok(())
}

/// A reminder is sent only if its appointment still has a scheduled visit in the next 25 hours.
async fn reminder_still_valid(db: &Db, item_id: Uuid, alert: &AlertRow, minute: i64) -> Result<bool> {
    let Some(appt_id) = alert.appointment_id else { return Ok(true) };
    let Some(a) = db.appointment(item_id, appt_id).await? else { return Ok(false) };
    if a.is_cancelled() { return Ok(false); }
    let today = today_in(a.tz());
    Ok(a.visits(today, today + chrono::Duration::days(2), 10).iter().any(|(v, status)| {
        let ms = local_millis(v.date, v.time, a.tz());
        status == "SCHEDULED" && ms > minute && ms - minute <= 25 * 3_600_000
    }))
}

/// Starts the consumers and the alert scheduler; used by care-worker, and inside care-api when
/// EVENT_BUS=memory.
pub fn spawn(bus: Arc<dyn EventBus>, db: Arc<Db>, files: Arc<dyn ObjectStore>, publisher: Publisher, stop: CancellationToken) -> Vec<tokio::task::JoinHandle<()>> {
    let audit: Arc<dyn EventHandler> = Arc::new(AuditWriter { db: db.clone() });
    let media: Arc<dyn EventHandler> = Arc::new(MediaProcessor { db: db.clone(), files, publisher: publisher.clone() });
    let (b1, s1) = (bus.clone(), stop.clone());
    let (b2, s2) = (bus, stop.clone());
    vec![
        tokio::spawn(async move {
            if let Err(e) = b1.consume("audit-writer", &[topics::AUDIT], true, audit, s1).await { tracing::error!("audit-writer stopped: {e:#}"); }
        }),
        tokio::spawn(async move {
            if let Err(e) = b2.consume("media", &[topics::ITEMS], true, media, s2).await { tracing::error!("media worker stopped: {e:#}"); }
        }),
        tokio::spawn(run_alert_scheduler(db, publisher, stop)),
    ]
}
