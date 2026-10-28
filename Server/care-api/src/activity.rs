//! `activity-log` consumer (Administration_Design.md section 8): turns the events the API already
//! emits into the platform administrator's activity log and per-day counts.
//!
//! The log says who did what and when, and whose record it was. It never stores titles, message
//! text, report contents or vital readings: the dashboard monitors use of the service, not
//! people's health. Automatic events (reminders firing, thumbnails being made) are left out.

use crate::{
    events::{topics, Envelope, EventHandler},
    store::{admin::{day_of, ActivityRow}, Db},
};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

pub const CONSUMER: &str = "activity-log";
/// Topics the consumer reads. `alerts.events` (reminders, vital alerts) and `notify.requests`
/// are automatic, and vital alerts would reveal health details.
pub const TOPICS: [&str; 4] = [topics::ITEMS, topics::CHAT, topics::ACCESS_CHANGES, topics::AUDIT];

/// What an event means for the dashboard.
#[derive(Debug, PartialEq, Eq)]
pub struct Described {
    pub category: &'static str,
    /// Counter name for the trend chart, e.g. "items_created".
    pub metric: &'static str,
    /// A second counter, e.g. "bookings" for a new appointment record.
    pub also: Option<&'static str>,
    pub detail: String,
    /// Whose name to show as the subject: the record's owner, or the account itself.
    pub subject: Subject,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Subject { ItemOwner, User, Nobody }

pub const RECORDS: &str = "RECORDS";
pub const SHARING: &str = "SHARING";
pub const ACCOUNTS: &str = "ACCOUNTS";

fn s<'a>(ev: &'a Envelope, key: &str) -> &'a str { ev.payload.get(key).and_then(|v| v.as_str()).unwrap_or("") }

fn d(category: &'static str, metric: &'static str, detail: String, subject: Subject) -> Option<Described> {
    Some(Described { category, metric, also: None, detail, subject })
}

fn kind_noun(kind: &str) -> &'static str {
    match kind {
        "APPOINTMENT" => "an appointment",
        "MESSAGE" => "a conversation",
        "ALERT" => "a reminder",
        "REPORT" => "a report",
        _ => "a record",
    }
}

fn role_noun(role: &str) -> &'static str {
    match role {
        "PATIENT" => "a user",
        "DOCTOR" => "a doctor",
        "ADMINISTRATOR" => "a hospital administrator",
        _ => "an",
    }
}

/// Maps an event to a log line, or None for events the dashboard doesn't show.
pub fn describe(ev: &Envelope) -> Option<Described> {
    use Subject::*;
    match ev.event_type.as_str() {
        "item.created" => {
            let kind = s(ev, "primary_kind");
            let also = match kind { "APPOINTMENT" => Some("bookings"), "REPORT" => Some("reports"), _ => None };
            Some(Described { category: RECORDS, metric: "items_created", also, detail: format!("created {}", kind_noun(kind)), subject: ItemOwner })
        }
        "item.updated" => d(RECORDS, "items_updated", "edited a record".into(), ItemOwner),
        "item.closed" => d(RECORDS, "items_closed", "closed a record".into(), ItemOwner),
        "item.reopened" => d(RECORDS, "items_reopened", "reopened a record".into(), ItemOwner),
        "item.appointment" => d(RECORDS, "appointments", "added or changed an appointment".into(), ItemOwner),
        "item.alert" => d(RECORDS, "reminders", "added or changed a reminder".into(), ItemOwner),
        "item.attachments_added" => {
            let n = ev.payload.get("attachment_ids").and_then(|v| v.as_array()).map_or(1, Vec::len);
            d(RECORDS, "uploads", format!("uploaded {n} file{}", if n == 1 { "" } else { "s" }), ItemOwner)
        }
        "message.created" => d(RECORDS, "messages", "sent a message".into(), ItemOwner),
        "grant.added" => d(SHARING, "grants_added", "shared a record".into(), ItemOwner),
        "grant.revoked" => d(SHARING, "grants_revoked", "stopped sharing a record".into(), ItemOwner),
        "affiliation.ended" => d(SHARING, "affiliations_ended", "ended a doctor's work at a hospital".into(), User),
        "delegation.added" => d(SHARING, "delegations", "let an assistant act for a patient".into(), User),
        // Every record opened is checked; only refusals are worth a line.
        "access.decision" if s(ev, "access") == "DENY" => d(SHARING, "access_denied", format!("was refused access: {}", s(ev, "reason")), ItemOwner),
        "user.registered" => d(ACCOUNTS, "users_registered", "signed up".into(), User),
        "user.created" => d(ACCOUNTS, "accounts_created", format!("created {} account", role_noun(s(ev, "role"))), User),
        "user.deactivated" => d(ACCOUNTS, "accounts_deactivated", "deactivated an account".into(), User),
        "user.reactivated" => d(ACCOUNTS, "accounts_reactivated", "reactivated an account".into(), User),
        "hospital.created" => d(ACCOUNTS, "hospitals_created", "added a hospital".into(), User),
        "hospital.updated" => d(ACCOUNTS, "hospitals_updated", "edited a hospital's profile".into(), User),
        "admin.appointed" => d(ACCOUNTS, "admins_appointed", format!("appointed a hospital administrator for {}", s(ev, "hospital_name")), User),
        "admin.removed" => d(ACCOUNTS, "admins_removed", "removed a hospital administrator".into(), User),
        "invite.created" => d(ACCOUNTS, "invites_created", format!("created an administrator invite code for {}", s(ev, "hospital_name")), User),
        "invite.redeemed" => d(ACCOUNTS, "admins_appointed", format!("became an administrator of {} with an invite code", s(ev, "hospital_name")), Nobody),
        _ => None,
    }
}

