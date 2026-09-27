//! Seed data for trials: the same people as the apps' demo mode, a hospital administrator and an
//! assistant (acceptance tests 4, 6, 7), and DataItem v2 cases showing every kind of part.
//! Sample images and PDFs are generated, so no patient-like data is shipped.

use crate::{
    files::{object_uri, ObjectStore},
    model::*,
    recurrence,
    store::{
        items::Item,
        parts::{local_millis, today_in, AlertRow, AppointmentRow, AttachmentRow, DEFAULT_TZ},
        users::NewUser,
        Db,
    },
};
use chrono::NaiveDate;
use anyhow::Result;
use std::io::Cursor;
use uuid::Uuid;

pub const LAKSHMI: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_000000000001);
pub const DR_RAO: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_000000000002);
pub const HOSPITAL_A: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_000000000003);
pub const CITY_LAB: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_000000000004);
pub const DR_SRINIVAS: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_000000000005);
pub const RAO_HEART: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_000000000006);
pub const RAO_DIAG: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_000000000007);
pub const PRIYA: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_000000000008);
pub const ADMIN_A: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_000000000009);
pub const ASSISTANT: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_00000000000a);

// Added for local testing (seeded into an existing trial database too, see `seed_extra`).
pub const RAVI: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_00000000000b);
pub const ANJALI: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_00000000000c);
pub const SURESH: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_00000000000d);
pub const DR_MEHTA: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_00000000000e);
pub const DR_KHAN: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_00000000000f);
pub const DR_MENON: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_000000000010);
pub const DR_NAIR: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_000000000011);
pub const SUNRISE_LAB: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_000000000012);
pub const GREEN_VALLEY_LAB: Uuid = Uuid::from_u128(0x5eed_0000_0000_4000_8000_000000000013);

pub const SEED_PUBLIC_IDS: [&str; 19] = [
    "HL-2M9P4", "HL-7R2C9", "HL-1H0A1", "HL-6L3D2", "HL-3K8M1", "HL-9P4T6", "HL-5W1Q3", "HL-4K7Q2", "HL-8A2D4", "HL-8S5T7",
    "HL-3R7V2", "HL-6A4S9", "HL-9S2Y5", "HL-4M8D3", "HL-7F5K2", "HL-2D5M8", "HL-8N3V6", "HL-5P9L4", "HL-6G2L7",
];

struct Person {
    id: Uuid,
    public_id: &'static str,
    name: &'static str,
    role: Role,
    hospital: Option<Uuid>,
    employer: Option<Uuid>,
    headline: Option<&'static str>,
    reg: Option<&'static str>,
}

fn people() -> Vec<Person> {
    let p = |id, public_id, name, role| Person { id, public_id, name, role, hospital: None, employer: None, headline: None, reg: None };
    vec![
        p(LAKSHMI, "HL-2M9P4", "Lakshmi K.", Role::Patient),
        Person { hospital: Some(HOSPITAL_A), headline: Some("Doctor · General Medicine"), reg: Some("REG-TEST-001"), ..p(DR_RAO, "HL-7R2C9", "Dr. Anitha Rao", Role::Doctor) },
        p(HOSPITAL_A, "HL-1H0A1", "Test Hospital A", Role::Hospital),
        p(CITY_LAB, "HL-6L3D2", "City Diagnostics", Role::Lab),
        Person { headline: Some("Doctor · Independent"), reg: Some("REG-TEST-002"), ..p(DR_SRINIVAS, "HL-3K8M1", "Dr. Srinivas Rao", Role::Doctor) },
        p(RAO_HEART, "HL-9P4T6", "Rao Heart Clinic", Role::Hospital),
        p(RAO_DIAG, "HL-5W1Q3", "Rao Diagnostics", Role::Lab),
        p(PRIYA, "HL-4K7Q2", "Priya Rao", Role::Patient),
        Person { hospital: Some(HOSPITAL_A), ..p(ADMIN_A, "HL-8A2D4", "Hospital A Admin", Role::Administrator) },
        Person { employer: Some(DR_RAO), headline: Some("Assistant to Dr. Anitha Rao"), ..p(ASSISTANT, "HL-8S5T7", "Meena S. (assistant)", Role::Assistant) },
    ]
}

