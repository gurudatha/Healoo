//! WebSocket gateway (design doc 4.4).
//!
//! * Auth: `Authorization: Bearer` on the upgrade (apps), `?token=` (doc), or a first frame
//!   `{"type":"auth","token":"…"}` within 10 s. Closed with 4001 when the token expires.
//! * Frames in:  subscribe, message.send, typing, ack, ping, pong.
//! * Frames out: message.new {item_id, message}, item.updated {item_id, change}, alert,
//!   typing {item_id}, message.sent, error, ping. (DataItem v2: messages belong to items.)
//! * Server pings every 25 s and drops the socket after 60 s of silence; max 5 sockets per user.
//! * [`Fanout`] consumes Kafka and pushes to connected sessions; offline users get a
//!   `notify.requests` event for the push notifier (doc 6.4 / 8).

use crate::{
    api::{parts::send_message_to, principal::{bearer, Principal}, AppState},
    events::{topics, Envelope, EventHandler, Publisher},
    store::Db,
};
use async_trait::async_trait;
use axum::{
    extract::{
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::HeaderMap,
    response::Response,
};
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    borrow::Cow,
    sync::{atomic::{AtomicU64, Ordering}, Arc},
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use uuid::Uuid;

pub const MAX_FRAME: usize = 64 * 1024;
pub const MAX_SOCKETS_PER_USER: usize = 5;
const PING_EVERY: Duration = Duration::from_secs(25);
const SILENCE_LIMIT: Duration = Duration::from_secs(60);

// ---------------- registry ----------------

#[derive(Default)]
pub struct Registry {
    sessions: DashMap<Uuid, Vec<(u64, mpsc::Sender<String>)>>,
    next: AtomicU64,
}

impl Registry {
    pub fn new() -> Self { Self::default() }

    fn register(&self, user: Uuid) -> Option<(u64, mpsc::Receiver<String>)> {
        let mut entry = self.sessions.entry(user).or_default();
        entry.retain(|(_, tx)| !tx.is_closed());
        if entry.len() >= MAX_SOCKETS_PER_USER { return None; }
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel(256);
        entry.push((id, tx));
        Some((id, rx))
    }

    fn unregister(&self, user: Uuid, id: u64) {
        if let Some(mut v) = self.sessions.get_mut(&user) { v.retain(|(i, _)| *i != id); }
        self.sessions.remove_if(&user, |_, v| v.is_empty());
    }

    pub fn is_online(&self, user: &Uuid) -> bool {
        self.sessions.get(user).map(|v| v.iter().any(|(_, tx)| !tx.is_closed())).unwrap_or(false)
    }

    /// Sends to every socket of `user`; returns true if at least one received it.
    pub fn send(&self, user: &Uuid, frame: &str) -> bool {
        let Some(v) = self.sessions.get(user) else { return false };
        v.iter().fold(false, |sent, (_, tx)| tx.try_send(frame.to_string()).is_ok() || sent)
    }

    pub fn online_count(&self) -> usize { self.sessions.len() }
}

// ---------------- upgrade + session ----------------

#[derive(Deserialize)]
pub struct WsQuery { token: Option<String> }

pub async fn upgrade(State(st): State<AppState>, headers: HeaderMap, Query(q): Query<WsQuery>, ws: WebSocketUpgrade) -> Response {
    let token = bearer(&headers).or(q.token);
    ws.max_message_size(MAX_FRAME).max_frame_size(MAX_FRAME).on_upgrade(move |socket| session(st, socket, token))
}

#[derive(Deserialize)]
struct Inbound {
    #[serde(rename = "type")]
    kind: String,
    token: Option<String>,
    item_id: Option<Uuid>,
    body: Option<String>,
    client_msg_id: Option<String>,
}

async fn close(mut socket: WebSocket, code: u16, reason: &'static str) {
    let _ = socket.send(Message::Close(Some(CloseFrame { code, reason: Cow::Borrowed(reason) }))).await;
}

async fn session(st: AppState, mut socket: WebSocket, token: Option<String>) {
    // 1. Authenticate: upgrade header / query, or the first frame.
    let token = match token {
        Some(t) => Some(t),
        None => match tokio::time::timeout(Duration::from_secs(10), socket.recv()).await {
            Ok(Some(Ok(Message::Text(t)))) => serde_json::from_str::<Inbound>(&t).ok().filter(|f| f.kind == "auth").and_then(|f| f.token),
            _ => None,
        },
    };
    let Some(token) = token else { return close(socket, 4001, "authentication required").await };
    let principal = match st.authenticate(&token).await {
        Ok(p) => p,
        Err(_) => return close(socket, 4001, "invalid or expired token").await,
    };
    let user = principal.id();
    let Some((conn_id, mut outbox)) = st.ws.register(user) else {
        return close(socket, 4008, "too many connections").await;
    };
    tracing::debug!(%user, conn_id, "ws connected");

    let (mut tx, mut rx) = socket.split();
    let mut last_seen = Instant::now();
    let mut ping = tokio::time::interval(PING_EVERY);
    let expires_in = (principal.token_exp - chrono::Utc::now().timestamp()).max(0) as u64;
    let token_expiry = tokio::time::sleep(Duration::from_secs(expires_in));
    tokio::pin!(token_expiry);

    loop {
        tokio::select! {
            out = outbox.recv() => match out {
                Some(frame) => if tx.send(Message::Text(frame)).await.is_err() { break },
                None => break,
            },
            inbound = rx.next() => {
                let Some(Ok(msg)) = inbound else { break };
                last_seen = Instant::now();
                match msg {
                    Message::Text(t) => {
                        if let Some(reply) = handle_frame(&st, &principal, &t).await {
                            if tx.send(Message::Text(reply)).await.is_err() { break }
                        }
                    }
                    Message::Close(_) => break,
                    _ => {} // binary ignored; protocol pings handled by axum
                }
            },
            _ = ping.tick() => {
                if last_seen.elapsed() > SILENCE_LIMIT { break }
                if tx.send(Message::Text(r#"{"type":"ping"}"#.into())).await.is_err() { break }
            },
            _ = &mut token_expiry => {
                // Doc 5.5: the client reconnects with a fresh token.
                let _ = tx.send(Message::Close(Some(CloseFrame { code: 4001, reason: Cow::Borrowed("token expired") }))).await;
                break;
            },
        }
    }
    st.ws.unregister(user, conn_id);
    tracing::debug!(%user, conn_id, "ws disconnected");
}

fn error_frame(code: &str, message: &str) -> String {
    json!({ "type": "error", "error": { "code": code, "message": message } }).to_string()
}

async fn handle_frame(st: &AppState, p: &Principal, text: &str) -> Option<String> {
    let f: Inbound = match serde_json::from_str(text) {
        Ok(f) => f,
        Err(_) => return Some(error_frame("BAD_FRAME", "frames must be JSON with a type")),
    };
    match f.kind.as_str() {
        "ping" => Some(r#"{"type":"pong"}"#.into()),
        "pong" | "ack" => None,
        "subscribe" => Some(json!({ "type": "subscribed" }).to_string()), // user channel is implicit
        "message.send" => {
            let (Some(item), Some(body)) = (f.item_id, f.body) else {
                return Some(error_frame("BAD_FRAME", "message.send needs item_id and body"));
            };
            let client_id = f.client_msg_id.unwrap_or_else(|| ulid::Ulid::new().to_string());
            match send_message_to(st, p, item, &body, &client_id).await {
                Ok(m) => Some(json!({ "type": "message.sent", "client_msg_id": client_id, "message": m }).to_string()),
                Err(e) => Some(error_frame(e.code, &e.message)),
            }
        }
        "typing" => {
            let item_id = f.item_id?;
            // Typing is ephemeral: relay directly to the item's participants, no Kafka.
            if let Ok(Some(item)) = st.db.item(item_id).await {
                if item.participants().contains(&p.id()) {
                    let frame = json!({ "type": "typing", "item_id": item_id, "user_id": p.id() }).to_string();
                    for u in item.participants().into_iter().filter(|u| *u != p.id()) { st.ws.send(&u, &frame); }
                }
            }
            None
        }
        _ => Some(error_frame("UNKNOWN_TYPE", "unsupported frame type")),
    }
}

// ---------------- Kafka → sockets ----------------

/// Consumer group `ws-fanout` (doc 6.4). Each API instance uses its own group id so every
/// instance sees every event and delivers to its own sockets.
pub struct Fanout {
    pub db: Arc<Db>,
    pub registry: Arc<Registry>,
    pub publisher: Publisher,
}

impl Fanout {
    /// Expands "hospital:<id>" into the doctors currently affiliated with it.
    async fn recipients(&self, audience: &[String]) -> Vec<Uuid> {
        let mut out = Vec::new();
        for a in audience {
            if let Some(h) = a.strip_prefix("hospital:") {
                if let Ok(h) = h.parse::<Uuid>() {
                    out.extend(self.db.hospital_doctors(h).await.unwrap_or_default());
                }
            } else if let Ok(u) = a.parse::<Uuid>() {
                out.push(u);
            }
        }
        out.sort();
        out.dedup();
        out
    }

    fn frame_for(topic: &str, ev: &Envelope) -> Option<(String, &'static str)> {
        match (topic, ev.event_type.as_str()) {
            (topics::CHAT, "message.created") => {
                let msg = ev.payload.get("message").cloned().unwrap_or(Value::Null);
                Some((json!({ "type": "message.new", "item_id": ev.subject_id, "message": msg }).to_string(), "message"))
            }
            (topics::ALERTS, _) => Some((json!({
                "type": "alert", "item_id": ev.subject_id,
                "alert_id": ev.payload.get("alert_id"), "text": ev.payload.get("text"),
            }).to_string(), "alert")),
            // The apps refresh on `item.updated`; `change` says what happened.
            (topics::ITEMS, "item.attachments_added") => None, // media worker only; item.attachment follows
            (topics::ITEMS, t) | (topics::ACCESS_CHANGES, t) if t.starts_with("item.") || t.starts_with("grant.") => {
                let change = match t {
                    "item.created" => "created".to_string(),
                    "grant.added" | "grant.revoked" => "shared".to_string(),
                    other => other.trim_start_matches("item.").to_string(),
                };
                let kind = if t == "item.created" { "report" } else if t == "grant.added" { "item" } else { "" };
                Some((json!({ "type": "item.updated", "item_id": ev.subject_id, "change": change }).to_string(), kind))
            }
            _ => None,
        }
    }
}

#[async_trait]
impl EventHandler for Fanout {
    async fn handle(&self, topic: &str, ev: Envelope) -> anyhow::Result<()> {
        let Some((frame, push_kind)) = Self::frame_for(topic, &ev) else { return Ok(()) };
        for user in self.recipients(&ev.audience).await {
            let delivered = self.registry.send(&user, &frame);
            // Offline and not the actor: ask the notifier for a push (doc 8.4). Payload has ids only.
            if !delivered && user != ev.actor_id && !push_kind.is_empty() && !self.registry.is_online(&user) {
                let req = Envelope::new("notify.requested", ev.actor_id, &ev.subject_id, vec![user.to_string()], json!({
                    "user_id": user, "type": push_kind, "source_event": ev.event_id, "event_type": ev.event_type,
                    "item_id": Value::from(ev.subject_id.clone()),
                    "thread_id": Value::Null,
                    "user_to_open": ev.actor_id,
                }));
                self.publisher.emit_fast(topics::NOTIFY, &user.to_string(), req).await;
            }
        }
        Ok(())
    }
}
