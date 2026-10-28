//! Invite codes for hospital administrators (Administration_Design.md section 11).
//!
//! * Platform administrators create, list and revoke codes for a hospital (the dashboard).
//! * Anyone signed in enters a code (the app: Settings → "I have an invite code"). A valid code
//!   makes them that hospital's administrator, once.

use super::{principal::Principal, users::load_user, ApiError, ApiResult, AppState};
use crate::{
    events::{topics, Envelope},
    model::*,
    policy::{self, Access},
    store::invites::{normalize, Invite, VALID_DAYS},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use dashmap::DashMap;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{sync::OnceLock, time::{Duration, Instant}};
use uuid::Uuid;

/// Wrong codes allowed per account in `FAILURE_WINDOW` before redeeming is refused for a while.
const MAX_FAILURES: u32 = 5;
const FAILURE_WINDOW: Duration = Duration::from_secs(15 * 60);

fn failures() -> &'static DashMap<Uuid, (Instant, u32)> {
    static F: OnceLock<DashMap<Uuid, (Instant, u32)>> = OnceLock::new();
    F.get_or_init(DashMap::new)
}

fn too_many_failures(user: Uuid) -> bool {
    failures().get(&user).is_some_and(|f| f.0.elapsed() < FAILURE_WINDOW && f.1 >= MAX_FAILURES)
}

fn record_failure(user: Uuid) {
    let mut e = failures().entry(user).or_insert((Instant::now(), 0));
    if e.0.elapsed() >= FAILURE_WINDOW { *e = (Instant::now(), 0); }
    e.1 += 1;
}

async fn hospital(st: &AppState, id: Uuid) -> ApiResult<crate::store::users::UserRec> {
    let h = load_user(st, id).await?;
    if !h.roles.contains(&Role::Hospital) { return Err(ApiError::not_found("no such hospital")); }
    Ok(h)
}

async fn invite_json(st: &AppState, i: &Invite) -> ApiResult<Value> {
    let used_by = match i.used_by { Some(u) => st.db.user(u).await?.map(|u| json!({ "user_id": u.user_id, "public_id": u.public_id, "display_name": u.display_name })), None => None };
    Ok(json!({
        "invite_id": i.invite_id, "hospital_id": i.hospital_id, "note": i.note,
        "status": i.status(Utc::now()), "created_at": i.created_at.to_rfc3339(), "expires_at": i.expires_at.to_rfc3339(),
        "used_at": i.used_at.map(|t| t.to_rfc3339()), "used_by": used_by,
    }))
}

#[derive(Deserialize)]
pub struct NewInvite {
    /// Who it is for, e.g. "Dr. Rao's office manager". Shown only to platform administrators.
    note: Option<String>,
}

/// `POST /v1/admin/hospitals/{id}/invites` — a new single-use code, valid for 7 days. The code is
/// in this response only; the server keeps just its hash.
pub async fn create(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>, Json(b): Json<NewInvite>) -> ApiResult<(StatusCode, Json<Value>)> {
    policy::platform_admin(&p.user.roles).map_err(ApiError::forbidden)?;
    let h = hospital(&st, id).await?;
    let max = crate::settings::settings().administrators.max_admins_per_hospital;
    let admins = st.db.hospital_admin_ids(h.user_id).await?.len();
    let waiting = st.db.hospital_invites(h.user_id).await?.iter().filter(|i| i.status(Utc::now()) == "PENDING").count();
    if admins + waiting >= max {
        return Err(ApiError::conflict(format!(
            "{} has {admins} administrator(s) and {waiting} unused invite code(s); at most {max} administrators are allowed. Remove an administrator or revoke a code first.",
            h.display_name)));
    }
    let note: Option<String> = b.note.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|s| s.chars().take(80).collect());
    let (invite, code) = st.db.create_invite(h.user_id, note.as_deref(), p.id()).await?;
    tracing::info!(admin = %p.id(), hospital = %h.user_id, invite = %invite.invite_id, "invite code created");
    let ev = Envelope::new("invite.created", p.id(), h.user_id, vec![], json!({ "invite_id": invite.invite_id, "hospital_id": h.user_id, "hospital_name": h.display_name }));
    st.events.emit(topics::ACCESS_CHANGES, &h.user_id.to_string(), ev).await?;
    let mut out = invite_json(&st, &invite).await?;
    out["code"] = json!(code);
    out["hospital_name"] = json!(h.display_name);
    out["valid_days"] = json!(VALID_DAYS);
    Ok((StatusCode::CREATED, Json(out)))
}

