//! REST API (design doc 4.3). All responses are JSON; errors use
//! `{"error": {"code", "message", "request_id"}}` with 400/401/403/404/409/429.

pub mod admin;
pub mod files;
pub mod grants;
pub mod items;
pub mod meta;
pub mod parts;
pub mod principal;
pub mod users;

use crate::{
    auth::{DevVerifier, TokenVerifier},
    config::{AuthMode, Config, NetworkMode},
    events::Publisher,
    files::{DiskStore, ObjectStore},
    net::Exposure,
    store::Db,
    ws,
};
use axum::{
    extract::DefaultBodyLimit,
    http::{HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
    Json, Router,
};
use dashmap::DashMap;
use serde_json::json;
use std::{ops::Deref, sync::Arc, time::Instant};
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
use uuid::Uuid;

// ---------------- state ----------------

pub struct Inner {
    pub cfg: Config,
    pub db: Arc<Db>,
    pub verifier: Arc<dyn TokenVerifier>,
    pub dev: Option<Arc<DevVerifier>>,
    pub exposure: Arc<dyn Exposure>,
    pub files: Arc<dyn ObjectStore>,
    pub disk: Option<Arc<DiskStore>>,
    pub events: Publisher,
    pub ws: Arc<ws::Registry>,
    pub(crate) principals: DashMap<String, principal::Cached>,
    pub(crate) limiter: DashMap<Uuid, (Instant, f64)>,
}

#[derive(Clone)]
pub struct AppState(pub Arc<Inner>);

impl Deref for AppState {
    type Target = Inner;
    fn deref(&self) -> &Inner { &self.0 }
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cfg: Config, db: Arc<Db>, verifier: Arc<dyn TokenVerifier>, dev: Option<Arc<DevVerifier>>, exposure: Arc<dyn Exposure>,
        files: Arc<dyn ObjectStore>, disk: Option<Arc<DiskStore>>, events: Publisher, ws: Arc<ws::Registry>,
    ) -> Self {
        AppState(Arc::new(Inner {
            cfg, db, verifier, dev, exposure, files, disk, events, ws,
            principals: DashMap::new(), limiter: DashMap::new(),
        }))
    }
}

// ---------------- errors ----------------

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self { Self { status, code, message: message.into() } }
    pub fn bad_request(m: impl Into<String>) -> Self { Self::new(StatusCode::BAD_REQUEST, "BAD_REQUEST", m) }
    pub fn unauthorized(m: impl Into<String>) -> Self { Self::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", m) }
    pub fn forbidden(m: impl Into<String>) -> Self { Self::new(StatusCode::FORBIDDEN, "ACCESS_DENIED", m) }
    pub fn not_found(m: impl Into<String>) -> Self { Self::new(StatusCode::NOT_FOUND, "NOT_FOUND", m) }
    pub fn conflict(m: impl Into<String>) -> Self { Self::new(StatusCode::CONFLICT, "CONFLICT", m) }
    pub fn too_many() -> Self { Self::new(StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED", "Too many requests; slow down") }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!("internal error: {e:#}");
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Something went wrong on the server")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let request_id = ulid::Ulid::new().to_string();
        if self.status.is_server_error() {
            tracing::error!(%request_id, code = self.code, "{}", self.message);
        }
        (self.status, Json(json!({ "error": { "code": self.code, "message": self.message, "request_id": request_id } }))).into_response()
    }
}

// ---------------- router ----------------

pub const MAX_BODY: usize = 20 * 1024 * 1024; // doc 4.5
pub const MAX_FILE_BODY: usize = 26 * 1024 * 1024; // PDFs up to 25 MB through /files (disk store)

pub fn router(state: AppState) -> Router {
    let cfg = &state.cfg;

    let mut v1 = Router::new()
        .route("/me", get(users::me).patch(users::update_me))
        .route("/me/notification-prefs", get(users::get_prefs).put(users::put_prefs))
        .route("/dashboard", get(items::dashboard))
        .route("/search", get(users::search))
        .route("/connections", get(users::connections).post(users::connect))
        .route("/connections/:id", delete(users::disconnect))
        .route("/users/:id", get(users::profile))
        .route("/users/:id/shared-items", get(items::shared_items))
        .route("/hospitals/:id/doctors", get(users::hospital_doctors))
        .route("/admin/users", post(admin::create_user))
        .route("/admin/users/:id", delete(admin::delete_user))
        .route("/admin/doctors", get(admin::doctors).post(admin::create_doctor))
        .route("/admin/doctors/:id", delete(admin::deactivate_doctor))
        .route("/admin/doctors/:id/reactivate", post(admin::reactivate_doctor))
        // DataItem v2 (Documentation/DataItem_Design.md section 4)
        .route("/items", get(items::list).post(items::create))
        .route("/items/:id", get(items::get_one).patch(items::patch))
        .route("/items/:id/close", post(items::close))
        .route("/items/:id/reopen", post(items::reopen))
        .route("/items/:id/messages", get(parts::messages_get).post(parts::messages_post))
        .route("/items/:id/attachments", get(parts::attachments_get).post(parts::attachments_post))
        .route("/items/:id/appointments", get(parts::appointments_get).post(parts::appointments_post))
        .route("/items/:id/appointments/:aid", patch(parts::appointment_patch))
        .route("/items/:id/appointments/:aid/visits/:date", post(parts::visit_action))
        .route("/items/:id/alerts", get(parts::alerts_get).post(parts::alerts_post))
        .route("/items/:id/alerts/:alert_id", delete(parts::alerts_delete))
        .route("/appointments", get(parts::calendar))
        .route("/conversations", get(parts::conversations))
        .route("/uploads/presign", post(items::presign))
        .route("/grants", get(grants::list).post(grants::create))
        .route("/grants/:id", delete(grants::revoke))
        .route("/affiliations/:doctor_id/end", post(grants::end_affiliation))
        .route("/delegations", post(grants::delegate))
        .route("/devices", post(users::add_device))
        .route("/devices/:token", delete(users::remove_device))
        .route("/ws", get(ws::upgrade))
        .route("/config", get(meta::config));
    if cfg.debug_routes {
        v1 = v1.route("/check/access", get(grants::check_access));
    }

    let mut app = Router::new()
        .nest("/v1", v1)
        .route("/ws", get(ws::upgrade)) // doc 4.4 path; /v1/ws is what the apps use
        .route("/healthz", get(meta::healthz))
        .route("/readyz", get(meta::readyz))
        .layer(DefaultBodyLimit::max(MAX_BODY));

    // Disk store: the API itself serves the signed /files URLs, with a larger body limit.
    if state.disk.is_some() {
        app = app.merge(
            Router::new()
                .route("/files/*key", get(files::download).put(files::upload))
                .layer(DefaultBodyLimit::max(MAX_FILE_BODY)),
        );
    }

    // Offline LAN trial: hand out dev tokens for seeded users. Never mounted on the internet.
    if cfg.mode == NetworkMode::Lan && cfg.auth_mode == AuthMode::Dev {
        app = app.route("/dev/token", post(meta::dev_token)).route("/dev/users", get(meta::dev_users));
    }

    let request_id = HeaderName::from_static("x-request-id");
    app.layer(state.exposure.cors())
        .layer(SetResponseHeaderLayer::overriding(HeaderName::from_static("x-content-type-options"), HeaderValue::from_static("nosniff")))
        .layer(PropagateRequestIdLayer::new(request_id.clone()))
        .layer(TraceLayer::new_for_http())
        .layer(SetRequestIdLayer::new(request_id, MakeRequestUuid))
        .with_state(state)
}
