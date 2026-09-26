//! devices_by_user, notification_prefs, outbox, processed_events, access_audit

use super::Db;
use crate::model::*;
use anyhow::Result;
use scylla::frame::value::CqlTimestamp;
use uuid::Uuid;

pub const OUTBOX_SHARDS: i32 = 8;

#[derive(Clone, Debug)]
pub struct OutboxRow {
    pub shard: i32,
    pub event_id: String,
    pub topic: String,
    pub key: String,
    pub payload: String,
}

impl Db {
    // ---- devices (FCM tokens, doc 8.2) ----
    pub async fn add_device(&self, user: Uuid, token: &str, platform: &str) -> Result<()> {
        self.exec("INSERT INTO devices_by_user (user_id, fcm_token, platform, updated_at) VALUES (?, ?, ?, ?)",
            (user, token, platform, now_ts())).await?;
        Ok(())
    }

    pub async fn remove_device(&self, user: Uuid, token: &str) -> Result<()> {
        self.exec("DELETE FROM devices_by_user WHERE user_id = ? AND fcm_token = ?", (user, token)).await?;
        Ok(())
    }

    pub async fn devices(&self, user: Uuid) -> Result<Vec<(String, String)>> {
        Ok(self.rows::<(String, Option<String>)>("SELECT fcm_token, platform FROM devices_by_user WHERE user_id = ?", (user,))
            .await?.into_iter().map(|(t, p)| (t, p.unwrap_or_default())).collect())
    }

    // ---- notification preferences ----
    pub async fn prefs(&self, user: Uuid) -> Result<NotificationPrefs> {
        let row = self.one::<(Option<bool>, Option<bool>, Option<bool>, Option<String>, Option<String>)>(
            "SELECT push_messages, push_reports, quiet_hours, quiet_start, quiet_end FROM notification_prefs WHERE user_id = ?", (user,)).await?;
        let d = NotificationPrefs::default();
        Ok(match row {
            None => d,
            Some(r) => NotificationPrefs {
                push_messages: r.0.unwrap_or(d.push_messages),
                push_reports: r.1.unwrap_or(d.push_reports),
                quiet_hours: r.2.unwrap_or(d.quiet_hours),
                quiet_start: r.3.unwrap_or(d.quiet_start),
                quiet_end: r.4.unwrap_or(d.quiet_end),
            },
        })
    }

    pub async fn save_prefs(&self, user: Uuid, p: &NotificationPrefs) -> Result<()> {
        self.exec(
            "INSERT INTO notification_prefs (user_id, push_messages, push_reports, quiet_hours, quiet_start, quiet_end) VALUES (?, ?, ?, ?, ?, ?)",
            (user, p.push_messages, p.push_reports, p.quiet_hours, &p.quiet_start, &p.quiet_end)).await?;
        Ok(())
    }

    // ---- outbox (doc 6.3: no event lost between write and publish) ----
    pub async fn outbox_put(&self, row: &OutboxRow) -> Result<()> {
        self.exec("INSERT INTO outbox (shard, event_id, topic, event_key, payload, created_at) VALUES (?, ?, ?, ?, ?, ?)",
            (row.shard, &row.event_id, &row.topic, &row.key, &row.payload, now_ts())).await?;
        Ok(())
    }

    pub async fn outbox_delete(&self, shard: i32, event_id: &str) -> Result<()> {
        self.exec("DELETE FROM outbox WHERE shard = ? AND event_id = ?", (shard, event_id)).await?;
        Ok(())
    }

    pub async fn outbox_pending(&self, shard: i32, older_than_ms: i64) -> Result<Vec<OutboxRow>> {
        let cutoff = chrono::Utc::now().timestamp_millis() - older_than_ms;
        let rows = self.rows::<(String, String, String, String, Option<CqlTimestamp>)>(
            "SELECT event_id, topic, event_key, payload, created_at FROM outbox WHERE shard = ? LIMIT 200", (shard,)).await?;
        Ok(rows.into_iter()
            .filter(|r| r.4.map(|t| t.0 < cutoff).unwrap_or(true))
            .map(|r| OutboxRow { shard, event_id: r.0, topic: r.1, key: r.2, payload: r.3 })
            .collect())
    }

    // ---- consumer idempotency (doc 6.3: dedupe table with 1-day TTL) ----
    pub async fn already_processed(&self, consumer: &str, event_id: &str) -> Result<bool> {
        Ok(self.one::<(String,)>("SELECT event_id FROM processed_events WHERE consumer = ? AND event_id = ?", (consumer, event_id))
            .await?.is_some())
    }

    pub async fn mark_processed(&self, consumer: &str, event_id: &str) -> Result<()> {
        self.exec("INSERT INTO processed_events (consumer, event_id) VALUES (?, ?) USING TTL 86400", (consumer, event_id)).await?;
        Ok(())
    }

    // ---- audit (doc 2.3: every decision logged) ----
    pub async fn write_audit(&self, item: Uuid, user: Uuid, access: &str, reason: &str, event_id: &str) -> Result<()> {
        self.exec(
            "INSERT INTO access_audit (item_id, decided_at, user_id, decision, reason, event_id) VALUES (?, ?, ?, ?, ?, ?)",
            (tu(item), tu(new_timeuuid()), user, access, reason, event_id)).await?;
        Ok(())
    }
}
