//! DataItem v2 header endpoints (Documentation/DataItem_Design.md section 4):
//! /v1/items, /v1/items/{id}, close, reopen, /v1/uploads/presign, /v1/dashboard,
//! /v1/users/{id}/shared-items. Child endpoints are in api/parts.rs.

use super::{
    parts::{self, NewAlert, NewAppointment, NewAttachment},
    principal::Principal,
    users::{load_user, user_dto},
    ApiError, ApiResult, AppState,
};
use crate::{
    events::{topics, Envelope},
    files::object_uri,
    model::*,
    policy::{self, Access, AssistantContext, Decision},
    store::items::{Item, ItemSummary},
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;
use uuid::Uuid;

pub const MAX_ATTACHMENTS: usize = 20;
pub const IMAGE_MAX: i64 = 10 * 1024 * 1024;
pub const PDF_MAX: i64 = 25 * 1024 * 1024;

// ---------------- policy glue ----------------

/// Runs the policy engine for `p` on `item` (loading assistant context when needed) and logs the
/// decision to `access.audit` (design doc 2.3). One decision covers the item and all its children.
pub async fn decide(st: &AppState, p: &Principal, item: &Item) -> ApiResult<Decision> {
    let assistant = if p.has(Role::Assistant) {
        match p.user.employer_doctor_id {
            Some(doc_id) => {
                let doctor_subject = match st.db.user(doc_id).await? {
                    Some(d) => Some(policy::Subject { user_id: d.user_id, roles: d.roles.clone(), active_hospitals: st.db.active_hospitals(d.user_id).await? }),
                    None => None,
                };
                Some(AssistantContext { doctor: doctor_subject, delegated_for_owner: st.db.delegation_exists(p.id(), doc_id, item.owner_id).await? })
            }
            None => None,
        }
    } else {
        None
    };
    let d = policy::decide(&item.facts(), &p.subject, assistant.as_ref());
    let ev = Envelope::new("access.decision", p.id(), item.id, vec![], json!({ "user_id": p.id(), "access": d.access, "reason": d.reason }));
    st.events.emit_fast(topics::AUDIT, &item.id.to_string(), ev).await;
    Ok(d)
}

/// Loads an item and requires full access (writes and child reads).
pub async fn load_full(st: &AppState, p: &Principal, id: Uuid) -> ApiResult<(Item, Decision)> {
    let item = load_item(st, id).await?;
    let d = decide(st, p, &item).await?;
    if d.access != Access::Full { return Err(ApiError::forbidden(d.reason)); }
    Ok((item, d))
}

pub async fn load_item(st: &AppState, id: Uuid) -> ApiResult<Item> {
    st.db.item(id).await?.ok_or_else(|| ApiError::not_found("no such item"))
}

pub fn require_open(item: &Item) -> ApiResult<()> {
    if item.status == ItemStatus::Closed { return Err(ApiError::conflict("this item is closed; reopen it to add to it")); }
    Ok(())
}

fn allowed_actions(p: &Principal, item: &Item, d: &Decision) -> Vec<String> {
    let owner = item.owner_id == p.id();
    let mut v: Vec<&str> = match (d.access, item.status) {
        (Access::Full, ItemStatus::Open) => vec!["read", "message", "attach", "book", "alert", "close"],
        (Access::Full, ItemStatus::Closed) => vec!["read"],
        (Access::MetadataOnly, _) => vec!["meta"],
        (Access::Deny, _) => vec![],
    };
    if d.access == Access::Full {
        if owner { v.extend(["share", "revoke"]); }
        else if p.user.roles.iter().any(|r| r.can_refer()) { v.push("share"); }   // to doctors only
        if owner && item.status == ItemStatus::Open { v.push("rate"); }
        if item.status == ItemStatus::Closed && (owner || p.has(Role::Doctor)) { v.push("reopen"); }
    }
    v.into_iter().map(str::to_string).collect()
}

fn sorted_kinds(k: &HashSet<String>) -> Vec<String> {
    let order = [part::REPORT, part::APPOINTMENT, part::MESSAGE, part::ALERT, part::ATTACHMENT];
    let mut v: Vec<String> = k.iter().cloned().collect();
    v.sort_by_key(|s| order.iter().position(|o| o == s).unwrap_or(99));
    v
}

fn closure_dto(c: &ClosureUdt) -> Option<Closure> {
    Some(Closure {
        closed_at: c.closed_at.map(|t| ts_to_dt(t).to_rfc3339()).unwrap_or_default(),
        closed_by: c.closed_by?,
        feedback: c.feedback.clone(),
        rating: c.rating,
    })
}

/// Full item with children (detail view), shaped by the access decision (section 3.8).
pub async fn item_dto(st: &AppState, p: &Principal, item: &Item, d: &Decision) -> ApiResult<ItemDto> {
    let full = d.access == Access::Full;
    let (mut messages, mut attachments, mut alerts) = (vec![], vec![], vec![]);
    let mut appointments = Vec::new();
    for a in st.db.appointments(item.id).await? {
        appointments.push(parts::appointment_dto(st, item.id, &a, !full).await?);
    }
    let mut unread = 0;
    if full {
        messages = st.db.item_messages(item.id, 100).await?;
        for a in st.db.attachments(item.id).await? { attachments.push(parts::attachment_dto(st, &a).await?); }
        let owner = item.owner_id == p.id();
        alerts = st.db.alerts(item.id).await?.iter()
            .filter(|a| owner || a.for_user == p.id())
            .map(|a| parts::alert_dto(item.id, a)).collect();
        unread = st.db.conversations_of(p.id()).await?.iter().filter(|c| c.item_id == item.id).map(|c| c.unread).sum();
    }
    let counts = ItemCounts {
        messages: messages.len() as i32,
        attachments: attachments.len() as i32,
        appointments: appointments.len() as i32,
        alerts: alerts.len() as i32,
        unread_messages: unread,
    };
    Ok(ItemDto {
        item_id: item.id,
        owner_id: item.owner_id,
        primary_kind: item.primary_kind,
        kinds: sorted_kinds(&item.kinds),
        title: item.title.clone(),
        keywords: item.keywords.iter().cloned().collect(),
        status: item.status,
        closure: item.closure.as_ref().and_then(closure_dto),
        access_list: if item.owner_id == p.id() { item.grant_list() } else { vec![] },
        counts,
        messages,
        attachments,
        appointments,
        alerts,
        links: if full { item.links.clone() } else { vec![] },
        created_by_name: item.created_by_name.clone(),
        created_at: ts_to_dt(item.created_at).to_rfc3339(),
        updated_at: ts_to_dt(item.updated_at).to_rfc3339(),
        allowed_actions: allowed_actions(p, item, d),
    })
}

/// Light version for lists: header only, children empty.
fn summary_dto(s: &ItemSummary, p: &Principal) -> ItemDto {
    ItemDto {
        item_id: s.id,
        owner_id: s.owner_id,
        primary_kind: s.primary_kind,
        kinds: sorted_kinds(&s.kinds),
        title: s.title.clone(),
        keywords: vec![],
        status: s.status,
        closure: None,
        access_list: vec![],
        counts: ItemCounts::default(),
        messages: vec![],
        attachments: vec![],
        appointments: vec![],
        alerts: vec![],
        links: vec![],
        created_by_name: String::new(),
        created_at: String::new(),
        updated_at: ts_to_dt(s.updated_at).to_rfc3339(),
        allowed_actions: if s.owner_id == p.id() || p.user.roles.iter().any(|r| r.can_refer()) { vec!["read".into(), "share".into()] } else { vec!["read".into()] },
    }
}

// ---------------- listing ----------------

/// Everything the caller may list, most recently updated first: own items, direct grants,
/// hospital grants through active affiliations, and delegated patients for assistants.
pub async fn visible(st: &AppState, p: &Principal, status: ItemStatus, limit: i32) -> ApiResult<Vec<ItemSummary>> {
    let mut all = st.db.owner_partition(p.id(), status, limit).await?;
    all.extend(st.db.grantee_partition(&grantee_key(GranteeType::User, p.id()), status, limit).await?);
    for h in &p.subject.active_hospitals {
        all.extend(st.db.grantee_partition(&grantee_key(GranteeType::Hospital, *h), status, limit).await?);
    }
    if p.has(Role::Assistant) {
        for patient in st.db.delegated_patients(p.id()).await? {
            all.extend(st.db.owner_partition(patient, status, limit).await?);
        }
    }
    let mut seen = HashSet::new();
    all.retain(|s| seen.insert(s.id));
    all.sort_by_key(|s| std::cmp::Reverse(s.updated_at.0.max(uuid_millis(&s.id))));
    all.truncate(limit as usize);
    Ok(all)
}

#[derive(Deserialize)]
pub struct ListQ { status: Option<String>, kind: Option<String>, limit: Option<i32> }

pub async fn list(State(st): State<AppState>, p: Principal, Query(q): Query<ListQ>) -> ApiResult<Json<Page<ItemDto>>> {
    let status = q.status.as_deref().and_then(parse).unwrap_or(ItemStatus::Open);
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let kind = q.kind.map(|k| k.to_uppercase());
    let rows = visible(&st, &p, status, if kind.is_some() { 100 } else { limit }).await?;
    let out: Vec<ItemDto> = rows.iter()
        .filter(|s| kind.as_ref().map_or(true, |k| s.kinds.contains(k) || text(&s.primary_kind) == *k))
        .take(limit as usize)
        .map(|s| summary_dto(s, &p)).collect();
    Ok(Json(Page::of(out)))
}

pub async fn dashboard(State(st): State<AppState>, p: Principal) -> ApiResult<Json<DashboardDto>> {
    let open = visible(&st, &p, ItemStatus::Open, 200).await?;
    let unread: i32 = st.db.conversations_of(p.id()).await?.iter().map(|c| c.unread).sum();
    let today = crate::store::parts::today_in(crate::store::parts::DEFAULT_TZ);
    let week = today + chrono::Duration::days(7);
    let visits = if p.has(Role::Doctor) { st.db.doctor_visits(p.id(), today, week).await? } else { st.db.patient_visits(p.id(), today, week).await? };
    let now = chrono::Utc::now().timestamp_millis();
    Ok(Json(DashboardDto {
        open_reports: open.iter().filter(|s| s.kinds.contains(part::REPORT)).count() as i32,
        unread_messages: unread,
        upcoming_appointments: visits.iter().filter(|v| v.0 >= now && v.4 == "SCHEDULED").count() as i32,
    }))
}

pub async fn get_one(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<ItemDto>> {
    let item = load_item(&st, id).await?;
    let d = decide(&st, &p, &item).await?;
    if d.access == Access::Deny { return Err(ApiError::forbidden(d.reason)); }
    Ok(Json(item_dto(&st, &p, &item, &d).await?))
}

/// Items shared between the caller and another user; empty unless connected (rule 8).
pub async fn shared_items(State(st): State<AppState>, p: Principal, Path(other_id): Path<Uuid>) -> ApiResult<Json<Page<ItemDto>>> {
    let other = load_user(&st, other_id).await?;
    if !st.db.is_connected(p.id(), other_id, other.primary_role()).await? {
        return Ok(Json(Page::of(vec![])));
    }
    let other_hospitals = if other.roles.contains(&Role::Doctor) { st.db.active_hospitals(other_id).await? } else { HashSet::new() };
    let mut out = Vec::new();
    for s in st.db.owner_partition(p.id(), ItemStatus::Open, 100).await? {
        if let Some(item) = st.db.item(s.id).await? {
            let visible_to_other = item.grant_list().iter().any(|g| match g.grantee_type {
                GranteeType::User => g.grantee_id == other_id,
                GranteeType::Hospital => other_hospitals.contains(&g.grantee_id),
            });
            if visible_to_other { out.push(summary_dto(&s, &p)); }
        }
    }
    for s in st.db.owner_partition(other_id, ItemStatus::Open, 100).await? {
        if let Some(item) = st.db.item(s.id).await? {
            if policy::decide(&item.facts(), &p.subject, None).access != Access::Deny { out.push(summary_dto(&s, &p)); }
        }
    }
    out.sort_by_key(|i| std::cmp::Reverse(i.updated_at.clone()));
    Ok(Json(Page::of(out)))
}

// ---------------- create ----------------

#[derive(Deserialize)]
pub struct NewReport {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub attachments: Vec<NewAttachment>,
    #[serde(default)]
    pub links: Vec<String>,
}

#[derive(Deserialize)]
pub struct NewMessage {
    pub body: String,
    pub client_msg_id: String,
    #[serde(default)]
    pub attachments: Vec<NewAttachment>,
}

/// POST /v1/items: header fields plus exactly one primary part.
#[derive(Deserialize)]
pub struct NewItem {
    owner_id: Option<Uuid>,
    #[serde(default)]
    title: String,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    share_with: Vec<Uuid>,
    appointment: Option<NewAppointment>,
    message: Option<NewMessage>,
    alert: Option<NewAlert>,
    report: Option<NewReport>,
}

async fn grant_udt(st: &AppState, grantee: Uuid, granted_by: Uuid) -> ApiResult<GrantUdt> {
    let u = load_user(st, grantee).await?;
    let kind = if u.primary_role() == Role::Hospital { GranteeType::Hospital } else { GranteeType::User };
    Ok(GrantUdt {
        grant_id: Some(Uuid::new_v4()),
        grantee_type: Some(text(&kind)),
        grantee_id: Some(grantee),
        grantee_name: Some(u.display_name),
        via_hospital_id: None,
        granted_by: Some(granted_by),
        granted_at: Some(now_ts()),
    })
}

pub async fn create(State(st): State<AppState>, p: Principal, Json(b): Json<NewItem>) -> ApiResult<Json<ItemDto>> {
    let parts_given = [b.appointment.is_some(), b.message.is_some(), b.alert.is_some(), b.report.is_some()].iter().filter(|x| **x).count();
    if parts_given != 1 {
        return Err(ApiError::bad_request("send exactly one of appointment, message, alert or report"));
    }
    let owner_id = b.owner_id.or(b.appointment.as_ref().map(|a| a.patient_id)).unwrap_or(p.id());
    let for_someone_else = owner_id != p.id();
    let owner = load_user(&st, owner_id).await?;
    if owner.primary_role() != Role::Patient && for_someone_else {
        return Err(ApiError::bad_request("items belong to a patient: owner_id must be a patient"));
    }
    if for_someone_else {
        if !p.user.roles.iter().any(|r| r.is_clinical()) {
            return Err(ApiError::forbidden("only doctors, assistants, labs and hospitals can create items for someone else"));
        }
        let connected = st.db.is_connected(p.id(), owner_id, Role::Patient).await?
            || (p.has(Role::Assistant) && p.user.employer_doctor_id.is_some()
                && st.db.is_connected(p.user.employer_doctor_id.unwrap(), owner_id, Role::Patient).await?);
        if !connected { return Err(ApiError::forbidden("you can create items only for patients in your contacts")); }
    }
    if b.report.is_some() && p.has(Role::Lab) && !for_someone_else {
        return Err(ApiError::bad_request("labs upload reports for a patient: set owner_id"));
    }

    let primary_kind = if b.appointment.is_some() { PrimaryKind::Appointment }
        else if b.message.is_some() { PrimaryKind::Message }
        else if b.alert.is_some() { PrimaryKind::Alert }
        else { PrimaryKind::Report };

    // Validate the primary part before writing anything.
    let report_files = match &b.report {
        Some(r) => {
            let files = parts::validate_attachments(&p, &r.attachments)?;
            let links: Vec<&String> = r.links.iter().filter(|l| l.starts_with("https://")).collect();
            if files.is_empty() && links.is_empty() { return Err(ApiError::bad_request("a report needs at least one file or link")); }
            files
        }
        None => vec![],
    };
    let message_files = match &b.message { Some(m) => parts::validate_attachments(&p, &m.attachments)?, None => vec![] };
    if let Some(a) = &b.appointment { parts::validate_appointment(&st, owner_id, a).await?; }
    if let Some(a) = &b.alert { parts::validate_alert(a)?; }

    // Access list: owner's shares; uploader keeps access; the appointment's doctor gets access.
    let mut grants: Vec<GrantUdt> = Vec::new();
    let mut add_grant = |g: GrantUdt, grants: &mut Vec<GrantUdt>| { if !grants.iter().any(|x| x.grantee_id == g.grantee_id) { grants.push(g); } };
    // Creating for a patient, the uploader may also refer the item to fellow doctors.
    if for_someone_else { add_grant(grant_udt(&st, p.id(), p.id()).await?, &mut grants); }
    for g in b.share_with.iter().collect::<HashSet<_>>() {
        if *g == p.id() || *g == owner_id { continue; }
        if for_someone_else && !policy::may_share_on_create(true, &load_user(&st, *g).await?.roles) {
            return Err(ApiError::forbidden("when creating for a patient you can share only with doctors"));
        }
        add_grant(grant_udt(&st, *g, p.id()).await?, &mut grants);
    }
    if let Some(a) = &b.appointment {
        if a.doctor_id != owner_id { add_grant(grant_udt(&st, a.doctor_id, p.id()).await?, &mut grants); }
    }

    let default_title = match (&b.report, &b.message, &b.alert, &b.appointment) {
        (Some(r), _, _, _) if !r.title.trim().is_empty() => r.title.trim().to_string(),
        (_, Some(m), _, _) => m.body.lines().next().unwrap_or("Message").chars().take(60).collect(),
        (_, _, Some(a), _) => a.text.clone(),
        (_, _, _, Some(a)) => format!("Appointment with {}", load_user(&st, a.doctor_id).await?.display_name),
        _ => primary_kind.label().to_string(),
    };
    let mut kinds: HashSet<String> = [text(&primary_kind)].into_iter().collect();
    if !report_files.is_empty() || !message_files.is_empty() { kinds.insert(part::ATTACHMENT.into()); }
    let now = now_ts();
    let item = Item {
        id: new_timeuuid(),
        owner_id,
        primary_kind,
        kinds,
        title: if b.title.trim().is_empty() { default_title } else { b.title.trim().chars().take(120).collect() },
        keywords: b.keywords.iter().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).take(20).collect(),
        grants,
        status: ItemStatus::Open,
        closure: None,
        has_report_files: !report_files.is_empty(),
        links: b.report.as_ref().map(|r| r.links.iter().filter(|l| l.starts_with("https://")).take(20).cloned().collect()).unwrap_or_default(),
        created_by: p.id(),
        created_by_name: p.user.display_name.clone(),
        created_at: now,
        updated_at: now,
    };
    st.db.insert_item(&item).await?;

    // Children of the primary part (kinds already recorded on the header).
    if b.report.is_some() {
        parts::store_attachments(&st, &p, &item, &report_files, true, None).await?;
    }
    if let Some(m) = b.message {
        let ids = parts::store_attachments(&st, &p, &item, &message_files, false, None).await?;
        parts::send_message(&st, &p, &item, &m.body, &m.client_msg_id, ids).await?;
    }
    if let Some(a) = b.appointment { parts::store_appointment(&st, &p, &item, a).await?; }
    if let Some(a) = b.alert { parts::store_alert(&st, &p, &item, a).await?; }

    let ev = Envelope::new("item.created", p.id(), item.id, item.audience(),
        json!({ "item_id": item.id, "owner_id": owner_id, "primary_kind": primary_kind, "created_by": p.id() }));
    st.events.emit(topics::ITEMS, &owner_id.to_string(), ev).await?;

    let fresh = load_item(&st, item.id).await?;
    let d = Decision { access: Access::Full, reason: "creator" };
    Ok(Json(item_dto(&st, &p, &fresh, &d).await?))
}

// ---------------- update / close / reopen ----------------

#[derive(Deserialize)]
pub struct Patch { title: Option<String>, keywords: Option<Vec<String>> }

pub async fn patch(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>, Json(b): Json<Patch>) -> ApiResult<Json<ItemDto>> {
    let item = load_item(&st, id).await?;
    if item.owner_id != p.id() { return Err(ApiError::forbidden("only the owner can edit the title or keywords")); }
    let title = b.title.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(|t| t.chars().take(120).collect::<String>());
    let kw: Option<HashSet<String>> = b.keywords.map(|v| v.into_iter().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).take(20).collect());
    st.db.set_title_keywords(&item, title.as_deref(), kw.as_ref()).await?;
    parts::notify_change(&st, &p, &item, "updated").await?;
    let fresh = load_item(&st, id).await?;
    let d = decide(&st, &p, &fresh).await?;
    Ok(Json(item_dto(&st, &p, &fresh, &d).await?))
}

