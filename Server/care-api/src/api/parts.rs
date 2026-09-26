//! DataItem v2 child endpoints (Documentation/DataItem_Design.md sections 3 and 4):
//! /v1/items/{id}/messages, /attachments, /appointments (+ visits), /alerts,
//! /v1/appointments (calendar) and /v1/conversations (Messages tab).

use super::{
    items::{load_full, load_item, require_open, upload_prefix, IMAGE_MAX, MAX_ATTACHMENTS, PDF_MAX},
    principal::Principal,
    users::load_user,
    ApiError, ApiResult, AppState,
};
use crate::{
    events::{topics, Envelope},
    files::{key_of, safe_key},
    model::*,
    recurrence::{self, ExceptionAction, Recurrence, VisitException},
    store::{
        items::Item,
        parts::{exception_udts, iso_in, local_millis, today_in, AlertRow, AppointmentRow, AttachmentRow, DEFAULT_TZ},
    },
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{Duration as ChronoDuration, NaiveDate};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use uuid::Uuid;

pub const MAX_BODY_CHARS: usize = 4000;
const UPCOMING_VISITS: usize = 12;

/// Sends `item.updated {item_id, change}` to everyone with access (WebSocket fan-out).
pub async fn notify_change(st: &AppState, p: &Principal, item: &Item, change: &str) -> ApiResult<()> {
    let ev = Envelope::new(&format!("item.{change}"), p.id(), item.id, item.audience(), json!({ "item_id": item.id, "change": change }));
    st.events.emit(topics::ITEMS, &item.owner_id.to_string(), ev).await?;
    Ok(())
}

async fn add_kind_if_missing(st: &AppState, item: &Item, kind: &str) -> ApiResult<()> {
    if !item.kinds.contains(kind) { st.db.add_kind(item, kind).await?; }
    Ok(())
}

// =====================================================================================
// Attachments
// =====================================================================================

#[derive(Deserialize, Clone)]
pub struct NewAttachment {
    pub kind: String, // IMAGE | PDF
    pub uri: String,  // obj: key from /v1/uploads/presign
    pub mime: String,
    pub size: i64,
    #[serde(default)]
    pub name: String,
}

/// Checks limits and that every file was uploaded by the caller.
pub fn validate_attachments(p: &Principal, list: &[NewAttachment]) -> ApiResult<Vec<NewAttachment>> {
    if list.len() > MAX_ATTACHMENTS { return Err(ApiError::bad_request(format!("at most {MAX_ATTACHMENTS} attachments per request"))); }
    let prefix = upload_prefix(p.id());
    for a in list {
        let key = key_of(&a.uri).ok_or_else(|| ApiError::bad_request("attachment uri must come from /v1/uploads/presign"))?;
        if !key.starts_with(&prefix) || safe_key(key).is_err() { return Err(ApiError::forbidden("attachments must be files you uploaded")); }
        let (max, ok_mime) = match a.kind.as_str() {
            "IMAGE" => (IMAGE_MAX, a.mime.starts_with("image/")),
            "PDF" => (PDF_MAX, a.mime == "application/pdf"),
            _ => return Err(ApiError::bad_request("attachment kind must be IMAGE or PDF")),
        };
        if !ok_mime || a.size <= 0 || a.size > max {
            return Err(ApiError::bad_request(format!("{} is not an allowed {} (type or size)", a.name, a.kind)));
        }
    }
    Ok(list.to_vec())
}

/// Appends files to the item in the order given. Returns their attachment ids.
pub async fn store_attachments(st: &AppState, p: &Principal, item: &Item, files: &[NewAttachment], is_report: bool, message_id: Option<Uuid>) -> ApiResult<Vec<Uuid>> {
    if files.is_empty() { return Ok(vec![]); }
    let mut pos = st.db.next_attachment_position(item.id).await?;
    let mut ids = Vec::new();
    for f in files {
        let id = Uuid::new_v4();
        st.db.insert_attachment(item.id, &AttachmentRow {
            position: pos,
            attachment_id: id,
            kind: Some(f.kind.clone()),
            uri: Some(f.uri.clone()),
            mime: Some(f.mime.clone()),
            size: Some(f.size),
            name: Some(if f.name.is_empty() { format!("file-{}", pos + 1) } else { f.name.clone() }),
            page_count: None,
            thumb_uri: None,
            sha256: None,
            added_by: Some(p.id()),
            added_at: Some(now_ts()),
            message_id: message_id.map(tu),
            is_report: Some(is_report),
        }).await?;
        ids.push(id);
        pos += 1;
    }
    add_kind_if_missing(st, item, part::ATTACHMENT).await?;
    if is_report {
        add_kind_if_missing(st, item, part::REPORT).await?;
        if !item.has_report_files { st.db.set_has_report_files(item.id).await?; }
    }
    // The media worker makes thumbnails and page counts for new files.
    let ev = Envelope::new("item.attachments_added", p.id(), item.id, item.audience(), json!({ "item_id": item.id, "attachment_ids": ids }));
    st.events.emit(topics::ITEMS, &item.owner_id.to_string(), ev).await?;
    Ok(ids)
}

pub async fn attachment_dto(st: &AppState, a: &AttachmentRow) -> ApiResult<AttachmentDto> {
    let sign = |u: &Option<String>| u.clone();
    let mut uri = sign(&a.uri).unwrap_or_default();
    if let Some(k) = key_of(&uri).map(str::to_string) { uri = st.files.presign_get(&k).await?; }
    let thumb = match a.thumb_uri.as_deref().and_then(key_of) {
        Some(k) => Some(st.files.presign_get(k).await?),
        None => a.thumb_uri.clone(),
    };
    Ok(AttachmentDto {
        attachment_id: a.attachment_id,
        kind: a.kind.clone().unwrap_or_else(|| "IMAGE".into()),
        uri,
        mime: a.mime.clone().unwrap_or_default(),
        size: a.size.unwrap_or(0),
        position: a.position,
        name: a.name.clone().unwrap_or_default(),
        page_count: a.page_count,
        thumb_uri: thumb,
        sha256: a.sha256.clone(),
        added_by: a.added_by.unwrap_or_default(),
        added_at: a.added_at.map(|t| ts_to_dt(t).to_rfc3339()).unwrap_or_default(),
        message_id: a.message_id.map(un),
        is_report: a.is_report.unwrap_or(false),
    })
}

pub async fn attachments_get(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<Page<AttachmentDto>>> {
    let (item, _) = load_full(&st, &p, id).await?;
    let mut out = Vec::new();
    for a in st.db.attachments(item.id).await? { out.push(attachment_dto(&st, &a).await?); }
    Ok(Json(Page::of(out)))
}

#[derive(Deserialize)]
pub struct AddAttachments {
    attachments: Vec<NewAttachment>,
    #[serde(default)]
    is_report: bool,
}

pub async fn attachments_post(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>, Json(b): Json<AddAttachments>) -> ApiResult<Json<Page<AttachmentDto>>> {
    let (item, _) = load_full(&st, &p, id).await?;
    require_open(&item)?;
    if b.attachments.is_empty() { return Err(ApiError::bad_request("send at least one attachment")); }
    let files = validate_attachments(&p, &b.attachments)?;
    let ids = store_attachments(&st, &p, &item, &files, b.is_report, None).await?;
    notify_change(&st, &p, &item, "attachment").await?;
    let mut out = Vec::new();
    for a in st.db.attachments(item.id).await?.iter().filter(|a| ids.contains(&a.attachment_id)) { out.push(attachment_dto(&st, a).await?); }
    Ok(Json(Page::of(out)))
}

// =====================================================================================
// Messages (D5: every message belongs to an item)
// =====================================================================================

/// Shared by POST /messages, item creation and the WebSocket `message.send` frame.
/// Idempotent on `client_msg_id`.
pub async fn send_message(st: &AppState, p: &Principal, item: &Item, body: &str, client_msg_id: &str, attachment_ids: Vec<Uuid>) -> ApiResult<MessageDto> {
    let body = body.trim();
    if body.is_empty() || body.chars().count() > MAX_BODY_CHARS {
        return Err(ApiError::bad_request(format!("message must be 1–{MAX_BODY_CHARS} characters")));
    }
    if client_msg_id.is_empty() || client_msg_id.len() > 64 {
        return Err(ApiError::bad_request("client_msg_id is required (max 64 chars)"));
    }
    require_open(item)?;
    if let Some(existing) = st.db.existing_item_message(item.id, client_msg_id).await? {
        if let Some(m) = st.db.item_message(item.id, existing).await? { return Ok(m); }
    }
    let id = new_timeuuid();
    let m = MessageDto {
        message_id: id,
        item_id: item.id,
        sender_id: p.id(),
        body: body.to_string(),
        sent_at: ts_to_dt(scylla::frame::value::CqlTimestamp(uuid_millis(&id))).to_rfc3339(),
        attachment_ids,
    };
    st.db.insert_item_message(&m, client_msg_id).await?;
    add_kind_if_missing(st, item, part::MESSAGE).await?;

    // Messages tab rows for every participant pair involving the sender.
    let preview: String = body.chars().take(120).collect();
    for other in item.participants().into_iter().filter(|u| *u != p.id()) {
        let unread = st.db.conversation_unread(other, p.id(), item.id).await? + 1;
        st.db.touch_conversation(other, p.id(), item.id, &preview, unread).await?;
        st.db.touch_conversation(p.id(), other, item.id, &preview, 0).await?;
    }
    let ev = Envelope::new("message.created", p.id(), item.id, item.audience(), json!({ "item_id": item.id, "message": m }));
    st.events.emit(topics::CHAT, &item.id.to_string(), ev).await?;
    Ok(m)
}

/// WebSocket helper: loads the item and checks full access first.
pub async fn send_message_to(st: &AppState, p: &Principal, item_id: Uuid, body: &str, client_msg_id: &str) -> ApiResult<MessageDto> {
    let (item, _) = load_full(st, p, item_id).await?;
    send_message(st, p, &item, body, client_msg_id, vec![]).await
}

#[derive(Deserialize)]
pub struct MessagesQ { limit: Option<i32> }

pub async fn messages_get(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>, Query(q): Query<MessagesQ>) -> ApiResult<Json<Page<MessageDto>>> {
    let (item, _) = load_full(&st, &p, id).await?;
    let list = st.db.item_messages(item.id, q.limit.unwrap_or(100).clamp(1, 500)).await?;
    st.db.mark_item_read(p.id(), item.id).await?;
    Ok(Json(Page::of(list)))
}

#[derive(Deserialize)]
pub struct SendBody {
    body: String,
    client_msg_id: String,
    #[serde(default)]
    attachments: Vec<NewAttachment>,
}

pub async fn messages_post(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>, Json(b): Json<SendBody>) -> ApiResult<Json<MessageDto>> {
    let (item, _) = load_full(&st, &p, id).await?;
    require_open(&item)?;
    let files = validate_attachments(&p, &b.attachments)?;
    // The message id is not known yet; files posted with a message are linked by attachment_ids.
    let ids = store_attachments(&st, &p, &item, &files, false, None).await?;
    let fresh = load_item(&st, id).await?;
    Ok(Json(send_message(&st, &p, &fresh, &b.body, &b.client_msg_id, ids).await?))
}

// =====================================================================================
// Appointments
// =====================================================================================

#[derive(Deserialize, Clone)]
pub struct NewAppointment {
    pub patient_id: Uuid,
    pub doctor_id: Uuid,
    pub date: String,
    pub time: String,
    pub timezone: Option<String>,
    pub duration_min: Option<i32>,
    pub hospital_id: Option<Uuid>,
    pub notes: Option<String>,
    pub recurrence: Option<Recurrence>,
}

fn parse_dt(a: &NewAppointment) -> ApiResult<(NaiveDate, chrono::NaiveTime)> {
    let d = recurrence::parse_date(&a.date).ok_or_else(|| ApiError::bad_request("date must be YYYY-MM-DD"))?;
    let t = recurrence::parse_time(&a.time).ok_or_else(|| ApiError::bad_request("time must be HH:MM (24-hour)"))?;
    Ok((d, t))
}

/// Section 3.6: the user is the item's owner, the doctor is a doctor, the rule is valid.
pub async fn validate_appointment(st: &AppState, owner_id: Uuid, a: &NewAppointment) -> ApiResult<()> {
    if a.patient_id != owner_id { return Err(ApiError::bad_request("patient_id must be the item's owner")); }
    let doctor = load_user(st, a.doctor_id).await?;
    if !doctor.roles.contains(&Role::Doctor) { return Err(ApiError::bad_request("doctor_id must be a doctor")); }
    let (d, _) = parse_dt(a)?;
    let tz = a.timezone.as_deref().unwrap_or(DEFAULT_TZ);
    if tz.parse::<chrono_tz::Tz>().is_err() { return Err(ApiError::bad_request("timezone must be an IANA name such as Asia/Kolkata")); }
    if d < today_in(tz) { return Err(ApiError::bad_request("the first visit can't be in the past")); }
    if let Some(m) = a.duration_min { if !(5..=240).contains(&m) { return Err(ApiError::bad_request("duration_min must be 5–240")); } }
    if let Some(h) = a.hospital_id {
        if !st.db.active_hospitals(a.doctor_id).await?.contains(&h) { return Err(ApiError::bad_request("the doctor doesn't work at that hospital")); }
    }
    recurrence::validate(d, a.recurrence.as_ref()).map_err(ApiError::bad_request)
}

/// Who may book on an item: the owner, the appointment's doctor, or that doctor's assistant.
fn may_book(p: &Principal, item: &Item, a: &NewAppointment) -> bool {
    p.id() == item.owner_id || p.id() == a.doctor_id || (p.has(Role::Assistant) && p.user.employer_doctor_id == Some(a.doctor_id))
}

pub async fn store_appointment(st: &AppState, p: &Principal, item: &Item, a: NewAppointment) -> ApiResult<AppointmentDto> {
    if !may_book(p, item, &a) { return Err(ApiError::forbidden("only the patient, the doctor or the doctor's assistant can book")); }
    let tz = a.timezone.clone().unwrap_or_else(|| DEFAULT_TZ.to_string());
    let row = AppointmentRow {
        appointment_id: Uuid::new_v4(),
        patient_id: a.patient_id,
        doctor_id: a.doctor_id,
        hospital_id: a.hospital_id,
        start_date: a.date.clone(),
        start_time: a.time.clone(),
        timezone: Some(tz),
        duration_min: Some(a.duration_min.unwrap_or(15)),
        notes: a.notes.clone().map(|n| n.chars().take(500).collect()),
        frequency: a.recurrence.map(|r| text(&r.frequency)),
        period: a.recurrence.and_then(|r| r.period).map(|p| text(&p)),
        exceptions: None,
        status: Some("SCHEDULED".into()),
        visit_status: None,
    };
    st.db.save_appointment(item.id, None, &row).await?;

    // The doctor needs access to their own appointment (section 3.8).
    if a.doctor_id != item.owner_id && !item.grants.iter().any(|g| g.grantee_id == Some(a.doctor_id)) {
        let doctor = load_user(st, a.doctor_id).await?;
        st.db.add_grant(item, GrantUdt {
            grant_id: Some(Uuid::new_v4()),
            grantee_type: Some(text(&GranteeType::User)),
            grantee_id: Some(a.doctor_id),
            grantee_name: Some(doctor.display_name),
            via_hospital_id: None,
            granted_by: Some(p.id()),
            granted_at: Some(now_ts()),
        }).await?;
    }
    add_kind_if_missing(st, item, part::APPOINTMENT).await?;
    create_reminders(st, item, &row).await?;
    appointment_dto(st, item.id, &row, false).await
}

/// Reminders for every visit: patient 1 day and 1 hour before, doctor 1 hour before (3.7).
/// They follow the series rule; the scheduler skips firings whose visit was cancelled or moved.
async fn create_reminders(st: &AppState, item: &Item, a: &AppointmentRow) -> ApiResult<()> {
    let first = local_millis(a.date(), a.time(), a.tz());
    let doctor_name = load_user(st, a.doctor_id).await?.display_name;
    for (user, offset_min, text_) in [
        (a.patient_id, 24 * 60, format!("Tomorrow at {}: appointment with {}", a.start_time, doctor_name)),
        (a.patient_id, 60, format!("In 1 hour: appointment with {}", doctor_name)),
        (a.doctor_id, 60, "In 1 hour: patient appointment".to_string()),
    ] {
        st.db.insert_alert(item.id, &AlertRow {
            alert_id: Uuid::new_v4(),
            kind: Some("APPOINTMENT_REMINDER".into()),
            text: Some(text_),
            for_user: user,
            fires_at: scylla::frame::value::CqlTimestamp(first - offset_min * 60_000),
            timezone: a.timezone.clone(),
            frequency: a.frequency.clone(),
            period: a.period.clone(),
            appointment_id: Some(a.appointment_id),
            active: Some(true),
        }).await?;
    }
    Ok(())
}

pub async fn appointment_dto(st: &AppState, item_id: Uuid, a: &AppointmentRow, metadata_only: bool) -> ApiResult<AppointmentDto> {
    let today = today_in(a.tz());
    let visits = a.visits(today, today + ChronoDuration::days(3660), UPCOMING_VISITS).into_iter().map(|(v, s)| VisitDto {
        date: recurrence::fmt_date(v.date),
        time: recurrence::fmt_time(v.time),
        original_date: recurrence::fmt_date(v.original_date),
        status: s,
    }).collect();
    let rec = a.recurrence();
    Ok(AppointmentDto {
        appointment_id: a.appointment_id,
        item_id,
        patient_id: a.patient_id,
        doctor_id: a.doctor_id,
        doctor_name: st.db.user(a.doctor_id).await?.map(|u| u.display_name).unwrap_or_default(),
        hospital_id: a.hospital_id,
        date: a.start_date.clone(),
        time: a.start_time.clone(),
        timezone: a.tz().to_string(),
        duration_min: a.duration_min.unwrap_or(15),
        notes: if metadata_only { None } else { a.notes.clone() },
        recurrence: rec,
        exceptions: if metadata_only { vec![] } else { a.exceptions() },
        status: a.status.clone().unwrap_or_else(|| "SCHEDULED".into()),
        visit_count: rec.and_then(|r| recurrence::visit_count(a.date(), &r)).or(if rec.is_none() { Some(1) } else { None }),
        visits,
    })
}

pub async fn appointments_get(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<Page<AppointmentDto>>> {
    let item = load_item(&st, id).await?;
    let d = super::items::decide(&st, &p, &item).await?;
    if d.access == crate::policy::Access::Deny { return Err(ApiError::forbidden(d.reason)); }
    let mut out = Vec::new();
    for a in st.db.appointments(item.id).await? {
        out.push(appointment_dto(&st, item.id, &a, d.access != crate::policy::Access::Full).await?);
    }
    Ok(Json(Page::of(out)))
}

pub async fn appointments_post(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>, Json(b): Json<NewAppointment>) -> ApiResult<Json<AppointmentDto>> {
    let (item, _) = load_full(&st, &p, id).await?;
    require_open(&item)?;
    validate_appointment(&st, item.owner_id, &b).await?;
    let dto = store_appointment(&st, &p, &item, b).await?;
    notify_change(&st, &p, &item, "appointment").await?;
    Ok(Json(dto))
}

#[derive(Deserialize)]
pub struct AppointmentPatch {
    date: Option<String>,
    time: Option<String>,
    duration_min: Option<i32>,
    notes: Option<String>,
    /// `{"recurrence": null}` turns a series into a single appointment.
    #[serde(default, deserialize_with = "some_or_null")]
    recurrence: Option<Option<Recurrence>>,
    status: Option<String>,
}

fn some_or_null<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<Recurrence>>, D::Error> {
    Ok(Some(Option::deserialize(d)?))
}

fn may_manage(p: &Principal, item: &Item, a: &AppointmentRow) -> bool {
    p.id() == item.owner_id || p.id() == a.doctor_id || (p.has(Role::Assistant) && p.user.employer_doctor_id == Some(a.doctor_id))
}

/// Changes the series (applies to the rule as a whole; past visit marks are kept) or cancels it.
pub async fn appointment_patch(State(st): State<AppState>, p: Principal, Path((id, aid)): Path<(Uuid, Uuid)>, Json(b): Json<AppointmentPatch>) -> ApiResult<Json<AppointmentDto>> {
    let (item, _) = load_full(&st, &p, id).await?;
    require_open(&item)?;
    let old = st.db.appointment(item.id, aid).await?.ok_or_else(|| ApiError::not_found("no such appointment"))?;
    if !may_manage(&p, &item, &old) { return Err(ApiError::forbidden("only the patient, the doctor or the doctor's assistant can change this")); }
    let mut new = old.clone();
    if let Some(s) = b.status.as_deref() {
        if s != "CANCELLED" { return Err(ApiError::bad_request("status can only be set to CANCELLED")); }
        new.status = Some("CANCELLED".into());
    }
    if let Some(d) = b.date { new.start_date = d; }
    if let Some(t) = b.time { new.start_time = t; }
    if let Some(m) = b.duration_min { new.duration_min = Some(m); }
    if let Some(n) = b.notes { new.notes = Some(n.chars().take(500).collect()); }
    if let Some(r) = b.recurrence {
        new.frequency = r.map(|r| text(&r.frequency));
        new.period = r.and_then(|r| r.period).map(|p| text(&p));
    }
    if new.status.as_deref() != Some("CANCELLED") {
        let check = NewAppointment {
            patient_id: new.patient_id, doctor_id: new.doctor_id, date: new.start_date.clone(), time: new.start_time.clone(),
            timezone: new.timezone.clone(), duration_min: new.duration_min, hospital_id: new.hospital_id, notes: None,
            recurrence: new.recurrence(),
        };
        let (d, _) = parse_dt(&check)?;
        recurrence::validate(d, check.recurrence.as_ref()).map_err(ApiError::bad_request)?;
    }
    st.db.save_appointment(item.id, Some(&old), &new).await?;
    // Reminders follow the new rule: stop the old ones, create new ones unless cancelled.
    for al in st.db.alerts(item.id).await?.iter().filter(|al| al.appointment_id == Some(aid)) {
        st.db.set_alert_active(item.id, al.alert_id, false).await?;
    }
    if new.status.as_deref() != Some("CANCELLED") { create_reminders(&st, &item, &new).await?; }
    notify_change(&st, &p, &item, "appointment").await?;
    Ok(Json(appointment_dto(&st, item.id, &new, false).await?))
}

#[derive(Deserialize)]
pub struct VisitAction { action: String, new_date: Option<String>, new_time: Option<String> }

/// One visit: CANCELLED / MOVED (patient, doctor, assistant) or COMPLETED / NO_SHOW (doctor side).
pub async fn visit_action(State(st): State<AppState>, p: Principal, Path((id, aid, date)): Path<(Uuid, Uuid, String)>, Json(b): Json<VisitAction>) -> ApiResult<Json<AppointmentDto>> {
    let (item, _) = load_full(&st, &p, id).await?;
    let old = st.db.appointment(item.id, aid).await?.ok_or_else(|| ApiError::not_found("no such appointment"))?;
    if !may_manage(&p, &item, &old) { return Err(ApiError::forbidden("you can't change this visit")); }
    let visit_date = recurrence::parse_date(&date).ok_or_else(|| ApiError::bad_request("visit date must be YYYY-MM-DD"))?;
    let is_visit = old.visits(visit_date, visit_date, 5).iter().any(|(v, _)| v.original_date == visit_date)
        || recurrence::visits(old.date(), old.time(), old.recurrence().as_ref(), &[], visit_date, visit_date, 1).iter().any(|v| v.original_date == visit_date);
    if !is_visit { return Err(ApiError::not_found("there is no visit on that date")); }
    let mut new = old.clone();
    match b.action.as_str() {
        "CANCELLED" | "MOVED" => {
            require_open(&item)?;
            let mut ex: Vec<VisitException> = old.exceptions().into_iter().filter(|e| e.date != visit_date).collect();
            if b.action == "CANCELLED" {
                ex.push(VisitException { date: visit_date, action: ExceptionAction::Cancelled, new_date: None, new_time: None });
            } else {
                let nd = b.new_date.as_deref().map(|s| recurrence::parse_date(s).ok_or_else(|| ApiError::bad_request("new_date must be YYYY-MM-DD"))).transpose()?;
                let nt = b.new_time.as_deref().map(|s| recurrence::parse_time(s).ok_or_else(|| ApiError::bad_request("new_time must be HH:MM"))).transpose()?;
                if nd.is_none() && nt.is_none() { return Err(ApiError::bad_request("MOVED needs new_date and/or new_time")); }
                ex.push(VisitException { date: visit_date, action: ExceptionAction::Moved, new_date: nd, new_time: nt });
            }
            new.exceptions = Some(exception_udts(&ex));
        }
        "COMPLETED" | "NO_SHOW" => {
            if !(p.id() == old.doctor_id || (p.has(Role::Assistant) && p.user.employer_doctor_id == Some(old.doctor_id))) {
                return Err(ApiError::forbidden("only the doctor or their assistant can mark a visit completed or missed"));
            }
            let mut marks: HashMap<String, String> = old.visit_status.clone().unwrap_or_default();
            marks.insert(recurrence::fmt_date(visit_date), b.action.clone());
            new.visit_status = Some(marks);
        }
        _ => return Err(ApiError::bad_request("action must be CANCELLED, MOVED, COMPLETED or NO_SHOW")),
    }
    st.db.save_appointment(item.id, Some(&old), &new).await?;
    notify_change(&st, &p, &item, "appointment").await?;
    Ok(Json(appointment_dto(&st, item.id, &new, false).await?))
}

/// Closing an item cancels future visits and stops its alerts (3.7 rule 5).
pub async fn end_future_parts(st: &AppState, item: &Item) -> ApiResult<()> {
    for a in st.db.appointments(item.id).await? {
        if a.is_cancelled() { continue; }
        let mut new = a.clone();
        new.status = Some("CANCELLED".into());
        st.db.save_appointment(item.id, Some(&a), &new).await?;
    }
    for al in st.db.alerts(item.id).await? {
        if al.active.unwrap_or(true) { st.db.set_alert_active(item.id, al.alert_id, false).await?; }
    }
    Ok(())
}

// =====================================================================================
// Alerts
// =====================================================================================

#[derive(Deserialize, Clone)]
pub struct NewAlert {
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
    pub for_user: Option<Uuid>,
    pub date: String,
    pub time: String,
    pub timezone: Option<String>,
    pub recurrence: Option<Recurrence>,
}

const ALERT_TYPES: [&str; 5] = ["MEDICATION", "APPOINTMENT_REMINDER", "FOLLOW_UP", "RESULT_READY", "CUSTOM"];

pub fn validate_alert(a: &NewAlert) -> ApiResult<()> {
    if !ALERT_TYPES.contains(&a.kind.as_str()) { return Err(ApiError::bad_request("type must be MEDICATION, APPOINTMENT_REMINDER, FOLLOW_UP, RESULT_READY or CUSTOM")); }
    let t = a.text.trim();
    if t.is_empty() || t.chars().count() > 200 { return Err(ApiError::bad_request("text must be 1–200 characters")); }
    let d = recurrence::parse_date(&a.date).ok_or_else(|| ApiError::bad_request("date must be YYYY-MM-DD"))?;
    recurrence::parse_time(&a.time).ok_or_else(|| ApiError::bad_request("time must be HH:MM"))?;
    if a.timezone.as_deref().unwrap_or(DEFAULT_TZ).parse::<chrono_tz::Tz>().is_err() { return Err(ApiError::bad_request("timezone must be an IANA name")); }
    recurrence::validate(d, a.recurrence.as_ref()).map_err(ApiError::bad_request)
}

pub async fn store_alert(st: &AppState, p: &Principal, item: &Item, a: NewAlert) -> ApiResult<AlertDto> {
    let for_user = a.for_user.unwrap_or(p.id());
    if !item.participants().contains(&for_user) { return Err(ApiError::bad_request("for_user must have access to this item")); }
    let tz = a.timezone.clone().unwrap_or_else(|| DEFAULT_TZ.to_string());
    let (d, t) = (recurrence::parse_date(&a.date).unwrap_or_default(), recurrence::parse_time(&a.time).unwrap_or_default());
    let row = AlertRow {
        alert_id: Uuid::new_v4(),
        kind: Some(a.kind.clone()),
        text: Some(a.text.trim().to_string()),
        for_user,
        fires_at: scylla::frame::value::CqlTimestamp(local_millis(d, t, &tz)),
        timezone: Some(tz),
        frequency: a.recurrence.map(|r| text(&r.frequency)),
        period: a.recurrence.and_then(|r| r.period).map(|p| text(&p)),
        appointment_id: None,
        active: Some(true),
    };
    st.db.insert_alert(item.id, &row).await?;
    add_kind_if_missing(st, item, part::ALERT).await?;
    Ok(alert_dto(item.id, &row))
}

pub fn alert_dto(item_id: Uuid, a: &AlertRow) -> AlertDto {
    AlertDto {
        alert_id: a.alert_id,
        item_id,
        kind: a.kind.clone().unwrap_or_else(|| "CUSTOM".into()),
        text: a.text.clone().unwrap_or_default(),
        for_user: a.for_user,
        fires_at: iso_in(a.fires_at.0, a.tz()),
        timezone: a.tz().to_string(),
        recurrence: a.recurrence(),
        appointment_id: a.appointment_id,
        active: a.active.unwrap_or(true),
    }
}

pub async fn alerts_get(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<Page<AlertDto>>> {
    let (item, _) = load_full(&st, &p, id).await?;
    let owner = item.owner_id == p.id();
    Ok(Json(Page::of(st.db.alerts(item.id).await?.iter().filter(|a| owner || a.for_user == p.id()).map(|a| alert_dto(item.id, a)).collect())))
}

pub async fn alerts_post(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>, Json(b): Json<NewAlert>) -> ApiResult<Json<AlertDto>> {
    let (item, _) = load_full(&st, &p, id).await?;
    require_open(&item)?;
    validate_alert(&b)?;
    let dto = store_alert(&st, &p, &item, b).await?;
    notify_change(&st, &p, &item, "alert").await?;
    Ok(Json(dto))
}

pub async fn alerts_delete(State(st): State<AppState>, p: Principal, Path((id, alert_id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    let (item, _) = load_full(&st, &p, id).await?;
    let a = st.db.alert(item.id, alert_id).await?.ok_or_else(|| ApiError::not_found("no such alert"))?;
    if !(item.owner_id == p.id() || a.for_user == p.id()) { return Err(ApiError::forbidden("only the owner or the alert's recipient can remove it")); }
    st.db.delete_alert(item.id, alert_id).await?;
    notify_change(&st, &p, &item, "alert").await?;
    Ok(StatusCode::NO_CONTENT)
}

// =====================================================================================
// Calendar and conversations
// =====================================================================================

#[derive(Deserialize)]
pub struct CalendarQ { from: Option<String>, to: Option<String> }

/// GET /v1/appointments?from=&to= : visits across items (doctor: theirs; others: own), max 62 days.
pub async fn calendar(State(st): State<AppState>, p: Principal, Query(q): Query<CalendarQ>) -> ApiResult<Json<Value>> {
    let today = today_in(DEFAULT_TZ);
    let from = q.from.as_deref().map(|s| recurrence::parse_date(s).ok_or_else(|| ApiError::bad_request("from must be YYYY-MM-DD"))).transpose()?.unwrap_or(today);
    let to = q.to.as_deref().map(|s| recurrence::parse_date(s).ok_or_else(|| ApiError::bad_request("to must be YYYY-MM-DD"))).transpose()?.unwrap_or(from + ChronoDuration::days(30));
    if to < from || (to - from).num_days() > 62 { return Err(ApiError::bad_request("choose a range of up to 62 days")); }
    let doctor = p.has(Role::Doctor);
    let mut rows = if doctor { st.db.doctor_visits(p.id(), from, to).await? } else { st.db.patient_visits(p.id(), from, to).await? };
    rows.sort_by_key(|r| r.0);
    let mut out = Vec::new();
    for (ms, appointment_id, item_id, other, status) in rows {
        let title = st.db.item(item_id).await?.map(|i| i.title).unwrap_or_default();
        let other_name = st.db.user(other).await?.map(|u| u.display_name).unwrap_or_default();
        out.push(json!({
            "starts_at": iso_in(ms, DEFAULT_TZ), "appointment_id": appointment_id, "item_id": item_id,
            "item_title": title, "with_user_id": other, "with_user_name": other_name, "status": status,
        }));
    }
    Ok(Json(json!({ "data": out, "page_state": null })))
}

#[derive(Deserialize)]
pub struct ConversationsQ { with: Option<Uuid> }

/// GET /v1/conversations[?with=user] : items with a discussion, grouped by the other person.
pub async fn conversations(State(st): State<AppState>, p: Principal, Query(q): Query<ConversationsQ>) -> ApiResult<Json<Page<ConversationDto>>> {
    let mut out = Vec::new();
    for c in st.db.conversations_of(p.id()).await?.into_iter().filter(|c| q.with.map_or(true, |w| c.other_user_id == w)) {
        let Some(item) = st.db.item(c.item_id).await? else { continue };
        let Some(other) = super::items::other_user(&st, c.other_user_id).await? else { continue };
        out.push(ConversationDto {
            item_id: c.item_id,
            item_title: item.title,
            primary_kind: item.primary_kind,
            other_user: other,
            last_message: c.last_message,
            last_message_at: c.last_message_at.map(|t| ts_to_dt(t).to_rfc3339()).unwrap_or_default(),
            unread: c.unread,
        });
    }
    Ok(Json(Page::of(out)))
}
