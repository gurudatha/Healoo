//! Domain model (design doc 2.2, DataItem v2 in Documentation/DataItem_Design.md) and the JSON
//! shapes the Android/iOS clients decode.

use chrono::{DateTime, TimeZone, Utc};
use scylla::frame::value::{CqlTimestamp, CqlTimeuuid};
use scylla::macros::{FromUserType, SerializeCql};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

// ---------- enums (stored as text in Cassandra) ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Role { Patient, Doctor, Assistant, Lab, Hospital, Administrator, AppAdministrator }

/// What a DataItem started as (fixed at creation). DataItem_Design.md D1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PrimaryKind { Appointment, Message, Alert, Report }

/// Part kinds recorded in `kinds` (D2): the primary kinds plus ATTACHMENT.
pub mod part {
    pub const APPOINTMENT: &str = "APPOINTMENT";
    pub const MESSAGE: &str = "MESSAGE";
    pub const ALERT: &str = "ALERT";
    pub const REPORT: &str = "REPORT";
    pub const ATTACHMENT: &str = "ATTACHMENT";
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ItemStatus { Open, Closed }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GranteeType { User, Hospital }

/// Serialise a unit enum to its wire/DB text, e.g. `Role::AppAdministrator` → "APP_ADMINISTRATOR".
pub fn text<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v).ok().and_then(|j| j.as_str().map(str::to_string)).unwrap_or_default()
}

pub fn parse<T: DeserializeOwned>(s: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
}

impl Role {
    /// May create items for a patient in their contacts (the patient owns them).
    pub fn is_clinical(&self) -> bool { matches!(self, Role::Doctor | Role::Assistant | Role::Lab | Role::Hospital) }
    /// May pass an item they can fully read on to a doctor without owning it (referral).
    pub fn can_refer(&self) -> bool { matches!(self, Role::Doctor | Role::Lab | Role::Hospital) }
    /// Auth0 role names (doc 5.2) → internal roles.
    pub fn from_auth0(name: &str) -> Option<Role> {
        Some(match name.to_lowercase().as_str() {
            "patient" => Role::Patient,
            "doctor" => Role::Doctor,
            "assistant" => Role::Assistant,
            "lab" => Role::Lab,
            "hospital" => Role::Hospital,
            "hospital_admin" | "administrator" => Role::Administrator,
            "app_admin" | "app_administrator" => Role::AppAdministrator,
            _ => return None,
        })
    }
    pub fn headline(&self) -> &'static str {
        match self {
            Role::Patient => "Patient",
            Role::Doctor => "Doctor",
            Role::Assistant => "Assistant",
            Role::Lab => "Lab",
            Role::Hospital => "Hospital",
            Role::Administrator => "Hospital administrator",
            Role::AppAdministrator => "Platform administrator",
        }
    }
}

impl PrimaryKind {
    pub fn label(&self) -> &'static str {
        match self { PrimaryKind::Appointment => "Appointment", PrimaryKind::Message => "Message", PrimaryKind::Alert => "Alert", PrimaryKind::Report => "Report" }
    }
}

// ---------- ids and time ----------

/// New time-based id for items, threads and messages (Cassandra `timeuuid`).
pub fn new_timeuuid() -> Uuid {
    use std::sync::OnceLock;
    static NODE: OnceLock<[u8; 6]> = OnceLock::new();
    let node = NODE.get_or_init(|| {
        let mut n: [u8; 6] = rand::random();
        n[0] |= 0x01; // multicast bit: marks a random node id (RFC 4122 §4.5)
        n
    });
    Uuid::now_v1(node)
}

