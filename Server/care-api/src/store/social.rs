//! connections_by_user, threads_by_user, threads_by_id, messages_by_thread, message_dedupe

use super::Db;
use crate::model::*;
use anyhow::Result;
use chrono::{Datelike, Duration as ChronoDuration, Utc};
use scylla::frame::value::{CqlTimestamp, CqlTimeuuid};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ThreadRow {
    pub other_user_id: Uuid,
    pub thread_id: Uuid,
    pub last_message: String,
    pub last_message_at: Option<CqlTimestamp>,
    pub unread: i32,
}

/// Month bucket bounds partition size (doc 7.2).
fn bucket(t: chrono::DateTime<Utc>) -> String { format!("{:04}-{:02}", t.year(), t.month()) }

impl Db {
    // ---- contacts (rule 8: connecting grants no data access by itself) ----

    pub async fn connect_users(&self, a: Uuid, a_role: Role, a_name: &str, b: Uuid, b_role: Role, b_name: &str) -> Result<()> {
        self.batch(
            &[
                "INSERT INTO connections_by_user (user_id, other_role, other_id, other_name, via) VALUES (?, ?, ?, ?, 'contact')",
                "INSERT INTO connections_by_user (user_id, other_role, other_id, other_name, via) VALUES (?, ?, ?, ?, 'contact')",
            ],
            ((a, text(&b_role), b, b_name), (b, text(&a_role), a, a_name)),
        ).await
    }

    pub async fn disconnect_users(&self, a: Uuid, a_role: Role, b: Uuid, b_role: Role) -> Result<()> {
        self.batch(
            &[
                "DELETE FROM connections_by_user WHERE user_id = ? AND other_role = ? AND other_id = ?",
                "DELETE FROM connections_by_user WHERE user_id = ? AND other_role = ? AND other_id = ?",
            ],
            ((a, text(&b_role), b), (b, text(&a_role), a)),
        ).await
    }

    pub async fn connection_ids(&self, user: Uuid) -> Result<Vec<(Role, Uuid)>> {
        let rows = self.rows::<(String, Uuid)>("SELECT other_role, other_id FROM connections_by_user WHERE user_id = ?", (user,)).await?;
        Ok(rows.into_iter().filter_map(|(r, id)| parse::<Role>(&r).map(|r| (r, id))).collect())
    }

    pub async fn is_connected(&self, user: Uuid, other: Uuid, other_role: Role) -> Result<bool> {
        Ok(self.one::<(Uuid,)>(
            "SELECT other_id FROM connections_by_user WHERE user_id = ? AND other_role = ? AND other_id = ?",
            (user, text(&other_role), other)).await?.is_some())
    }

    // ---- threads ----

    pub async fn thread_with(&self, user: Uuid, other: Uuid) -> Result<Option<ThreadRow>> {
        let row = self.one::<(CqlTimeuuid, Option<String>, Option<CqlTimestamp>, Option<i32>)>(
            "SELECT thread_id, last_message, last_message_at, unread FROM threads_by_user WHERE user_id = ? AND other_user_id = ? LIMIT 1",
            (user, other)).await?;
        Ok(row.map(|r| ThreadRow { other_user_id: other, thread_id: un(r.0), last_message: r.1.unwrap_or_default(), last_message_at: r.2, unread: r.3.unwrap_or(0) }))
    }

    pub async fn threads_of(&self, user: Uuid) -> Result<Vec<ThreadRow>> {
        let rows = self.rows::<(Uuid, CqlTimeuuid, Option<String>, Option<CqlTimestamp>, Option<i32>)>(
            "SELECT other_user_id, thread_id, last_message, last_message_at, unread FROM threads_by_user WHERE user_id = ?",
            (user,)).await?;
        let mut v: Vec<ThreadRow> = rows.into_iter()
            .map(|r| ThreadRow { other_user_id: r.0, thread_id: un(r.1), last_message: r.2.unwrap_or_default(), last_message_at: r.3, unread: r.4.unwrap_or(0) })
            .collect();
        v.sort_by_key(|t| std::cmp::Reverse(t.last_message_at.map(|t| t.0).unwrap_or(0)));
        Ok(v)
    }

    pub async fn create_thread(&self, a: Uuid, b: Uuid) -> Result<Uuid> {
        let id = new_timeuuid();
        let now = now_ts();
        self.batch(
            &[
                "INSERT INTO threads_by_id (thread_id, user_a, user_b, created_at) VALUES (?, ?, ?, ?)",
                "INSERT INTO threads_by_user (user_id, other_user_id, thread_id, last_message, last_message_at, unread) VALUES (?, ?, ?, '', ?, 0)",
                "INSERT INTO threads_by_user (user_id, other_user_id, thread_id, last_message, last_message_at, unread) VALUES (?, ?, ?, '', ?, 0)",
            ],
            ((tu(id), a, b, now), (a, b, tu(id), now), (b, a, tu(id), now)),
        ).await?;
        Ok(id)
    }

