//! Internet profile: the API sits on a private network behind Caddy or Nginx (doc 9), which
//! terminate TLS with a public certificate. The API never listens on a public interface itself.

use super::{forwarded_ip, Exposure};
use crate::config::{Config, NetworkMode};
use async_trait::async_trait;
use axum::{
    http::{header, HeaderMap, HeaderValue, Method},
    Router,
};
use ipnet::IpNet;
use std::net::{IpAddr, SocketAddr};
use tokio_util::sync::CancellationToken;
use tower_http::cors::{AllowOrigin, CorsLayer};
use url::Url;

pub struct InternetExposure {
    bind: SocketAddr,
    base: Url,
    trusted: Vec<IpNet>,
    origins: Vec<HeaderValue>,
}

impl InternetExposure {
    pub fn new(cfg: &Config) -> anyhow::Result<Self> {
        let base = cfg.public_base_url.clone().expect("validated in Config");
        if cfg.bind.ip().is_unspecified() {
            tracing::info!("bound on all interfaces; make sure only the reverse proxy can reach port {}", cfg.bind.port());
        }
        let origins = cfg.cors_origins.iter().filter_map(|o| HeaderValue::from_str(o).ok()).collect();
        tracing::info!(%base, "internet profile: public URL {base}");
        Ok(Self { bind: cfg.bind, base, trusted: cfg.trusted_proxies.clone(), origins })
    }
}

#[async_trait]
impl Exposure for InternetExposure {
    fn mode(&self) -> NetworkMode {
        NetworkMode::Internet
    }

    fn public_base(&self) -> Url {
        self.base.clone()
    }

    fn client_ip(&self, peer: SocketAddr, headers: &HeaderMap) -> IpAddr {
        forwarded_ip(peer, headers, &self.trusted)
    }

    /// Native apps don't need CORS; only listed web origins (e.g. an admin console) get it.
    fn cors(&self) -> CorsLayer {
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(self.origins.clone()))
            .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::PUT, Method::DELETE])
            .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
    }

    async fn serve(&self, app: Router, shutdown: CancellationToken) -> anyhow::Result<()> {
        let listener = tokio::net::TcpListener::bind(self.bind).await?;
        tracing::info!(bind = %self.bind, "listening (HTTP behind reverse proxy)");
        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
            .with_graceful_shutdown(async move { shutdown.cancelled().await })
            .await?;
        Ok(())
    }
}