/// `GET /v1/admin/hospitals/{id}/invites` — the hospital's invites, newest first (no codes).
pub async fn list(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    policy::platform_admin(&p.user.roles).map_err(ApiError::forbidden)?;
    hospital(&st, id).await?;
    let mut data = Vec::new();
    for i in st.db.hospital_invites(id).await? { data.push(invite_json(&st, &i).await?); }
    Ok(Json(json!({ "data": data })))
}

/// `DELETE /v1/admin/hospitals/{id}/invites/{invite_id}` — the code stops working. A used code
/// can't be revoked (remove the administrator instead).
pub async fn revoke(State(st): State<AppState>, p: Principal, Path((id, invite_id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    policy::platform_admin(&p.user.roles).map_err(ApiError::forbidden)?;
    let i = st.db.invite(invite_id).await?.filter(|i| i.hospital_id == id).ok_or_else(|| ApiError::not_found("no such invite"))?;
    if i.used_by.is_some() { return Err(ApiError::conflict("this code was already used; remove the administrator instead")); }
    st.db.revoke_invite(invite_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct Redeem { code: String }

/// `POST /v1/invites/redeem` — the signed-in account becomes the hospital's administrator.
/// Every wrong, used, expired or revoked code gets the same answer, so codes can't be probed.
pub async fn redeem(State(st): State<AppState>, p: Principal, Json(b): Json<Redeem>) -> ApiResult<Json<Value>> {
    if too_many_failures(p.id()) {
        return Err(ApiError::too_many_with("Too many wrong codes. Wait 15 minutes and try again."));
    }
    let invalid = || ApiError::bad_request("That invite code is not valid. Check it, or ask for a new one.");
    let Some(code) = normalize(&b.code) else { record_failure(p.id()); return Err(invalid()) };
    let Some(invite) = st.db.invite_by_code(&code).await? else { record_failure(p.id()); return Err(invalid()) };
    if invite.status(Utc::now()) != "PENDING" { record_failure(p.id()); return Err(invalid()); }

    let d = policy::may_appoint_admin(&p.user.roles);
    if d.access != Access::Full { return Err(ApiError::forbidden(d.reason)); }
    let h = hospital(&st, invite.hospital_id).await?;
    if p.user.roles.contains(&Role::Administrator) && p.user.primary_hospital_id.is_some_and(|x| x != h.user_id) {
        return Err(ApiError::conflict("you already administer another hospital; ask a platform administrator to move you"));
    }
    let max = crate::settings::settings().administrators.max_admins_per_hospital;
    let already = p.user.roles.contains(&Role::Administrator) && p.user.primary_hospital_id == Some(h.user_id);
    if !already && st.db.hospital_admin_ids(h.user_id).await?.len() >= max {
        return Err(ApiError::conflict(format!("{} already has {max} administrators, the most allowed. Ask a platform administrator.", h.display_name)));
    }
    if !st.db.use_invite(invite.invite_id, p.id()).await? { record_failure(p.id()); return Err(invalid()); }

    let mut roles = p.user.roles.clone();
    roles.insert(Role::Administrator);
    st.db.set_roles(&p.user, &roles, Some(h.user_id)).await?;
    st.db.connect_users(p.id(), Role::Administrator, &p.user.display_name, h.user_id, Role::Hospital, &h.display_name).await?;
    st.principals.clear();
    failures().remove(&p.id());
    tracing::info!(user_id = %p.id(), hospital = %h.user_id, invite = %invite.invite_id, "invite code redeemed");
    let ev = Envelope::new("invite.redeemed", p.id(), p.id(), vec![p.id().to_string()], json!({ "user_id": p.id(), "hospital_id": h.user_id, "hospital_name": h.display_name, "invite_id": invite.invite_id }));
    st.events.emit(topics::ACCESS_CHANGES, &p.id().to_string(), ev).await?;
    Ok(Json(json!({ "hospital_id": h.user_id, "hospital_name": h.display_name })))
}
