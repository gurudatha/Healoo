//! Domain model (doc 2.2) and the JSON shapes the Android/iOS clients decode.

use chrono::{DateTime, TimeZone, Utc};
use scylla::frame::value::{CqlTimestamp, CqlTimeuuid};
use scylla::macros::{FromUserType, SerializeCql};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

// ---------- enums (stored as text in Cassandra) ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Role { Patient, Doctor, Assistant, Lab, Hospital, Administrator, AppAdministrator }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CoreItemType { Report, Alert, Booking, Feedback, Payment, Message }

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
    pub fn is_clinical(&self) -> bool { matches!(self, Role::Doctor | Role::Assistant | Role::Lab) }
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

impl CoreItemType {
    pub fn label(&self) -> String {
        let t = text(self).to_lowercase();
        let mut c = t.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
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
pub struct AttachmentUdt {
    pub kind: Option<String>,
    pub uri: Option<String>,
    pub mime: Option<String>,
    pub size: Option<i64>,
    pub sha256: Option<String>,
    pub position: Option<i32>,
    pub page_count: Option<i32>,
    pub thumb_uri: Option<String>,
    pub name: Option<String>,
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
pub struct Attachment {
    pub kind: String, // IMAGE | PDF
    pub uri: String,
    pub mime: String,
    pub size: i64,
    pub position: i32,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_count: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumb_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

impl From<AttachmentUdt> for Attachment {
    fn from(a: AttachmentUdt) -> Self {
        Attachment {
            kind: a.kind.unwrap_or_else(|| "IMAGE".into()),
            uri: a.uri.unwrap_or_default(),
            mime: a.mime.unwrap_or_default(),
            size: a.size.unwrap_or(0),
            position: a.position.unwrap_or(0),
            name: a.name.unwrap_or_default(),
            page_count: a.page_count,
            thumb_uri: a.thumb_uri,
            sha256: a.sha256,
        }
    }
}

impl From<&Attachment> for AttachmentUdt {
    fn from(a: &Attachment) -> Self {
        AttachmentUdt {
            kind: Some(a.kind.clone()),
            uri: Some(a.uri.clone()),
            mime: Some(a.mime.clone()),
            size: Some(a.size),
            sha256: a.sha256.clone(),
            position: Some(a.position),
            page_count: a.page_count,
            thumb_uri: a.thumb_uri.clone(),
            name: Some(a.name.clone()),
        }
    }
}

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
pub struct ItemDto {
    pub item_id: Uuid,
    pub date: String,
    pub core_item_type: CoreItemType,
    pub title: String,
    pub subtitle: String,
    pub keywords: Vec<String>,
    pub core_item_data: Vec<Attachment>,
    pub links: Vec<String>,
    pub owner_id: Uuid,
    pub created_by_name: String,
    pub access_list: Vec<Grant>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pointer_to_message: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pointer_item_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<i8>,
    pub status: ItemStatus,
    pub allowed_actions: Vec<String>,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MessageDto {
    pub message_id: Uuid,
    pub thread_id: Uuid,
    pub sender_id: Uuid,
    pub body: String,
    pub sent_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linked_item_id: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThreadDto {
    pub thread_id: Uuid,
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
    pub core_item_type: CoreItemType,
}

#[derive(Serialize)]
pub struct Page<T: Serialize> {
    pub data: Vec<T>,
    pub page_state: Option<String>,
}

impl<T: Serialize> Page<T> {
    pub fn of(data: Vec<T>) -> Self { Page { data, page_state: None } }
}
