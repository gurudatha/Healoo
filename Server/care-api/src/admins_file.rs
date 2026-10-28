//! The encrypted administrators file (Administration_Design.md section 10): who the platform
//! administrators are, and optionally hospital administrators, with their emails and Healoo IDs.
//!
//! The file (`config/admins/admins.enc`, never committed) is encrypted with a key phrase that
//! only the server's operators know (`ADMINS_KEY`). `care-api` reads it at start-up and makes the
//! accounts match it; `care-admin config …` creates, shows, re-keys and applies it.
//!
//! Format: the line `HEALOO-ADMINS-1`, then base64 of salt (16 bytes) | nonce (12) | ciphertext.
//! The key is Argon2id(key phrase, salt); the cipher is ChaCha20-Poly1305, so a wrong key phrase
//! or an edited file is detected and nothing is applied.

use crate::{model::Role, store::{users::NewUser, Db}};
use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chacha20poly1305::{aead::{Aead, KeyInit}, ChaCha20Poly1305, Key, Nonce};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

const MAGIC: &str = "HEALOO-ADMINS-1";

/// What the file holds (as JSON, before encryption).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct AdminsFile {
    /// Exactly these accounts are platform administrators: anyone else loses the role.
    #[serde(default)]
    pub platform_admins: Vec<AdminEntry>,
    /// These accounts are made administrators of the named hospital. Administrators appointed in
    /// the dashboard and not listed here are left alone.
    #[serde(default)]
    pub hospital_admins: Vec<HospitalAdminEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AdminEntry {
    pub name: String,
    /// Their sign-in email. An account that doesn't exist yet is created with it, and their first
    /// Auth0 sign-in with this (verified) email is linked to it.
    #[serde(default)]
    pub email: Option<String>,
    /// Healoo ID (e.g. "HL-4K7Q2") or account UUID of an existing account.
    #[serde(default)]
    pub user_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HospitalAdminEntry {
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    /// The hospital's Healoo ID or UUID.
    pub hospital: String,
}

/// A starting point for a new file.
pub fn example() -> AdminsFile {
    AdminsFile {
        platform_admins: vec![AdminEntry { name: "Healoo Platform Admin".into(), email: None, user_id: Some("HL-9A7D2".into()) }],
        hospital_admins: vec![HospitalAdminEntry { name: "Hospital A Admin".into(), email: None, user_id: Some("HL-8A2D4".into()), hospital: "HL-1H0A1".into() }],
    }
}

// ---------------- encryption ----------------

fn derive(phrase: &str, salt: &[u8]) -> Result<Key> {
    if phrase.is_empty() { bail!("the key phrase is empty"); }
    let mut key = [0u8; 32];
    argon2::Argon2::default().hash_password_into(phrase.as_bytes(), salt, &mut key).map_err(|e| anyhow!("deriving the key: {e}"))?;
    Ok(Key::from(key))
}

/// Encrypts `plain` with the key phrase; a fresh salt and nonce every time.
pub fn encrypt(plain: &[u8], phrase: &str) -> Result<String> {
    let salt: [u8; 16] = rand::random();
    let nonce: [u8; 12] = rand::random();
    let cipher = ChaCha20Poly1305::new(&derive(phrase, &salt)?);
    let sealed = cipher.encrypt(Nonce::from_slice(&nonce), plain).map_err(|_| anyhow!("encryption failed"))?;
    let mut blob = Vec::with_capacity(28 + sealed.len());
    blob.extend_from_slice(&salt);
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(&sealed);
    Ok(format!("{MAGIC}\n{}\n", B64.encode(blob)))
}

pub fn decrypt(text: &str, phrase: &str) -> Result<Vec<u8>> {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some(MAGIC) { bail!("not a Healoo administrators file"); }
    let blob = B64.decode(lines.collect::<String>().trim()).context("the file is damaged (not base64)")?;
    if blob.len() < 28 + 16 { bail!("the file is damaged (too short)"); }
    let (salt, rest) = blob.split_at(16);
    let (nonce, sealed) = rest.split_at(12);
    ChaCha20Poly1305::new(&derive(phrase, salt)?)
        .decrypt(Nonce::from_slice(nonce), sealed)
        .map_err(|_| anyhow!("wrong key phrase, or the file was changed"))
}

pub fn parse(plain: &[u8]) -> Result<AdminsFile> {
    let f: AdminsFile = serde_json::from_slice(plain).context("the administrators list is not valid JSON")?;
    for e in &f.platform_admins { check_entry(&e.name, &e.email, &e.user_id)?; }
    let max = crate::settings::settings().administrators.max_platform_admins;
    if f.platform_admins.len() > max {
        bail!("the file lists {} platform administrators; at most {max} are allowed (administrators.max_platform_admins in server.json)", f.platform_admins.len());
    }
    for e in &f.hospital_admins {
        check_entry(&e.name, &e.email, &e.user_id)?;
        if e.hospital.trim().is_empty() { bail!("{}: hospital is missing", e.name); }
    }
    Ok(f)
}

fn check_entry(name: &str, email: &Option<String>, user_id: &Option<String>) -> Result<()> {
    if name.trim().is_empty() || name.chars().count() > 80 { bail!("every administrator needs a name of 1–80 characters"); }
    if email.as_deref().unwrap_or("").trim().is_empty() && user_id.as_deref().unwrap_or("").trim().is_empty() {
        bail!("{name}: give an email, a user_id (Healoo ID), or both");
    }
    if let Some(e) = email.as_deref().map(str::trim).filter(|e| !e.is_empty()) {
        if !e.split_once('@').is_some_and(|(a, d)| !a.is_empty() && d.contains('.')) { bail!("{name}: {e} is not a valid email"); }
    }
    Ok(())
}

pub fn read(path: &std::path::Path, phrase: &str) -> Result<AdminsFile> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse(&decrypt(&text, phrase)?)
}

