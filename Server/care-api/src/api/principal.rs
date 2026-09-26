//! The authenticated caller. Verifies the bearer token with whichever [`TokenVerifier`] the
//! profile selected, maps the token subject to a Cassandra user (provisioning a PATIENT on first
//! login, doc 5.5), loads active affiliations for the policy engine and applies the per-user
//! rate limit (20 req/s, doc 4.5).

use super::{ApiError, ApiResult, AppState, Inner};
use crate::{
    model::Role,
    policy::Subject,
    store::users::{NewUser, UserRec},
};
use axum::{async_trait, extract::FromRequestParts, http::request::Parts};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Principal {
    pub user: UserRec,
    pub subject: Subject,
    pub token_exp: i64,
}

impl Principal {
    pub fn id(&self) -> Uuid { self.user.user_id }
    pub fn has(&self, r: Role) -> bool { self.user.roles.contains(&r) }
}

#[derive(Clone)]
pub(crate) struct Cached {
    user_id: Uuid,
    at: Instant,
}

const CACHE_TTL: Duration = Duration::from_secs(300);
const RATE_PER_SEC: f64 = 20.0;
const BURST: f64 = 40.0;

pub fn bearer(headers: &axum::http::HeaderMap) -> Option<String> {
    headers.get(axum::http::header::AUTHORIZATION)?.to_str().ok()?
        .strip_prefix("Bearer ").map(|t| t.trim().to_string())
}

impl Inner {
    pub async fn authenticate(&self, token: &str) -> ApiResult<Principal> {
        let t = self.verifier.verify(token).await.map_err(|e| ApiError::unauthorized(e.to_string()))?;

        let user_id = match self.principals.get(&t.sub).filter(|c| c.at.elapsed() < CACHE_TTL).map(|c| c.user_id) {
            Some(id) => id,
            None => {
                let id = self.resolve_user(&t).await?;
                self.principals.insert(t.sub.clone(), Cached { user_id: id, at: Instant::now() });
                id
            }
        };
        let user = self.db.user(user_id).await?.ok_or_else(|| ApiError::unauthorized("account no longer exists"))?;
        self.rate_limit(user_id)?;

        let active_hospitals = if user.roles.contains(&Role::Doctor) { self.db.active_hospitals(user_id).await? } else { Default::default() };
        let subject = Subject { user_id, roles: user.roles.clone(), active_hospitals };
        Ok(Principal { user, subject, token_exp: t.exp })
    }

    async fn resolve_user(&self, t: &crate::auth::VerifiedToken) -> ApiResult<Uuid> {
        if let Some(id) = self.db.user_id_by_sub(&t.sub).await? {
            return Ok(id);
        }
        // Auth0 Action put the Cassandra id in the token (doc 5.3): link it.
        if let Some(uid) = t.uid {
            if self.db.user(uid).await?.is_some() {
                self.db.link_sub(&t.sub, uid).await?;
                return Ok(uid);
            }
        }
        // First login: everyone starts as a PATIENT; other roles are granted by admins (doc 5.5).
        let name = t.name.clone().unwrap_or_else(|| "New Healoo user".to_string());
        let u = self.db.create_user(NewUser {
            user_id: t.uid.unwrap_or_else(Uuid::new_v4),
            public_id: None,
            auth0_sub: Some(&t.sub),
            display_name: &name,
            roles: &[Role::Patient],
            location: None,
            official_number: None,
            primary_hospital_id: None,
            employer_doctor_id: None,
            headline: None,
        }).await?;
        tracing::info!(user_id = %u.user_id, public_id = %u.public_id, "provisioned new patient on first login");
        Ok(u.user_id)
    }

    fn rate_limit(&self, user: Uuid) -> ApiResult<()> {
        let now = Instant::now();
        let mut e = self.limiter.entry(user).or_insert((now, BURST));
        let (last, tokens) = *e;
        let refill = (tokens + now.duration_since(last).as_secs_f64() * RATE_PER_SEC).min(BURST);
        if refill < 1.0 {
            *e = (now, refill);
            return Err(ApiError::too_many());
        }
        *e = (now, refill - 1.0);
        Ok(())
    }
}

#[async_trait]
impl FromRequestParts<AppState> for Principal {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = bearer(&parts.headers).ok_or_else(|| ApiError::unauthorized("missing bearer token"))?;
        state.authenticate(&token).await
    }
}
