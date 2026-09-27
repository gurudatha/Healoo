//! users, users_by_public_id, users_by_sub, affiliations_*, delegations_by_assistant

use super::Db;
use crate::model::{now_ts, parse, text, Role};
use anyhow::Result;
use rand::Rng;
use scylla::frame::value::CqlTimestamp;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct UserRec {
    pub user_id: Uuid,
    pub public_id: String,
    pub display_name: String,
    pub roles: HashSet<Role>,
    pub photo_uri: Option<String>,
    pub location: Option<String>,
    pub official_number: Option<String>,
    pub primary_hospital_id: Option<Uuid>,
    /// Assistants: the doctor they work for (doc 2.1).
    pub employer_doctor_id: Option<Uuid>,
    pub headline: Option<String>,
    /// Set when an administrator created the account (links the first Auth0 sign-in).
    pub email: Option<String>,
    /// False once an administrator deactivated the account: no sign-in, hidden from search.
    pub active: bool,
}

impl UserRec {
    pub fn primary_role(&self) -> Role {
        for r in [Role::AppAdministrator, Role::Administrator, Role::Hospital, Role::Lab, Role::Doctor, Role::Assistant] {
            if self.roles.contains(&r) { return r; }
        }
        Role::Patient
    }
}

type UserRow = (
    Uuid, Option<String>, Option<String>, Option<HashSet<String>>, Option<String>, Option<String>,
    Option<String>, Option<Uuid>, Option<Uuid>, Option<String>, Option<String>, Option<String>,
);

pub const DEACTIVATED: &str = "DEACTIVATED";

fn to_rec(r: UserRow) -> UserRec {
    UserRec {
        user_id: r.0,
        public_id: r.1.unwrap_or_default(),
        display_name: r.2.unwrap_or_default(),
        roles: r.3.unwrap_or_default().iter().filter_map(|s| parse::<Role>(s)).collect(),
        photo_uri: r.4,
        location: r.5,
        official_number: r.6,
        primary_hospital_id: r.7,
        employer_doctor_id: r.8,
        headline: r.9,
        email: r.10,
        active: r.11.as_deref() != Some(DEACTIVATED),
    }
}

/// Lower-case words of a name, used by the SAI index for name search.
pub fn name_tokens(name: &str) -> HashSet<String> {
    name.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !matches!(w.to_lowercase().as_str(), "dr" | "mr" | "mrs" | "ms"))
        .map(|w| w.to_lowercase())
        .collect()
}

pub struct NewUser<'a> {
    pub user_id: Uuid,
    pub public_id: Option<&'a str>,
    pub auth0_sub: Option<&'a str>,
    pub display_name: &'a str,
    pub roles: &'a [Role],
    pub location: Option<&'a str>,
    pub official_number: Option<&'a str>,
    pub primary_hospital_id: Option<Uuid>,
    pub employer_doctor_id: Option<Uuid>,
    pub headline: Option<&'a str>,
    /// Administrator-created accounts: the person's email and who created it.
    pub email: Option<&'a str>,
    pub created_by: Option<Uuid>,
}

impl Db {
    pub async fn user(&self, id: Uuid) -> Result<Option<UserRec>> {
        const Q: &str = "SELECT user_id, public_id, display_name, roles, photo_uri, location, official_number, primary_hospital_id, employer_doctor_id, headline, email, status FROM users WHERE user_id = ?";
        Ok(self.one::<UserRow>(Q, (id,)).await?.map(to_rec))
    }

    pub async fn user_id_by_email(&self, email: &str) -> Result<Option<Uuid>> {
        Ok(self.one::<(Uuid,)>("SELECT user_id FROM users_by_email WHERE email = ?", (email.trim().to_lowercase(),)).await?.map(|r| r.0))
    }

    /// Deactivate (administrator "delete") or reactivate an account. Nothing else is removed.
    pub async fn set_active(&self, user: Uuid, active: bool, by: Uuid) -> Result<()> {
        if active {
            self.exec("UPDATE users SET status = null, deactivated_at = null, deactivated_by = null WHERE user_id = ?", (user,)).await?;
        } else {
            self.exec("UPDATE users SET status = ?, deactivated_at = ?, deactivated_by = ? WHERE user_id = ?", (DEACTIVATED, now_ts(), by, user)).await?;
        }
        Ok(())
    }

