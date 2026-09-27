//! /v1/admin/* — hospital administrators manage accounts (rule 10,
//! Documentation/Administration_Design.md): add users (patients) and doctors, delete
//! (deactivate) and reactivate doctors. Users can never be deleted.

use super::{principal::Principal, users::{load_user, user_dto}, ApiError, ApiResult, AppState};
use crate::{
    events::{topics, Envelope},
    model::*,
    policy::{self, Access},
    store::users::{NewUser, UserRec},
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

/// A profile as the administrator sees it: the public profile plus email and status.
#[derive(Serialize)]
pub struct AdminUserDto {
    #[serde(flatten)]
    user: UserDto,
    email: Option<String>,
    active: bool,
}

async fn admin_dto(st: &AppState, u: &UserRec) -> ApiResult<AdminUserDto> {
    Ok(AdminUserDto { user: user_dto(st, u, false).await?, email: u.email.clone(), active: u.active })
}

#[derive(Deserialize)]
pub struct NewAccount {
    display_name: String,
    email: String,
    location: Option<String>,
    /// Doctors: e.g. "Cardiologist". Shown as "Doctor · Cardiologist" (no department field yet).
    designation: Option<String>,
    /// Doctors: registration number.
    official_number: Option<String>,
    /// Platform administrators name the hospital; hospital administrators may omit it.
    hospital_id: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct HospitalQ { hospital_id: Option<Uuid> }

fn hospital_for(p: &Principal, requested: Option<Uuid>) -> ApiResult<Uuid> {
    policy::admin_hospital(&p.user.roles, p.user.primary_hospital_id, requested).map_err(ApiError::forbidden)
}

fn clean(s: &Option<String>, max: usize) -> Option<String> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|s| s.chars().take(max).collect())
}

/// Checks the form and the email's uniqueness; returns (name, email).
async fn validate(st: &AppState, b: &NewAccount) -> ApiResult<(String, String)> {
    let name = b.display_name.trim();
    if name.is_empty() || name.chars().count() > 80 { return Err(ApiError::bad_request("display_name must be 1–80 characters")); }
    let email = b.email.trim().to_lowercase();
    let valid = email.len() <= 254 && email.split_once('@').is_some_and(|(a, d)| !a.is_empty() && d.contains('.') && !d.starts_with('.') && !d.ends_with('.'));
    if !valid { return Err(ApiError::bad_request("email must be a valid address")); }
    if st.db.user_id_by_email(&email).await?.is_some() { return Err(ApiError::conflict("an account with that email already exists")); }
    Ok((name.to_string(), email))
}

async fn create(st: &AppState, p: &Principal, b: &NewAccount, role: Role, hospital: Uuid) -> ApiResult<UserRec> {
    let d = policy::may_create_account(&p.user.roles, role);
    if d.access != Access::Full { return Err(ApiError::forbidden(d.reason)); }
    let (name, email) = validate(st, b).await?;
    let doctor = role == Role::Doctor;
    let headline = if doctor { clean(&b.designation, 40).map(|d| format!("Doctor · {d}")) } else { None };
    let location = clean(&b.location, 80);
    let reg = if doctor { clean(&b.official_number, 40) } else { None };
    let u = st.db.create_user(NewUser {
        user_id: Uuid::new_v4(),
        public_id: None,
        auth0_sub: None,
        display_name: &name,
        roles: &[role],
        location: location.as_deref(),
        official_number: reg.as_deref(),
        primary_hospital_id: doctor.then_some(hospital),
        employer_doctor_id: None,
        headline: headline.as_deref(),
        email: Some(&email),
        created_by: Some(p.id()),
    }).await?;
    if doctor {
        st.db.add_affiliation(u.user_id, hospital).await?;
        let h = load_user(st, hospital).await?;
        st.db.connect_users(u.user_id, Role::Doctor, &u.display_name, h.user_id, h.primary_role(), &h.display_name).await?;
    }
    tracing::info!(admin = %p.id(), user_id = %u.user_id, public_id = %u.public_id, role = ?role, "administrator created account");
    Ok(u)
}

/// `POST /v1/admin/users` — a new user (patient) account.
pub async fn create_user(State(st): State<AppState>, p: Principal, Json(b): Json<NewAccount>) -> ApiResult<(StatusCode, Json<AdminUserDto>)> {
    let hospital = hospital_for(&p, b.hospital_id)?;
    let u = create(&st, &p, &b, Role::Patient, hospital).await?;
    Ok((StatusCode::CREATED, Json(admin_dto(&st, &u).await?)))
}

