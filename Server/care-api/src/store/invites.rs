//! Hospital administrator invite codes (Administration_Design.md section 11): a platform
//! administrator creates a single-use code for a hospital; whoever enters it in the app becomes
//! that hospital's administrator. Only a SHA-256 of the code is stored.

use super::Db;
use crate::model::*;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use scylla::frame::value::CqlTimestamp;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Letters and digits that can't be misread (no 0/O, 1/I/L).
const ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";
/// 12 characters from 31: about 59 bits, far beyond guessing at the redeem rate limit.
const CODE_LEN: usize = 12;
pub const VALID_DAYS: i64 = 7;

/// A new code, shown as `ABCD-EFGH-JKMN`.
pub fn new_code() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let raw: String = (0..CODE_LEN).map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char).collect();
    format!("{}-{}-{}", &raw[0..4], &raw[4..8], &raw[8..12])
}

/// What people type is forgiving: case, spaces and dashes don't matter.
pub fn normalize(code: &str) -> Option<String> {
    let c: String = code.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect();
    (c.len() == CODE_LEN && c.bytes().all(|b| ALPHABET.contains(&b))).then_some(c)
}

pub fn hash(normalized: &str) -> String { hex::encode(Sha256::digest(format!("healoo-invite:{normalized}").as_bytes())) }

#[derive(Clone, Debug)]
pub struct Invite {
    pub invite_id: Uuid,
    pub hospital_id: Uuid,
    pub note: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub used_by: Option<Uuid>,
    pub used_at: Option<DateTime<Utc>>,
    pub revoked: bool,
}

impl Invite {
    /// "PENDING" | "USED" | "REVOKED" | "EXPIRED"
    pub fn status(&self, now: DateTime<Utc>) -> &'static str {
        if self.used_by.is_some() { "USED" } else if self.revoked { "REVOKED" } else if now >= self.expires_at { "EXPIRED" } else { "PENDING" }
    }
}

type InviteRow = (Uuid, Option<Uuid>, Option<String>, Option<Uuid>, Option<CqlTimestamp>, Option<CqlTimestamp>, Option<Uuid>, Option<CqlTimestamp>, Option<bool>);

fn to_invite(r: InviteRow) -> Invite {
    Invite {
        invite_id: r.0,
        hospital_id: r.1.unwrap_or_default(),
        note: r.2,
        created_by: r.3,
        created_at: r.4.map(ts_to_dt).unwrap_or_default(),
        expires_at: r.5.map(ts_to_dt).unwrap_or_default(),
        used_by: r.6,
        used_at: r.7.map(ts_to_dt),
        revoked: r.8.unwrap_or(false),
    }
}

const SELECT: &str = "SELECT invite_id, hospital_id, note, created_by, created_at, expires_at, used_by, used_at, revoked FROM admin_invites WHERE invite_id = ?";

impl Db {
    /// Stores a new invite; returns it and the code (the only time the code exists in clear).
    pub async fn create_invite(&self, hospital: Uuid, note: Option<&str>, by: Uuid) -> Result<(Invite, String)> {
        let code = new_code();
        let h = hash(&normalize(&code).expect("generated codes are valid"));
        let id = Uuid::new_v4();
        let now = Utc::now();
        let expires = now + Duration::days(VALID_DAYS);
        self.exec(
            "INSERT INTO admin_invites (invite_id, code_hash, hospital_id, note, created_by, created_at, expires_at, revoked) VALUES (?, ?, ?, ?, ?, ?, ?, false)",
            (id, &h, hospital, note, by, CqlTimestamp(now.timestamp_millis()), CqlTimestamp(expires.timestamp_millis())),
        ).await?;
        self.exec("INSERT INTO admin_invites_by_code (code_hash, invite_id) VALUES (?, ?)", (&h, id)).await?;
        self.exec("INSERT INTO admin_invites_by_hospital (hospital_id, invite_id) VALUES (?, ?)", (hospital, id)).await?;
        Ok((self.invite(id).await?.expect("just inserted"), code))
    }

    pub async fn invite(&self, id: Uuid) -> Result<Option<Invite>> {
        Ok(self.one::<InviteRow>(SELECT, (id,)).await?.map(to_invite))
    }

    pub async fn invite_by_code(&self, normalized: &str) -> Result<Option<Invite>> {
        match self.one::<(Uuid,)>("SELECT invite_id FROM admin_invites_by_code WHERE code_hash = ?", (hash(normalized),)).await? {
            Some((id,)) => self.invite(id).await,
            None => Ok(None),
        }
    }

    pub async fn hospital_invites(&self, hospital: Uuid) -> Result<Vec<Invite>> {
        let ids = self.rows::<(Uuid,)>("SELECT invite_id FROM admin_invites_by_hospital WHERE hospital_id = ?", (hospital,)).await?;
        let mut out = Vec::new();
        for (id,) in ids { if let Some(i) = self.invite(id).await? { out.push(i); } }
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(out)
    }

    pub async fn revoke_invite(&self, id: Uuid) -> Result<()> {
        self.exec("UPDATE admin_invites SET revoked = true WHERE invite_id = ?", (id,)).await?;
        Ok(())
    }

    /// Marks the invite used, only if nobody used it first (lightweight transaction), so one code
    /// can't make two administrators even when entered twice at the same moment.
    pub async fn use_invite(&self, id: Uuid, user: Uuid) -> Result<bool> {
        let res = self.exec(
            "UPDATE admin_invites SET used_by = ?, used_at = ? WHERE invite_id = ? IF used_by = null",
            (user, now_ts(), id)).await?;
        let applied = res.rows_typed::<(bool,)>().ok().and_then(|mut r| r.next()).and_then(|r| r.ok()).map(|r| r.0);
        Ok(applied.unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_readable_unique_and_forgiving() {
        let a = new_code();
        assert_eq!(a.len(), 14);
        assert_eq!(a.matches('-').count(), 2);
        assert_ne!(a, new_code());
        let n = normalize(&a).unwrap();
        assert_eq!(normalize(&a.to_lowercase().replace('-', " ")), Some(n.clone()));
        assert_eq!(hash(&n), hash(&normalize(&format!("  {} ", a.to_lowercase())).unwrap()));
    }

    #[test]
    fn malformed_codes_are_rejected_before_any_lookup() {
        assert!(normalize("ABCD-EFGH-JKM").is_none(), "too short");
        assert!(normalize("ABCD-EFGH-JKMN-P").is_none(), "too long");
        assert!(normalize("ABCD-EFGH-JK0N").is_none(), "0 is not in the alphabet");
        assert!(normalize("").is_none());
    }

    #[test]
    fn status_follows_use_revocation_and_expiry() {
        let now = Utc::now();
        let base = Invite { invite_id: Uuid::nil(), hospital_id: Uuid::nil(), note: None, created_by: None, created_at: now,
            expires_at: now + Duration::days(1), used_by: None, used_at: None, revoked: false };
        assert_eq!(base.status(now), "PENDING");
        assert_eq!(Invite { revoked: true, ..base.clone() }.status(now), "REVOKED");
        assert_eq!(Invite { used_by: Some(Uuid::nil()), revoked: true, ..base.clone() }.status(now), "USED");
        assert_eq!(base.status(now + Duration::days(2)), "EXPIRED");
    }
}