/// More people for local testing. The designation (Cardiologist, Urologist, Paediatrician) is only
/// shown in the headline for now; there is no department field yet.
fn extra_people() -> Vec<Person> {
    let p = |id, public_id, name, role| Person { id, public_id, name, role, hospital: None, employer: None, headline: None, reg: None };
    vec![
        p(RAVI, "HL-3R7V2", "Ravi Kumar", Role::Patient),
        p(ANJALI, "HL-6A4S9", "Anjali Sharma", Role::Patient),
        p(SURESH, "HL-9S2Y5", "Suresh Iyer", Role::Patient),
        // Independent doctors
        Person { headline: Some("Doctor · Paediatrician"), reg: Some("REG-TEST-003"), ..p(DR_MEHTA, "HL-4M8D3", "Dr. Arjun Mehta", Role::Doctor) },
        Person { headline: Some("Doctor · Cardiologist"), reg: Some("REG-TEST-004"), ..p(DR_KHAN, "HL-7F5K2", "Dr. Farah Khan", Role::Doctor) },
        // Doctors at Test Hospital A
        Person { hospital: Some(HOSPITAL_A), headline: Some("Doctor · Cardiologist"), reg: Some("REG-TEST-005"),
                 ..p(DR_MENON, "HL-2D5M8", "Dr. Kavya Menon", Role::Doctor) },
        Person { hospital: Some(HOSPITAL_A), headline: Some("Doctor · Urologist"), reg: Some("REG-TEST-006"),
                 ..p(DR_NAIR, "HL-8N3V6", "Dr. Vikram Nair", Role::Doctor) },
        p(SUNRISE_LAB, "HL-5P9L4", "Sunrise Pathology", Role::Lab),
        p(GREEN_VALLEY_LAB, "HL-6G2L7", "Green Valley Labs", Role::Lab),
    ]
}

/// Contacts for the extra people, so each flow can be tried straight away.
const EXTRA_CONNECTIONS: [(Uuid, Uuid); 11] = [
    (RAVI, DR_MENON), (RAVI, HOSPITAL_A), (RAVI, SUNRISE_LAB),
    (ANJALI, DR_MEHTA), (ANJALI, GREEN_VALLEY_LAB),
    (SURESH, DR_KHAN), (SURESH, DR_NAIR), (SURESH, SUNRISE_LAB),
    (DR_MENON, HOSPITAL_A), (DR_NAIR, HOSPITAL_A), (DR_RAO, DR_MENON),
];

async fn create(db: &Db, p: &Person) -> Result<()> {
    db.create_user(NewUser {
        user_id: p.id,
        public_id: Some(p.public_id),
        auth0_sub: None,
        display_name: p.name,
        roles: &[p.role],
        location: Some("[City]"),
        official_number: p.reg,
        primary_hospital_id: p.hospital,
        employer_doctor_id: p.employer,
        headline: p.headline,
        email: None,
        created_by: None,
    }).await?;
    Ok(())
}

/// Adds the extra people that aren't in the database yet, with their hospital affiliation and
/// contacts. Safe to run on an existing trial database: people already present are left alone.
async fn seed_extra(db: &Db) -> Result<usize> {
    let mut added = Vec::new();
    for p in extra_people() {
        if db.user_id_by_public_id(p.public_id).await?.is_some() { continue; }
        create(db, &p).await?;
        if let Some(h) = p.hospital { db.add_affiliation(p.id, h).await?; }
        added.push(p.id);
    }
    let everyone: Vec<Person> = people().into_iter().chain(extra_people()).collect();
    let who = |id: Uuid| everyone.iter().find(|p| p.id == id).map(|p| (p.role, p.name)).unwrap();
    for (a, b) in EXTRA_CONNECTIONS {
        if !added.contains(&a) && !added.contains(&b) { continue; }
        let ((ra, na), (rb, nb)) = (who(a), who(b));
        db.connect_users(a, ra, na, b, rb, nb).await?;
    }
    if !added.is_empty() { tracing::info!("seeded {} extra users", added.len()); }
    Ok(added.len())
}

