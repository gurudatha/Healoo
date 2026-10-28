//! /v1/admin/* for platform administrators only (rule 11, Administration_Design.md section 8):
//! the dashboard's overview, account list and activity log, and hospitals with their
//! administrators.

use super::{
    admin::{emit_created, hospital_dto, validate_hospital, HospitalProfileDto, HospitalUpdate},
    principal::Principal,
    users::load_user,
    ApiError, ApiResult, AppState,
};
use crate::{
    events::{topics, Envelope},
    model::*,
    policy::{self, Access},
    store::{admin::{day_of, AccountRow, ActivityRow}, users::NewUser},
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

fn require(p: &Principal) -> ApiResult<()> { policy::platform_admin(&p.user.roles).map_err(ApiError::forbidden) }

/// Counters shown on the overview and in the trend chart, in display order.
pub const METRICS: [&str; 23] = [
    "users_registered", "accounts_created", "items_created", "bookings", "reports", "items_updated", "items_closed", "items_reopened",
    "appointments", "reminders", "uploads", "messages", "grants_added", "grants_revoked", "affiliations_ended",
    "delegations", "access_denied", "accounts_deactivated", "accounts_reactivated", "hospitals_created", "admins_appointed", "admins_removed", "invites_created",
];

// ---------- overview ----------

#[derive(Deserialize)]
pub struct OverviewQ { days: Option<i64> }

/// `GET /v1/admin/overview` — account totals, today's counts, a daily trend and the latest activity.
pub async fn overview(State(st): State<AppState>, p: Principal, Query(q): Query<OverviewQ>) -> ApiResult<Json<Value>> {
    require(&p)?;
    let accounts = st.db.all_accounts().await?;
    let mut by_role: BTreeMap<String, usize> = BTreeMap::new();
    for a in &accounts { *by_role.entry(text(&a.primary_role())).or_default() += 1; }
    let active = accounts.iter().filter(|a| a.active).count();
    let never = accounts.iter().filter(|a| a.active && !a.signed_in).count();

    let days = q.days.unwrap_or(14).clamp(1, 90);
    let today = Utc::now().date_naive();
    let mut trend = Vec::new();
    for back in (0..days).rev() {
        let day = (today - Duration::days(back)).format("%Y-%m-%d").to_string();
        let counts: HashMap<String, i64> = st.db.counts(&day).await?.into_iter().collect();
        let mut row = Map::new();
        row.insert("day".into(), json!(day));
        row.insert("active_users".into(), json!(st.db.active_users(&day).await?));
        for m in METRICS { row.insert(m.into(), json!(counts.get(m).copied().unwrap_or(0))); }
        trend.push(Value::Object(row));
    }
    let recent: Vec<Value> = st.db.activity(&day_of(Utc::now()), 10).await?.iter().map(activity_json).collect();
    Ok(Json(json!({
        "accounts": { "total": accounts.len(), "active": active, "deactivated": accounts.len() - active, "never_signed_in": never, "by_role": by_role },
        "today": trend.last().cloned().unwrap_or_default(),
        "trend": trend,
        "recent": recent,
        "metrics": METRICS,
    })))
}

// ---------- usage ----------

/// The periods on the Usage page: the last 1, 7, 30 and 365 days, today included.
const PERIODS: [(&str, i64); 4] = [("day", 1), ("week", 7), ("month", 30), ("year", 365)];

/// `GET /v1/admin/usage` — totals, active users and active hospitals over the last day, week,
/// month and year, and how much the database and file store hold.
///
/// A hospital counts as active when the hospital account, one of its administrators or one of
/// its doctors did something in the period. Reads a year of daily rows: a second or so.
pub async fn usage(State(st): State<AppState>, p: Principal) -> ApiResult<Json<Value>> {
    use futures::{stream, StreamExt, TryStreamExt};
    require(&p)?;
    let today = Utc::now().date_naive();
    let days: Vec<String> = (0..365).map(|back| (today - Duration::days(back)).format("%Y-%m-%d").to_string()).collect();
    // Newest day first, so the first n entries are the last n days.
    let per_day: Vec<(HashMap<String, i64>, Vec<Uuid>)> = stream::iter(days)
        .map(|day| { let st = st.clone(); async move {
            let counts = st.db.counts(&day).await?.into_iter().collect();
            anyhow::Ok((counts, st.db.active_user_ids(&day).await?))
        } })
        .buffered(16)
        .try_collect().await?;

    let accounts = st.db.all_accounts().await?;
    let hospital_of: HashMap<Uuid, Uuid> = accounts.iter().filter_map(|a| {
        if a.roles.contains(&Role::Hospital) { Some((a.user_id, a.user_id)) }
        else if a.roles.contains(&Role::Doctor) || a.roles.contains(&Role::Administrator) { a.hospital_id.map(|h| (a.user_id, h)) }
        else { None }
    }).collect();
    let hospitals_total = accounts.iter().filter(|a| a.roles.contains(&Role::Hospital) && a.active).count();
    let users_total = accounts.iter().filter(|a| a.active).count();

    let periods: Vec<Value> = PERIODS.iter().map(|(label, n)| {
        let window = &per_day[..*n as usize];
        let mut totals = Map::new();
        for m in METRICS { totals.insert(m.into(), json!(window.iter().map(|(c, _)| c.get(m).copied().unwrap_or(0)).sum::<i64>())); }
        let users: HashSet<Uuid> = window.iter().flat_map(|(_, u)| u.iter().copied()).collect();
        let hospitals: HashSet<Uuid> = users.iter().filter_map(|u| hospital_of.get(u).copied()).collect();
        json!({ "period": label, "days": n, "active_users": users.len(), "active_hospitals": hospitals.len(), "totals": totals })
    }).collect();

    let tables = st.db.table_sizes(&st.cfg.keyspace).await.unwrap_or_else(|e| { tracing::warn!("table sizes: {e:#}"); Vec::new() });
    let mut tables: Vec<Value> = tables.into_iter().map(|(t, mib)| json!({ "table": t, "mib": mib })).collect();
    tables.sort_by_key(|t| std::cmp::Reverse(t["mib"].as_i64().unwrap_or(0)));
    let db_mib: i64 = tables.iter().filter_map(|t| t["mib"].as_i64()).sum();
    let records: Map<String, Value> = st.db.record_counts().await.into_iter().map(|(k, v)| (k.to_string(), json!(v))).collect();
    let files = match st.files.usage().await {
        Ok((count, bytes)) => json!({ "count": count, "bytes": bytes }),
        Err(e) => { tracing::warn!("file store usage: {e:#}"); json!({ "count": null, "bytes": null }) }
    };

    Ok(Json(json!({
        "periods": periods,
        "users_total": users_total,
        "hospitals_total": hospitals_total,
        "storage": { "database_mib": db_mib, "tables": tables, "records": records, "files": files },
    })))
}

// ---------- accounts ----------

#[derive(Deserialize)]
pub struct AccountsQ {
    q: Option<String>,
    role: Option<String>,
    /// "active" | "deactivated" | "never_signed_in"
    status: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Serialize)]
