//! /v1/grants, /v1/affiliations/{doctor}/end, /v1/delegations, /v1/check/access

use super::{principal::Principal, users::load_user, ApiError, ApiResult, AppState};
use crate::{
    events::{topics, Envelope},
    model::*,
    policy::{self, AssistantContext},
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct GrantBody {
    item_ids: Vec<Uuid>,
    grantee_id: Uuid,
    /// Rule 4: a share given to a doctor acting for a hospital is stored as a HOSPITAL grant.
    via_hospital_id: Option<Uuid>,
}

pub async fn create(State(st): State<AppState>, p: Principal, Json(b): Json<GrantBody>) -> ApiResult<StatusCode> {
    if b.item_ids.is_empty() || b.item_ids.len() > 50 { return Err(ApiError::bad_request("send 1–50 item_ids")); }
    let grantee = load_user(&st, b.via_hospital_id.unwrap_or(b.grantee_id)).await?;
    let kind = if grantee.primary_role() == Role::Hospital { GranteeType::Hospital } else { GranteeType::User };

    for id in &b.item_ids {
        let item = st.db.item(*id).await?.ok_or_else(|| ApiError::not_found("no such item"))?;
        if item.owner_id != p.id() { return Err(ApiError::forbidden("only the owner can share an item")); }
        if item.grant_list().iter().any(|g| g.grantee_id == grantee.user_id) { continue; }
        let g = GrantUdt {
            grant_id: Some(Uuid::new_v4()),
            grantee_type: Some(text(&kind)),
            grantee_id: Some(grantee.user_id),
            grantee_name: Some(grantee.display_name.clone()),
            via_hospital_id: b.via_hospital_id,
            granted_by: Some(p.id()),
            granted_at: Some(now_ts()),
        };
        st.db.add_grant(&item, g).await?;
        let updated = st.db.item(*id).await?.unwrap_or(item);
        let ev = Envelope::new("grant.added", p.id(), id, updated.audience(), json!({ "item_id": id, "grantee": grantee_key(kind, grantee.user_id) }));
        st.events.emit(topics::ACCESS_CHANGES, &p.id().to_string(), ev).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct ListQ { owner: Option<String> }

/// `GET /v1/grants?owner=me` — the Active sharing screen.
pub async fn list(State(st): State<AppState>, p: Principal, Query(q): Query<ListQ>) -> ApiResult<Json<Page<OwnedGrantDto>>> {
    if q.owner.as_deref().unwrap_or("me") != "me" { return Err(ApiError::forbidden("you can list only your own shares")); }
    Ok(Json(Page::of(st.db.grants_by_owner(p.id()).await?)))
}

pub async fn revoke(State(st): State<AppState>, p: Principal, Path(grant_id): Path<Uuid>) -> ApiResult<StatusCode> {
    let (item_id, owner) = st.db.grant_lookup(grant_id).await?.ok_or_else(|| ApiError::not_found("no such grant"))?;
    if owner != p.id() { return Err(ApiError::forbidden("only the owner can revoke")); }
    let item = st.db.item(item_id).await?.ok_or_else(|| ApiError::not_found("no such item"))?;
    let audience = item.audience(); // before removal, so the revoked party is told too
    let udt = item.grants.iter().find(|g| g.grant_id == Some(grant_id)).cloned()
        .ok_or_else(|| ApiError::not_found("grant already removed"))?;
    st.db.remove_grant(&item, &udt).await?;
    let ev = Envelope::new("grant.revoked", p.id(), item_id, audience, json!({ "item_id": item_id, "grant_id": grant_id }));
    st.events.emit(topics::ACCESS_CHANGES, &p.id().to_string(), ev).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct EndBody { hospital_id: Uuid }

/// Rule 5: ends a doctor–hospital affiliation; access through that hospital stops at once.
pub async fn end_affiliation(State(st): State<AppState>, p: Principal, Path(doctor): Path<Uuid>, Json(b): Json<EndBody>) -> ApiResult<StatusCode> {
    let allowed = p.has(Role::AppAdministrator) || (p.has(Role::Administrator) && p.user.primary_hospital_id == Some(b.hospital_id));
    if !allowed { return Err(ApiError::forbidden("only this hospital's administrator can end affiliations")); }
    st.db.end_affiliation(doctor, b.hospital_id).await?;
    st.principals.clear(); // drop cached principals so hospital lists reload
    let ev = Envelope::new("affiliation.ended", p.id(), doctor, vec![doctor.to_string()], json!({ "doctor_id": doctor, "hospital_id": b.hospital_id }));
    st.events.emit(topics::ACCESS_CHANGES, &b.hospital_id.to_string(), ev).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct DelegateBody { assistant_id: Uuid, patient_id: Uuid }

/// Rule 7: a doctor lets their assistant see one patient's content.
pub async fn delegate(State(st): State<AppState>, p: Principal, Json(b): Json<DelegateBody>) -> ApiResult<StatusCode> {
    if !p.has(Role::Doctor) { return Err(ApiError::forbidden("only doctors can delegate")); }
    let assistant = load_user(&st, b.assistant_id).await?;
    if !assistant.roles.contains(&Role::Assistant) || assistant.employer_doctor_id != Some(p.id()) {
        return Err(ApiError::forbidden("that assistant does not work for you"));
    }
    if !st.db.is_connected(p.id(), b.patient_id, Role::Patient).await? {
        return Err(ApiError::forbidden("you can delegate only your own patients"));
    }
    st.db.add_delegation(b.assistant_id, p.id(), b.patient_id).await?;
    let ev = Envelope::new("delegation.added", p.id(), b.patient_id, vec![b.assistant_id.to_string()], json!({ "assistant_id": b.assistant_id, "patient_id": b.patient_id }));
    st.events.emit(topics::ACCESS_CHANGES, &b.patient_id.to_string(), ev).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct CheckQ { item: Uuid, user: Uuid }

/// Debug (LAN or DEBUG_ROUTES=true): the policy decision and reason for any user and item.
pub async fn check_access(State(st): State<AppState>, p: Principal, Query(q): Query<CheckQ>) -> ApiResult<Json<Value>> {
    if !(p.has(Role::AppAdministrator) || st.cfg.mode == crate::config::NetworkMode::Lan) {
        return Err(ApiError::forbidden("platform administrators only"));
    }
    let item = st.db.item(q.item).await?.ok_or_else(|| ApiError::not_found("no such item"))?;
    let u = load_user(&st, q.user).await?;
    let subject = policy::Subject {
        user_id: u.user_id,
        roles: u.roles.clone(),
        active_hospitals: if u.roles.contains(&Role::Doctor) { st.db.active_hospitals(u.user_id).await? } else { Default::default() },
    };
    let ctx = match (u.roles.contains(&Role::Assistant), u.employer_doctor_id) {
        (true, Some(doc)) => {
            let d = load_user(&st, doc).await?;
            Some(AssistantContext {
                doctor: Some(policy::Subject { user_id: doc, roles: d.roles, active_hospitals: st.db.active_hospitals(doc).await? }),
                delegated_for_owner: st.db.delegation_exists(u.user_id, doc, item.owner_id).await?,
            })
        }
        _ => None,
    };
    let d = policy::decide(&item.facts(), &subject, ctx.as_ref());
    Ok(Json(json!({ "item_id": q.item, "user_id": q.user, "access": d.access, "reason": d.reason })))
}