/// `POST /v1/admin/doctors` — a new doctor, working at the administrator's hospital.
pub async fn create_doctor(State(st): State<AppState>, p: Principal, Json(b): Json<NewAccount>) -> ApiResult<(StatusCode, Json<AdminUserDto>)> {
    let hospital = hospital_for(&p, b.hospital_id)?;
    let u = create(&st, &p, &b, Role::Doctor, hospital).await?;
    Ok((StatusCode::CREATED, Json(admin_dto(&st, &u).await?)))
}

/// `GET /v1/admin/doctors` — the hospital's doctors, deactivated ones included (active = false).
pub async fn doctors(State(st): State<AppState>, p: Principal, Query(q): Query<HospitalQ>) -> ApiResult<Json<Page<AdminUserDto>>> {
    let hospital = hospital_for(&p, q.hospital_id)?;
    let mut out = Vec::new();
    for (id, affiliated) in st.db.hospital_doctors_all(hospital).await? {
        let Some(u) = st.db.user(id).await? else { continue };
        // Doctors who left the hospital without being deactivated belong to someone else now.
        if affiliated || (!u.active && u.primary_hospital_id == Some(hospital)) { out.push(admin_dto(&st, &u).await?); }
    }
    out.sort_by(|a, b| (!a.active, &a.user.display_name).cmp(&(!b.active, &b.user.display_name)));
    Ok(Json(Page::of(out)))
}

async fn doctor_decision(st: &AppState, p: &Principal, doctor: &UserRec) -> ApiResult<()> {
    let admin_hospital = hospital_for(p, None).ok();
    let hospitals = st.db.active_hospitals(doctor.user_id).await?;
    let d = policy::may_deactivate(&p.user.roles, admin_hospital, &doctor.roles, &hospitals, doctor.primary_hospital_id);
    if d.access != Access::Full { return Err(ApiError::forbidden(d.reason)); }
    Ok(())
}

/// `DELETE /v1/admin/doctors/{id}` — deactivate: the doctor can't sign in, is hidden from search
/// and leaves the hospital (access through it stops at once). Items, messages and appointments stay.
pub async fn deactivate_doctor(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    let doctor = load_user(&st, id).await?;
    doctor_decision(&st, &p, &doctor).await?;
    if !doctor.active { return Ok(StatusCode::NO_CONTENT); }
    let hospitals = st.db.active_hospitals(id).await?;
    for h in &hospitals { st.db.end_affiliation(id, *h).await?; }
    st.db.set_active(id, false, p.id()).await?;
    st.principals.clear(); // drop cached sign-ins and hospital lists
    let ev = Envelope::new("user.deactivated", p.id(), id, vec![id.to_string()], json!({ "user_id": id, "hospitals": hospitals }));
    st.events.emit(topics::ACCESS_CHANGES, &id.to_string(), ev).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /v1/admin/doctors/{id}/reactivate` — undo a deactivation; rejoins the hospital.
pub async fn reactivate_doctor(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<Json<AdminUserDto>> {
    let doctor = load_user(&st, id).await?;
    doctor_decision(&st, &p, &doctor).await?;
    let hospital = hospital_for(&p, doctor.primary_hospital_id)?;
    st.db.set_active(id, true, p.id()).await?;
    st.db.add_affiliation(id, hospital).await?;
    st.principals.clear();
    let ev = Envelope::new("user.reactivated", p.id(), id, vec![id.to_string()], json!({ "user_id": id, "hospital_id": hospital }));
    st.events.emit(topics::ACCESS_CHANGES, &id.to_string(), ev).await?;
    Ok(Json(admin_dto(&st, &load_user(&st, id).await?).await?))
}

/// `DELETE /v1/admin/users/{id}` — always refused: users own health records and can't be
/// deleted by an administrator (rule 10). Doctors are deleted with `/v1/admin/doctors/{id}`.
pub async fn delete_user(State(st): State<AppState>, p: Principal, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    let target = load_user(&st, id).await?;
    let admin_hospital = hospital_for(&p, None).ok();
    let d = policy::may_deactivate(&p.user.roles, admin_hospital, &target.roles, &Default::default(), target.primary_hospital_id);
    Err(ApiError::forbidden(if d.access == Access::Full { "doctors are deleted with DELETE /v1/admin/doctors/{id}" } else { d.reason }))
}
