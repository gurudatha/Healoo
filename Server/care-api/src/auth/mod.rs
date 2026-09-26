//! Token verification (design doc 5). Two interchangeable verifiers:
//!
//! * [`Auth0Verifier`] — RS256 tokens from Auth0, JWKS cached 10 min and refreshed on unknown
//!   `kid`; checks `iss`, `aud`, `exp` (doc 5.5). Needs internet access to the Auth0 tenant.
//! * [`DevVerifier`] — HS256 tokens signed with a local secret (`AUTH_MODE=dev`, doc 5.5), so a
//!   LAN trial works with no internet at all. Refused in internet mode by `Config::validate`.

use crate::config::{AuthMode, Config};
use async_trait::async_trait;
use jsonwebtoken::{decode, decode_header, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct VerifiedToken {
    pub sub: String,
    pub roles: Vec<String>,
    /// Cassandra user_id from the post-login Action (doc 5.3), when present.
    pub uid: Option<Uuid>,
    pub name: Option<String>,
    pub exp: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("missing bearer token")]
    Missing,
    #[error("token rejected: {0}")]
    Invalid(String),
    #[error("identity provider unavailable: {0}")]
    Unavailable(String),
}

#[async_trait]
pub trait TokenVerifier: Send + Sync + 'static {
    async fn verify(&self, token: &str) -> Result<VerifiedToken, AuthError>;
    fn mode(&self) -> AuthMode;
}

pub fn build(cfg: &Config) -> Arc<dyn TokenVerifier> {
    match cfg.auth_mode {
        AuthMode::Auth0 => Arc::new(Auth0Verifier::new(&cfg.auth0_domain, &cfg.auth0_audience, &cfg.claims_namespace)),
        AuthMode::Dev => Arc::new(DevVerifier::new(&cfg.dev_jwt_secret, &cfg.claims_namespace)),
    }
}

#[derive(Deserialize)]
struct RawClaims {
    sub: String,
    exp: i64,
    #[serde(default)]
    name: Option<String>,
    #[serde(flatten)]
    extra: HashMap<String, Value>,
}

fn to_verified(c: RawClaims, ns: &str) -> VerifiedToken {
    let roles = c.extra.get(&format!("{ns}roles"))
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|r| r.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let uid = c.extra.get(&format!("{ns}uid")).and_then(|v| v.as_str()).and_then(|s| s.parse().ok());
    VerifiedToken { sub: c.sub, roles, uid, name: c.name, exp: c.exp }
}

// ---------------- Auth0 ----------------

pub struct Auth0Verifier {
    issuer: String,
    audience: String,
    ns: String,
    jwks_url: String,
    http: reqwest::Client,
    keys: RwLock<(HashMap<String, DecodingKey>, Option<Instant>)>,
}

#[derive(Deserialize)]
struct Jwks { keys: Vec<Jwk> }
#[derive(Deserialize)]
struct Jwk { kid: String, n: String, e: String }

const JWKS_TTL: Duration = Duration::from_secs(600);

impl Auth0Verifier {
    pub fn new(domain: &str, audience: &str, ns: &str) -> Self {
        let domain = domain.trim_start_matches("https://").trim_end_matches('/');
        Self {
            issuer: format!("https://{domain}/"),
            audience: audience.to_string(),
            ns: ns.to_string(),
            jwks_url: format!("https://{domain}/.well-known/jwks.json"),
            http: reqwest::Client::builder().timeout(Duration::from_secs(5)).build().expect("http client"),
            keys: RwLock::new((HashMap::new(), None)),
        }
    }

    async fn refresh(&self) -> Result<(), AuthError> {
        let jwks: Jwks = self.http.get(&self.jwks_url).send().await
            .and_then(|r| r.error_for_status())
            .map_err(|e| AuthError::Unavailable(e.to_string()))?
            .json().await
            .map_err(|e| AuthError::Unavailable(e.to_string()))?;
        let mut map = HashMap::new();
        for k in jwks.keys {
            if let Ok(key) = DecodingKey::from_rsa_components(&k.n, &k.e) {
                map.insert(k.kid, key);
            }
        }
        *self.keys.write().await = (map, Some(Instant::now()));
        Ok(())
    }

