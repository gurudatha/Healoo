//! connections_by_user and conversations_by_user (the Messages tab: items with a discussion,
//! per other person). Messages themselves are in store/parts.rs.

use super::Db;
use crate::model::*;
use anyhow::Result;
use scylla::frame::value::{CqlTimestamp, CqlTimeuuid};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ConversationRow {
    pub other_user_id: Uuid,
    pub item_id: Uuid,
    pub last_message: String,
    pub last_message_at: Option<CqlTimestamp>,
    pub unread: i32,
}

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

    // ---- conversations (Messages tab) ----

    pub async fn conversations_of(&self, user: Uuid) -> Result<Vec<ConversationRow>> {
        let rows = self.rows::<(Uuid, CqlTimeuuid, Option<String>, Option<CqlTimestamp>, Option<i32>)>(
            "SELECT other_user_id, item_id, last_message, last_message_at, unread FROM conversations_by_user WHERE user_id = ?",
            (user,)).await?;
        let mut v: Vec<ConversationRow> = rows.into_iter().map(|r| ConversationRow {
            other_user_id: r.0, item_id: un(r.1), last_message: r.2.unwrap_or_default(), last_message_at: r.3, unread: r.4.unwrap_or(0),
        }).collect();
        v.sort_by_key(|c| std::cmp::Reverse(c.last_message_at.map(|t| t.0).unwrap_or(0)));
        Ok(v)
    }

    pub async fn conversation_unread(&self, user: Uuid, other: Uuid, item: Uuid) -> Result<i32> {
        Ok(self.one::<(Option<i32>,)>("SELECT unread FROM conversations_by_user WHERE user_id = ? AND other_user_id = ? AND item_id = ?",
            (user, other, tu(item))).await?.and_then(|r| r.0).unwrap_or(0))
    }

    pub async fn touch_conversation(&self, user: Uuid, other: Uuid, item: Uuid, last: &str, unread: i32) -> Result<()> {
        self.exec(
            "UPDATE conversations_by_user SET last_message = ?, last_message_at = ?, unread = ? WHERE user_id = ? AND other_user_id = ? AND item_id = ?",
            (last, now_ts(), unread, user, other, tu(item))).await?;
        Ok(())
    }

    /// Opening an item's discussion clears the reader's unread counts for it.
    pub async fn mark_item_read(&self, user: Uuid, item: Uuid) -> Result<()> {
        for c in self.conversations_of(user).await?.into_iter().filter(|c| c.item_id == item && c.unread > 0) {
            self.exec("UPDATE conversations_by_user SET unread = 0 WHERE user_id = ? AND other_user_id = ? AND item_id = ?",
                (user, c.other_user_id, tu(item))).await?;
        }
        Ok(())
    }
}
