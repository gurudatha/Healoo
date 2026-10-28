//! /v1/vitals/* (Documentation/Vitals_Design.md section 5): a person records readings; their
//! connected doctors see them consolidated as red, yellow and green alerts, set the person's
//! ranges and add vitals for them.

use super::{principal::Principal, users::load_user, ApiError, ApiResult, AppState};
use crate::{
    events::{topics, Envelope},
    model::*,
    store::vitals::{ReadingRow, VitalAlertRow},
    vitals::{self, Component, RangeSpec, Severity, VitalType, SCHEMA_VERSION},
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;


#[derive(Serialize)]
pub struct ReadingDto {
    reading_id: Uuid,
    user_id: Uuid,
    code: String,
    values: HashMap<String, f64>,
    taken_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    entered_by: Uuid,
    severity: Severity,
    schema_version: i32,
}

fn reading_dto(r: ReadingRow) -> ReadingDto {
    ReadingDto {
        reading_id: r.reading_id, user_id: r.user_id, code: r.code, values: r.vals,
        taken_at: ts_to_dt(r.taken_at).to_rfc3339(), note: r.note, entered_by: r.entered_by,
        severity: Severity::parse(&r.severity), schema_version: SCHEMA_VERSION,
    }
}

#[derive(Serialize)]
pub struct VitalAlertDto {
    alert_id: Uuid,
    patient_id: Uuid,
    patient_name: String,
    code: String,
    label: String,
    icon: String,
    unit: String,
    values: HashMap<String, f64>,
    severity: Severity,
    taken_at: String,
    schema_version: i32,
}

#[derive(Serialize, Default)]
pub struct AlertCounts { red: usize, yellow: usize, green: usize }

#[derive(Serialize)]
pub struct AlertPage { counts: AlertCounts, data: Vec<VitalAlertDto> }

#[derive(Deserialize)]
pub struct UserQ { user: Option<Uuid> }

/// Whose vitals the call is about, and whether the caller sees them as that person's doctor.
async fn subject(st: &AppState, p: &Principal, user: Option<Uuid>) -> ApiResult<(Uuid, bool)> {
    let owner = user.unwrap_or(p.id());
    if owner == p.id() { return Ok((owner, false)); }
    let other = load_user(st, owner).await?;
    let connected = st.db.is_connected(p.id(), owner, other.primary_role()).await?;
    if !vitals::may_read(owner, p.id(), &p.user.roles, connected) {
        return Err(ApiError::forbidden("vitals are visible to the person and to doctors in their contacts"));
    }
    Ok((owner, true))
}

/// Built-in vitals plus the ones a doctor added for this person, each with the range that
/// applies to them (the doctor's, or else the default).
async fn types_for(st: &AppState, owner: Uuid) -> ApiResult<Vec<VitalType>> {
    let mut all = vitals::builtin();
    all.extend(st.db.custom_vital_types(owner).await?);
    let ranges = st.db.vital_ranges(owner).await?;
    for t in &mut all {
        if let Some((spec, margin)) = ranges.get(&t.code) { t.apply_range(spec, *margin); }
    }
    all.sort_by_key(|t| (t.position, t.label.clone()));
    Ok(all)
}

/// `GET /v1/vitals/types?user=` — ranges are included only for the person's doctor.
pub async fn types(State(st): State<AppState>, p: Principal, Query(q): Query<UserQ>) -> ApiResult<Json<Page<VitalType>>> {
    let (owner, as_doctor) = subject(&st, &p, q.user).await?;
    let all = types_for(&st, owner).await?;
    Ok(Json(Page::of(if as_doctor { all } else { all.into_iter().map(VitalType::without_ranges).collect() })))
}

/// `GET /v1/vitals/latest?user=` — the newest reading of each vital (the home-screen tiles).
pub async fn latest(State(st): State<AppState>, p: Principal, Query(q): Query<UserQ>) -> ApiResult<Json<Page<ReadingDto>>> {
    let (owner, _) = subject(&st, &p, q.user).await?;
    let mut out = Vec::new();
    for t in types_for(&st, owner).await? {
        out.extend(st.db.readings_of(owner, &t.code, 1).await?.into_iter().map(reading_dto));
    }
    Ok(Json(Page::of(out)))
}

#[derive(Deserialize)]
pub struct ReadingsQ { user: Option<Uuid>, code: Option<String>, limit: Option<i32> }

/// `GET /v1/vitals/readings?user=&code=&limit=` — newest entry first; `code` narrows to one vital.
pub async fn readings(State(st): State<AppState>, p: Principal, Query(q): Query<ReadingsQ>) -> ApiResult<Json<Page<ReadingDto>>> {
    let (owner, _) = subject(&st, &p, q.user).await?;
    let l = &vitals::config().limits;
    let limit = q.limit.unwrap_or(l.readings_default_limit).clamp(1, l.readings_max_limit);
    let rows = match q.code.as_deref().filter(|c| !c.is_empty()) {
        Some(code) => st.db.readings_of(owner, code, limit).await?,
        None => st.db.readings(owner, limit).await?,
    };
    Ok(Json(Page::of(rows.into_iter().map(reading_dto).collect())))
}

#[derive(Deserialize)]
pub struct NewReading {
    code: String,
    values: HashMap<String, f64>,
    /// When it was measured (RFC 3339); now if omitted.
    taken_at: Option<String>,
    note: Option<String>,
}

/// `POST /v1/vitals/readings` — the caller records their own reading. It is coloured against
/// their range and placed in the inbox of every doctor in their contacts.
pub async fn record(State(st): State<AppState>, p: Principal, Json(b): Json<NewReading>) -> ApiResult<(StatusCode, Json<ReadingDto>)> {
    let owner = p.id();
    let t = types_for(&st, owner).await?.into_iter().find(|t| t.code == b.code)
        .ok_or_else(|| ApiError::bad_request("no such vital for you"))?;
    vitals::validate_values(&t, &b.values).map_err(ApiError::bad_request)?;
    // The time is chosen when the reading is entered (up to limits.backdate_hours ago) and never
    // changes afterwards: there is no endpoint that edits a reading.
    let given = match b.taken_at.as_deref() {
        None => None,
        Some(s) => Some(chrono::DateTime::parse_from_rfc3339(s).map_err(|_| ApiError::bad_request("taken_at must be an RFC 3339 time"))?.timestamp_millis()),
    };
    let taken_at = scylla::frame::value::CqlTimestamp(
        vitals::taken_at_millis(given, now_ts().0, vitals::config().limits.backdate_hours).map_err(ApiError::bad_request)?);
    let severity = t.severity(&b.values);
    let row = ReadingRow {
        user_id: owner,
        reading_id: new_timeuuid(),
        code: t.code.clone(),
        vals: b.values,
        taken_at,
        note: b.note.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(|n| n.chars().take(vitals::config().limits.note_max_chars).collect()),
        entered_by: p.id(),
        severity: severity.as_str().to_string(),
    };
    st.db.insert_reading(&row).await?;

    // Consolidate for the doctors in the person's contacts (deactivated accounts excluded).
    let mut doctors = Vec::new();
    for (role, id) in st.db.connection_ids(owner).await? {
        if role == Role::Doctor && st.db.user(id).await?.is_some_and(|d| d.active) { doctors.push(id); }
    }
    for d in &doctors {
        st.db.insert_vital_alert(*d, &VitalAlertRow {
            alert_id: new_timeuuid(), patient_id: owner, patient_name: p.user.display_name.clone(),
            code: t.code.clone(), label: t.label.clone(), icon: t.icon.clone(), unit: t.unit.clone(),
            vals: row.vals.clone(), severity: row.severity.clone(), is_read: false, reading_id: row.reading_id, taken_at,
        }).await?;
    }
    if !doctors.is_empty() {
        let ev = Envelope::new("vital.alert", owner, owner, doctors.iter().map(Uuid::to_string).collect(),
            json!({ "patient_id": owner, "code": t.code, "label": t.label, "severity": severity, "reading_id": row.reading_id }));
        st.events.emit(topics::ALERTS, &owner.to_string(), ev).await?;
    }
    Ok((StatusCode::CREATED, Json(reading_dto(row))))
}

/// Only a doctor in the patient's contacts configures their vitals.
async fn configure(st: &AppState, p: &Principal, patient: Uuid) -> ApiResult<()> {
    let other = load_user(st, patient).await?;
    let connected = st.db.is_connected(p.id(), patient, other.primary_role()).await?;
    if !vitals::may_configure(patient, p.id(), &p.user.roles, connected) {
        return Err(ApiError::forbidden("only a doctor in the patient's contacts can set their vitals and ranges"));
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct RangeBody { components: RangeSpec, margin_pct: Option<i32> }

/// `PUT /v1/vitals/ranges/{user}/{code}` — the doctor's range for this patient replaces the default.
pub async fn set_range(State(st): State<AppState>, p: Principal, Path((user, code)): Path<(Uuid, String)>, Json(b): Json<RangeBody>) -> ApiResult<Json<VitalType>> {
    configure(&st, &p, user).await?;
    let t = types_for(&st, user).await?.into_iter().find(|t| t.code == code).ok_or_else(|| ApiError::not_found("no such vital for this patient"))?;
    vitals::validate_range(&t, &b.components, b.margin_pct).map_err(ApiError::bad_request)?;
    // Components the doctor left out keep the range they had.
    let mut spec = t.range();
    spec.extend(b.components);
    st.db.set_vital_range(user, &code, &spec, b.margin_pct.or(t.margin_pct), p.id()).await?;
    let updated = types_for(&st, user).await?.into_iter().find(|t| t.code == code).ok_or_else(|| ApiError::not_found("no such vital"))?;
    Ok(Json(updated))
}

#[derive(Deserialize)]
pub struct NewType {
    user_id: Uuid,
    label: String,
    icon: String,
    #[serde(default)]
    unit: String,
    low: Option<f64>,
    high: Option<f64>,
    margin_pct: Option<i32>,
}

/// `POST /v1/vitals/types` — a doctor adds a vital for one patient, with an icon from the
/// predefined set and its range.
pub async fn add_type(State(st): State<AppState>, p: Principal, Json(b): Json<NewType>) -> ApiResult<(StatusCode, Json<VitalType>)> {
    configure(&st, &p, b.user_id).await?;
    let cfg = vitals::config();
    let label: String = b.label.trim().chars().take(cfg.limits.custom_label_max_chars).collect();
    let code = vitals::custom_code(&label).ok_or_else(|| ApiError::bad_request("label needs at least one letter or digit"))?;
    if !cfg.icons.contains(&b.icon) { return Err(ApiError::bad_request(format!("icon must be one of: {}", cfg.icons.join(", ")))); }
    let existing = types_for(&st, b.user_id).await?;
    if existing.iter().any(|t| t.code == code || t.label.eq_ignore_ascii_case(&label)) {
        return Err(ApiError::conflict("this patient already has a vital with that name"));
    }
    let t = VitalType {
        code, label: label.clone(), icon: b.icon, unit: b.unit.trim().chars().take(cfg.limits.custom_unit_max_chars).collect(),
        components: vec![Component { key: "value".into(), label, low: b.low, high: b.high }],
        margin_pct: Some(b.margin_pct.unwrap_or(cfg.colours.yellow_margin_pct)),
        ratio: None,
        custom: true,
        position: existing.iter().map(|t| t.position).max().unwrap_or(0) + 1,
        schema_version: SCHEMA_VERSION,
    };
    vitals::validate_range(&t, &t.range(), t.margin_pct).map_err(ApiError::bad_request)?;
    st.db.insert_custom_vital_type(b.user_id, &t, p.id()).await?;
    Ok((StatusCode::CREATED, Json(t)))
}

#[derive(Deserialize)]
pub struct AlertsQ { severity: Option<String>, sort: Option<String> }

/// `GET /v1/vitals/alerts?severity=&sort=time|user|vital` — the doctor's unread alerts and the
/// red / yellow / green counts. Alerts from people no longer in the doctor's contacts are left out.
pub async fn alerts(State(st): State<AppState>, p: Principal, Query(q): Query<AlertsQ>) -> ApiResult<Json<AlertPage>> {
    if !p.has(Role::Doctor) { return Ok(Json(AlertPage { counts: AlertCounts::default(), data: vec![] })); }
    let connected: std::collections::HashSet<Uuid> = st.db.connection_ids(p.id()).await?.into_iter().map(|(_, id)| id).collect();
    let unread: Vec<VitalAlertRow> = st.db.vital_alerts(p.id(), vitals::config().limits.doctor_inbox_window).await?.into_iter()
        .filter(|a| !a.is_read && connected.contains(&a.patient_id)).collect();
    let count = |s: Severity| unread.iter().filter(|a| Severity::parse(&a.severity) == s).count();
    let counts = AlertCounts { red: count(Severity::Red), yellow: count(Severity::Yellow), green: count(Severity::Green) };

    let wanted = q.severity.as_deref().map(str::to_uppercase).filter(|s| !s.is_empty() && s != "ALL");
    let mut data: Vec<VitalAlertRow> = unread.into_iter().filter(|a| wanted.as_ref().map_or(true, |w| &a.severity == w)).collect();
    // Rows arrive newest first; a stable sort keeps that order inside each patient or vital.
    match q.sort.as_deref() {
        Some("user") => data.sort_by(|a, b| a.patient_name.to_lowercase().cmp(&b.patient_name.to_lowercase())),
        Some("vital") => data.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase())),
        _ => {}
    }
    let data = data.into_iter().map(|a| VitalAlertDto {
        alert_id: a.alert_id, patient_id: a.patient_id, patient_name: a.patient_name, code: a.code, label: a.label, icon: a.icon,
        unit: a.unit, values: a.vals, severity: Severity::parse(&a.severity), taken_at: ts_to_dt(a.taken_at).to_rfc3339(),
        schema_version: SCHEMA_VERSION,
    }).collect();
    Ok(Json(AlertPage { counts, data }))
}

/// `POST /v1/vitals/alerts/{id}/read` — removes the alert from this doctor's active list only.
pub async fn read_alert(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    st.db.mark_vital_alert_read(p.id(), id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /v1/vitals/config` — what the apps need from the configuration file: the icons a doctor
/// can choose and the margin a doctor may set. Ranges are never part of this.
pub async fn settings(_p: Principal) -> Json<serde_json::Value> {
    let c = vitals::config();
    Json(json!({
        "icons": c.icons,
        "yellow_margin_pct": c.colours.yellow_margin_pct,
        "margin_pct_min": c.colours.margin_pct_min,
        "margin_pct_max": c.colours.margin_pct_max,
        "value_max": c.limits.value_max,
        "home_columns": c.home.columns,
        "home_rows": c.home.rows,
        "backdate_hours": c.limits.backdate_hours,
        "schema_version": SCHEMA_VERSION,
    }))
}