pub fn tu(u: Uuid) -> CqlTimeuuid { CqlTimeuuid::from(u) }
pub fn un(t: CqlTimeuuid) -> Uuid { Uuid::from(t) }
pub fn now_ts() -> CqlTimestamp { CqlTimestamp(Utc::now().timestamp_millis()) }
pub fn ts_to_dt(t: CqlTimestamp) -> DateTime<Utc> { Utc.timestamp_millis_opt(t.0).single().unwrap_or_default() }
pub fn date_to_ts(s: &str) -> Option<CqlTimestamp> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| CqlTimestamp(dt.and_utc().timestamp_millis()))
}
pub fn ts_to_date(t: CqlTimestamp) -> String { ts_to_dt(t).format("%Y-%m-%d").to_string() }
/// Millisecond time encoded in a v1 uuid, for merge-sorting items from several partitions.
pub fn uuid_millis(u: &Uuid) -> i64 {
    u.get_timestamp().map(|t| { let (s, n) = t.to_unix(); s as i64 * 1000 + (n / 1_000_000) as i64 }).unwrap_or(0)
}

// ---------- Cassandra user-defined types (field order = CQL order, doc 7.2) ----------

#[derive(Clone, Debug, FromUserType, SerializeCql)]
pub struct ClosureUdt {
    pub closed_at: Option<CqlTimestamp>,
    pub closed_by: Option<Uuid>,
    pub feedback: Option<String>,
    pub rating: Option<i8>,
}

#[derive(Clone, Debug, FromUserType, SerializeCql)]
pub struct VisitExceptionUdt {
    pub visit_date: Option<String>,
    pub action: Option<String>,
    pub new_date: Option<String>,
    pub new_time: Option<String>,
}

#[derive(Clone, Debug, FromUserType, SerializeCql)]
pub struct GrantUdt {
    pub grant_id: Option<Uuid>,
    pub grantee_type: Option<String>,
    pub grantee_id: Option<Uuid>,
    pub grantee_name: Option<String>,
    pub via_hospital_id: Option<Uuid>,
    pub granted_by: Option<Uuid>,
    pub granted_at: Option<CqlTimestamp>,
}

// ---------- JSON shapes (match the apps' Models.kt / Models.swift) ----------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Grant {
    pub grant_id: Uuid,
    pub grantee_type: GranteeType,
    pub grantee_id: Uuid,
    pub grantee_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via_hospital_id: Option<Uuid>,
}

impl Grant {
    pub fn from_udt(g: &GrantUdt) -> Option<Grant> {
        Some(Grant {
            grant_id: g.grant_id?,
            grantee_type: parse(g.grantee_type.as_deref()?)?,
            grantee_id: g.grantee_id?,
            grantee_name: g.grantee_name.clone().unwrap_or_default(),
            via_hospital_id: g.via_hospital_id,
        })
    }
    /// Partition key used in `items_by_grantee` (doc 7.2).
    pub fn key(&self) -> String { grantee_key(self.grantee_type, self.grantee_id) }
}

pub fn grantee_key(t: GranteeType, id: Uuid) -> String {
    match t { GranteeType::User => format!("user:{id}"), GranteeType::Hospital => format!("hospital:{id}") }
}

#[derive(Clone, Debug, Serialize)]
pub struct UserDto {
    pub user_id: Uuid,
    pub public_id: String,
    pub display_name: String,
    pub roles: Vec<Role>,
    pub headline: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub photo_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hospital: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub official_number: Option<String>,
    pub connected: bool,
}

// ---------- DataItem v2 (Documentation/DataItem_Design.md section 3) ----------

