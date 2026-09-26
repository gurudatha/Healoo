//! Healoo backend (`care-api`): design doc sections 4 (REST + WebSocket), 5 (Auth0),
//! 6 (Kafka) and 7 (Cassandra), with one abstraction per concern that differs between a
//! local-network trial and an internet deployment:
//!
//! | Concern   | Trait                         | LAN trial                    | Internet                    |
//! |-----------|-------------------------------|------------------------------|-----------------------------|
//! | Network   | [`net::Exposure`]             | `LanExposure`                | `InternetExposure`          |
//! | Identity  | [`auth::TokenVerifier`]       | `DevVerifier` (or Auth0)     | `Auth0Verifier`             |
//! | Files     | [`files::ObjectStore`]        | `DiskStore`                  | `S3Store`                   |
//! | Events    | [`events::EventBus`]          | `KafkaBus` (or `MemoryBus`)  | `KafkaBus`                  |
//!
//! Handlers only see the traits, so the same code serves both profiles.

pub mod api;
pub mod auth;
pub mod config;
pub mod events;
pub mod files;
pub mod model;
pub mod net;
pub mod policy;
pub mod recurrence;
pub mod seed;
pub mod store;
pub mod worker;
pub mod ws;

/// Logging: JSON lines on the internet (for log shipping), readable text on the LAN.
pub fn init_tracing(json: bool) {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,care_api=debug,tower_http=info"));
    if json {
        fmt().with_env_filter(filter).json().init();
    } else {
        fmt().with_env_filter(filter).compact().init();
    }
}

/// Resolves on Ctrl-C or SIGTERM (docker stop).
pub async fn shutdown_signal() {
    let ctrl_c = async { let _ = tokio::signal::ctrl_c().await; };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) { s.recv().await; }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
}