struct AccountDto {
    user_id: Uuid,
    public_id: String,
    display_name: String,
    role: Role,
    roles: Vec<Role>,
    hospital_id: Option<Uuid>,
    hospital_name: Option<String>,
    email: Option<String>,
    active: bool,
    signed_in: bool,
    created_at: Option<String>,
}

fn matches(a: &AccountRow, q: &AccountsQ) -> bool {
    if let Some(r) = q.role.as_deref().filter(|r| !r.is_empty()) {
        if text(&a.primary_role()) != r.to_uppercase() { return false; }
    }
    match q.status.as_deref() {
        Some("active") if !a.active => return false,
        Some("deactivated") if a.active => return false,
        Some("never_signed_in") if !a.active || a.signed_in => return false,
        _ => {}
    }
    match q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => true,
        Some(s) => {
            let s = s.to_lowercase();
            a.display_name.to_lowercase().contains(&s) || a.public_id.to_lowercase().contains(&s)
                || a.email.as_deref().is_some_and(|e| e.contains(&s))
        }
    }
}

/// `GET /v1/admin/accounts` — every account, filtered by name / Healoo ID / email, role and status.
pub async fn accounts(State(st): State<AppState>, p: Principal, Query(q): Query<AccountsQ>) -> ApiResult<Json<Value>> {
    require(&p)?;
    let all = st.db.all_accounts().await?;
    let hospital_names: HashMap<Uuid, String> = all.iter().filter(|a| a.roles.contains(&Role::Hospital)).map(|a| (a.user_id, a.display_name.clone())).collect();
    let mut found: Vec<&AccountRow> = all.iter().filter(|a| matches(a, &q)).collect();
    found.sort_by_key(|a| a.display_name.to_lowercase());
    let total = found.len();
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let page: Vec<AccountDto> = found.into_iter().skip(q.offset.unwrap_or(0)).take(limit).map(|a| {
        let mut roles: Vec<Role> = a.roles.iter().copied().collect();
        roles.sort_by_key(text);
        AccountDto {
            user_id: a.user_id,
            public_id: a.public_id.clone(),
            display_name: a.display_name.clone(),
            role: a.primary_role(),
            roles,
            hospital_id: a.hospital_id,
            hospital_name: a.hospital_id.and_then(|h| hospital_names.get(&h).cloned()),
            email: a.email.clone(),
            active: a.active,
            signed_in: a.signed_in,
            created_at: a.created_at.map(|t| t.to_rfc3339()),
        }
    }).collect();
    Ok(Json(json!({ "data": page, "total": total })))
}

