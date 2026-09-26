//! Configuration from environment variables.
//!
//! `NETWORK_MODE` picks the deployment profile and fills in safe defaults for everything else:
//!
//! | Setting        | `lan` (trial on a local network)         | `internet` (deployed)                     |
//! |----------------|------------------------------------------|-------------------------------------------|
//! | Listen         | 0.0.0.0:8443, reachable from phones      | 0.0.0.0:8080 inside the private network   |
//! | TLS            | built-in (own local CA) / proxy / off    | terminated by Caddy or Nginx              |
//! | Public URL     | https://<LAN IP>:8443/ (auto-detected)   | PUBLIC_BASE_URL (required, https)         |
//! | Discovery      | mDNS `_healoo._tcp` on the LAN           | none                                      |
//! | Auth           | dev tokens (offline) or Auth0            | Auth0 only                                |
//! | Files          | local disk, served by this API           | S3 / MinIO presigned URLs                 |
//! | Client IP      | socket peer (or proxy on the same host)  | X-Forwarded-For from trusted proxies only |
//! | Debug routes   | on                                       | off                                       |
//!
//! Every value can still be overridden individually.

use anyhow::{bail, Context, Result};
use ipnet::IpNet;
use std::{net::SocketAddr, path::PathBuf, str::FromStr};
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkMode {
    Lan,
    Internet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LanTls {
    /// The API serves HTTPS itself with a certificate from its own local CA.
    Builtin,
    /// Caddy (`tls internal`) or Nginx on the same machine terminates TLS (doc 9).
    Proxy,
    /// Plain HTTP — only for curl tests; phones refuse cleartext.
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthMode {
    Auth0,
    Dev,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusKind {
    Kafka,
    /// In-process bus: quick LAN runs without Kafka. API and worker must then run in one process.
    Memory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreKind {
    Disk,
    S3,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub mode: NetworkMode,
    pub bind: SocketAddr,
    pub public_base_url: Option<Url>,
    pub lan_tls: LanTls,
    pub tls_dir: PathBuf,
    pub mdns: bool,
    pub trusted_proxies: Vec<IpNet>,
    pub cors_origins: Vec<String>,

    pub auth_mode: AuthMode,
    pub auth0_domain: String,
    pub auth0_audience: String,
    pub claims_namespace: String,
    pub dev_jwt_secret: String,

    pub cassandra_nodes: Vec<String>,
    pub keyspace: String,

    pub bus: BusKind,
    pub kafka_brokers: String,

    pub store: StoreKind,
    pub files_dir: PathBuf,
    pub files_secret: String,
    pub s3_bucket: String,
    pub s3_endpoint: Option<String>,
    pub s3_public_endpoint: Option<String>,
    pub s3_region: String,

    pub instance_id: String,
    pub debug_routes: bool,
}

const DEFAULT_DEV_SECRET: &str = "healoo-dev-secret-change-me";
const DEFAULT_FILES_SECRET: &str = "healoo-files-secret-change-me";

fn var(k: &str) -> Option<String> {
    std::env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn var_or(k: &str, default: &str) -> String {
    var(k).unwrap_or_else(|| default.to_string())
}

fn flag(k: &str, default: bool) -> bool {
    var(k).map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on")).unwrap_or(default)
}

fn list(k: &str, default: &str) -> Vec<String> {
    var_or(k, default).split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let mode = match var_or("NETWORK_MODE", "lan").to_lowercase().as_str() {
            "lan" | "local" => NetworkMode::Lan,
            "internet" | "public" | "cloud" => NetworkMode::Internet,
            other => bail!("NETWORK_MODE must be 'lan' or 'internet', got '{other}'"),
        };
        let lan = mode == NetworkMode::Lan;

        let lan_tls = match var_or("LAN_TLS", "builtin").to_lowercase().as_str() {
            "builtin" => LanTls::Builtin,
            "proxy" => LanTls::Proxy,
            "off" => LanTls::Off,
            other => bail!("LAN_TLS must be builtin, proxy or off, got '{other}'"),
        };

        let default_bind = if lan && lan_tls == LanTls::Builtin { "0.0.0.0:8443" } else { "0.0.0.0:8080" };
        let bind = SocketAddr::from_str(&var_or("BIND", default_bind)).context("BIND must look like 0.0.0.0:8080")?;

        let public_base_url = var("PUBLIC_BASE_URL")
            .map(|u| Url::parse(&if u.ends_with('/') { u } else { format!("{u}/") }))
            .transpose()
            .context("PUBLIC_BASE_URL must be a URL such as https://api.example.com/")?;

        let auth_mode = match var_or("AUTH_MODE", if lan { "dev" } else { "auth0" }).to_lowercase().as_str() {
            "auth0" => AuthMode::Auth0,
            "dev" => AuthMode::Dev,
            other => bail!("AUTH_MODE must be auth0 or dev, got '{other}'"),
        };

        let bus = match var_or("EVENT_BUS", "kafka").to_lowercase().as_str() {
            "kafka" => BusKind::Kafka,
            "memory" => BusKind::Memory,
            other => bail!("EVENT_BUS must be kafka or memory, got '{other}'"),
        };

        let store = match var_or("FILE_STORE", if lan { "disk" } else { "s3" }).to_lowercase().as_str() {
            "disk" => StoreKind::Disk,
            "s3" | "minio" => StoreKind::S3,
            other => bail!("FILE_STORE must be disk or s3, got '{other}'"),
        };

        let trusted_default = if lan { "127.0.0.1/32,::1/128" } else { "10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,127.0.0.1/32" };
        let trusted_proxies = list("TRUSTED_PROXIES", trusted_default)
            .iter()
            .map(|s| s.parse::<IpNet>().with_context(|| format!("TRUSTED_PROXIES entry '{s}' is not a CIDR")))
            .collect::<Result<Vec<_>>>()?;

        let cfg = Config {
            mode,
            bind,
            public_base_url,
            lan_tls,
            tls_dir: PathBuf::from(var_or("TLS_DIR", "./data/tls")),
            mdns: flag("MDNS", lan),
            trusted_proxies,
            cors_origins: list("CORS_ORIGINS", ""),

            auth_mode,
            auth0_domain: var_or("AUTH0_DOMAIN", ""),
            auth0_audience: var_or("AUTH0_AUDIENCE", "https://api.careconnect.local"),
            claims_namespace: var_or("CLAIMS_NAMESPACE", "https://careconnect.local/"),
            dev_jwt_secret: var_or("DEV_JWT_SECRET", DEFAULT_DEV_SECRET),

            cassandra_nodes: list("CASSANDRA_NODES", "127.0.0.1:9042"),
            keyspace: var_or("CASSANDRA_KEYSPACE", "careconnect"),

            bus,
            kafka_brokers: var_or("KAFKA_BROKERS", "127.0.0.1:9092"),

            store,
            files_dir: PathBuf::from(var_or("FILES_DIR", "./data/files")),
            files_secret: var_or("FILES_SECRET", DEFAULT_FILES_SECRET),
            s3_bucket: var_or("S3_BUCKET", "careconnect"),
            s3_endpoint: var("S3_ENDPOINT"),
            s3_public_endpoint: var("S3_PUBLIC_ENDPOINT"),
            s3_region: var_or("S3_REGION", "us-east-1"),

            instance_id: var("INSTANCE_ID").unwrap_or_else(|| format!("api-{}", &uuid::Uuid::new_v4().simple().to_string()[..8])),
            debug_routes: flag("DEBUG_ROUTES", lan),
        };
        cfg.validate()?;
        Ok(cfg)
    }

    /// Refuse unsafe combinations before anything starts listening.
    fn validate(&self) -> Result<()> {
        if self.auth_mode == AuthMode::Auth0 && self.auth0_domain.is_empty() {
            bail!("AUTH_MODE=auth0 needs AUTH0_DOMAIN (e.g. careconnect-dev.us.auth0.com)");
        }
        if self.mode == NetworkMode::Internet {
            if self.auth_mode == AuthMode::Dev {
                bail!("NETWORK_MODE=internet cannot run with AUTH_MODE=dev: anyone could mint tokens");
            }
            match &self.public_base_url {
                Some(u) if u.scheme() == "https" => {}
                _ => bail!("NETWORK_MODE=internet needs PUBLIC_BASE_URL starting with https://"),
            }
            if self.files_secret == DEFAULT_FILES_SECRET && self.store == StoreKind::Disk {
                bail!("Set FILES_SECRET to a long random value before exposing disk file storage to the internet");
            }
            if self.debug_routes {
                tracing::warn!("DEBUG_ROUTES is on in internet mode; /v1/check/access will be reachable");
            }
        }
        if self.auth_mode == AuthMode::Dev && self.dev_jwt_secret == DEFAULT_DEV_SECRET {
            tracing::warn!("Using the default DEV_JWT_SECRET — fine on a trial LAN, never on the internet");
        }
        Ok(())
    }
}