#[derive(Deserialize)]
pub struct CloseBody { feedback: Option<String>, rating: Option<i8> }

/// Section 3.2: feedback from whoever closes; rating only from the patient (owner).
pub async fn close(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>, Json(b): Json<CloseBody>) -> ApiResult<Json<ItemDto>> {
    let (item, _) = load_full(&st, &p, id).await?;
    require_open(&item)?;
    if let Some(r) = b.rating {
        if item.owner_id != p.id() { return Err(ApiError::bad_request("only the patient can rate, when closing their own item")); }
        if !(1..=5).contains(&r) { return Err(ApiError::bad_request("rating must be 1–5")); }
    }
    let feedback = b.feedback.map(|f| f.trim().chars().take(1000).collect::<String>()).filter(|f| !f.is_empty());
    let closure = ClosureUdt { closed_at: Some(now_ts()), closed_by: Some(p.id()), feedback, rating: b.rating };
    st.db.set_status(&item, ItemStatus::Closed, Some(closure)).await?;
    parts::end_future_parts(&st, &item).await?; // rule 5: future visits cancelled, alerts stopped
    parts::notify_change(&st, &p, &item, "closed").await?;
    let fresh = load_item(&st, id).await?;
    let d = decide(&st, &p, &fresh).await?;
    Ok(Json(item_dto(&st, &p, &fresh, &d).await?))
}

