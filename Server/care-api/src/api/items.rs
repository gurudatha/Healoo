//! /v1/items, /v1/items/{id}, /v1/items/{id}/attachments, /v1/uploads/presign,
//! /v1/dashboard, /v1/users/{id}/shared-items

use super::{principal::Principal, users::load_user, ApiError, ApiResult, AppState};
use crate::{
    events::{topics, Envelope},
    files::{key_of, object_uri, safe_key},
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
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub const MAX_ATTACHMENTS: usize = 20;
pub const IMAGE_MAX: i64 = 10 * 1024 * 1024;
pub const PDF_MAX: i64 = 25 * 1024 * 1024;

// ---------------- policy glue ----------------

/// Runs the policy engine for `p` on `item`, loading assistant context when needed, and logs
/// the decision to `access.audit` (doc 2.3).
pub async fn decide(st: &AppState, p: &Principal, item: &Item) -> ApiResult<Decision> {
    let assistant = if p.has(Role::Assistant) {
        match p.user.employer_doctor_id {
            Some(doc_id) => {
                let doctor = st.db.user(doc_id).await?;
                let doctor_subject = match doctor {
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

fn allowed_actions(p: &Principal, item: &Item, d: &Decision) -> Vec<String> {
    let v: &[&str] = match d.access {
        Access::Full if item.owner_id == p.id() => &["read", "share", "revoke", "status", "rate", "comment"],
        Access::Full if p.has(Role::Doctor) => &["read", "status", "comment"],
        Access::Full => &["read", "comment"],
        Access::MetadataOnly => &["meta"],
        Access::Deny => &[],
    };
    v.iter().map(|s| s.to_string()).collect()
}

/// Converts stored `obj:` URIs into short-lived URLs from the active object store.
async fn signed(st: &AppState, a: &AttachmentUdt) -> ApiResult<Attachment> {
    let mut out = Attachment::from(a.clone());
    if let Some(k) = key_of(&out.uri) { out.uri = st.files.presign_get(k).await?; }
    if let Some(t) = out.thumb_uri.clone() {
        if let Some(k) = key_of(&t) { out.thumb_uri = Some(st.files.presign_get(k).await?); }
    }
    Ok(out)
}

pub async fn item_dto(st: &AppState, p: &Principal, item: &Item, d: &Decision) -> ApiResult<ItemDto> {
    let full = d.access == Access::Full;
    let mut attachments = Vec::new();
    if full {
        let mut sorted = item.attachments.clone();
        sorted.sort_by_key(|a| a.position.unwrap_or(0));
        for a in &sorted { attachments.push(signed(st, a).await?); }
    }
    Ok(ItemDto {
        item_id: item.id,
        date: ts_to_date(item.date),
        core_item_type: item.item_type,
        title: item.title.clone(),
        subtitle: item.subtitle.clone(),
        keywords: item.keywords.iter().cloned().collect(),
        core_item_data: attachments,
        links: if full { item.links.clone() } else { vec![] },
        owner_id: item.owner_id,
        created_by_name: item.created_by_name.clone(),
        // Only the owner sees the full list of who else has access.
        access_list: if item.owner_id == p.id() { item.grant_list() } else { vec![] },
        pointer_to_message: if full { item.pointer_to_message } else { None },
        pointer_item_id: item.pointer_item_id,
        rating: item.rating,
        status: item.status,
        allowed_actions: allowed_actions(p, item, d),
    })
}

fn summary_dto(s: &ItemSummary, p: &Principal) -> ItemDto {
    ItemDto {
        item_id: s.id,
        date: ts_to_date(s.date),
        core_item_type: s.item_type,
        title: s.title.clone(),
        subtitle: s.subtitle.clone(),
        keywords: vec![],
        core_item_data: vec![],
        links: vec![],
        owner_id: s.owner_id,
        created_by_name: String::new(),
        access_list: vec![],
        pointer_to_message: None,
        pointer_item_id: None,
        rating: None,
        status: s.status,
        allowed_actions: if s.owner_id == p.id() { vec!["read".into(), "share".into(), "status".into()] } else { vec!["read".into()] },
    }
}

async fn load_item(st: &AppState, id: Uuid) -> ApiResult<Item> {
    st.db.item(id).await?.ok_or_else(|| ApiError::not_found("no such item"))
}

// ---------------- listing ----------------

/// Everything the caller may list, newest first: own items, direct grants, hospital grants
/// through active affiliations, and delegated patients for assistants (doc 7.1 / 7.3).
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
    all.sort_by_key(|s| std::cmp::Reverse(uuid_millis(&s.id)));
    all.truncate(limit as usize);
    Ok(all)
}

#[derive(Deserialize)]
pub struct ListQ { status: Option<String>, #[serde(rename = "type")] kind: Option<String>, limit: Option<i32> }

pub async fn list(State(st): State<AppState>, p: Principal, Query(q): Query<ListQ>) -> ApiResult<Json<Page<ItemDto>>> {
    let status = q.status.as_deref().and_then(parse).unwrap_or(ItemStatus::Open);
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let kind: Option<CoreItemType> = q.kind.as_deref().and_then(parse);
    let rows = visible(&st, &p, status, limit).await?;
    Ok(Json(Page::of(rows.iter().filter(|s| kind.map_or(true, |k| s.item_type == k)).map(|s| summary_dto(s, &p)).collect())))
}

pub async fn dashboard(State(st): State<AppState>, p: Principal) -> ApiResult<Json<DashboardDto>> {
    let open = visible(&st, &p, ItemStatus::Open, 200).await?;
    let today = chrono::Utc::now().timestamp_millis() - 86_400_000;
    let unread: i32 = st.db.threads_of(p.id()).await?.iter().map(|t| t.unread).sum();
    Ok(Json(DashboardDto {
        open_reports: open.iter().filter(|s| s.item_type == CoreItemType::Report).count() as i32,
        unread_messages: unread,
        upcoming_appointments: open.iter().filter(|s| s.item_type == CoreItemType::Booking && s.date.0 >= today).count() as i32,
    }))
}

pub async fn get_one(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<ItemDto>> {
    let item = load_item(&st, id).await?;
    let d = decide(&st, &p, &item).await?;
    if d.access == Access::Deny { return Err(ApiError::forbidden(d.reason)); }
    Ok(Json(item_dto(&st, &p, &item, &d).await?))
}

pub async fn attachments(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<Page<Attachment>>> {
    let item = load_item(&st, id).await?;
    let d = decide(&st, &p, &item).await?;
    if d.access != Access::Full { return Err(ApiError::forbidden(d.reason)); }
    let mut sorted = item.attachments.clone();
    sorted.sort_by_key(|a| a.position.unwrap_or(0));
    let mut out = Vec::new();
    for a in &sorted { out.push(signed(&st, a).await?); }
    Ok(Json(Page::of(out)))
}

/// Items shared between the caller and another user; empty unless connected (rule 8).
pub async fn shared_items(State(st): State<AppState>, p: Principal, Path(other_id): Path<Uuid>) -> ApiResult<Json<Page<ItemDto>>> {
    let other = load_user(&st, other_id).await?;
    if !st.db.is_connected(p.id(), other_id, other.primary_role()).await? {
        return Ok(Json(Page::of(vec![])));
    }
    let other_hospitals = if other.roles.contains(&Role::Doctor) { st.db.active_hospitals(other_id).await? } else { HashSet::new() };
    let mut out = Vec::new();

    // Mine, visible to them.
    for s in st.db.owner_partition(p.id(), ItemStatus::Open, 100).await? {
        if let Some(item) = st.db.item(s.id).await? {
            let visible_to_other = item.grant_list().iter().any(|g| match g.grantee_type {
                GranteeType::User => g.grantee_id == other_id,
                GranteeType::Hospital => other_hospitals.contains(&g.grantee_id),
            });
            if visible_to_other { out.push(summary_dto(&s, &p)); }
        }
    }
    // Theirs, visible to me.
    for s in st.db.owner_partition(other_id, ItemStatus::Open, 100).await? {
        if let Some(item) = st.db.item(s.id).await? {
            if policy::decide(&item.facts(), &p.subject, None).access != Access::Deny { out.push(summary_dto(&s, &p)); }
        }
    }
    out.sort_by_key(|i| std::cmp::Reverse(uuid_millis(&i.item_id)));
    Ok(Json(Page::of(out)))
}

// ---------------- create / update ----------------

#[derive(Deserialize)]
pub struct NewItem {
    owner_id: Option<Uuid>,
    core_item_type: CoreItemType,
    #[serde(default)]
    title: String,
    date: Option<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    core_item_data: Vec<Attachment>,
    #[serde(default)]
    links: Vec<String>,
    status: Option<ItemStatus>,
    #[serde(default)]
    share_with: Vec<Uuid>,
    pointer_to_message: Option<Uuid>,
    pointer_item_id: Option<Uuid>,
}

fn upload_prefix(user: Uuid) -> String { format!("uploads/{user}/") }

fn validate_attachments(p: &Principal, list: &[Attachment]) -> ApiResult<Vec<AttachmentUdt>> {
    if list.len() > MAX_ATTACHMENTS { return Err(ApiError::bad_request(format!("at most {MAX_ATTACHMENTS} attachments per item"))); }
    let prefix = upload_prefix(p.id());
    let mut out = Vec::new();
    for (i, a) in list.iter().enumerate() {
        let key = key_of(&a.uri).ok_or_else(|| ApiError::bad_request("attachment uri must come from /v1/uploads/presign"))?;
        if !key.starts_with(&prefix) || safe_key(key).is_err() {
            return Err(ApiError::forbidden("attachments must be files you uploaded"));
        }
        let (max, ok_mime) = match a.kind.as_str() {
            "IMAGE" => (IMAGE_MAX, a.mime.starts_with("image/")),
            "PDF" => (PDF_MAX, a.mime == "application/pdf"),
            _ => return Err(ApiError::bad_request("attachment kind must be IMAGE or PDF")),
        };
        if !ok_mime || a.size > max { return Err(ApiError::bad_request(format!("{} is not an allowed {} (type or size)", a.name, a.kind))); }
        let mut udt = AttachmentUdt::from(a);
        udt.position = Some(i as i32); // order as sent = order shown (doc 3.6)
        udt.thumb_uri = None;
        udt.page_count = None; // filled in by the media worker
        out.push(udt);
    }
    Ok(out)
}

async fn grant_udt(st: &AppState, grantee: Uuid, granted_by: Uuid, via_hospital: Option<Uuid>) -> ApiResult<GrantUdt> {
    let u = load_user(st, grantee).await?;
    let kind = if u.primary_role() == Role::Hospital { GranteeType::Hospital } else { GranteeType::User };
    Ok(GrantUdt {
        grant_id: Some(Uuid::new_v4()),
        grantee_type: Some(text(&kind)),
        grantee_id: Some(grantee),
        grantee_name: Some(u.display_name),
        via_hospital_id: via_hospital,
        granted_by: Some(granted_by),
        granted_at: Some(now_ts()),
    })
}

pub fn audience_of(item: &Item) -> Vec<String> {
    let mut a = vec![item.owner_id.to_string()];
    a.extend(item.grant_list().iter().map(|g| match g.grantee_type {
        GranteeType::User => g.grantee_id.to_string(),
        GranteeType::Hospital => format!("hospital:{}", g.grantee_id),
    }));
    a
}

pub async fn create(State(st): State<AppState>, p: Principal, Json(b): Json<NewItem>) -> ApiResult<Json<ItemDto>> {
    let owner_id = b.owner_id.unwrap_or(p.id());
    let for_someone_else = owner_id != p.id();
    let owner = load_user(&st, owner_id).await?;

    // Clinicians may create records that a connected patient owns (labs, doctors, assistants).
    if for_someone_else {
        if !p.user.roles.iter().any(|r| r.is_clinical()) {
            return Err(ApiError::forbidden("only doctors, assistants and labs can create items for someone else"));
        }
        if owner.primary_role() != Role::Patient || !st.db.is_connected(p.id(), owner_id, Role::Patient).await? {
            return Err(ApiError::forbidden("you can upload only for patients in your contacts"));
        }
    }
    if b.core_item_type == CoreItemType::Report && !for_someone_else && p.has(Role::Lab) {
        return Err(ApiError::bad_request("labs upload reports for a patient: set owner_id"));
    }

    let attachments = validate_attachments(&p, &b.core_item_data)?;
    let links: Vec<String> = b.links.into_iter().filter(|l| l.starts_with("https://")).take(20).collect();
    if attachments.is_empty() && links.is_empty() && b.core_item_type == CoreItemType::Report {
        return Err(ApiError::bad_request("a report needs at least one file or link"));
    }

    // Only the owner decides sharing; an uploader keeps access through its own grant (doc 2.3).
    let mut grants = Vec::new();
    if for_someone_else {
        grants.push(grant_udt(&st, p.id(), p.id(), None).await?);
    } else {
        for g in b.share_with.iter().collect::<HashSet<_>>() {
            if *g != p.id() { grants.push(grant_udt(&st, *g, p.id(), None).await?); }
        }
    }

    let n = attachments.len();
    let item = Item {
        id: new_timeuuid(),
        owner_id,
        created_by: p.id(),
        created_by_name: p.user.display_name.clone(),
        date: b.date.as_deref().and_then(date_to_ts).unwrap_or_else(now_ts),
        item_type: b.core_item_type,
        title: if b.title.trim().is_empty() { b.core_item_type.label() } else { b.title.trim().chars().take(120).collect() },
        subtitle: if for_someone_else { format!("{} · {n} file{}", p.user.display_name, if n == 1 { "" } else { "s" }) }
                  else if n > 0 { format!("{n} attachment{}", if n == 1 { "" } else { "s" }) }
                  else { String::new() },
        keywords: b.keywords.into_iter().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).take(20).collect(),
        attachments,
        links,
        grants,
        pointer_to_message: b.pointer_to_message,
        pointer_item_id: b.pointer_item_id,
        rating: None,
        status: b.status.unwrap_or(ItemStatus::Open),
    };
    st.db.insert_item(&item).await?;

    let ev = Envelope::new("item.created", p.id(), item.id, audience_of(&item),
        json!({ "item_id": item.id, "owner_id": owner_id, "core_item_type": item.item_type, "created_by": p.id() }));
    st.events.emit(topics::ITEMS, &owner_id.to_string(), ev).await?;

    let d = Decision { access: Access::Full, reason: "creator" };
    Ok(Json(item_dto(&st, &p, &item, &d).await?))
}

#[derive(Deserialize)]
pub struct Patch { status: Option<ItemStatus>, rating: Option<i8>, keywords: Option<Vec<String>> }

pub async fn patch(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>, Json(b): Json<Patch>) -> ApiResult<Json<ItemDto>> {
    let item = load_item(&st, id).await?;
    let d = decide(&st, &p, &item).await?;
    let owner = item.owner_id == p.id();
    let mut events = Vec::new();

    if let Some(s) = b.status {
        if !(owner || (d.access == Access::Full && p.has(Role::Doctor))) {
            return Err(ApiError::forbidden("only the owner or a treating doctor can change status"));
        }
        st.db.set_status(&item, s).await?;
        events.push("item.status_changed");
    }
    if b.rating.is_some() || b.keywords.is_some() {
        if !owner { return Err(ApiError::forbidden("only the owner can rate or edit keywords")); }
        if let Some(r) = b.rating { if !(1..=5).contains(&r) { return Err(ApiError::bad_request("rating must be 1–5")); } }
        let kw: Option<HashSet<String>> = b.keywords.map(|v| v.into_iter().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).collect());
        st.db.set_rating_keywords(id, b.rating, kw.as_ref()).await?;
        events.push(if b.rating.is_some() { "item.rated" } else { "item.updated" });
    }

    let updated = load_item(&st, id).await?;
    for e in events {
        let ev = Envelope::new(e, p.id(), id, audience_of(&updated), json!({ "item_id": id, "status": updated.status }));
        st.events.emit(topics::ITEMS, &updated.owner_id.to_string(), ev).await?;
    }
    let d = decide(&st, &p, &updated).await?;
    Ok(Json(item_dto(&st, &p, &updated, &d).await?))
}

// ---------------- uploads ----------------

#[derive(Deserialize)]
pub struct PresignReq { files: Vec<PresignFile> }
#[derive(Deserialize)]
pub struct PresignFile { name: String, mime: String, size: i64 }

/// One call for many files (doc 4.3). Returns an upload URL and the `obj:` URI to put in the item.
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

/// Group helper used by several handlers: user id → display name.
pub async fn names(st: &AppState, ids: &[Uuid]) -> ApiResult<HashMap<Uuid, String>> {
    let mut m = HashMap::new();
    for id in ids { if let Some(u) = st.db.user(*id).await? { m.insert(*id, u.display_name); } }
    Ok(m)
}
