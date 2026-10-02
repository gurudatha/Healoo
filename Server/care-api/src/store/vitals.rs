//! vital_readings, vital_readings_by_type, vital_types_by_user, vital_ranges,
//! vital_alerts_by_doctor (Documentation/Vitals_Design.md section 4).

use super::Db;
use crate::{
    model::now_ts,
    vitals::{Component, RangeSpec, VitalType, SCHEMA_VERSION},
};
use anyhow::Result;
use scylla::frame::value::CqlTimestamp;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ReadingRow {
    pub user_id: Uuid,
    pub reading_id: Uuid,
    pub code: String,
    pub vals: HashMap<String, f64>,
    pub taken_at: CqlTimestamp,
    pub note: Option<String>,
    pub entered_by: Uuid,
    pub severity: String,
}

#[derive(Clone, Debug)]
pub struct VitalAlertRow {
    pub alert_id: Uuid,
    pub patient_id: Uuid,
    pub patient_name: String,
    pub code: String,
    pub label: String,
    pub icon: String,
    pub unit: String,
    pub vals: HashMap<String, f64>,
    pub severity: String,
    pub is_read: bool,
    pub reading_id: Uuid,
    pub taken_at: CqlTimestamp,
}

type ReadingCols = (Uuid, Option<String>, Option<HashMap<String, f64>>, Option<CqlTimestamp>, Option<String>, Option<Uuid>, Option<String>);

fn reading(user_id: Uuid, r: ReadingCols) -> ReadingRow {
    ReadingRow {
        user_id,
        reading_id: r.0,
        code: r.1.unwrap_or_default(),
        vals: r.2.unwrap_or_default(),
        taken_at: r.3.unwrap_or(CqlTimestamp(0)),
        note: r.4,
        entered_by: r.5.unwrap_or(user_id),
        severity: r.6.unwrap_or_default(),
    }
}

impl Db {
    // ---- readings ----

    /// Writes the reading to both per-user tables together.
    pub async fn insert_reading(&self, r: &ReadingRow) -> Result<()> {
        self.batch(
            &[
                "INSERT INTO vital_readings (user_id, reading_id, code, vals, taken_at, note, entered_by, severity, schema_version) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                "INSERT INTO vital_readings_by_type (user_id, code, reading_id, vals, taken_at, note, entered_by, severity, schema_version) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            ],
            (
                (r.user_id, r.reading_id, &r.code, &r.vals, r.taken_at, &r.note, r.entered_by, &r.severity, SCHEMA_VERSION),
                (r.user_id, &r.code, r.reading_id, &r.vals, r.taken_at, &r.note, r.entered_by, &r.severity, SCHEMA_VERSION),
            ),
        ).await
    }

    /// A person's readings of every vital, newest entry first.
    pub async fn readings(&self, user: Uuid, limit: i32) -> Result<Vec<ReadingRow>> {
        let rows = self.rows::<ReadingCols>(
            "SELECT reading_id, code, vals, taken_at, note, entered_by, severity FROM vital_readings WHERE user_id = ? LIMIT ?", (user, limit)).await?;
        Ok(rows.into_iter().map(|r| reading(user, r)).collect())
    }

    /// A person's readings of one vital, newest entry first.
    pub async fn readings_of(&self, user: Uuid, code: &str, limit: i32) -> Result<Vec<ReadingRow>> {
        let rows = self.rows::<ReadingCols>(
            "SELECT reading_id, code, vals, taken_at, note, entered_by, severity FROM vital_readings_by_type WHERE user_id = ? AND code = ? LIMIT ?",
            (user, code, limit)).await?;
        Ok(rows.into_iter().map(|r| reading(user, r)).collect())
    }

    // ---- vitals a doctor added for a patient ----

    pub async fn custom_vital_types(&self, user: Uuid) -> Result<Vec<VitalType>> {
        type Row = (String, Option<String>, Option<String>, Option<String>, Option<String>, Option<i32>, Option<i32>, Option<bool>);
        let rows = self.rows::<Row>(
            "SELECT code, label, icon, unit, spec, margin_pct, position, active FROM vital_types_by_user WHERE user_id = ?", (user,)).await?;
        Ok(rows.into_iter().filter(|r| r.7.unwrap_or(true)).map(|r| {
            let label = r.1.unwrap_or_default();
            // An unreadable spec (written by a much newer server) degrades to one plain number.
            let components = r.4.as_deref().and_then(|s| serde_json::from_str::<Vec<Component>>(s).ok())
                .filter(|c| !c.is_empty())
                .unwrap_or_else(|| vec![Component { key: "value".into(), label: label.clone(), low: None, high: None }]);
            VitalType {
                code: r.0, label, icon: r.2.unwrap_or_else(|| "other".into()), unit: r.3.unwrap_or_default(),
                components, margin_pct: r.5, ratio: None, custom: true, position: r.6.unwrap_or(100), schema_version: SCHEMA_VERSION,
            }
        }).collect())
    }

