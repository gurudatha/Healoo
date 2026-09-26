//! LAN profile: phones and emulators on the same network talk straight to this machine.

use super::{forwarded_ip, tls, Exposure};
use crate::config::{Config, LanTls, NetworkMode};
use anyhow::Context;
use async_trait::async_trait;
use axum::{http::HeaderMap, Router};
use ipnet::IpNet;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    time::Duration,
};
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;
use url::Url;

pub struct LanExposure {
    bind: SocketAddr,
    tls: LanTls,
    tls_dir: PathBuf,
    lan_ip: IpAddr,
    base: Url,
    trusted: Vec<IpNet>,
    _mdns: Option<ServiceDaemon>,
}

impl LanExposure {
    pub fn new(cfg: &Config) -> anyhow::Result<Self> {
        // Inside Docker, auto-detection would find the container address; an IP in
        // PUBLIC_BASE_URL (the host's LAN address) wins, and is also put in the certificate.
        let from_url = cfg.public_base_url.as_ref().and_then(|u| match u.host() {
            Some(url::Host::Ipv4(ip)) => Some(IpAddr::V4(ip)),
            Some(url::Host::Ipv6(ip)) => Some(IpAddr::V6(ip)),
            _ => None,
        });
        let lan_ip = from_url
            .or_else(|| local_ip_address::local_ip().ok())
            .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));

        // Proxy mode: Caddy/Nginx listens on 443 of this machine; otherwise we are the endpoint.
        let base = match &cfg.public_base_url {
            Some(u) => u.clone(),
            None => {
                let s = match cfg.lan_tls {
                    LanTls::Builtin => format!("https://{}:{}/", host(lan_ip), cfg.bind.port()),
                    LanTls::Proxy => format!("https://{}/", host(lan_ip)),
                    LanTls::Off => format!("http://{}:{}/", host(lan_ip), cfg.bind.port()),
                };
                Url::parse(&s)?
            }
        };

        let mdns = if cfg.mdns { advertise(lan_ip, &base).map_err(|e| tracing::warn!("mDNS disabled: {e}")).ok() } else { None };

        tracing::info!(%base, %lan_ip, tls = ?cfg.lan_tls, "LAN profile: phones on this network use {base}");
        Ok(Self {
            bind: cfg.bind,
            tls: cfg.lan_tls,
            tls_dir: cfg.tls_dir.clone(),
            lan_ip,
            base,
            trusted: cfg.trusted_proxies.clone(),
            _mdns: mdns,
        })
    }
}

fn host(ip: IpAddr) -> String {
    match ip {
        IpAddr::V6(v6) => format!("[{v6}]"),
        IpAddr::V4(v4) => v4.to_string(),
    }
}

/// Advertise `_healoo._tcp.local.` so apps (or `dns-sd -B _healoo._tcp`) can find the server.
fn advertise(ip: IpAddr, base: &Url) -> anyhow::Result<ServiceDaemon> {
    let daemon = ServiceDaemon::new()?;
    let port = base.port_or_known_default().unwrap_or(443);
    let mut props = HashMap::new();
    props.insert("base".to_string(), base.to_string());
    props.insert("api".to_string(), "v1".to_string());
    let info = ServiceInfo::new("_healoo._tcp.local.", "Healoo trial server", "healoo-trial.local.", ip, port, props)?;
    daemon.register(info)?;
    Ok(daemon)
}

#[async_trait]
impl Exposure for LanExposure {
    fn mode(&self) -> NetworkMode {
        NetworkMode::Lan
    }

    fn public_base(&self) -> Url {
        self.base.clone()
    }

    fn client_ip(&self, peer: SocketAddr, headers: &HeaderMap) -> std::net::IpAddr {
        match self.tls {
            // Only the local proxy may speak for someone else.
            LanTls::Proxy => forwarded_ip(peer, headers, &self.trusted),
            _ => peer.ip(),
        }
    }

    fn cors(&self) -> CorsLayer {
        CorsLayer::permissive()
    }

    async fn serve(&self, app: Router, shutdown: CancellationToken) -> anyhow::Result<()> {
        let make = app.into_make_service_with_connect_info::<SocketAddr>();
        match self.tls {
            LanTls::Builtin => {
                let (cert, key, ca) = tls::ensure_local_certs(&self.tls_dir, self.lan_ip)?;
                tracing::info!("Install {} on test phones/emulators to trust this server", ca.display());
                let config = axum_server::tls_rustls::RustlsConfig::from_pem_file(&cert, &key)
                    .await
                    .context("loading LAN TLS certificate")?;
                let handle = axum_server::Handle::new();
                let h = handle.clone();
                tokio::spawn(async move {
                    shutdown.cancelled().await;
                    h.graceful_shutdown(Some(Duration::from_secs(10)));
                });
                tracing::info!(bind = %self.bind, "listening (HTTPS, built-in)");
                axum_server::bind_rustls(self.bind, config).handle(handle).serve(make).await?;
            }
            LanTls::Proxy | LanTls::Off => {
                let listener = tokio::net::TcpListener::bind(self.bind).await?;
                tracing::info!(bind = %self.bind, "listening (HTTP{})", if self.tls == LanTls::Proxy { ", TLS at local proxy" } else { ", no TLS" });
                axum::serve(listener, make).with_graceful_shutdown(async move { shutdown.cancelled().await }).await?;
            }
        }
        Ok(())
    }
}