    pub async fn thread_members(&self, thread: Uuid) -> Result<Option<(Uuid, Uuid)>> {
        self.one::<(Uuid, Uuid)>("SELECT user_a, user_b FROM threads_by_id WHERE thread_id = ?", (tu(thread),)).await
    }

    // ---- messages ----

    /// Idempotent by (thread, client_msg_id): a resend returns the first message id (doc 4.4).
    pub async fn existing_message(&self, thread: Uuid, client_msg_id: &str) -> Result<Option<Uuid>> {
        Ok(self.one::<(CqlTimeuuid,)>("SELECT message_id FROM message_dedupe WHERE thread_id = ? AND client_msg_id = ?",
            (tu(thread), client_msg_id)).await?.map(|r| un(r.0)))
    }

    pub async fn insert_message(&self, m: &MessageDto, client_msg_id: &str) -> Result<()> {
        let at = crate::model::ts_to_dt(CqlTimestamp(uuid_millis(&m.message_id)));
        self.batch(
            &[
                "INSERT INTO messages_by_thread (thread_id, bucket, message_id, sender_id, body, client_msg_id, linked_item_id) VALUES (?, ?, ?, ?, ?, ?, ?)",
                "INSERT INTO message_dedupe (thread_id, client_msg_id, message_id) VALUES (?, ?, ?) USING TTL 86400",
            ],
            (
                (tu(m.thread_id), bucket(at), tu(m.message_id), m.sender_id, &m.body, client_msg_id, m.linked_item_id.map(tu)),
                (tu(m.thread_id), client_msg_id, tu(m.message_id)),
            ),
        ).await
    }

    pub async fn message(&self, thread: Uuid, id: Uuid) -> Result<Option<MessageDto>> {
        let at = crate::model::ts_to_dt(CqlTimestamp(uuid_millis(&id)));
        let row = self.one::<(Uuid, Option<String>, Option<CqlTimeuuid>)>(
            "SELECT sender_id, body, linked_item_id FROM messages_by_thread WHERE thread_id = ? AND bucket = ? AND message_id = ?",
            (tu(thread), bucket(at), tu(id))).await?;
        Ok(row.map(|r| MessageDto { message_id: id, thread_id: thread, sender_id: r.0, body: r.1.unwrap_or_default(), sent_at: at.to_rfc3339(), linked_item_id: r.2.map(un) }))
    }

    /// Latest messages of the current and previous month, oldest first (as the apps render them).
    pub async fn messages(&self, thread: Uuid, limit: i32) -> Result<Vec<MessageDto>> {
        let now = Utc::now();
        let mut out = Vec::new();
        for b in [bucket(now), bucket(now - ChronoDuration::days(31))] {
            let rows = self.rows::<(CqlTimeuuid, Uuid, Option<String>, Option<CqlTimeuuid>)>(
                "SELECT message_id, sender_id, body, linked_item_id FROM messages_by_thread WHERE thread_id = ? AND bucket = ? LIMIT ?",
                (tu(thread), b, limit)).await?;
            out.extend(rows.into_iter().map(|r| {
                let id = un(r.0);
                MessageDto {
                    message_id: id, thread_id: thread, sender_id: r.1, body: r.2.unwrap_or_default(),
                    sent_at: ts_to_dt(CqlTimestamp(uuid_millis(&id))).to_rfc3339(), linked_item_id: r.3.map(un),
                }
            }));
            if out.len() as i32 >= limit { break; }
        }
        out.sort_by_key(|m| uuid_millis(&m.message_id));
        let skip = out.len().saturating_sub(limit as usize);
        Ok(out.into_iter().skip(skip).collect())
    }

    pub async fn touch_thread(&self, user: Uuid, other: Uuid, thread: Uuid, last: &str, unread: i32) -> Result<()> {
        self.exec(
            "UPDATE threads_by_user SET last_message = ?, last_message_at = ?, unread = ? WHERE user_id = ? AND other_user_id = ? AND thread_id = ?",
            (last, now_ts(), unread, user, other, tu(thread))).await?;
        Ok(())
    }

    pub async fn mark_read(&self, user: Uuid, other: Uuid, thread: Uuid) -> Result<()> {
        self.exec("UPDATE threads_by_user SET unread = 0 WHERE user_id = ? AND other_user_id = ? AND thread_id = ?",
            (user, other, tu(thread))).await?;
        Ok(())
    }
}
