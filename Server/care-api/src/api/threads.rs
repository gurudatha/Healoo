//! /v1/threads, /v1/threads/{id}/messages

use super::{principal::Principal, users::{load_user, user_dto}, ApiError, ApiResult, AppState};
use crate::{
    events::{topics, Envelope},
    model::*,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

pub const MAX_BODY_CHARS: usize = 4000;

#[derive(Deserialize)]
pub struct ThreadsQ { with: Option<Uuid> }

/// All threads, or the thread with one user (created on first use if the two are connected).
pub async fn list(State(st): State<AppState>, p: Principal, Query(q): Query<ThreadsQ>) -> ApiResult<Json<Page<ThreadDto>>> {
    let rows = match q.with {
        Some(other_id) => {
            let other = load_user(&st, other_id).await?;
            match st.db.thread_with(p.id(), other_id).await? {
                Some(t) => vec![t],
                None if st.db.is_connected(p.id(), other_id, other.primary_role()).await? => {
                    st.db.create_thread(p.id(), other_id).await?;
                    st.db.thread_with(p.id(), other_id).await?.into_iter().collect()
                }
                None => vec![],
            }
        }
        None => st.db.threads_of(p.id()).await?,
    };
    let mut out = Vec::new();
    for t in rows {
        let Some(other) = st.db.user(t.other_user_id).await? else { continue };
        out.push(ThreadDto {
            thread_id: t.thread_id,
            other_user: user_dto(&st, &other, true).await?,
            last_message: t.last_message,
            last_message_at: t.last_message_at.map(|x| ts_to_dt(x).to_rfc3339()).unwrap_or_default(),
            unread: t.unread,
        });
    }
    Ok(Json(Page::of(out)))
}

/// The other member of a thread, or 403 if the caller is not in it.
pub async fn other_member(st: &AppState, me: Uuid, thread: Uuid) -> ApiResult<Uuid> {
    let (a, b) = st.db.thread_members(thread).await?.ok_or_else(|| ApiError::not_found("no such thread"))?;
    if a == me { Ok(b) } else if b == me { Ok(a) } else { Err(ApiError::forbidden("you are not part of this conversation")) }
}

pub async fn messages(State(st): State<AppState>, p: Principal, Path(thread): Path<Uuid>) -> ApiResult<Json<Page<MessageDto>>> {
    let other = other_member(&st, p.id(), thread).await?;
    let list = st.db.messages(thread, 100).await?;
    st.db.mark_read(p.id(), other, thread).await?;
    Ok(Json(Page::of(list)))
}

#[derive(Deserialize)]
pub struct SendBody {
    body: String,
    client_msg_id: String,
    linked_item_id: Option<Uuid>,
}

pub async fn send(State(st): State<AppState>, p: Principal, Path(thread): Path<Uuid>, Json(b): Json<SendBody>) -> ApiResult<Json<MessageDto>> {
    Ok(Json(send_message(&st, &p, thread, &b.body, &b.client_msg_id, b.linked_item_id).await?))
}

/// Shared by REST and the WebSocket `message.send` frame. Idempotent on `client_msg_id`.
pub async fn send_message(st: &AppState, p: &Principal, thread: Uuid, body: &str, client_msg_id: &str, linked: Option<Uuid>) -> ApiResult<MessageDto> {
    let body = body.trim();
    if body.is_empty() || body.chars().count() > MAX_BODY_CHARS {
        return Err(ApiError::bad_request(format!("message must be 1–{MAX_BODY_CHARS} characters")));
    }
    if client_msg_id.is_empty() || client_msg_id.len() > 64 {
        return Err(ApiError::bad_request("client_msg_id is required (max 64 chars)"));
    }
    let other = other_member(st, p.id(), thread).await?;

    if let Some(existing) = st.db.existing_message(thread, client_msg_id).await? {
        if let Some(m) = st.db.message(thread, existing).await? { return Ok(m); }
    }

    let id = new_timeuuid();
    let m = MessageDto {
        message_id: id,
        thread_id: thread,
        sender_id: p.id(),
        body: body.to_string(),
        sent_at: ts_to_dt(scylla::frame::value::CqlTimestamp(uuid_millis(&id))).to_rfc3339(),
        linked_item_id: linked,
    };
    st.db.insert_message(&m, client_msg_id).await?;

    let preview: String = body.chars().take(120).collect();
    let other_unread = st.db.thread_with(other, p.id()).await?.map(|t| t.unread).unwrap_or(0) + 1;
    st.db.touch_thread(p.id(), other, thread, &preview, 0).await?;
    st.db.touch_thread(other, p.id(), thread, &preview, other_unread).await?;

    let ev = Envelope::new("message.created", p.id(), thread, vec![other.to_string(), p.id().to_string()], json!({ "message": m }));
    st.events.emit(topics::CHAT, &thread.to_string(), ev).await?;
    Ok(m)
}