pub struct ActivityLog {
    pub db: Arc<Db>,
}

impl ActivityLog {
    async fn name(&self, id: Uuid) -> String {
        match self.db.user(id).await { Ok(Some(u)) => u.display_name, _ => String::new() }
    }

    async fn subject_name(&self, ev: &Envelope, subject: &Subject) -> String {
        match subject {
            Subject::Nobody => String::new(),
            Subject::User => match ev.subject_id.parse() { Ok(id) => self.name(id).await, Err(_) => String::new() },
            Subject::ItemOwner => {
                let Ok(item) = ev.subject_id.parse::<Uuid>() else { return String::new() };
                match self.db.item(item).await { Ok(Some(i)) => self.name(i.owner_id).await, _ => String::new() }
            }
        }
    }
}

#[async_trait]
impl EventHandler for ActivityLog {
    async fn handle(&self, _topic: &str, ev: Envelope) -> Result<()> {
        let Some(d) = describe(&ev) else { return Ok(()) };
        if self.db.already_processed(CONSUMER, &ev.event_id).await? { return Ok(()); }
        // access.decision: the person refused is in the payload; the actor is the same person.
        let actor = ev.payload.get("user_id").and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
            .filter(|_| ev.event_type == "access.decision").unwrap_or(ev.actor_id);
        let day = day_of(ev.occurred_at);
        let row = ActivityRow {
            day: day.clone(),
            at: crate::model::new_timeuuid(),
            occurred_at: ev.occurred_at,
            category: d.category.to_string(),
            event_type: ev.event_type.clone(),
            actor_id: actor,
            actor_name: self.name(actor).await,
            subject_id: ev.subject_id.clone(),
            subject_name: self.subject_name(&ev, &d.subject).await,
            detail: d.detail,
        };
        self.db.record_activity(&row).await?;
        self.db.bump_count(&day, d.metric).await?;
        if let Some(m) = d.also { self.db.bump_count(&day, m).await?; }
        self.db.mark_active(&day, actor).await?;
        self.db.mark_processed(CONSUMER, &ev.event_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ev(t: &str, payload: serde_json::Value) -> Envelope { Envelope::new(t, Uuid::new_v4(), Uuid::new_v4(), vec![], payload) }

    #[test]
    fn records_sharing_and_accounts_are_logged() {
        let created = describe(&ev("item.created", json!({ "primary_kind": "APPOINTMENT" }))).unwrap();
        assert_eq!((created.category, created.metric, created.detail.as_str()), (RECORDS, "items_created", "created an appointment"));
        assert_eq!(created.also, Some("bookings"));
        assert_eq!(describe(&ev("item.created", json!({ "primary_kind": "REPORT" }))).unwrap().also, Some("reports"));
        assert_eq!(describe(&ev("item.attachments_added", json!({ "attachment_ids": [1, 2] }))).unwrap().detail, "uploaded 2 files");
        assert_eq!(describe(&ev("grant.added", json!({}))).unwrap().category, SHARING);
        assert_eq!(describe(&ev("user.created", json!({ "role": "DOCTOR" }))).unwrap().detail, "created a doctor account");
        assert_eq!(describe(&ev("user.created", json!({ "role": "ADMINISTRATOR" }))).unwrap().detail, "created a hospital administrator account");
    }

    #[test]
    fn message_text_never_reaches_the_log() {
        let d = describe(&ev("message.created", json!({ "message": { "body": "my test results are bad" } }))).unwrap();
        assert_eq!(d.detail, "sent a message");
    }

    #[test]
    fn only_refused_access_is_logged() {
        assert!(describe(&ev("access.decision", json!({ "access": "FULL" }))).is_none());
        assert!(describe(&ev("access.decision", json!({ "access": "METADATA_ONLY" }))).is_none());
        assert_eq!(describe(&ev("access.decision", json!({ "access": "DENY", "reason": "not shared" }))).unwrap().metric, "access_denied");
    }

    #[test]
    fn automatic_and_health_events_are_left_out() {
        for t in ["item.attachment", "alert.fired", "vital.alert", "notify.requested"] {
            assert!(describe(&ev(t, json!({}))).is_none(), "{t}");
        }
    }
}