#[derive(Clone, Debug, Serialize)]
pub struct Closure {
    pub closed_at: String,
    pub closed_by: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feedback: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<i8>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ItemCounts {
    pub messages: i32,
    pub attachments: i32,
    pub appointments: i32,
    pub alerts: i32,
    pub unread_messages: i32,
}

#[derive(Clone, Debug, Serialize)]
pub struct AttachmentDto {
    pub attachment_id: Uuid,
    pub kind: String, // IMAGE | PDF
    pub uri: String,  // short-lived signed URL
    pub mime: String,
    pub size: i64,
    pub position: i32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumb_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    pub added_by: Uuid,
    pub added_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<Uuid>,
    pub is_report: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MessageDto {
    pub message_id: Uuid,
    pub item_id: Uuid,
    pub sender_id: Uuid,
    pub body: String,
    pub sent_at: String,
    #[serde(default)]
    pub attachment_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VisitDto {
    pub date: String,
    pub time: String,
    pub original_date: String,
    /// SCHEDULED | COMPLETED | CANCELLED | NO_SHOW
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct AppointmentDto {
    pub appointment_id: Uuid,
    pub item_id: Uuid,
    pub patient_id: Uuid,
    pub doctor_id: Uuid,
    pub doctor_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hospital_id: Option<Uuid>,
    pub date: String,
    pub time: String,
    pub timezone: String,
    pub duration_min: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<crate::recurrence::Recurrence>,
    pub exceptions: Vec<crate::recurrence::VisitException>,
    /// SCHEDULED | COMPLETED | CANCELLED
    pub status: String,
    /// Total visits for a series with a period; absent when it runs until cancelled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visit_count: Option<u32>,
    /// Upcoming visits (next 12).
    pub visits: Vec<VisitDto>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AlertDto {
    pub alert_id: Uuid,
    pub item_id: Uuid,
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
    pub for_user: Uuid,
    pub fires_at: String,
    pub timezone: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<crate::recurrence::Recurrence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appointment_id: Option<Uuid>,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ItemDto {
    pub item_id: Uuid,
    pub owner_id: Uuid,
    pub primary_kind: PrimaryKind,
    pub kinds: Vec<String>,
    pub title: String,
    pub keywords: Vec<String>,
    pub status: ItemStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closure: Option<Closure>,
    /// Only the owner sees who else has access.
    pub access_list: Vec<Grant>,
    pub counts: ItemCounts,
    pub messages: Vec<MessageDto>,
    pub attachments: Vec<AttachmentDto>,
    pub appointments: Vec<AppointmentDto>,
    pub alerts: Vec<AlertDto>,
    /// https links attached to a report (full access only).
    pub links: Vec<String>,
    pub created_by_name: String,
    pub created_at: String,
    pub updated_at: String,
    /// read, message, attach, book, alert, share, revoke, close, rate, reopen, meta
    pub allowed_actions: Vec<String>,
}

/// Messages tab: one row per item with a discussion, per other person.
#[derive(Clone, Debug, Serialize)]
pub struct ConversationDto {
    pub item_id: Uuid,
    pub item_title: String,
    pub primary_kind: PrimaryKind,
    /// OPEN or CLOSED: the apps grey out discussions of closed items.
    pub status: ItemStatus,
    pub other_user: UserDto,
    pub last_message: String,
    pub last_message_at: String,
    pub unread: i32,
}

#[derive(Clone, Debug, Serialize)]
pub struct DashboardDto {
    pub open_reports: i32,
    pub unread_messages: i32,
    pub upcoming_appointments: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotificationPrefs {
    pub push_messages: bool,
    pub push_reports: bool,
    pub quiet_hours: bool,
    pub quiet_start: String,
    pub quiet_end: String,
}

impl Default for NotificationPrefs {
    fn default() -> Self {
        Self { push_messages: true, push_reports: true, quiet_hours: false, quiet_start: "22:00".into(), quiet_end: "07:00".into() }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct OwnedGrantDto {
    pub grant_id: Uuid,
    pub grantee_type: GranteeType,
    pub grantee_id: Uuid,
    pub grantee_name: String,
    pub item_id: Uuid,
    pub item_title: String,
    pub primary_kind: PrimaryKind,
}

#[derive(Serialize)]
pub struct Page<T: Serialize> {
    pub data: Vec<T>,
    pub page_state: Option<String>,
}

impl<T: Serialize> Page<T> {
    pub fn of(data: Vec<T>) -> Self { Page { data, page_state: None } }
}
