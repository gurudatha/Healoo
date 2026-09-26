//! Seed data for trials: the same people and records as the apps' demo mode, plus a hospital
//! administrator and an assistant so acceptance tests 4, 6 and 7 (doc 10.4) can run.
//! Sample images and PDFs are generated, so no patient-like data is shipped.

use crate::{
    files::{object_uri, ObjectStore},
    model::*,
    store::{items::Item, users::NewUser, Db},
};
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

pub const SEED_PUBLIC_IDS: [&str; 10] = [
    "HL-2M9P4", "HL-7R2C9", "HL-1H0A1", "HL-6L3D2", "HL-3K8M1", "HL-9P4T6", "HL-5W1Q3", "HL-4K7Q2", "HL-8A2D4", "HL-8S5T7",
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

#[allow(clippy::too_many_arguments)]
fn item(owner: Uuid, by: Uuid, by_name: &str, date: &str, ty: CoreItemType, title: &str, subtitle: &str, keywords: &[&str], grants: Vec<GrantUdt>) -> Item {
    Item {
        id: new_timeuuid(),
        owner_id: owner,
        created_by: by,
        created_by_name: by_name.to_string(),
        date: date_to_ts(date).unwrap_or_else(now_ts),
        item_type: ty,
        title: title.to_string(),
        subtitle: subtitle.to_string(),
        keywords: keywords.iter().map(|k| k.to_string()).collect(),
        attachments: vec![],
        links: vec![],
        grants,
        pointer_to_message: None,
        pointer_item_id: None,
        rating: None,
        status: ItemStatus::Open,
    }
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

async fn upload(files: &dyn ObjectStore, owner: Uuid, name: &str, mime: &str, data: Vec<u8>, position: i32, pages: Option<i32>) -> Result<AttachmentUdt> {
    let key = format!("uploads/{owner}/seed-{name}");
    let size = data.len() as i64;
    files.put(&key, data.into(), mime).await?;
    Ok(AttachmentUdt {
        kind: Some(if mime == "application/pdf" { "PDF" } else { "IMAGE" }.to_string()),
        uri: Some(object_uri(&key)),
        mime: Some(mime.to_string()),
        size: Some(size),
        sha256: None,
        position: Some(position),
        page_count: pages,
        thumb_uri: None,
        name: Some(name.to_string()),
    })
}

pub async fn run(db: &Db, files: &dyn ObjectStore) -> Result<bool> {
    if db.user_id_by_public_id("HL-2M9P4").await?.is_some() {
        tracing::info!("seed data already present — nothing to do");
        return Ok(false);
    }

    for p in people() {
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
        }).await?;
    }
    db.add_affiliation(DR_RAO, HOSPITAL_A).await?;

    let role_of = |id: Uuid| people().into_iter().find(|p| p.id == id).map(|p| (p.role, p.name)).unwrap();
    for (a, b) in [
        (LAKSHMI, DR_RAO), (LAKSHMI, HOSPITAL_A), (LAKSHMI, CITY_LAB), (LAKSHMI, DR_SRINIVAS),
        (DR_RAO, HOSPITAL_A), (DR_RAO, CITY_LAB), (PRIYA, CITY_LAB), (DR_RAO, ASSISTANT), (ADMIN_A, HOSPITAL_A),
    ] {
        let ((ra, na), (rb, nb)) = (role_of(a), role_of(b));
        db.connect_users(a, ra, na, b, rb, nb).await?;
    }

    // CBC report uploaded by the lab: 2 images + 2 PDFs in the chosen order (test 13/14 shape).
    let mut cbc = item(LAKSHMI, CITY_LAB, "City Diagnostics", "2026-09-20", CoreItemType::Report,
        "CBC – Complete blood count", "City Diagnostics · Lab report", &["CBC", "Haemoglobin", "Routine check"],
        vec![
            grant(GranteeType::Hospital, HOSPITAL_A, "Test Hospital A", LAKSHMI),
            grant(GranteeType::User, DR_RAO, "Dr. Anitha Rao", LAKSHMI),
            grant(GranteeType::User, CITY_LAB, "City Diagnostics", CITY_LAB),
        ]);
    cbc.attachments = vec![
        upload(files, CITY_LAB, "cbc_scan_1.jpg", "image/jpeg", sample_jpeg(90)?, 0, None).await?,
        upload(files, CITY_LAB, "cbc_scan_2.jpg", "image/jpeg", sample_jpeg(120)?, 1, None).await?,
        upload(files, CITY_LAB, "CBC_20Sep2026.pdf", "application/pdf", sample_pdf("CBC report", 3)?, 2, Some(3)).await?,
        upload(files, CITY_LAB, "Reference_ranges.pdf", "application/pdf", sample_pdf("Reference ranges", 2)?, 3, Some(2)).await?,
    ];
    let mut followup = item(LAKSHMI, DR_RAO, "Dr. Anitha Rao", "2026-09-24", CoreItemType::Booking,
        "Follow-up consultation", "Test Hospital A · 24 Sep, 11:30", &["Follow-up"],
        vec![grant(GranteeType::Hospital, HOSPITAL_A, "Test Hospital A", LAKSHMI)]);
    followup.pointer_item_id = Some(cbc.id);
    cbc.pointer_item_id = Some(followup.id);

    let rao = || grant(GranteeType::User, DR_RAO, "Dr. Anitha Rao", LAKSHMI);
    let seeded = vec![
        cbc,
        followup,
        item(LAKSHMI, DR_RAO, "Dr. Anitha Rao", "2026-09-22", CoreItemType::Message, "Dr. Anitha Rao", "Please share your latest BP readings", &[], vec![rao()]),
        item(LAKSHMI, DR_RAO, "Dr. Anitha Rao", "2026-09-22", CoreItemType::Alert, "Medication reminder", "Evening dose · 8:00 PM", &["Medication"], vec![rao()]),
        item(LAKSHMI, HOSPITAL_A, "Test Hospital A", "2026-09-18", CoreItemType::Payment, "Consultation fee", "₹600 · Payment pending", &[],
            vec![grant(GranteeType::Hospital, HOSPITAL_A, "Test Hospital A", LAKSHMI)]),
        item(LAKSHMI, DR_RAO, "Dr. Anitha Rao", "2026-09-10", CoreItemType::Feedback, "Visit feedback", "Rate your 10 Sep consultation", &[], vec![rao()]),
        item(PRIYA, CITY_LAB, "City Diagnostics", "2026-09-19", CoreItemType::Report, "Lipid profile", "City Diagnostics · Lab report", &["Lipid"],
            vec![grant(GranteeType::User, CITY_LAB, "City Diagnostics", CITY_LAB)]),
    ];
    for it in &seeded {
        db.insert_item(it).await?;
    }

    // One conversation, as in the apps' demo.
    let thread = db.create_thread(LAKSHMI, DR_RAO).await?;
    for (from, body) in [(DR_RAO, "Please share your latest BP readings before Thursday."), (LAKSHMI, "Sure, I will upload them tonight.")] {
        let id = new_timeuuid();
        let m = MessageDto { message_id: id, thread_id: thread, sender_id: from, body: body.into(), sent_at: String::new(), linked_item_id: None };
        db.insert_message(&m, &format!("seed-{id}")).await?;
    }
    db.touch_thread(LAKSHMI, DR_RAO, thread, "Sure, I will upload them tonight.", 1).await?;
    db.touch_thread(DR_RAO, LAKSHMI, thread, "Sure, I will upload them tonight.", 1).await?;

    tracing::info!("seeded {} users, {} items and 1 conversation", people().len(), seeded.len());
    Ok(true)
}