fn grant(kind: GranteeType, id: Uuid, name: &str, by: Uuid) -> GrantUdt {
    GrantUdt {
        grant_id: Some(Uuid::new_v4()),
        grantee_type: Some(text(&kind)),
        grantee_id: Some(id),
        grantee_name: Some(name.to_string()),
        via_hospital_id: None,
        granted_by: Some(by),
        granted_at: Some(now_ts()),
    }
}

/// Builds a DataItem v2 header. `kinds` lists every part the seed adds to it.
fn item(owner: Uuid, by: Uuid, by_name: &str, primary: PrimaryKind, kinds: &[&str], title: &str, keywords: &[&str], grants: Vec<GrantUdt>) -> Item {
    let now = now_ts();
    Item {
        id: new_timeuuid(),
        owner_id: owner,
        primary_kind: primary,
        kinds: kinds.iter().map(|k| k.to_string()).collect(),
        title: title.to_string(),
        keywords: keywords.iter().map(|k| k.to_string()).collect(),
        grants,
        status: ItemStatus::Open,
        closure: None,
        has_report_files: kinds.contains(&part::REPORT),
        links: vec![],
        created_by: by,
        created_by_name: by_name.to_string(),
        created_at: now,
        updated_at: now,
    }
}

async fn message(db: &Db, it: &Item, from: Uuid, to: Uuid, body: &str) -> Result<()> {
    let id = new_timeuuid();
    let m = MessageDto { message_id: id, item_id: it.id, sender_id: from, body: body.into(), sent_at: String::new(), attachment_ids: vec![] };
    db.insert_item_message(&m, &format!("seed-{id}")).await?;
    let unread = db.conversation_unread(to, from, it.id).await? + 1;
    db.touch_conversation(to, from, it.id, body, unread).await?;
    db.touch_conversation(from, to, it.id, body, 0).await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn appointment(db: &Db, it: &Item, doctor: Uuid, hospital: Option<Uuid>, date: NaiveDate, time: &str, rec: Option<(&str, Option<&str>)>, notes: &str) -> Result<()> {
    let row = AppointmentRow {
        appointment_id: Uuid::new_v4(),
        patient_id: it.owner_id,
        doctor_id: doctor,
        hospital_id: hospital,
        start_date: recurrence::fmt_date(date),
        start_time: time.into(),
        timezone: Some(DEFAULT_TZ.into()),
        duration_min: Some(15),
        notes: Some(notes.into()),
        frequency: rec.map(|r| r.0.to_string()),
        period: rec.and_then(|r| r.1).map(str::to_string),
        exceptions: None,
        status: Some("SCHEDULED".into()),
        visit_status: None,
    };
    db.save_appointment(it.id, None, &row).await
}

async fn alert(db: &Db, it: &Item, for_user: Uuid, kind: &str, text: &str, date: NaiveDate, time: &str, rec: Option<(&str, Option<&str>)>) -> Result<()> {
    let t = recurrence::parse_time(time).unwrap_or_default();
    db.insert_alert(it.id, &AlertRow {
        alert_id: Uuid::new_v4(),
        kind: Some(kind.into()),
        text: Some(text.into()),
        for_user,
        fires_at: scylla::frame::value::CqlTimestamp(local_millis(date, t, DEFAULT_TZ)),
        timezone: Some(DEFAULT_TZ.into()),
        frequency: rec.map(|r| r.0.to_string()),
        period: rec.and_then(|r| r.1).map(str::to_string),
        appointment_id: None,
        active: Some(true),
    }).await
}

/// A plain "scanned page" placeholder image.
fn sample_jpeg(label_shade: u8) -> Result<Vec<u8>> {
    let img = image::RgbImage::from_fn(600, 800, |x, y| {
        let line = (y > 120 && y % 60 < 14 && x > 60 && x < 540 - (y % 180)) || (y > 40 && y < 90 && x > 60 && x < 360);
        if line { image::Rgb([label_shade, label_shade + 10, label_shade + 20]) } else { image::Rgb([245, 243, 238]) }
    });
    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Jpeg)?;
    Ok(buf)
}

