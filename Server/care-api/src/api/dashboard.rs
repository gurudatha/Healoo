//! The platform administrator's web dashboard (Administration_Design.md section 8), served at
//! `/admin/`. Three static files built into the binary, plus `/admin/config.json`, which tells
//! the page how to sign in. All data comes from the `/v1/admin/*` API with the signed-in
//! administrator's token; the pages themselves hold nothing secret.

use super::AppState;
use crate::config::{AuthMode, NetworkMode};
use axum::{
    extract::State,
    http::{header, HeaderValue},
    response::{IntoResponse, Redirect, Response},
    routing::get,
    Json, Router,
};
use serde_json::json;

const INDEX: &str = include_str!("../dashboard/index.html");
const APP_JS: &str = include_str!("../dashboard/app.js");
const STYLE: &str = include_str!("../dashboard/style.css");

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin", get(|| async { Redirect::permanent("/admin/") }))
        .route("/admin/", get(index))
        .route("/admin/app.js", get(|| async { file(APP_JS, "text/javascript; charset=utf-8") }))
        .route("/admin/style.css", get(|| async { file(STYLE, "text/css; charset=utf-8") }))
        .route("/admin/config.json", get(config))
}

fn file(body: &'static str, content_type: &'static str) -> Response {
    ([(header::CONTENT_TYPE, content_type), (header::CACHE_CONTROL, "no-cache")], body).into_response()
}

/// The page may talk to this server and, for sign-in, to the Auth0 tenant; nothing else.
async fn index(State(st): State<AppState>) -> Response {
    let auth0 = if st.cfg.auth_mode == AuthMode::Auth0 { format!(" https://{}", st.cfg.auth0_domain) } else { String::new() };
    let csp = format!(
        "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'{auth0}; \
         frame-ancestors 'none'; base-uri 'none'; form-action 'self'{auth0}"
    );
    let mut r = file(INDEX, "text/html; charset=utf-8");
    if let Ok(v) = HeaderValue::from_str(&csp) { r.headers_mut().insert(header::CONTENT_SECURITY_POLICY, v); }
    r.headers_mut().insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    r
}

/// How the page signs in: Auth0 (Single Page Application with PKCE), or, on a LAN trial with
/// AUTH_MODE=dev, the trial server's test accounts.
async fn config(State(st): State<AppState>) -> impl IntoResponse {
    let cfg = &st.cfg;
    let dev = cfg.mode == NetworkMode::Lan && cfg.auth_mode == AuthMode::Dev;
    Json(json!({
        "auth_mode": if dev { "dev" } else { "auth0" },
        "auth0_domain": cfg.auth0_domain.trim_start_matches("https://").trim_end_matches('/'),
        "client_id": cfg.admin_client_id,
        "audience": cfg.auth0_audience,
    }))
}
