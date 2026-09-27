//! /v1/me, /v1/users, /v1/search, /v1/connections, /v1/me/notification-prefs, /v1/devices

use super::{principal::Principal, ApiError, ApiResult, AppState};
use crate::{
    model::*,
    store::users::UserRec,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::collections::HashSet;
use uuid::Uuid;

/// Profile fields only — never item data (rule 8).
pub async fn user_dto(st: &AppState, u: &UserRec, connected: bool) -> ApiResult<UserDto> {
    let role = u.primary_role();
    let hospital = match u.primary_hospital_id {
        Some(h) if role == Role::Doctor || role == Role::Administrator => st.db.user(h).await?.map(|h| h.display_name),
        _ => None,
    };
    let headline = u.headline.clone().unwrap_or_else(|| match (&hospital, role) {
        (Some(h), Role::Doctor) => format!("Doctor · {h}"),
        _ => role.headline().to_string(),
    });
    let mut roles: Vec<Role> = u.roles.iter().copied().collect();
    roles.sort_by_key(|r| if *r == role { 0 } else { 1 }); // primary role first (apps use roles[0])
    Ok(UserDto {
        user_id: u.user_id,
        public_id: u.public_id.clone(),
        display_name: u.display_name.clone(),
        roles,
        headline,
        photo_uri: u.photo_uri.clone(),
        location: u.location.clone(),
        hospital,
        official_number: u.official_number.clone(),
        connected,
    })
}

pub async fn load_user(st: &AppState, id: Uuid) -> ApiResult<UserRec> {
    st.db.user(id).await?.ok_or_else(|| ApiError::not_found("no such user"))
}

pub async fn is_connected(st: &AppState, me: Uuid, other: &UserRec) -> ApiResult<bool> {
    Ok(st.db.is_connected(me, other.user_id, other.primary_role()).await?)
}

pub async fn me(State(st): State<AppState>, p: Principal) -> ApiResult<Json<UserDto>> {
    Ok(Json(user_dto(&st, &p.user, true).await?))
}

#[derive(Deserialize)]
pub struct ProfileUpdate { display_name: String, location: Option<String> }

pub async fn update_me(State(st): State<AppState>, p: Principal, Json(b): Json<ProfileUpdate>) -> ApiResult<Json<UserDto>> {
    let name = b.display_name.trim();
    if name.is_empty() || name.len() > 80 {
        return Err(ApiError::bad_request("display_name must be 1–80 characters"));
    }
    let location = b.location.as_deref().map(str::trim).filter(|s| !s.is_empty());
    st.db.update_profile(&p.user, name, location).await?;
    let u = load_user(&st, p.id()).await?;
    Ok(Json(user_dto(&st, &u, true).await?))
}

pub async fn profile(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<UserDto>> {
    let u = load_user(&st, id).await?;
    let connected = id == p.id() || is_connected(&st, p.id(), &u).await?;
    Ok(Json(user_dto(&st, &u, connected).await?))
}

#[derive(Deserialize)]
pub struct SearchQ { #[serde(default)] q: String, #[serde(rename = "type")] kind: Option<String> }

/// Global search by Healoo ID or name (doc 4.3). Returns profiles only.
pub async fn search(State(st): State<AppState>, p: Principal, Query(q): Query<SearchQ>) -> ApiResult<Json<Page<UserDto>>> {
    let text_q = q.q.trim().to_lowercase();
    let role_filter: Option<Role> = match q.kind.as_deref().map(str::to_uppercase).as_deref() {
        None | Some("") | Some("ALL") => None,
        Some("USER") => Some(Role::Patient),
        Some(other) => parse::<Role>(other),
    };

    let mut found: Vec<UserRec> = Vec::new();
    let upper = text_q.to_uppercase();
    if upper.starts_with("HL-") {
        if let Some(id) = st.db.user_id_by_public_id(&upper).await? {
            found.extend(st.db.user(id).await?);
        }
    } else if !text_q.is_empty() {
        let tokens: Vec<String> = crate::store::users::name_tokens(&text_q).into_iter().collect();
        if let Some(first) = tokens.first() {
            found = st.db.search_by_name(first).await?;
            found.retain(|u| { let n = u.display_name.to_lowercase(); tokens.iter().all(|t| n.contains(t)) });
        }
    } else {
        // Empty query: show the caller's contacts so the screen is never blank.
        for (_, id) in st.db.connection_ids(p.id()).await? { found.extend(st.db.user(id).await?); }
    }

    let connected: HashSet<Uuid> = st.db.connection_ids(p.id()).await?.into_iter().map(|(_, id)| id).collect();
    let mut out = Vec::new();
    for u in found.iter().filter(|u| u.user_id != p.id() && u.active).filter(|u| role_filter.map_or(true, |r| u.primary_role() == r)) {
        out.push(user_dto(&st, u, connected.contains(&u.user_id)).await?);
    }
    Ok(Json(Page::of(out)))
}

pub async fn connections(State(st): State<AppState>, p: Principal) -> ApiResult<Json<Page<UserDto>>> {
    let mut out = Vec::new();
    for (_, id) in st.db.connection_ids(p.id()).await? {
        if let Some(u) = st.db.user(id).await? { out.push(user_dto(&st, &u, true).await?); }
    }
    Ok(Json(Page::of(out)))
}

/// `GET /v1/hospitals/{id}/doctors` — doctors with an active affiliation (profiles only), by name.
pub async fn hospital_doctors(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<Page<UserDto>>> {
    let hospital = load_user(&st, id).await?;
    if hospital.primary_role() != Role::Hospital { return Err(ApiError::bad_request("that user is not a hospital")); }
    let connected: HashSet<Uuid> = st.db.connection_ids(p.id()).await?.into_iter().map(|(_, id)| id).collect();
    let mut out = Vec::new();
    for doctor in st.db.hospital_doctors(id).await? {
        if let Some(u) = st.db.user(doctor).await?.filter(|u| u.active) { out.push(user_dto(&st, &u, connected.contains(&u.user_id)).await?); }
    }
    out.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(Json(Page::of(out)))
}

#[derive(Deserialize)]
pub struct ConnectBody { user_id: Uuid }

/// Add to contacts (rule 8): no grant is created, so no data becomes visible by this alone.
pub async fn connect(State(st): State<AppState>, p: Principal, Json(b): Json<ConnectBody>) -> ApiResult<Json<UserDto>> {
    if b.user_id == p.id() { return Err(ApiError::bad_request("you can't add yourself")); }
    let other = load_user(&st, b.user_id).await?;
    st.db.connect_users(p.id(), p.user.primary_role(), &p.user.display_name, other.user_id, other.primary_role(), &other.display_name).await?;
    Ok(Json(user_dto(&st, &other, true).await?))
}

pub async fn disconnect(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    let other = load_user(&st, id).await?;
    st.db.disconnect_users(p.id(), p.user.primary_role(), other.user_id, other.primary_role()).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_prefs(State(st): State<AppState>, p: Principal) -> ApiResult<Json<NotificationPrefs>> {
    Ok(Json(st.db.prefs(p.id()).await?))
}

pub async fn put_prefs(State(st): State<AppState>, p: Principal, Json(b): Json<NotificationPrefs>) -> ApiResult<Json<NotificationPrefs>> {
    let valid_time = |s: &str| chrono::NaiveTime::parse_from_str(s, "%H:%M").is_ok();
    if !valid_time(&b.quiet_start) || !valid_time(&b.quiet_end) {
        return Err(ApiError::bad_request("quiet_start and quiet_end must be HH:MM"));
    }
    st.db.save_prefs(p.id(), &b).await?;
    Ok(Json(b))
}

#[derive(Deserialize)]
pub struct DeviceBody {
    platform: String,
    #[serde(alias = "fcm_token")]
    token: String,
}

pub async fn add_device(State(st): State<AppState>, p: Principal, Json(b): Json<DeviceBody>) -> ApiResult<StatusCode> {
    if !matches!(b.platform.as_str(), "android" | "ios") || b.token.len() < 20 || b.token.len() > 4096 {
        return Err(ApiError::bad_request("platform must be android or ios, with a valid FCM token"));
    }
    st.db.add_device(p.id(), &b.token, &b.platform).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn remove_device(State(st): State<AppState>, p: Principal, Path(token): Path<String>) -> ApiResult<StatusCode> {
    st.db.remove_device(p.id(), &token).await?;
    Ok(StatusCode::NO_CONTENT)
}
