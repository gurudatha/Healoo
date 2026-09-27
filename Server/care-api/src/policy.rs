//! Access rules from design doc 2.3. Pure functions: the caller loads the facts (grants,
//! active affiliations, delegations) and this module decides. Every decision carries a reason
//! that ends up in `GET /v1/check/access` and the `access.audit` topic.

use crate::model::{GranteeType, Role};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Access {
    Full,
    MetadataOnly,
    Deny,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Decision {
    pub access: Access,
    pub reason: &'static str,
}

impl Decision {
    fn full(reason: &'static str) -> Self { Decision { access: Access::Full, reason } }
    fn meta(reason: &'static str) -> Self { Decision { access: Access::MetadataOnly, reason } }
    fn deny(reason: &'static str) -> Self { Decision { access: Access::Deny, reason } }
}

/// What the policy needs to know about an item.
#[derive(Clone, Debug)]
pub struct ItemFacts {
    pub owner_id: Uuid,
    pub created_by: Uuid,
    /// True when the item contains report files (access rule 2 applies to the whole item).
    pub has_report_files: bool,
    pub grants: Vec<(GranteeType, Uuid)>,
}

/// What the policy needs to know about the caller.
#[derive(Clone, Debug, Default)]
pub struct Subject {
    pub user_id: Uuid,
    pub roles: HashSet<Role>,
    /// Hospitals with an active affiliation (ended_at is null) — rule 3/5.
    pub active_hospitals: HashSet<Uuid>,
}

/// For assistants (rule 7): the doctor they work for, and whether that doctor delegated this patient.
#[derive(Clone, Debug, Default)]
pub struct AssistantContext {
    pub doctor: Option<Subject>,
    pub delegated_for_owner: bool,
}

pub fn decide(item: &ItemFacts, who: &Subject, assistant: Option<&AssistantContext>) -> Decision {
    // Rule 1: the owner can always read and manage.
    if item.owner_id == who.user_id {
        return Decision::full("owner");
    }
    let is_doctor = who.roles.contains(&Role::Doctor);

    // Rule 2 (labs and hospitals): they see the items they uploaded for a patient.
    if item.created_by == who.user_id && (who.roles.contains(&Role::Lab) || who.roles.contains(&Role::Hospital)) {
        return Decision::full("lab or hospital uploaded this item");
    }

    // Rules 3, 4, 6: direct grant, or hospital grant + active affiliation.
    let direct = item.grants.iter().any(|(t, id)| *t == GranteeType::User && *id == who.user_id);
    let via_hospital = is_doctor
        && item.grants.iter().any(|(t, id)| *t == GranteeType::Hospital && who.active_hospitals.contains(id));

    if direct || via_hospital {
        // Rule 2: items with report files are readable by doctors only (and the owner, above).
        if item.has_report_files && !is_doctor {
            return Decision::meta("report content is readable by doctors only");
        }
        return Decision::full(if direct { "direct grant" } else { "hospital grant with active affiliation" });
    }

    // Rule 7: assistants.
    if who.roles.contains(&Role::Assistant) {
        if let Some(ctx) = assistant {
            if let Some(doc) = &ctx.doctor {
                let doctor_decision = decide(item, doc, None);
                if doctor_decision.access == Access::Full {
                    return if ctx.delegated_for_owner {
                        Decision::full("assistant with delegation from the doctor")
                    } else {
                        Decision::meta("assistant without delegation: metadata only")
                    };
                }
            }
        }
    }

    if is_doctor && item.grants.iter().any(|(t, _)| *t == GranteeType::Hospital) {
        return Decision::deny("hospital grant, but no active affiliation with that hospital");
    }
    Decision::deny("no grant for this user")
}

/// Rule 9 (Documentation/PageOperations_Design.md 5): who may add a grant to an existing item.
/// The owner shares with anyone. A doctor, lab or hospital that can fully read the item may pass
/// it on to a doctor (a referral); the owner still sees that grant and can revoke it.
pub fn may_share(owner_id: Uuid, who: &Subject, access: Access, grantee_roles: &HashSet<Role>, via_hospital: bool) -> Decision {
    if owner_id == who.user_id {
        return Decision::full("owner shares with anyone");
    }
    if !who.roles.iter().any(|r| r.can_refer()) {
        return Decision::deny("only the owner can share an item; doctors, labs and hospitals can pass it to a doctor");
    }
    if access != Access::Full {
        return Decision::deny("you can pass on only items you can fully read");
    }
    if via_hospital || !grantee_roles.contains(&Role::Doctor) {
        return Decision::deny("an item you don't own can be passed on to doctors only");
    }
    Decision::full("referral to a doctor")
}

/// Rule 9 at creation: someone creating an item for a patient may also share it, with doctors only.
pub fn may_share_on_create(for_someone_else: bool, grantee_roles: &HashSet<Role>) -> bool {
    !for_someone_else || grantee_roles.contains(&Role::Doctor)
}

/// Rule 10 (Documentation/Administration_Design.md): the hospital an administrator acts for.
/// A hospital administrator acts only for their own hospital; a platform administrator for the
/// hospital they name. Anyone else is refused.
pub fn admin_hospital(roles: &HashSet<Role>, own_hospital: Option<Uuid>, requested: Option<Uuid>) -> Result<Uuid, &'static str> {
    if roles.contains(&Role::AppAdministrator) {
        return requested.or(own_hospital).ok_or("platform administrators must name the hospital (hospital_id)");
    }
    if !roles.contains(&Role::Administrator) {
        return Err("only hospital administrators can manage accounts");
    }
    let own = own_hospital.ok_or("this administrator account has no hospital")?;
    match requested {
        Some(h) if h != own => Err("you can manage accounts only for your own hospital"),
        _ => Ok(own),
    }
}

/// Rule 10: administrators create patients (users) and doctors, nothing else.
pub fn may_create_account(admin_roles: &HashSet<Role>, role: Role) -> Decision {
    if !(admin_roles.contains(&Role::Administrator) || admin_roles.contains(&Role::AppAdministrator)) {
        return Decision::deny("only administrators can create accounts");
    }
    match role {
        Role::Patient => Decision::full("administrator creates a user"),
        Role::Doctor => Decision::full("administrator creates a doctor"),
        _ => Decision::deny("administrators can create users and doctors only"),
    }
}

/// Rule 10: "delete" is deactivation, allowed for doctors only — a doctor of the administrator's
/// hospital (any doctor for a platform administrator). Users (patients) and every other account
/// can never be deleted by an administrator: they own health records.
pub fn may_deactivate(admin_roles: &HashSet<Role>, admin_hospital: Option<Uuid>, target_roles: &HashSet<Role>, target_hospitals: &HashSet<Uuid>, target_home_hospital: Option<Uuid>) -> Decision {
    if !target_roles.contains(&Role::Doctor) {
        return Decision::deny("users can't be deleted; administrators can delete (deactivate) doctors only");
    }
    if admin_roles.contains(&Role::AppAdministrator) {
        return Decision::full("platform administrator");
    }
    if !admin_roles.contains(&Role::Administrator) {
        return Decision::deny("only hospital administrators can delete doctors");
    }
    match admin_hospital {
        Some(h) if target_hospitals.contains(&h) || target_home_hospital == Some(h) => Decision::full("doctor of the administrator's hospital"),
        _ => Decision::deny("you can delete only doctors of your own hospital"),
    }
}

#[cfg(test)]
mod tests {
    //! Mirrors acceptance tests 2–7 of design doc 10.4, and tests P1–P6 of
    //! Documentation/PageOperations_Design.md 7 (hospital uploads and referrals).
    use super::*;

    fn roles(r: &[Role]) -> HashSet<Role> { r.iter().copied().collect() }

    fn subject(roles: &[Role], hospitals: &[Uuid]) -> Subject {
        Subject { user_id: Uuid::new_v4(), roles: roles.iter().copied().collect(), active_hospitals: hospitals.iter().copied().collect() }
    }

    fn report(owner: Uuid, created_by: Uuid, grants: Vec<(GranteeType, Uuid)>) -> ItemFacts {
        ItemFacts { owner_id: owner, created_by, has_report_files: true, grants }
    }

    #[test]
    fn owner_always_reads() {
        let p = subject(&[Role::Patient], &[]);
        assert_eq!(decide(&report(p.user_id, Uuid::new_v4(), vec![]), &p, None).access, Access::Full);
    }

    #[test]
    fn test2_lab_upload_visible_to_lab_and_patient_not_doctor() {
        let patient = subject(&[Role::Patient], &[]);
        let lab = subject(&[Role::Lab], &[]);
        let doctor = subject(&[Role::Doctor], &[]);
        let item = report(patient.user_id, lab.user_id, vec![]);
        assert_eq!(decide(&item, &patient, None).access, Access::Full);
        assert_eq!(decide(&item, &lab, None).access, Access::Full);
        assert_eq!(decide(&item, &doctor, None).access, Access::Deny);
    }

    #[test]
    fn p2_owner_shares_with_anyone() {
        let owner = subject(&[Role::Patient], &[]);
        for grantee in [Role::Patient, Role::Doctor, Role::Hospital, Role::Lab] {
            assert_eq!(may_share(owner.user_id, &owner, Access::Full, &roles(&[grantee]), false).access, Access::Full);
        }
    }

    #[test]
    fn p3_doctor_lab_and_hospital_refer_to_a_doctor() {
        let owner = Uuid::new_v4();
        for sharer in [Role::Doctor, Role::Lab, Role::Hospital] {
            let who = subject(&[sharer], &[]);
            assert_eq!(may_share(owner, &who, Access::Full, &roles(&[Role::Doctor]), false).access, Access::Full, "{sharer:?}");
        }
    }

    #[test]
    fn p4_referral_only_to_doctors_and_only_with_full_access() {
        let owner = Uuid::new_v4();
        let doctor = subject(&[Role::Doctor], &[]);
        for grantee in [Role::Patient, Role::Hospital, Role::Lab] {
            assert_eq!(may_share(owner, &doctor, Access::Full, &roles(&[grantee]), false).access, Access::Deny, "{grantee:?}");
        }
        assert_eq!(may_share(owner, &doctor, Access::Full, &roles(&[Role::Doctor]), true).access, Access::Deny); // as a hospital grant
        assert_eq!(may_share(owner, &doctor, Access::MetadataOnly, &roles(&[Role::Doctor]), false).access, Access::Deny);
    }

    #[test]
    fn p5_patients_and_assistants_cannot_share_what_they_do_not_own() {
        let owner = Uuid::new_v4();
        for sharer in [Role::Patient, Role::Assistant] {
            let who = subject(&[sharer], &[]);
            assert_eq!(may_share(owner, &who, Access::Full, &roles(&[Role::Doctor]), false).access, Access::Deny, "{sharer:?}");
        }
    }

    #[test]
    fn p6_creating_for_a_patient_shares_with_doctors_only() {
        assert!(may_share_on_create(true, &roles(&[Role::Doctor])));
        assert!(!may_share_on_create(true, &roles(&[Role::Patient])));
        assert!(!may_share_on_create(true, &roles(&[Role::Hospital])));
        assert!(may_share_on_create(false, &roles(&[Role::Patient])));   // own item: anyone
    }

    // ---- Rule 10: administrators (Documentation/Administration_Design.md, tests A1–A6) ----

    #[test]
    fn a1_admin_creates_users_and_doctors_only() {
        for admin in [Role::Administrator, Role::AppAdministrator] {
            assert_eq!(may_create_account(&roles(&[admin]), Role::Patient).access, Access::Full);
            assert_eq!(may_create_account(&roles(&[admin]), Role::Doctor).access, Access::Full);
            for other in [Role::Lab, Role::Hospital, Role::Assistant, Role::Administrator, Role::AppAdministrator] {
                assert_eq!(may_create_account(&roles(&[admin]), other).access, Access::Deny, "{other:?}");
            }
        }
    }

    #[test]
    fn a2_non_admins_cannot_create_accounts() {
        for who in [Role::Patient, Role::Doctor, Role::Hospital, Role::Lab, Role::Assistant] {
            assert_eq!(may_create_account(&roles(&[who]), Role::Patient).access, Access::Deny, "{who:?}");
        }
    }

    #[test]
    fn a3_admin_deletes_doctor_of_own_hospital() {
        let h = Uuid::new_v4();
        let doctor = roles(&[Role::Doctor]);
        assert_eq!(may_deactivate(&roles(&[Role::Administrator]), Some(h), &doctor, &[h].into(), Some(h)).access, Access::Full);
        // already off the hospital's list, but it is still their home hospital
        assert_eq!(may_deactivate(&roles(&[Role::Administrator]), Some(h), &doctor, &HashSet::new(), Some(h)).access, Access::Full);
    }

    #[test]
    fn a4_admin_cannot_delete_doctor_of_another_hospital() {
        let (mine, other) = (Uuid::new_v4(), Uuid::new_v4());
        let d = may_deactivate(&roles(&[Role::Administrator]), Some(mine), &roles(&[Role::Doctor]), &[other].into(), Some(other));
        assert_eq!(d.access, Access::Deny);
        assert_eq!(may_deactivate(&roles(&[Role::AppAdministrator]), None, &roles(&[Role::Doctor]), &[other].into(), Some(other)).access, Access::Full);
    }

    #[test]
    fn a5_admin_can_never_delete_a_user() {
        let h = Uuid::new_v4();
        for admin in [Role::Administrator, Role::AppAdministrator] {
            for target in [Role::Patient, Role::Lab, Role::Hospital, Role::Assistant, Role::Administrator] {
                let d = may_deactivate(&roles(&[admin]), Some(h), &roles(&[target]), &[h].into(), Some(h));
                assert_eq!(d.access, Access::Deny, "{admin:?} deleting {target:?}");
            }
        }
        for who in [Role::Patient, Role::Doctor, Role::Hospital] {
            assert_eq!(may_deactivate(&roles(&[who]), Some(h), &roles(&[Role::Doctor]), &[h].into(), Some(h)).access, Access::Deny);
        }
    }

    #[test]
    fn a6_admin_acts_only_for_own_hospital() {
        let (mine, other) = (Uuid::new_v4(), Uuid::new_v4());
        let admin = roles(&[Role::Administrator]);
        assert_eq!(admin_hospital(&admin, Some(mine), None), Ok(mine));
        assert_eq!(admin_hospital(&admin, Some(mine), Some(mine)), Ok(mine));
        assert!(admin_hospital(&admin, Some(mine), Some(other)).is_err());
        assert!(admin_hospital(&admin, None, None).is_err());
        assert!(admin_hospital(&roles(&[Role::Doctor]), Some(mine), None).is_err());
        assert_eq!(admin_hospital(&roles(&[Role::AppAdministrator]), None, Some(other)), Ok(other));
        assert!(admin_hospital(&roles(&[Role::AppAdministrator]), None, None).is_err());
    }

    #[test]
    fn p1_hospital_upload_visible_to_hospital_and_patient() {
        let patient = subject(&[Role::Patient], &[]);
        let hospital = subject(&[Role::Hospital], &[]);
        let item = report(patient.user_id, hospital.user_id, vec![(GranteeType::Hospital, hospital.user_id)]);
        assert_eq!(decide(&item, &hospital, None).access, Access::Full);
        assert_eq!(decide(&item, &patient, None).access, Access::Full);
    }

    #[test]
    fn test3_hospital_share_reaches_affiliated_doctors_only() {
        let hospital = Uuid::new_v4();
        let owner = Uuid::new_v4();
        let affiliated = subject(&[Role::Doctor], &[hospital]);
        let independent = subject(&[Role::Doctor], &[]);
        let item = report(owner, owner, vec![(GranteeType::Hospital, hospital)]);
        assert_eq!(decide(&item, &affiliated, None).access, Access::Full);
        assert_eq!(decide(&item, &independent, None).access, Access::Deny);
    }

    #[test]
    fn test4_and_5_leaving_hospital_then_direct_share() {
        let hospital = Uuid::new_v4();
        let owner = Uuid::new_v4();
        let mut doctor = subject(&[Role::Doctor], &[hospital]);
        let mut item = report(owner, owner, vec![(GranteeType::Hospital, hospital)]);
        assert_eq!(decide(&item, &doctor, None).access, Access::Full);
        doctor.active_hospitals.clear(); // affiliation ended
        assert_eq!(decide(&item, &doctor, None).access, Access::Deny);
        item.grants.push((GranteeType::User, doctor.user_id)); // patient shares directly
        assert_eq!(decide(&item, &doctor, None).access, Access::Full);
    }

    #[test]
    fn test6_and_7_assistant_needs_delegation() {
        let owner = Uuid::new_v4();
        let doctor = subject(&[Role::Doctor], &[]);
        let assistant = subject(&[Role::Assistant], &[]);
        let item = report(owner, owner, vec![(GranteeType::User, doctor.user_id)]);
        let without = AssistantContext { doctor: Some(doctor.clone()), delegated_for_owner: false };
        let with = AssistantContext { doctor: Some(doctor), delegated_for_owner: true };
        assert_eq!(decide(&item, &assistant, Some(&without)).access, Access::MetadataOnly);
        assert_eq!(decide(&item, &assistant, Some(&with)).access, Access::Full);
    }

    #[test]
    fn item_without_report_files_is_fully_shared_with_non_doctor() {
        let owner = Uuid::new_v4();
        let family = subject(&[Role::Patient], &[]);
        let item = ItemFacts { owner_id: owner, created_by: owner, has_report_files: false, grants: vec![(GranteeType::User, family.user_id)] };
        assert_eq!(decide(&item, &family, None).access, Access::Full);
    }

    #[test]
    fn report_shared_with_non_doctor_is_metadata_only() {
        let owner = Uuid::new_v4();
        let friend = subject(&[Role::Patient], &[]);
        let item = report(owner, owner, vec![(GranteeType::User, friend.user_id)]);
        assert_eq!(decide(&item, &friend, None).access, Access::MetadataOnly);
    }
}
