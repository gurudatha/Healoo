//! Network exposure abstraction.
//!
//! Everything that differs between "phones on the same Wi-Fi" and "deployed on the internet"
//! sits behind [`Exposure`]. Handlers never look at the mode; they ask the exposure for the
//! public URL, the real client IP and so on. Two implementations:
//!
//! * [`lan::LanExposure`] — binds on all interfaces, serves HTTPS with its own local CA (or sits
//!   behind Caddy on the same machine), advertises itself over mDNS, permissive CORS.
//! * [`internet::InternetExposure`] — binds inside the private network behind Caddy/Nginx,
//!   trusts `X-Forwarded-For` only from configured proxy ranges, strict CORS, HTTPS-only URLs.

pub mod internet;
pub mod lan;
pub mod tls;

use crate::config::{Config, NetworkMode};
use async_trait::async_trait;
use axum::{http::HeaderMap, Router};
use ipnet::IpNet;
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;
use url::Url;

#[async_trait]
pub trait Exposure: Send + Sync + 'static {
    fn mode(&self) -> NetworkMode;

    /// Base URL phones use to reach this API, always ending in '/'.
    fn public_base(&self) -> Url;

    /// WebSocket URL advertised to clients (same host, ws/wss scheme).
    fn ws_url(&self) -> Url {
        let mut u = self.public_base().join("v1/ws").expect("valid base url");
        let scheme = if u.scheme() == "https" { "wss" } else { "ws" };
        let _ = u.set_scheme(scheme);
        u
    }

    /// The real client address, taking proxies into account only where the profile allows it.
    fn client_ip(&self, peer: SocketAddr, headers: &HeaderMap) -> IpAddr;

    fn cors(&self) -> CorsLayer;

    /// Run the server until `shutdown` is cancelled.
    async fn serve(&self, app: Router, shutdown: CancellationToken) -> anyhow::Result<()>;
}

pub async fn build(cfg: &Config) -> anyhow::Result<Arc<dyn Exposure>> {
    Ok(match cfg.mode {
        NetworkMode::Lan => Arc::new(lan::LanExposure::new(cfg)?),
        NetworkMode::Internet => Arc::new(internet::InternetExposure::new(cfg)?),
    })
}

/// Right-most `X-Forwarded-For` hop that is not one of our proxies. Only consulted when the
/// socket peer itself is a trusted proxy, so clients cannot spoof their address.
pub fn forwarded_ip(peer: SocketAddr, headers: &HeaderMap, trusted: &[IpNet]) -> IpAddr {
    let is_trusted = |ip: &IpAddr| trusted.iter().any(|n| n.contains(ip));
    if !is_trusted(&peer.ip()) {
        return peer.ip();
    }
    let hops: Vec<IpAddr> = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    hops.into_iter().rev().find(|ip| !is_trusted(ip)).unwrap_or(peer.ip())
}
