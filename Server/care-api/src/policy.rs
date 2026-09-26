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

    // Rule 2 (labs): a lab sees the reports it uploaded.
    if who.roles.contains(&Role::Lab) && item.created_by == who.user_id {
        return Decision::full("lab uploaded this item");
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

#[cfg(test)]
mod tests {
    //! Mirrors acceptance tests 2–7 of design doc 10.4.
    use super::*;

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