pub fn write(path: &std::path::Path, f: &AdminsFile, phrase: &str) -> Result<()> {
    let plain = serde_json::to_vec_pretty(f)?;
    if let Some(dir) = path.parent() { std::fs::create_dir_all(dir)?; }
    std::fs::write(path, encrypt(&plain, phrase)?).with_context(|| format!("writing {}", path.display()))
}

// ---------------- applying it to the accounts ----------------

/// What `apply` changed, one line each.
#[derive(Debug, Default)]
pub struct Report { pub lines: Vec<String>, pub unchanged: bool }
impl Report { fn say(&mut self, s: String) { tracing::info!(target: "admins_file", "{s}"); self.lines.push(s); } }

async fn resolve_id(db: &Db, id: &str) -> Result<Option<Uuid>> {
    let id = id.trim();
    if let Ok(u) = id.parse::<Uuid>() { return Ok(db.user(u).await?.map(|u| u.user_id)); }
    db.user_id_by_public_id(id).await
}

/// Finds the account by user_id, then by email; creates it from the email when neither exists.
async fn account(db: &Db, name: &str, email: &Option<String>, user_id: &Option<String>, role: Role, hospital: Option<Uuid>, r: &mut Report) -> Result<Option<Uuid>> {
    let email = email.as_deref().map(|e| e.trim().to_lowercase()).filter(|e| !e.is_empty());
    let mut found = match user_id.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(id) => match resolve_id(db, id).await? { Some(u) => Some(u), None => { r.say(format!("{name}: no account with user_id {id}")); None } },
        None => None,
    };
    if found.is_none() { if let Some(e) = &email { found = db.user_id_by_email(e).await?; } }
    if let Some(id) = found {
        // Give the account its email so a later Auth0 sign-in with it links here.
        if let Some(e) = &email {
            let u = db.user(id).await?.ok_or_else(|| anyhow!("account vanished"))?;
            if u.email.is_none() && db.user_id_by_email(e).await?.is_none() { db.set_email(id, e).await?; }
        }
        return Ok(Some(id));
    }
    let Some(e) = email else { return Ok(None) };
    let u = db.create_user(NewUser {
        user_id: Uuid::new_v4(), public_id: None, auth0_sub: None, display_name: name.trim(), roles: &[role],
        location: None, official_number: None, primary_hospital_id: hospital, employer_doctor_id: None,
        headline: None, email: Some(&e), created_by: None,
    }).await?;
    r.say(format!("{name}: created account {} for {e}", u.public_id));
    Ok(Some(u.user_id))
}

