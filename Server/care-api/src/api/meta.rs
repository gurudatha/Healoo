//! /healthz, /readyz, /v1/config, and the LAN-only /dev/* helpers.

use super::{ApiError, ApiResult, AppState};
use crate::{config::AuthMode, model::text};
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

pub async fn healthz() -> &'static str { "ok" }

pub async fn readyz(State(st): State<AppState>) -> (StatusCode, &'static str) {
    if st.db.ping().await { (StatusCode::OK, "ready") } else { (StatusCode::SERVICE_UNAVAILABLE, "cassandra unavailable") }
}

/// Unauthenticated: tells an app how to reach and sign in to this server.
pub async fn config(State(st): State<AppState>) -> Json<Value> {
    let auth = match st.cfg.auth_mode {
        AuthMode::Auth0 => json!({ "mode": "auth0", "domain": st.cfg.auth0_domain, "audience": st.cfg.auth0_audience }),
        AuthMode::Dev => json!({ "mode": "dev", "token_endpoint": st.exposure.public_base().join("dev/token").map(|u| u.to_string()).ok() }),
    };
    Json(json!({
        "network_mode": format!("{:?}", st.exposure.mode()).to_lowercase(),
        "api_base": st.exposure.public_base().join("v1/").map(|u| u.to_string()).ok(),
        "ws_url": st.exposure.ws_url().to_string(),
        "auth": auth,
        "limits": {
            "attachments_per_item": crate::api::items::MAX_ATTACHMENTS,
            "image_max_bytes": crate::api::items::IMAGE_MAX,
            "pdf_max_bytes": crate::api::items::PDF_MAX,
            "ws_frame_bytes": crate::ws::MAX_FRAME,
        },
    }))
}

#[derive(Deserialize)]
pub struct DevTokenReq { public_id: String }

/// LAN + AUTH_MODE=dev only (see router): a 12-hour token for a seeded user, e.g.
/// `curl -k -X POST https://<lan-ip>:8443/dev/token -d '{"public_id":"HL-2M9P4"}' -H 'content-type: application/json'`
pub async fn dev_token(State(st): State<AppState>, Json(b): Json<DevTokenReq>) -> ApiResult<Json<Value>> {
    let dev = st.dev.as_ref().ok_or_else(|| ApiError::not_found("dev tokens are disabled"))?;
    let id = st.db.user_id_by_public_id(&b.public_id).await?.ok_or_else(|| ApiError::not_found("no user with that Healoo ID"))?;
    let u = st.db.user(id).await?.ok_or_else(|| ApiError::not_found("no such user"))?;
    let roles: Vec<String> = u.roles.iter().map(|r| text(r).to_lowercase()).collect();
    let role_refs: Vec<&str> = roles.iter().map(String::as_str).collect();
    let token = dev.mint(&format!("dev|{}", u.public_id), &u.display_name, &role_refs, Some(u.user_id), Duration::from_secs(12 * 3600));
    Ok(Json(json!({ "access_token": token, "token_type": "Bearer", "expires_in": 43200, "user_id": u.user_id })))
}

/// LAN + dev only: the seeded accounts, so testers know which Healoo IDs to use.
pub async fn dev_users(State(st): State<AppState>) -> ApiResult<Json<Value>> {
    let mut out = Vec::new();
    for pid in crate::seed::SEED_PUBLIC_IDS {
        if let Some(id) = st.db.user_id_by_public_id(pid).await? {
            if let Some(u) = st.db.user(id).await? {
                out.push(json!({ "public_id": u.public_id, "display_name": u.display_name, "role": u.primary_role() }));
            }
        }
    }
    Ok(Json(json!({ "data": out })))
}