/// A small multi-page PDF with one line of text per page.
fn sample_pdf(title: &str, pages: usize) -> Result<Vec<u8>> {
    use lopdf::{content::{Content, Operation}, dictionary, Document, Object, Stream};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
    let resources_id = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font_id } });
    let mut kids = Vec::new();
    for n in 1..=pages {
        let content = Content { operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 20.into()]),
            Operation::new("Td", vec![60.into(), 760.into()]),
            Operation::new("Tj", vec![Object::string_literal(format!("{title} - sample page {n} of {pages} (test data)"))]),
            Operation::new("ET", vec![]),
        ]};
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode()?));
        let page_id = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id, "Contents" => content_id });
        kids.push(page_id.into());
    }
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! {
        "Type" => "Pages", "Kids" => kids, "Count" => pages as i64, "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    }));
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    let mut buf = Vec::new();
    doc.save_to(&mut buf)?;
    Ok(buf)
}

async fn upload(files: &dyn ObjectStore, db: &Db, it: &Item, by: Uuid, name: &str, mime: &str, data: Vec<u8>, position: i32, pages: Option<i32>) -> Result<()> {
    let key = format!("uploads/{by}/seed-{name}");
    let size = data.len() as i64;
    files.put(&key, data.into(), mime).await?;
    db.insert_attachment(it.id, &AttachmentRow {
        position,
        attachment_id: Uuid::new_v4(),
        kind: Some(if mime == "application/pdf" { "PDF" } else { "IMAGE" }.to_string()),
        uri: Some(object_uri(&key)),
        mime: Some(mime.to_string()),
        size: Some(size),
        name: Some(name.to_string()),
        page_count: pages,
        thumb_uri: None,
        sha256: None,
        added_by: Some(by),
        added_at: Some(now_ts()),
        message_id: None,
        is_report: Some(true),
    }).await
}

/// Seeds an empty database, and adds any missing extra people to an existing one.
/// Returns true when anything was added.
pub async fn run(db: &Db, files: &dyn ObjectStore) -> Result<bool> {
    let base = if db.user_id_by_public_id("HL-2M9P4").await?.is_some() {
        tracing::info!("base seed data already present");
        false
    } else {
        seed_base(db, files).await?;
        true
    };
    let extra = seed_extra(db).await? > 0;
    seed_media(db, files).await?;
    Ok(extra || base)
}

/// Seeded files are written straight to the store, so they never pass through the media
/// consumer: make their thumbnails and page counts here. Also fills in files seeded before
/// thumbnails existed. Cheap when nothing is missing.
async fn seed_media(db: &Db, files: &dyn ObjectStore) -> Result<()> {
    let mut items = Vec::new();
    let owners = people().into_iter().chain(extra_people()).filter(|p| p.role == Role::Patient).map(|p| p.id);
    for owner in owners {
        for status in [ItemStatus::Open, ItemStatus::Closed] {
            items.extend(db.owner_partition(owner, status, 500).await?.into_iter().map(|s| s.id));
        }
    }
    let n = crate::worker::backfill_media(db, files, &items).await?;
    if n > 0 { tracing::info!("made thumbnails for {n} seeded files"); }
    Ok(())
}