// ---------- activity ----------

#[derive(Deserialize)]
pub struct ActivityQ {
    /// "YYYY-MM-DD" (UTC); today when missing.
    day: Option<String>,
    /// RECORDS | SHARING | ACCOUNTS
    category: Option<String>,
    limit: Option<i32>,
}

fn activity_json(a: &ActivityRow) -> Value {
    json!({
        "at": a.occurred_at.to_rfc3339(), "category": a.category, "event_type": a.event_type,
        "actor_id": a.actor_id, "actor_name": a.actor_name, "subject_id": a.subject_id,
        "subject_name": a.subject_name, "detail": a.detail,
    })
}

/// `GET /v1/admin/activity` — one day's activity log, newest first.
pub async fn activity(State(st): State<AppState>, p: Principal, Query(q): Query<ActivityQ>) -> ApiResult<Json<Value>> {
    require(&p)?;
    let day = match q.day.as_deref().filter(|d| !d.is_empty()) {
        Some(d) => NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| ApiError::bad_request("day must be YYYY-MM-DD"))?.format("%Y-%m-%d").to_string(),
        None => day_of(Utc::now()),
    };
    let rows = st.db.activity(&day, q.limit.unwrap_or(500).clamp(1, 2000)).await?;
    let category = q.category.as_deref().filter(|c| !c.is_empty()).map(str::to_uppercase);
    let data: Vec<Value> = rows.iter().filter(|a| category.as_deref().map_or(true, |c| a.category == c)).map(activity_json).collect();
    Ok(Json(json!({ "day": day, "data": data })))
}

// ---------- hospitals ----------

#[derive(Serialize)]
struct HospitalListDto {
    #[serde(flatten)]
    profile: HospitalProfileDto,
    doctors: usize,
    administrators: Vec<Value>,
}

