//! Platform administrator dashboard (Administration_Design.md section 8): the account list,
//! the activity log and its per-day counts, hospital profiles and administrator appointments.

use super::{users::{name_tokens, UserRec, DEACTIVATED}, Db};
use crate::model::*;
use anyhow::Result;
use chrono::{DateTime, Utc};
use scylla::frame::value::{CqlTimestamp, CqlTimeuuid};
use std::collections::HashSet;
use uuid::Uuid;

/// One account in the administrator's list.
#[derive(Clone, Debug)]
pub struct AccountRow {
    pub user_id: Uuid,
    pub public_id: String,
    pub display_name: String,
    pub roles: HashSet<Role>,
    pub hospital_id: Option<Uuid>,
    pub email: Option<String>,
    pub active: bool,
    pub created_at: Option<DateTime<Utc>>,
    /// Has signed in at least once (an Auth0 or developer sign-in is linked).
    pub signed_in: bool,
}

impl AccountRow {
    pub fn primary_role(&self) -> Role {
        for r in [Role::AppAdministrator, Role::Administrator, Role::Hospital, Role::Lab, Role::Doctor, Role::Assistant] {
            if self.roles.contains(&r) { return r; }
        }
        Role::Patient
    }
}

/// One line of the activity log. Never holds titles, message text or other health details.
#[derive(Clone, Debug)]
pub struct ActivityRow {
    pub day: String,
    pub at: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub category: String,
    pub event_type: String,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub subject_id: String,
    pub subject_name: String,
    pub detail: String,
}

type AccountDbRow = (Uuid, Option<String>, Option<String>, Option<HashSet<String>>, Option<Uuid>, Option<String>, Option<String>, Option<CqlTimestamp>, Option<String>);
type ActivityDbRow = (CqlTimeuuid, Option<CqlTimestamp>, Option<String>, Option<String>, Option<Uuid>, Option<String>, Option<String>, Option<String>, Option<String>);

/// The UTC day an activity line or count belongs to, e.g. "2026-10-25".
pub fn day_of(t: DateTime<Utc>) -> String { t.format("%Y-%m-%d").to_string() }

impl Db {
    /// Every account, unsorted. A full scan: fine for a trial or a single hospital group; a
    /// large deployment would keep a separate listing table.
    pub async fn all_accounts(&self) -> Result<Vec<AccountRow>> {
        let rows = self.rows_all::<AccountDbRow>(
            "SELECT user_id, public_id, display_name, roles, primary_hospital_id, email, status, created_at, auth0_sub FROM users", ()).await?;
        Ok(rows.into_iter().map(|r| AccountRow {
            user_id: r.0,
            public_id: r.1.unwrap_or_default(),
            display_name: r.2.unwrap_or_default(),
            roles: r.3.unwrap_or_default().iter().filter_map(|s| parse::<Role>(s)).collect(),
            hospital_id: r.4,
            email: r.5,
            active: r.6.as_deref() != Some(DEACTIVATED),
            created_at: r.7.map(ts_to_dt),
            signed_in: r.8.is_some(),
        }).collect())
    }

    /// Accounts that are platform administrators now (for the limit in server.json).
    pub async fn platform_admin_ids(&self) -> Result<Vec<Uuid>> {
        Ok(self.all_accounts().await?.into_iter().filter(|a| a.roles.contains(&Role::AppAdministrator)).map(|a| a.user_id).collect())
    }

    /// The hospital's administrators now (for the limit in server.json).
    pub async fn hospital_admin_ids(&self, hospital: Uuid) -> Result<Vec<Uuid>> {
        Ok(self.all_accounts().await?.into_iter()
            .filter(|a| a.roles.contains(&Role::Administrator) && a.hospital_id == Some(hospital))
            .map(|a| a.user_id).collect())
    }

    // ---- activity log ----

    pub async fn record_activity(&self, a: &ActivityRow) -> Result<()> {
        self.exec(
            "INSERT INTO activity_by_day (day, at, occurred_at, category, event_type, actor_id, actor_name, subject_id, subject_name, detail) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (&a.day, tu(a.at), CqlTimestamp(a.occurred_at.timestamp_millis()), &a.category, &a.event_type, a.actor_id, &a.actor_name, &a.subject_id, &a.subject_name, &a.detail),
        ).await?;
        Ok(())
    }

    /// One day's activity, newest first, at most `limit` lines.
    pub async fn activity(&self, day: &str, limit: i32) -> Result<Vec<ActivityRow>> {
        let rows = self.rows::<ActivityDbRow>(
            "SELECT at, occurred_at, category, event_type, actor_id, actor_name, subject_id, subject_name, detail FROM activity_by_day WHERE day = ? LIMIT ?",
            (day, limit)).await?;
        Ok(rows.into_iter().map(|r| ActivityRow {
            day: day.to_string(),
            at: un(r.0),
            occurred_at: r.1.map(ts_to_dt).unwrap_or_default(),
            category: r.2.unwrap_or_default(),
            event_type: r.3.unwrap_or_default(),
            actor_id: r.4.unwrap_or_default(),
            actor_name: r.5.unwrap_or_default(),
            subject_id: r.6.unwrap_or_default(),
            subject_name: r.7.unwrap_or_default(),
            detail: r.8.unwrap_or_default(),
        }).collect())
    }

