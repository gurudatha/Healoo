//! `/files/<key>` for the disk store (LAN profile). URLs are issued by the API after the
//! policy check and carry an HMAC signature, method, content type and 10-minute expiry.

use super::{ApiError, ApiResult, AppState};
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Signed {
    m: String,
    exp: i64,
    #[serde(default)]
    ct: String,
    sig: String,
}

pub async fn download(State(st): State<AppState>, Path(key): Path<String>, Query(q): Query<Signed>) -> ApiResult<Response> {
    let disk = st.disk.as_ref().ok_or_else(|| ApiError::not_found("not found"))?;
    if q.m != "GET" || !disk.verify("GET", &key, q.exp, &q.ct, &q.sig) {
        return Err(ApiError::forbidden("link expired or invalid"));
    }
    let path = disk.path(&key).map_err(|_| ApiError::bad_request("bad key"))?;
    let data = tokio::fs::read(&path).await.map_err(|_| ApiError::not_found("file not found"))?;
    let ct = if q.ct.is_empty() { "application/octet-stream".to_string() } else { q.ct };
    Ok(([(header::CONTENT_TYPE, ct), (header::CACHE_CONTROL, "private, max-age=600".to_string())], data).into_response())
}

pub async fn upload(State(st): State<AppState>, Path(key): Path<String>, Query(q): Query<Signed>, headers: HeaderMap, body: Bytes) -> ApiResult<StatusCode> {
    let disk = st.disk.as_ref().ok_or_else(|| ApiError::not_found("not found"))?;
    if q.m != "PUT" || !disk.verify("PUT", &key, q.exp, &q.ct, &q.sig) {
        return Err(ApiError::forbidden("upload link expired or invalid"));
    }
    let sent_ct = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("");
    if !q.ct.is_empty() && !sent_ct.is_empty() && sent_ct.split(';').next().unwrap_or("").trim() != q.ct {
        return Err(ApiError::bad_request("Content-Type does not match the upload link"));
    }
    let max = if q.ct == "application/pdf" { crate::api::items::PDF_MAX } else { crate::api::items::IMAGE_MAX };
    if body.len() as i64 > max { return Err(ApiError::bad_request("file is larger than allowed")); }
    use crate::files::ObjectStore;
    disk.put(&key, body, &q.ct).await?;
    Ok(StatusCode::OK)
}