/// `GET /v1/admin/hospitals` — every hospital with its doctor count and administrators.
pub async fn hospitals(State(st): State<AppState>, p: Principal) -> ApiResult<Json<Value>> {
    require(&p)?;
    let all = st.db.all_accounts().await?;
    let mut out = Vec::new();
    for a in all.iter().filter(|a| a.roles.contains(&Role::Hospital)) {
        let Some(h) = st.db.user(a.user_id).await? else { continue };
        let admins: Vec<Value> = all.iter()
            .filter(|u| u.roles.contains(&Role::Administrator) && u.hospital_id == Some(h.user_id))
            .map(|u| json!({ "user_id": u.user_id, "public_id": u.public_id, "display_name": u.display_name, "email": u.email, "active": u.active }))
            .collect();
        out.push(HospitalListDto { profile: hospital_dto(&h), doctors: st.db.hospital_doctors(h.user_id).await?.len(), administrators: admins });
    }
    out.sort_by_key(|h| h.profile.display_name.to_lowercase());
    let limits = &crate::settings::settings().administrators;
    Ok(Json(json!({ "data": out, "max_admins_per_hospital": limits.max_admins_per_hospital, "max_platform_admins": limits.max_platform_admins })))
}

#[derive(Deserialize)]
pub struct NewHospital {
    #[serde(flatten)]
    profile: HospitalUpdate,
    /// Optional: create the hospital's first administrator too. Their first Auth0 sign-in with
    /// this (verified) email is linked to the account.
    admin_name: Option<String>,
    admin_email: Option<String>,
}

/// `POST /v1/admin/hospitals` — a new hospital, and optionally its first administrator.
pub async fn create_hospital(State(st): State<AppState>, p: Principal, Json(b): Json<NewHospital>) -> ApiResult<(StatusCode, Json<Value>)> {
    require(&p)?;
    let (name, location, phone, reg, headline) = validate_hospital(&b.profile)?;
    let admin = match (b.admin_name.as_deref().map(str::trim).filter(|s| !s.is_empty()), b.admin_email.as_deref().map(str::trim).filter(|s| !s.is_empty())) {
        (None, None) => None,
        (Some(n), Some(e)) => {
            if n.chars().count() > 80 { return Err(ApiError::bad_request("admin_name must be 1–80 characters")); }
            let e = e.to_lowercase();
            if !e.split_once('@').is_some_and(|(a, d)| !a.is_empty() && d.contains('.')) { return Err(ApiError::bad_request("admin_email must be a valid address")); }
            if st.db.user_id_by_email(&e).await?.is_some() { return Err(ApiError::conflict("an account with that email already exists")); }
            Some((n.to_string(), e))
        }
        _ => return Err(ApiError::bad_request("give both admin_name and admin_email, or neither")),
    };

    let h = st.db.create_user(NewUser {
        user_id: Uuid::new_v4(), public_id: None, auth0_sub: None, display_name: &name, roles: &[Role::Hospital],
        location: location.as_deref(), official_number: reg.as_deref(), primary_hospital_id: None,
        employer_doctor_id: None, headline: headline.as_deref(), email: None, created_by: Some(p.id()),
    }).await?;
    if phone.is_some() { st.db.update_hospital(&h, &name, location.as_deref(), phone.as_deref(), reg.as_deref(), headline.as_deref()).await?; }
    tracing::info!(admin = %p.id(), hospital = %h.user_id, public_id = %h.public_id, "platform administrator created hospital");
    let ev = Envelope::new("hospital.created", p.id(), h.user_id, vec![], json!({ "hospital_id": h.user_id }));
    st.events.emit(topics::ACCESS_CHANGES, &h.user_id.to_string(), ev).await?;

    let mut admin_out = Value::Null;
    if let Some((admin_name, email)) = admin {
        let a = st.db.create_user(NewUser {
            user_id: Uuid::new_v4(), public_id: None, auth0_sub: None, display_name: &admin_name, roles: &[Role::Administrator],
            location: None, official_number: None, primary_hospital_id: Some(h.user_id), employer_doctor_id: None,
            headline: None, email: Some(&email), created_by: Some(p.id()),
        }).await?;
        st.db.connect_users(a.user_id, Role::Administrator, &a.display_name, h.user_id, Role::Hospital, &h.display_name).await?;
        emit_created(&st, &p, &a, Role::Administrator).await?;
        admin_out = json!({ "user_id": a.user_id, "public_id": a.public_id, "display_name": a.display_name, "email": a.email });
    }
    let h = load_user(&st, h.user_id).await?;
    Ok((StatusCode::CREATED, Json(json!({ "hospital": hospital_dto(&h), "administrator": admin_out }))))
}