async fn seed_base(db: &Db, files: &dyn ObjectStore) -> Result<()> {
    for p in people() { create(db, &p).await?; }
    db.add_affiliation(DR_RAO, HOSPITAL_A).await?;

    let role_of = |id: Uuid| people().into_iter().find(|p| p.id == id).map(|p| (p.role, p.name)).unwrap();
    for (a, b) in [
        (LAKSHMI, DR_RAO), (LAKSHMI, HOSPITAL_A), (LAKSHMI, CITY_LAB), (LAKSHMI, DR_SRINIVAS),
        (DR_RAO, HOSPITAL_A), (DR_RAO, CITY_LAB), (PRIYA, CITY_LAB), (DR_RAO, ASSISTANT), (ADMIN_A, HOSPITAL_A),
    ] {
        let ((ra, na), (rb, nb)) = (role_of(a), role_of(b));
        db.connect_users(a, ra, na, b, rb, nb).await?;
    }

    let today = today_in(DEFAULT_TZ);
    let days = |n: i64| today + chrono::Duration::days(n);
    let rao = || grant(GranteeType::User, DR_RAO, "Dr. Anitha Rao", LAKSHMI);
    let mut count = 0;

    // 1. A lab report that grew into a full case: files, discussion, recurring follow-up, medication alert.
    let cbc = item(LAKSHMI, CITY_LAB, "City Diagnostics", PrimaryKind::Report,
        &[part::REPORT, part::ATTACHMENT, part::MESSAGE, part::APPOINTMENT, part::ALERT],
        "CBC – Complete blood count", &["CBC", "Haemoglobin", "Routine check"],
        vec![
            grant(GranteeType::Hospital, HOSPITAL_A, "Test Hospital A", LAKSHMI),
            rao(),
            grant(GranteeType::User, CITY_LAB, "City Diagnostics", CITY_LAB),
        ]);
    db.insert_item(&cbc).await?;
    upload(files, db, &cbc, CITY_LAB, "cbc_scan_1.jpg", "image/jpeg", sample_jpeg(90)?, 0, None).await?;
    upload(files, db, &cbc, CITY_LAB, "cbc_scan_2.jpg", "image/jpeg", sample_jpeg(120)?, 1, None).await?;
    upload(files, db, &cbc, CITY_LAB, "CBC_report.pdf", "application/pdf", sample_pdf("CBC report", 3)?, 2, Some(3)).await?;
    upload(files, db, &cbc, CITY_LAB, "Reference_ranges.pdf", "application/pdf", sample_pdf("Reference ranges", 2)?, 3, Some(2)).await?;
    message(db, &cbc, DR_RAO, LAKSHMI, "Haemoglobin is slightly low. Let's review every two weeks for a while.").await?;
    message(db, &cbc, LAKSHMI, DR_RAO, "Thank you, doctor. I'll book the visits.").await?;
    appointment(db, &cbc, DR_RAO, Some(HOSPITAL_A), days(7), "11:30", Some(("BIWEEKLY", Some("THREE_MONTHS"))), "Haemoglobin review").await?;
    alert(db, &cbc, LAKSHMI, "MEDICATION", "Iron tablet after dinner", today, "20:30", Some(("DAILY", Some("ONE_MONTH")))).await?;
    count += 1;

    // 2. A standalone discussion (primary kind MESSAGE).
    let bp = item(LAKSHMI, DR_RAO, "Dr. Anitha Rao", PrimaryKind::Message, &[part::MESSAGE], "Latest BP readings", &["BP"], vec![rao()]);
    db.insert_item(&bp).await?;
    message(db, &bp, DR_RAO, LAKSHMI, "Please share your latest BP readings before Thursday.").await?;
    message(db, &bp, LAKSHMI, DR_RAO, "Sure, I will upload them tonight.").await?;
    count += 1;

    // 3. A standalone appointment (single visit).
    let follow = item(LAKSHMI, LAKSHMI, "Lakshmi K.", PrimaryKind::Appointment, &[part::APPOINTMENT],
        "Appointment with Dr. Anitha Rao", &["Follow-up"],
        vec![rao(), grant(GranteeType::Hospital, HOSPITAL_A, "Test Hospital A", LAKSHMI)]);
    db.insert_item(&follow).await?;
    appointment(db, &follow, DR_RAO, Some(HOSPITAL_A), days(3), "10:00", None, "General follow-up").await?;
    count += 1;

    // 4. A standalone alert.
    let bp_alert = item(LAKSHMI, LAKSHMI, "Lakshmi K.", PrimaryKind::Alert, &[part::ALERT], "Check blood pressure", &["BP"], vec![rao()]);
    db.insert_item(&bp_alert).await?;
    alert(db, &bp_alert, LAKSHMI, "CUSTOM", "Check blood pressure", today, "08:00", Some(("WEEKLY", Some("THREE_MONTHS")))).await?;
    count += 1;

    // 5. A closed item with feedback and the patient's rating.
    let closed = item(LAKSHMI, DR_RAO, "Dr. Anitha Rao", PrimaryKind::Message, &[part::MESSAGE], "Consultation on 10 Sep", &[], vec![rao()]);
    db.insert_item(&closed).await?;
    message(db, &closed, DR_RAO, LAKSHMI, "Continue the same dose for two more weeks.").await?;
    db.set_status(&closed, ItemStatus::Closed, Some(ClosureUdt {
        closed_at: Some(now_ts()), closed_by: Some(LAKSHMI),
        feedback: Some("Explained everything clearly.".into()), rating: Some(5),
    })).await?;
    count += 1;

    // 6. Priya's report, uploaded by the lab, not shared yet.
    let lipid = item(PRIYA, CITY_LAB, "City Diagnostics", PrimaryKind::Report, &[part::REPORT, part::ATTACHMENT], "Lipid profile", &["Lipid"],
        vec![grant(GranteeType::User, CITY_LAB, "City Diagnostics", CITY_LAB)]);
    db.insert_item(&lipid).await?;
    upload(files, db, &lipid, CITY_LAB, "Lipid_profile.pdf", "application/pdf", sample_pdf("Lipid profile", 2)?, 0, Some(2)).await?;
    count += 1;

    tracing::info!("seeded {} users and {} items", people().len(), count);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn seed_ids_are_unique_and_listed_for_dev_sign_in() {
        let all: Vec<Person> = people().into_iter().chain(extra_people()).collect();
        let ids: HashSet<Uuid> = all.iter().map(|p| p.id).collect();
        let pids: HashSet<&str> = all.iter().map(|p| p.public_id).collect();
        assert_eq!(ids.len(), all.len());
        assert_eq!(pids.len(), all.len());
        assert_eq!(pids, SEED_PUBLIC_IDS.into_iter().collect::<HashSet<_>>());
        // Generated Healoo IDs never use 0, O, 1 or I (store/users.rs); the new ones follow that
        // (two of the original seed IDs predate the rule).
        for p in extra_people().iter().map(|p| p.public_id) { assert!(p.len() == 8 && p[3..].chars().all(|c| "23456789ABCDEFGHJKLMNPQRSTUVWXYZ".contains(c)), "{p}"); }
        for (a, b) in EXTRA_CONNECTIONS { assert!(ids.contains(&a) && ids.contains(&b)); }
    }

    #[test]
    fn extra_people_cover_the_requested_mix() {
        let extra = extra_people();
        let count = |f: &dyn Fn(&Person) -> bool| extra.iter().filter(|p| f(p)).count();
        assert_eq!(count(&|p| p.role == Role::Patient), 3);
        assert_eq!(count(&|p| p.role == Role::Doctor && p.hospital.is_none()), 2);
        assert_eq!(count(&|p| p.role == Role::Doctor && p.hospital == Some(HOSPITAL_A)), 2);
        assert_eq!(count(&|p| p.role == Role::Lab), 2);
        for d in ["Cardiologist", "Urologist", "Paediatrician"] {
            assert!(extra.iter().any(|p| p.headline.is_some_and(|h| h.ends_with(d))), "{d}");
        }
    }

    #[test]
    fn sample_pdf_first_page_becomes_a_320px_thumbnail() {
        let pdf = sample_pdf("Reference ranges", 2).unwrap();
        // None = pdftoppm isn't installed here (the Docker image has it); nothing to check then.
        let Some(jpeg) = crate::worker::pdf_thumbnail(&pdf).unwrap() else { return };
        let img = image::load_from_memory(&jpeg).unwrap();
        assert_eq!(img.width().max(img.height()), 320);
    }
}