    pub async fn user_id_by_sub(&self, sub: &str) -> Result<Option<Uuid>> {
        Ok(self.one::<(Uuid,)>("SELECT user_id FROM users_by_sub WHERE auth0_sub = ?", (sub,)).await?.map(|r| r.0))
    }

    pub async fn user_id_by_public_id(&self, public_id: &str) -> Result<Option<Uuid>> {
        Ok(self.one::<(Uuid,)>("SELECT user_id FROM users_by_public_id WHERE public_id = ?", (public_id.to_uppercase(),)).await?.map(|r| r.0))
    }

    /// Name search via the SAI index on `name_tokens` (doc 7.1). Profiles only.
    pub async fn search_by_name(&self, token: &str) -> Result<Vec<UserRec>> {
        const Q: &str = "SELECT user_id, public_id, display_name, roles, photo_uri, location, official_number, primary_hospital_id, employer_doctor_id, headline, email, status FROM users WHERE name_tokens CONTAINS ? LIMIT 50";
        Ok(self.rows::<UserRow>(Q, (token.to_lowercase(),)).await?.into_iter().map(to_rec).collect())
    }

    /// Short shareable id such as `HL-4K7Q2` (no 0/O/1/I to avoid misreading).
    pub async fn new_public_id(&self) -> Result<String> {
        const ALPHABET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
        loop {
            let id: String = {
                let mut rng = rand::thread_rng();
                (0..5).map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char).collect()
            };
            let id = format!("HL-{id}");
            if self.user_id_by_public_id(&id).await?.is_none() {
                return Ok(id);
            }
        }
    }

    pub async fn create_user(&self, u: NewUser<'_>) -> Result<UserRec> {
        let public_id = match u.public_id { Some(p) => p.to_string(), None => self.new_public_id().await? };
        let roles: HashSet<String> = u.roles.iter().map(text).collect();
        let tokens = name_tokens(u.display_name);
        let email = u.email.map(|e| e.trim().to_lowercase());
        self.exec(
            "INSERT INTO users (user_id, auth0_sub, public_id, roles, display_name, name_tokens, location, official_number, primary_hospital_id, employer_doctor_id, headline, created_at, email, created_by) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (u.user_id, u.auth0_sub, &public_id, &roles, u.display_name, &tokens, u.location, u.official_number, u.primary_hospital_id, u.employer_doctor_id, u.headline, now_ts(), &email, u.created_by),
        ).await?;
        if let Some(e) = &email {
            self.exec("INSERT INTO users_by_email (email, user_id) VALUES (?, ?)", (e, u.user_id)).await?;
        }
        self.exec(
            "INSERT INTO users_by_public_id (public_id, user_id, display_name, roles, location) VALUES (?, ?, ?, ?, ?)",
            (&public_id, u.user_id, u.display_name, &roles, u.location),
        ).await?;
        if let Some(sub) = u.auth0_sub {
            self.link_sub(sub, u.user_id).await?;
        }
        Ok(self.user(u.user_id).await?.expect("just inserted"))
    }

    pub async fn link_sub(&self, sub: &str, user_id: Uuid) -> Result<()> {
        self.exec("INSERT INTO users_by_sub (auth0_sub, user_id) VALUES (?, ?)", (sub, user_id)).await?;
        self.exec("UPDATE users SET auth0_sub = ? WHERE user_id = ?", (sub, user_id)).await?;
        Ok(())
    }

    pub async fn update_profile(&self, u: &UserRec, display_name: &str, location: Option<&str>) -> Result<()> {
        self.batch(
            &[
                "UPDATE users SET display_name = ?, name_tokens = ?, location = ? WHERE user_id = ?",
                "UPDATE users_by_public_id SET display_name = ?, location = ? WHERE public_id = ?",
            ],
            ((display_name, name_tokens(display_name), location, u.user_id), (display_name, location, &u.public_id)),
        ).await
    }

    /// Profile picture: an `obj:` URI of an uploaded image, or None to remove it.
    pub async fn set_photo(&self, u: &UserRec, photo_uri: Option<&str>) -> Result<()> {
        self.batch(
            &[
                "UPDATE users SET photo_uri = ? WHERE user_id = ?",
                "UPDATE users_by_public_id SET photo_uri = ? WHERE public_id = ?",
            ],
            ((photo_uri, u.user_id), (photo_uri, &u.public_id)),
        ).await
    }

    // ---- affiliations (rules 3–5) ----

    pub async fn active_hospitals(&self, doctor: Uuid) -> Result<HashSet<Uuid>> {
        let rows = self.rows::<(Uuid, Option<CqlTimestamp>)>(
            "SELECT hospital_id, ended_at FROM affiliations_by_doctor WHERE doctor_id = ?", (doctor,)).await?;
        Ok(rows.into_iter().filter(|r| r.1.is_none()).map(|r| r.0).collect())
    }

    pub async fn hospital_doctors(&self, hospital: Uuid) -> Result<Vec<Uuid>> {
        let rows = self.rows::<(Uuid, Option<CqlTimestamp>)>(
            "SELECT doctor_id, ended_at FROM affiliations_by_hospital WHERE hospital_id = ?", (hospital,)).await?;
        Ok(rows.into_iter().filter(|r| r.1.is_none()).map(|r| r.0).collect())
    }

    /// Every doctor ever affiliated with the hospital, with whether the affiliation is active.
    pub async fn hospital_doctors_all(&self, hospital: Uuid) -> Result<Vec<(Uuid, bool)>> {
        let rows = self.rows::<(Uuid, Option<CqlTimestamp>)>(
            "SELECT doctor_id, ended_at FROM affiliations_by_hospital WHERE hospital_id = ?", (hospital,)).await?;
        Ok(rows.into_iter().map(|r| (r.0, r.1.is_none())).collect())
    }

    pub async fn add_affiliation(&self, doctor: Uuid, hospital: Uuid) -> Result<()> {
        let now = now_ts();
        self.batch(
            &[
                "INSERT INTO affiliations_by_doctor (doctor_id, hospital_id, role, started_at) VALUES (?, ?, 'DOCTOR', ?)",
                "INSERT INTO affiliations_by_hospital (hospital_id, doctor_id, started_at) VALUES (?, ?, ?)",
            ],
            ((doctor, hospital, now), (hospital, doctor, now)),
        ).await
    }

    /// Rule 5: set ended_at in both tables; no item rows change.
    pub async fn end_affiliation(&self, doctor: Uuid, hospital: Uuid) -> Result<()> {
        let now = now_ts();
        self.batch(
            &[
                "UPDATE affiliations_by_doctor SET ended_at = ? WHERE doctor_id = ? AND hospital_id = ?",
                "UPDATE affiliations_by_hospital SET ended_at = ? WHERE hospital_id = ? AND doctor_id = ?",
            ],
            ((now, doctor, hospital), (now, hospital, doctor)),
        ).await
    }

    // ---- delegations (rule 7) ----

    pub async fn delegation_exists(&self, assistant: Uuid, doctor: Uuid, patient: Uuid) -> Result<bool> {
        Ok(self.one::<(Uuid,)>(
            "SELECT patient_id FROM delegations_by_assistant WHERE assistant_id = ? AND doctor_id = ? AND patient_id = ?",
            (assistant, doctor, patient)).await?.is_some())
    }

    pub async fn add_delegation(&self, assistant: Uuid, doctor: Uuid, patient: Uuid) -> Result<()> {
        self.exec("INSERT INTO delegations_by_assistant (assistant_id, doctor_id, patient_id, granted_at) VALUES (?, ?, ?, ?)",
            (assistant, doctor, patient, now_ts())).await?;
        Ok(())
    }

    pub async fn delegated_patients(&self, assistant: Uuid) -> Result<Vec<Uuid>> {
        Ok(self.rows::<(Uuid,)>("SELECT patient_id FROM delegations_by_assistant WHERE assistant_id = ?", (assistant,))
            .await?.into_iter().map(|r| r.0).collect())
    }
}