#[derive(Deserialize)]
pub struct AppointQ {
    /// The account's Healoo ID, e.g. "HL-4K7Q2". The person signs up in the app first.
    public_id: String,
}

/// `POST /v1/admin/hospitals/{id}/administrators` — make an existing account the hospital's
/// administrator. They see Settings → Administration the next time they open the app.
pub async fn appoint_admin(State(st): State<AppState>, p: Principal, Path(hospital): Path<Uuid>, Json(b): Json<AppointQ>) -> ApiResult<Json<Value>> {
    require(&p)?;
    let h = load_user(&st, hospital).await?;
    if !h.roles.contains(&Role::Hospital) { return Err(ApiError::not_found("no such hospital")); }
    let id = st.db.user_id_by_public_id(b.public_id.trim()).await?.ok_or_else(|| ApiError::not_found("no account with that Healoo ID"))?;
    let u = load_user(&st, id).await?;
    let d = policy::may_appoint_admin(&u.roles);
    if d.access != Access::Full { return Err(ApiError::forbidden(d.reason)); }
    if !u.active { return Err(ApiError::bad_request("that account is deactivated")); }
    let already = u.roles.contains(&Role::Administrator) && u.primary_hospital_id == Some(h.user_id);
    let max = crate::settings::settings().administrators.max_admins_per_hospital;
    if !already && st.db.hospital_admin_ids(h.user_id).await?.len() >= max {
        return Err(ApiError::conflict(format!("{} already has {max} administrators, the most allowed; remove one first", h.display_name)));
    }
    let mut roles: HashSet<Role> = u.roles.clone();
    roles.insert(Role::Administrator);
    st.db.set_roles(&u, &roles, Some(h.user_id)).await?;
    st.db.connect_users(u.user_id, Role::Administrator, &u.display_name, h.user_id, Role::Hospital, &h.display_name).await?;
    st.principals.clear();
    tracing::info!(admin = %p.id(), user_id = %u.user_id, hospital = %h.user_id, "hospital administrator appointed");
    let ev = Envelope::new("admin.appointed", p.id(), u.user_id, vec![u.user_id.to_string()], json!({ "user_id": u.user_id, "hospital_id": h.user_id, "hospital_name": h.display_name }));
    st.events.emit(topics::ACCESS_CHANGES, &u.user_id.to_string(), ev).await?;
    Ok(Json(json!({ "user_id": u.user_id, "public_id": u.public_id, "display_name": u.display_name, "hospital_id": h.user_id })))
}

/// `DELETE /v1/admin/hospitals/{id}/administrators/{user_id}` — the account stops being the
/// hospital's administrator (it keeps its other roles, e.g. patient).
pub async fn remove_admin(State(st): State<AppState>, p: Principal, Path((hospital, id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    require(&p)?;
    let u = load_user(&st, id).await?;
    if !u.roles.contains(&Role::Administrator) || u.primary_hospital_id != Some(hospital) {
        return Err(ApiError::not_found("that account is not this hospital's administrator"));
    }
    let mut roles = u.roles.clone();
    roles.remove(&Role::Administrator);
    if roles.is_empty() { roles.insert(Role::Patient); }
    st.db.set_roles(&u, &roles, None).await?;
    st.principals.clear();
    tracing::info!(admin = %p.id(), user_id = %u.user_id, hospital = %hospital, "hospital administrator removed");
    let ev = Envelope::new("admin.removed", p.id(), u.user_id, vec![u.user_id.to_string()], json!({ "user_id": u.user_id, "hospital_id": hospital }));
    st.events.emit(topics::ACCESS_CHANGES, &u.user_id.to_string(), ev).await?;
    Ok(StatusCode::NO_CONTENT)
}