    async fn key(&self, kid: &str) -> Result<DecodingKey, AuthError> {
        {
            let guard = self.keys.read().await;
            let fresh = guard.1.map(|t| t.elapsed() < JWKS_TTL).unwrap_or(false);
            if let (true, Some(k)) = (fresh, guard.0.get(kid)) {
                return Ok(k.clone());
            }
        }
        self.refresh().await?; // stale cache or unknown kid (key rotation)
        self.keys.read().await.0.get(kid).cloned().ok_or_else(|| AuthError::Invalid("unknown signing key".into()))
    }
}

#[async_trait]
impl TokenVerifier for Auth0Verifier {
    async fn verify(&self, token: &str) -> Result<VerifiedToken, AuthError> {
        let header = decode_header(token).map_err(|e| AuthError::Invalid(e.to_string()))?;
        let kid = header.kid.ok_or_else(|| AuthError::Invalid("token has no kid".into()))?;
        let key = self.key(&kid).await?;
        let mut v = Validation::new(Algorithm::RS256);
        v.set_issuer(&[&self.issuer]);
        v.set_audience(&[&self.audience]);
        let data = decode::<RawClaims>(token, &key, &v).map_err(|e| AuthError::Invalid(e.to_string()))?;
        Ok(to_verified(data.claims, &self.ns))
    }
    fn mode(&self) -> AuthMode { AuthMode::Auth0 }
}

// ---------------- Dev (offline LAN) ----------------

pub const DEV_ISSUER: &str = "healoo-dev";

pub struct DevVerifier {
    decoding: DecodingKey,
    encoding: EncodingKey,
    ns: String,
}

#[derive(Serialize)]
struct DevClaims<'a> {
    sub: &'a str,
    iss: &'a str,
    aud: &'a str,
    exp: i64,
    iat: i64,
    name: &'a str,
    #[serde(flatten)]
    custom: HashMap<String, Value>,
}

impl DevVerifier {
    pub fn new(secret: &str, ns: &str) -> Self {
        Self { decoding: DecodingKey::from_secret(secret.as_bytes()), encoding: EncodingKey::from_secret(secret.as_bytes()), ns: ns.to_string() }
    }

    /// Mint a token that looks like an Auth0 access token (same custom claims).
    pub fn mint(&self, sub: &str, name: &str, roles: &[&str], uid: Option<Uuid>, ttl: Duration) -> String {
        let now = chrono::Utc::now().timestamp();
        let mut custom = HashMap::new();
        custom.insert(format!("{}roles", self.ns), Value::from(roles.iter().map(|r| Value::from(*r)).collect::<Vec<_>>()));
        if let Some(u) = uid {
            custom.insert(format!("{}uid", self.ns), Value::from(u.to_string()));
        }
        let claims = DevClaims { sub, iss: DEV_ISSUER, aud: DEV_ISSUER, exp: now + ttl.as_secs() as i64, iat: now, name, custom };
        encode(&Header::new(Algorithm::HS256), &claims, &self.encoding).expect("HS256 encoding cannot fail")
    }
}

#[async_trait]
impl TokenVerifier for DevVerifier {
    async fn verify(&self, token: &str) -> Result<VerifiedToken, AuthError> {
        let mut v = Validation::new(Algorithm::HS256);
        v.set_issuer(&[DEV_ISSUER]);
        v.set_audience(&[DEV_ISSUER]);
        let data = decode::<RawClaims>(token, &self.decoding, &v).map_err(|e| AuthError::Invalid(e.to_string()))?;
        Ok(to_verified(data.claims, &self.ns))
    }
    fn mode(&self) -> AuthMode { AuthMode::Dev }
}