/// Makes the accounts match the file: listed platform administrators get the role, unlisted ones
/// lose it; listed hospital administrators get their hospital.
pub async fn apply(db: &Db, f: &AdminsFile) -> Result<Report> {
    let mut r = Report::default();
    let mut keep: HashSet<Uuid> = HashSet::new();
    for e in &f.platform_admins {
        let Some(id) = account(db, &e.name, &e.email, &e.user_id, Role::AppAdministrator, None, &mut r).await? else {
            r.say(format!("{}: skipped (no account found and no email to create one)", e.name));
            continue;
        };
        let u = db.user(id).await?.ok_or_else(|| anyhow!("account vanished"))?;
        if u.roles.iter().any(|x| x.is_clinical()) {
            r.say(format!("{} ({}): skipped, a {:?} account can't be a platform administrator", e.name, u.public_id, u.primary_role()));
            continue;
        }
        keep.insert(id);
        if !u.roles.contains(&Role::AppAdministrator) {
            let mut roles = u.roles.clone();
            roles.insert(Role::AppAdministrator);
            db.set_roles(&u, &roles, u.primary_hospital_id).await?;
            r.say(format!("{} ({}) is now a platform administrator", u.display_name, u.public_id));
        }
    }
    for a in db.all_accounts().await?.into_iter().filter(|a| a.roles.contains(&Role::AppAdministrator) && !keep.contains(&a.user_id)) {
        let Some(u) = db.user(a.user_id).await? else { continue };
        let mut roles = u.roles.clone();
        roles.remove(&Role::AppAdministrator);
        if roles.is_empty() { roles.insert(Role::Patient); }
        db.set_roles(&u, &roles, u.primary_hospital_id).await?;
        r.say(format!("{} ({}) is no longer a platform administrator (not in the file)", u.display_name, u.public_id));
    }

    for e in &f.hospital_admins {
        let Some(hid) = resolve_id(db, &e.hospital).await? else { r.say(format!("{}: no hospital {}", e.name, e.hospital)); continue };
        let h = db.user(hid).await?.ok_or_else(|| anyhow!("hospital vanished"))?;
        if !h.roles.contains(&Role::Hospital) { r.say(format!("{}: {} is not a hospital", e.name, e.hospital)); continue; }
        let Some(id) = account(db, &e.name, &e.email, &e.user_id, Role::Administrator, Some(hid), &mut r).await? else {
            r.say(format!("{}: skipped (no account found and no email to create one)", e.name));
            continue;
        };
        let u = db.user(id).await?.ok_or_else(|| anyhow!("account vanished"))?;
        if crate::policy::may_appoint_admin(&u.roles).access == crate::policy::Access::Deny && !u.roles.contains(&Role::Administrator) {
            r.say(format!("{} ({}): skipped, a {:?} account can't be a hospital administrator", e.name, u.public_id, u.primary_role()));
            continue;
        }
        if !u.roles.contains(&Role::Administrator) || u.primary_hospital_id != Some(hid) {
            let max = crate::settings::settings().administrators.max_admins_per_hospital;
            if db.hospital_admin_ids(hid).await?.len() >= max {
                r.say(format!("{} ({}): skipped, {} already has {max} administrators (administrators.max_admins_per_hospital)", e.name, u.public_id, h.display_name));
                continue;
            }
            let mut roles = u.roles.clone();
            roles.insert(Role::Administrator);
            db.set_roles(&u, &roles, Some(hid)).await?;
            db.connect_users(u.user_id, Role::Administrator, &u.display_name, h.user_id, Role::Hospital, &h.display_name).await?;
            r.say(format!("{} ({}) is now an administrator of {}", u.display_name, u.public_id, h.display_name));
        }
    }
    if r.lines.is_empty() { r.unchanged = true; r.say("administrators already match the file".into()); }
    Ok(r)
}