    pub async fn bump_count(&self, day: &str, metric: &str) -> Result<()> {
        self.exec("UPDATE activity_counts SET n = n + 1 WHERE day = ? AND metric = ?", (day, metric)).await?;
        Ok(())
    }

    /// The day's counts by metric (e.g. "items_created" → 12).
    pub async fn counts(&self, day: &str) -> Result<Vec<(String, i64)>> {
        let rows = self.rows::<(String, scylla::frame::value::Counter)>("SELECT metric, n FROM activity_counts WHERE day = ?", (day,)).await?;
        Ok(rows.into_iter().map(|(m, n)| (m, n.0)).collect())
    }

    pub async fn mark_active(&self, day: &str, user: Uuid) -> Result<()> {
        self.exec("INSERT INTO active_users_by_day (day, user_id) VALUES (?, ?)", (day, user)).await?;
        Ok(())
    }

    /// How many different people did something on that day.
    pub async fn active_users(&self, day: &str) -> Result<i64> {
        Ok(self.one::<(i64,)>("SELECT COUNT(*) FROM active_users_by_day WHERE day = ?", (day,)).await?.map(|r| r.0).unwrap_or(0))
    }

    /// Everyone who did something on that day.
    pub async fn active_user_ids(&self, day: &str) -> Result<Vec<Uuid>> {
        Ok(self.rows_all::<(Uuid,)>("SELECT user_id FROM active_users_by_day WHERE day = ?", (day,)).await?.into_iter().map(|r| r.0).collect())
    }

    // ---- storage ----

    /// On-disk size of each table in the keyspace, in MiB (Cassandra rounds down, and recent
    /// writes still in memory are not counted until they are flushed).
    pub async fn table_sizes(&self, keyspace: &str) -> Result<Vec<(String, i64)>> {
        let rows = self.rows_all::<(String, Option<i64>)>(
            "SELECT table_name, mebibytes FROM system_views.disk_usage WHERE keyspace_name = ?", (keyspace,)).await?;
        Ok(rows.into_iter().map(|(t, m)| (t, m.unwrap_or(0))).collect())
    }

    /// Row counts of the main record tables. Each is a full scan, so a large database answers
    /// None (timed out) rather than holding the dashboard up.
    pub async fn record_counts(&self) -> Vec<(&'static str, Option<i64>)> {
        async fn count(db: &Db, cql: &'static str) -> Option<i64> {
            match tokio::time::timeout(std::time::Duration::from_secs(10), db.one::<(i64,)>(cql, ())).await {
                Ok(Ok(r)) => r.map(|r| r.0),
                Ok(Err(e)) => { tracing::warn!("{cql}: {e:#}"); None }
                Err(_) => None,
            }
        }
        vec![
            ("records", count(self, "SELECT COUNT(*) FROM items").await),
            ("messages", count(self, "SELECT COUNT(*) FROM item_messages").await),
            ("files", count(self, "SELECT COUNT(*) FROM item_attachments").await),
            ("appointments", count(self, "SELECT COUNT(*) FROM item_appointments").await),
            ("vital_readings", count(self, "SELECT COUNT(*) FROM vital_readings").await),
        ]
    }

    // ---- hospitals ----

    /// A hospital administrator edits their hospital's profile.
    pub async fn update_hospital(&self, h: &UserRec, name: &str, location: Option<&str>, phone: Option<&str>, registration: Option<&str>, headline: Option<&str>) -> Result<()> {
        self.batch(
            &[
                "UPDATE users SET display_name = ?, name_tokens = ?, location = ?, phone = ?, official_number = ?, headline = ? WHERE user_id = ?",
                "UPDATE users_by_public_id SET display_name = ?, location = ? WHERE public_id = ?",
            ],
            ((name, name_tokens(name), location, phone, registration, headline, h.user_id), (name, location, &h.public_id)),
        ).await
    }

    /// Gives an account an email, so a later Auth0 sign-in with that verified email links to it.
    pub async fn set_email(&self, user: Uuid, email: &str) -> Result<()> {
        let email = email.trim().to_lowercase();
        self.exec("UPDATE users SET email = ? WHERE user_id = ?", (&email, user)).await?;
        self.exec("INSERT INTO users_by_email (email, user_id) VALUES (?, ?)", (&email, user)).await?;
        Ok(())
    }

    /// Changes an account's roles and home hospital, e.g. when a platform administrator appoints
    /// a hospital administrator.
    pub async fn set_roles(&self, u: &UserRec, roles: &HashSet<Role>, hospital: Option<Uuid>) -> Result<()> {
        let roles: HashSet<String> = roles.iter().map(text).collect();
        self.batch(
            &[
                "UPDATE users SET roles = ?, primary_hospital_id = ? WHERE user_id = ?",
                "UPDATE users_by_public_id SET roles = ? WHERE public_id = ?",
            ],
            ((&roles, hospital, u.user_id), (&roles, &u.public_id)),
        ).await
    }
}