pub async fn reopen(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<ItemDto>> {
    let (item, _) = load_full(&st, &p, id).await?;
    if item.status == ItemStatus::Open { return Err(ApiError::conflict("this item is already open")); }
    if !(item.owner_id == p.id() || p.has(Role::Doctor)) { return Err(ApiError::forbidden("only the owner or a treating doctor can reopen")); }
    st.db.set_status(&item, ItemStatus::Open, None).await?;
    parts::notify_change(&st, &p, &item, "reopened").await?;
    let fresh = load_item(&st, id).await?;
    let d = decide(&st, &p, &fresh).await?;
    Ok(Json(item_dto(&st, &p, &fresh, &d).await?))
}

// ---------------- uploads ----------------

#[derive(Deserialize)]
pub struct PresignReq { files: Vec<PresignFile> }
#[derive(Deserialize)]
pub struct PresignFile { name: String, mime: String, size: i64 }

pub fn upload_prefix(user: Uuid) -> String { format!("uploads/{user}/") }

/// One call for many files (doc 4.3). Returns an upload URL and the `obj:` URI to use in the item.
pub async fn presign(State(st): State<AppState>, p: Principal, Json(b): Json<PresignReq>) -> ApiResult<Json<serde_json::Value>> {
    if b.files.is_empty() || b.files.len() > MAX_ATTACHMENTS {
        return Err(ApiError::bad_request(format!("send 1–{MAX_ATTACHMENTS} files")));
    }
    let mut uploads = Vec::new();
    for f in &b.files {
        let max = if f.mime == "application/pdf" { PDF_MAX } else if f.mime.starts_with("image/") { IMAGE_MAX } else {
            return Err(ApiError::bad_request(format!("{}: only images and PDFs are accepted", f.name)));
        };
        if f.size <= 0 || f.size > max {
            return Err(ApiError::bad_request(format!("{} is larger than {} MB", f.name, max / 1024 / 1024)));
        }
        let clean: String = f.name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' }).take(80).collect();
        let key = format!("{}{}-{}", upload_prefix(p.id()), ulid::Ulid::new(), clean);
        uploads.push(json!({ "upload_url": st.files.presign_put(&key, &f.mime).await?, "uri": object_uri(&key) }));
    }
    Ok(Json(json!({ "uploads": uploads })))
}

/// Profile of the other person for list rows (used by conversations).
pub async fn other_user(st: &AppState, id: Uuid) -> ApiResult<Option<UserDto>> {
    match st.db.user(id).await? { Some(u) => Ok(Some(user_dto(st, &u, true).await?)), None => Ok(None) }
}