/// Where the file is and its key phrase, from ADMINS_FILE and ADMINS_KEY.
pub fn location() -> (std::path::PathBuf, Option<String>) {
    let path = std::env::var("ADMINS_FILE").unwrap_or_else(|_| "config/admins/admins.enc".into());
    (path.into(), std::env::var("ADMINS_KEY").ok().filter(|k| !k.is_empty()))
}

/// care-api start-up: applies the file when it and its key are there. A missing file is fine;
/// a wrong key or a damaged file changes nothing and is logged as an error.
pub async fn apply_at_startup(db: &Db) {
    let (path, key) = location();
    if !path.exists() { tracing::info!("no administrators file at {}; administrators are managed with care-admin", path.display()); return; }
    let Some(key) = key else { tracing::warn!("{} exists but ADMINS_KEY is not set; administrators not updated", path.display()); return; };
    match read(&path, &key) {
        Ok(f) => match apply(db, &f).await {
            Ok(r) => tracing::info!("administrators file applied: {} change(s)", if r.unchanged { 0 } else { r.lines.len() }),
            Err(e) => tracing::error!("applying the administrators file failed: {e:#}"),
        },
        Err(e) => tracing::error!("administrators file not applied: {e:#}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_with_the_right_key_only() {
        let plain = serde_json::to_vec(&example()).unwrap();
        let sealed = encrypt(&plain, "friends").unwrap();
        assert!(sealed.starts_with(MAGIC));
        assert!(!sealed.contains("HL-9A7D2"), "the contents must not be readable");
        assert_eq!(parse(&decrypt(&sealed, "friends").unwrap()).unwrap(), example());
        assert!(decrypt(&sealed, "Friends").is_err());
        assert!(decrypt(&sealed, "").is_err());
    }

    #[test]
    fn each_encryption_differs_and_tampering_is_detected() {
        let a = encrypt(b"{}", "friends").unwrap();
        let b = encrypt(b"{}", "friends").unwrap();
        assert_ne!(a, b, "fresh salt and nonce every time");
        let mut blob = B64.decode(a.lines().nth(1).unwrap()).unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 1;
        assert!(decrypt(&format!("{MAGIC}\n{}\n", B64.encode(blob)), "friends").is_err());
    }

    #[test]
    fn more_platform_admins_than_the_limit_is_refused() {
        let max = crate::settings::settings().administrators.max_platform_admins;
        let entry = |i: usize| format!(r#"{{"name":"Admin {i}","email":"a{i}@example.com"}}"#);
        let list = |n: usize| format!(r#"{{"platform_admins":[{}]}}"#, (0..n).map(entry).collect::<Vec<_>>().join(","));
        assert!(parse(list(max).as_bytes()).is_ok());
        assert!(parse(list(max + 1).as_bytes()).is_err());
    }

    #[test]
    fn entries_need_a_name_and_an_email_or_id() {
        assert!(parse(br#"{"platform_admins":[{"name":"","user_id":"HL-9A7D2"}]}"#).is_err());
        assert!(parse(br#"{"platform_admins":[{"name":"A"}]}"#).is_err());
        assert!(parse(br#"{"platform_admins":[{"name":"A","email":"not-an-email"}]}"#).is_err());
        assert!(parse(br#"{"platform_admins":[{"name":"A","email":"a@example.com"}]}"#).is_ok());
        assert!(parse(br#"{"hospital_admins":[{"name":"A","user_id":"HL-8A2D4","hospital":""}]}"#).is_err());
    }
}