    pub async fn insert_custom_vital_type(&self, user: Uuid, t: &VitalType, by: Uuid) -> Result<()> {
        let spec = serde_json::to_string(&t.components)?;
        self.exec(
            "INSERT INTO vital_types_by_user (user_id, code, label, icon, unit, spec, margin_pct, position, created_by, created_at, active, schema_version) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, true, ?)",
            (user, &t.code, &t.label, &t.icon, &t.unit, spec, t.margin_pct, t.position, by, now_ts(), SCHEMA_VERSION),
        ).await?;
        Ok(())
    }

    // ---- ranges a doctor set for a patient ----

    pub async fn vital_ranges(&self, user: Uuid) -> Result<HashMap<String, (RangeSpec, Option<i32>)>> {
        let rows = self.rows::<(String, Option<String>, Option<i32>)>(
            "SELECT code, spec, margin_pct FROM vital_ranges WHERE user_id = ?", (user,)).await?;
        Ok(rows.into_iter().filter_map(|(code, spec, margin)| {
            let spec = serde_json::from_str::<RangeSpec>(spec.as_deref()?).ok()?;
            Some((code, (spec, margin)))
        }).collect())
    }

    pub async fn set_vital_range(&self, user: Uuid, code: &str, spec: &RangeSpec, margin_pct: Option<i32>, by: Uuid) -> Result<()> {
        self.exec(
            "INSERT INTO vital_ranges (user_id, code, spec, margin_pct, set_by, set_at, schema_version) VALUES (?, ?, ?, ?, ?, ?, ?)",
            (user, code, serde_json::to_string(spec)?, margin_pct, by, now_ts(), SCHEMA_VERSION),
        ).await?;
        Ok(())
    }

    // ---- doctor inbox ----

    pub async fn insert_vital_alert(&self, doctor: Uuid, a: &VitalAlertRow) -> Result<()> {
        self.exec(
            "INSERT INTO vital_alerts_by_doctor (doctor_id, alert_id, patient_id, patient_name, code, label, icon, unit, vals, severity, is_read, reading_id, taken_at, schema_version) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, false, ?, ?, ?)",
            (doctor, a.alert_id, a.patient_id, &a.patient_name, &a.code, &a.label, &a.icon, &a.unit, &a.vals, &a.severity, a.reading_id, a.taken_at, SCHEMA_VERSION),
        ).await?;
        Ok(())
    }

    /// The doctor's most recent alerts, read ones included (the caller filters), newest first.
    pub async fn vital_alerts(&self, doctor: Uuid, limit: i32) -> Result<Vec<VitalAlertRow>> {
        type Row = (Uuid, Option<Uuid>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>,
                    Option<HashMap<String, f64>>, Option<String>, Option<bool>, Option<Uuid>, Option<CqlTimestamp>);
        let rows = self.rows::<Row>(
            "SELECT alert_id, patient_id, patient_name, code, label, icon, unit, vals, severity, is_read, reading_id, taken_at FROM vital_alerts_by_doctor WHERE doctor_id = ? LIMIT ?",
            (doctor, limit)).await?;
        Ok(rows.into_iter().filter_map(|r| Some(VitalAlertRow {
            alert_id: r.0,
            patient_id: r.1?,
            patient_name: r.2.unwrap_or_default(),
            code: r.3.unwrap_or_default(),
            label: r.4.unwrap_or_default(),
            icon: r.5.unwrap_or_else(|| "other".into()),
            unit: r.6.unwrap_or_default(),
            vals: r.7.unwrap_or_default(),
            severity: r.8.unwrap_or_default(),
            is_read: r.9.unwrap_or(false),
            reading_id: r.10.unwrap_or(r.0),
            taken_at: r.11.unwrap_or(CqlTimestamp(0)),
        })).collect())
    }

    /// Swipe-away: the alert leaves the doctor's active list. `IF EXISTS` so that a late call
    /// can't create an empty row after the 90-day expiry.
    pub async fn mark_vital_alert_read(&self, doctor: Uuid, alert_id: Uuid) -> Result<()> {
        self.exec("UPDATE vital_alerts_by_doctor SET is_read = true WHERE doctor_id = ? AND alert_id = ? IF EXISTS", (doctor, alert_id)).await?;
        Ok(())
    }
}
