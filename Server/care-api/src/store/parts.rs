//! DataItem v2 children: item_attachments, item_messages (+ dedupe), item_appointments with the
//! visits_by_doctor / visits_by_patient calendars, item_alerts with the alerts_due queue.

use super::Db;
use crate::model::*;
use crate::recurrence::{self, ExceptionAction, Frequency, Period, Recurrence, VisitException};
use anyhow::Result;
use chrono::{Datelike, Duration as ChronoDuration, NaiveDate, NaiveTime, TimeZone, Timelike, Utc};
use scylla::frame::value::{CqlTimestamp, CqlTimeuuid};
use std::collections::HashMap;
use uuid::Uuid;

pub const DEFAULT_TZ: &str = "Asia/Kolkata";
/// Calendar rows are kept for visits in the next 6 months.
pub const CALENDAR_DAYS: i64 = 183;

/// Local date + time in an IANA zone -> UTC milliseconds.
pub fn local_millis(date: NaiveDate, time: NaiveTime, tz: &str) -> i64 {
    let zone: chrono_tz::Tz = tz.parse().unwrap_or(chrono_tz::Asia::Kolkata);
    zone.from_local_datetime(&date.and_time(time)).earliest()
        .map(|d| d.with_timezone(&Utc).timestamp_millis())
        .unwrap_or_else(|| date.and_time(time).and_utc().timestamp_millis())
}

/// UTC milliseconds -> ISO-8601 with the zone's offset, e.g. 2026-10-05T11:30:00+05:30.
pub fn iso_in(ms: i64, tz: &str) -> String {
    let zone: chrono_tz::Tz = tz.parse().unwrap_or(chrono_tz::Asia::Kolkata);
    Utc.timestamp_millis_opt(ms).single().map(|d| d.with_timezone(&zone).to_rfc3339()).unwrap_or_default()
}

pub fn today_in(tz: &str) -> NaiveDate {
    let zone: chrono_tz::Tz = tz.parse().unwrap_or(chrono_tz::Asia::Kolkata);
    Utc::now().with_timezone(&zone).date_naive()
}

fn bucket(t: chrono::DateTime<Utc>) -> String { format!("{:04}-{:02}", t.year(), t.month()) }
fn month_of(d: NaiveDate) -> String { format!("{:04}-{:02}", d.year(), d.month()) }
fn minute_floor(ms: i64) -> CqlTimestamp { CqlTimestamp(ms - ms.rem_euclid(60_000)) }

// ---------------- attachments ----------------

#[derive(Clone, Debug, scylla::macros::FromRow)]
pub struct AttachmentRow {
    pub position: i32,
    pub attachment_id: Uuid,
    pub kind: Option<String>,
    pub uri: Option<String>,
    pub mime: Option<String>,
    pub size: Option<i64>,
    pub name: Option<String>,
    pub page_count: Option<i32>,
    pub thumb_uri: Option<String>,
    pub sha256: Option<String>,
    pub added_by: Option<Uuid>,
    pub added_at: Option<CqlTimestamp>,
    pub message_id: Option<CqlTimeuuid>,
    pub is_report: Option<bool>,
}

// ---------------- appointments ----------------

#[derive(Clone, Debug, scylla::macros::FromRow)]
pub struct AppointmentRow {
    pub appointment_id: Uuid,
    pub patient_id: Uuid,
    pub doctor_id: Uuid,
    pub hospital_id: Option<Uuid>,
    pub start_date: String,
    pub start_time: String,
    pub timezone: Option<String>,
    pub duration_min: Option<i32>,
    pub notes: Option<String>,
    pub frequency: Option<String>,
    pub period: Option<String>,
    pub exceptions: Option<Vec<VisitExceptionUdt>>,
    pub status: Option<String>,
    pub visit_status: Option<HashMap<String, String>>,
}

impl AppointmentRow {
    pub fn tz(&self) -> &str { self.timezone.as_deref().unwrap_or(DEFAULT_TZ) }
    pub fn date(&self) -> NaiveDate { recurrence::parse_date(&self.start_date).unwrap_or_default() }
    pub fn time(&self) -> NaiveTime { recurrence::parse_time(&self.start_time).unwrap_or_default() }
    pub fn recurrence(&self) -> Option<Recurrence> {
        let f: Frequency = parse(self.frequency.as_deref()?)?;
        Some(Recurrence { frequency: f, period: self.period.as_deref().and_then(parse::<Period>) })
    }
    pub fn exceptions(&self) -> Vec<VisitException> {
        self.exceptions.clone().unwrap_or_default().into_iter().filter_map(|e| Some(VisitException {
            date: recurrence::parse_date(e.visit_date.as_deref()?)?,
            action: parse::<ExceptionAction>(e.action.as_deref()?)?,
            new_date: e.new_date.as_deref().and_then(recurrence::parse_date),
            new_time: e.new_time.as_deref().and_then(recurrence::parse_time),
        })).collect()
    }
    pub fn is_cancelled(&self) -> bool { self.status.as_deref() == Some("CANCELLED") }

    /// Visits in [from, until], with per-visit status.
    pub fn visits(&self, from: NaiveDate, until: NaiveDate, max: usize) -> Vec<(recurrence::Visit, String)> {
        let rec = self.recurrence();
        let marks = self.visit_status.clone().unwrap_or_default();
        recurrence::visits(self.date(), self.time(), rec.as_ref(), &self.exceptions(), from, until, max)
            .into_iter()
            .map(|v| {
                let st = if v.cancelled || self.is_cancelled() { "CANCELLED".to_string() }
                    else { marks.get(&recurrence::fmt_date(v.original_date)).cloned().unwrap_or_else(|| "SCHEDULED".into()) };
                (v, st)
            })
            .collect()
    }
}

pub fn exception_udts(ex: &[VisitException]) -> Vec<VisitExceptionUdt> {
    ex.iter().map(|e| VisitExceptionUdt {
        visit_date: Some(recurrence::fmt_date(e.date)),
        action: Some(text(&e.action)),
        new_date: e.new_date.map(recurrence::fmt_date),
        new_time: e.new_time.map(recurrence::fmt_time),
    }).collect()
}

// ---------------- alerts ----------------

#[derive(Clone, Debug, scylla::macros::FromRow)]
pub struct AlertRow {
    pub alert_id: Uuid,
    pub kind: Option<String>,
    pub text: Option<String>,
    pub for_user: Uuid,
    pub fires_at: CqlTimestamp,
    pub timezone: Option<String>,
    pub frequency: Option<String>,
    pub period: Option<String>,
    pub appointment_id: Option<Uuid>,
    pub active: Option<bool>,
}

impl AlertRow {
    pub fn tz(&self) -> &str { self.timezone.as_deref().unwrap_or(DEFAULT_TZ) }
    pub fn recurrence(&self) -> Option<Recurrence> {
        let f: Frequency = parse(self.frequency.as_deref()?)?;
        Some(Recurrence { frequency: f, period: self.period.as_deref().and_then(parse::<Period>) })
    }
    /// The first firing after `after_ms`, following the recurrence (None when it has ended).
    pub fn next_after(&self, after_ms: i64) -> Option<i64> {
        let zone: chrono_tz::Tz = self.tz().parse().unwrap_or(chrono_tz::Asia::Kolkata);
        let first = Utc.timestamp_millis_opt(self.fires_at.0).single()?.with_timezone(&zone);
        let rec = self.recurrence()?;
        let (d0, t0) = (first.date_naive(), NaiveTime::from_hms_opt(first.hour(), first.minute(), 0)?);
        let end = recurrence::end_date(d0, &rec);
        for n in 1..5000u32 {
            let d = recurrence::nth_date(d0, rec.frequency, n)?;
            if end.map_or(false, |e| d >= e) { return None; }
            let ms = local_millis(d, t0, self.tz());
            if ms > after_ms { return Some(ms); }
        }
        None
    }
}

impl Db {
    // ---- attachments ----

    pub async fn attachments(&self, item: Uuid) -> Result<Vec<AttachmentRow>> {
        self.rows::<AttachmentRow>(
            "SELECT position, attachment_id, kind, uri, mime, size, name, page_count, thumb_uri, sha256, added_by, added_at, message_id, is_report FROM item_attachments WHERE item_id = ?",
            (tu(item),)).await
    }

    pub async fn next_attachment_position(&self, item: Uuid) -> Result<i32> {
        Ok(self.one::<(i32,)>("SELECT position FROM item_attachments WHERE item_id = ? ORDER BY position DESC LIMIT 1", (tu(item),))
            .await?.map(|r| r.0 + 1).unwrap_or(0))
    }

    pub async fn insert_attachment(&self, item: Uuid, a: &AttachmentRow) -> Result<()> {
        self.exec(
            "INSERT INTO item_attachments (item_id, position, attachment_id, kind, uri, mime, size, name, page_count, thumb_uri, sha256, added_by, added_at, message_id, is_report) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (tu(item), a.position, a.attachment_id, &a.kind, &a.uri, &a.mime, a.size, &a.name, a.page_count, &a.thumb_uri, &a.sha256, a.added_by, a.added_at, a.message_id, a.is_report),
        ).await?;
        Ok(())
    }

    /// Filled in by the media worker (doc 7.4).
    pub async fn set_attachment_media(&self, item: Uuid, position: i32, page_count: Option<i32>, thumb_uri: Option<&str>, sha256: &str, size: i64) -> Result<()> {
        self.exec(
            "UPDATE item_attachments SET page_count = ?, thumb_uri = ?, sha256 = ?, size = ? WHERE item_id = ? AND position = ?",
            (page_count, thumb_uri, sha256, size, tu(item), position)).await?;
        Ok(())
    }

    // ---- messages ----

    pub async fn existing_item_message(&self, item: Uuid, client_msg_id: &str) -> Result<Option<Uuid>> {
        Ok(self.one::<(CqlTimeuuid,)>("SELECT message_id FROM item_message_dedupe WHERE item_id = ? AND client_msg_id = ?", (tu(item), client_msg_id))
            .await?.map(|r| un(r.0)))
    }

    pub async fn insert_item_message(&self, m: &MessageDto, client_msg_id: &str) -> Result<()> {
        let at = ts_to_dt(CqlTimestamp(uuid_millis(&m.message_id)));
        self.batch(
            &[
                "INSERT INTO item_messages (item_id, bucket, message_id, sender_id, body, client_msg_id, attachment_ids) VALUES (?, ?, ?, ?, ?, ?, ?)",
                "INSERT INTO item_message_dedupe (item_id, client_msg_id, message_id) VALUES (?, ?, ?)",
            ],
            (
                (tu(m.item_id), bucket(at), tu(m.message_id), m.sender_id, &m.body, client_msg_id, &m.attachment_ids),
                (tu(m.item_id), client_msg_id, tu(m.message_id)),
            ),
        ).await
    }

    pub async fn item_message(&self, item: Uuid, id: Uuid) -> Result<Option<MessageDto>> {
        let at = ts_to_dt(CqlTimestamp(uuid_millis(&id)));
        let row = self.one::<(Uuid, Option<String>, Option<Vec<Uuid>>)>(
            "SELECT sender_id, body, attachment_ids FROM item_messages WHERE item_id = ? AND bucket = ? AND message_id = ?",
            (tu(item), bucket(at), tu(id))).await?;
        Ok(row.map(|r| MessageDto { message_id: id, item_id: item, sender_id: r.0, body: r.1.unwrap_or_default(), sent_at: at.to_rfc3339(), attachment_ids: r.2.unwrap_or_default() }))
    }

    /// The latest `limit` messages (looking back up to 12 months), oldest first.
    pub async fn item_messages(&self, item: Uuid, limit: i32) -> Result<Vec<MessageDto>> {
        let mut out = Vec::new();
        let mut month = Utc::now();
        for _ in 0..12 {
            let rows = self.rows::<(CqlTimeuuid, Uuid, Option<String>, Option<Vec<Uuid>>)>(
                "SELECT message_id, sender_id, body, attachment_ids FROM item_messages WHERE item_id = ? AND bucket = ? LIMIT ?",
                (tu(item), bucket(month), limit)).await?;
            out.extend(rows.into_iter().map(|r| {
                let id = un(r.0);
                MessageDto { message_id: id, item_id: item, sender_id: r.1, body: r.2.unwrap_or_default(),
                    sent_at: ts_to_dt(CqlTimestamp(uuid_millis(&id))).to_rfc3339(), attachment_ids: r.3.unwrap_or_default() }
            }));
            if out.len() as i32 >= limit { break; }
            month = month - ChronoDuration::days(31);
        }
        out.sort_by_key(|m| uuid_millis(&m.message_id));
        let skip = out.len().saturating_sub(limit as usize);
        Ok(out.into_iter().skip(skip).collect())
    }

    // ---- appointments ----

    pub async fn appointments(&self, item: Uuid) -> Result<Vec<AppointmentRow>> {
        self.rows::<AppointmentRow>(
            "SELECT appointment_id, patient_id, doctor_id, hospital_id, start_date, start_time, timezone, duration_min, notes, frequency, period, exceptions, status, visit_status FROM item_appointments WHERE item_id = ?",
            (tu(item),)).await
    }

    pub async fn appointment(&self, item: Uuid, id: Uuid) -> Result<Option<AppointmentRow>> {
        self.one::<AppointmentRow>(
            "SELECT appointment_id, patient_id, doctor_id, hospital_id, start_date, start_time, timezone, duration_min, notes, frequency, period, exceptions, status, visit_status FROM item_appointments WHERE item_id = ? AND appointment_id = ?",
            (tu(item), id)).await
    }

    /// Insert or replace a series, and rebuild its calendar rows (old visits removed, new added).
    pub async fn save_appointment(&self, item: Uuid, old: Option<&AppointmentRow>, a: &AppointmentRow) -> Result<()> {
        self.exec(
            "INSERT INTO item_appointments (item_id, appointment_id, patient_id, doctor_id, hospital_id, start_date, start_time, timezone, duration_min, notes, frequency, period, exceptions, status, visit_status) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (tu(item), a.appointment_id, a.patient_id, a.doctor_id, a.hospital_id, &a.start_date, &a.start_time, &a.timezone,
             a.duration_min, &a.notes, &a.frequency, &a.period, &a.exceptions, &a.status, &a.visit_status),
        ).await?;
        if let Some(o) = old { self.delete_calendar(o).await?; }
        self.write_calendar(item, a).await
    }

    fn calendar_window(a: &AppointmentRow) -> (NaiveDate, NaiveDate) {
        let today = today_in(a.tz());
        (today - ChronoDuration::days(1), today + ChronoDuration::days(CALENDAR_DAYS))
    }

    async fn delete_calendar(&self, a: &AppointmentRow) -> Result<()> {
        let (from, until) = Self::calendar_window(a);
        for (v, _) in a.visits(from, until, 1000) {
            let ms = local_millis(v.date, v.time, a.tz());
            let day = recurrence::fmt_date(v.date);
            self.exec("DELETE FROM visits_by_doctor WHERE doctor_id = ? AND day = ? AND starts_at = ? AND appointment_id = ?",
                (a.doctor_id, &day, CqlTimestamp(ms), a.appointment_id)).await?;
            self.exec("DELETE FROM visits_by_patient WHERE patient_id = ? AND month = ? AND starts_at = ? AND appointment_id = ?",
                (a.patient_id, month_of(v.date), CqlTimestamp(ms), a.appointment_id)).await?;
        }
        Ok(())
    }

    async fn write_calendar(&self, item: Uuid, a: &AppointmentRow) -> Result<()> {
        let (from, until) = Self::calendar_window(a);
        for (v, status) in a.visits(from, until, 1000) {
            let ms = local_millis(v.date, v.time, a.tz());
            self.exec("INSERT INTO visits_by_doctor (doctor_id, day, starts_at, appointment_id, item_id, patient_id, status) VALUES (?, ?, ?, ?, ?, ?, ?)",
                (a.doctor_id, recurrence::fmt_date(v.date), CqlTimestamp(ms), a.appointment_id, tu(item), a.patient_id, &status)).await?;
            self.exec("INSERT INTO visits_by_patient (patient_id, month, starts_at, appointment_id, item_id, doctor_id, status) VALUES (?, ?, ?, ?, ?, ?, ?)",
                (a.patient_id, month_of(v.date), CqlTimestamp(ms), a.appointment_id, tu(item), a.doctor_id, &status)).await?;
        }
        Ok(())
    }

    /// (starts_at ms, appointment, item, other person, status) for a doctor, one day at a time.
    pub async fn doctor_visits(&self, doctor: Uuid, from: NaiveDate, to: NaiveDate) -> Result<Vec<(i64, Uuid, Uuid, Uuid, String)>> {
        let mut out = Vec::new();
        let mut d = from;
        while d <= to {
            let rows = self.rows::<(CqlTimestamp, Uuid, CqlTimeuuid, Uuid, Option<String>)>(
                "SELECT starts_at, appointment_id, item_id, patient_id, status FROM visits_by_doctor WHERE doctor_id = ? AND day = ?",
                (doctor, recurrence::fmt_date(d))).await?;
            out.extend(rows.into_iter().map(|r| (r.0 .0, r.1, un(r.2), r.3, r.4.unwrap_or_default())));
            d = d + ChronoDuration::days(1);
        }
        Ok(out)
    }

    /// Same for a patient, one month partition at a time.
    pub async fn patient_visits(&self, patient: Uuid, from: NaiveDate, to: NaiveDate) -> Result<Vec<(i64, Uuid, Uuid, Uuid, String)>> {
        let mut out = Vec::new();
        let mut m = NaiveDate::from_ymd_opt(from.year(), from.month(), 1).unwrap_or(from);
        while m <= to {
            let rows = self.rows::<(CqlTimestamp, Uuid, CqlTimeuuid, Uuid, Option<String>)>(
                "SELECT starts_at, appointment_id, item_id, doctor_id, status FROM visits_by_patient WHERE patient_id = ? AND month = ?",
                (patient, month_of(m))).await?;
            out.extend(rows.into_iter().map(|r| (r.0 .0, r.1, un(r.2), r.3, r.4.unwrap_or_default())));
            m = m.checked_add_months(chrono::Months::new(1)).unwrap_or(to + ChronoDuration::days(1));
        }
        let (lo, hi) = (local_millis(from, NaiveTime::MIN, DEFAULT_TZ), local_millis(to + ChronoDuration::days(1), NaiveTime::MIN, DEFAULT_TZ));
        out.retain(|v| v.0 >= lo && v.0 < hi);
        Ok(out)
    }

    // ---- alerts ----

    pub async fn alerts(&self, item: Uuid) -> Result<Vec<AlertRow>> {
        self.rows::<AlertRow>(
            "SELECT alert_id, type, text, for_user, fires_at, timezone, frequency, period, appointment_id, active FROM item_alerts WHERE item_id = ?",
            (tu(item),)).await
    }

    pub async fn alert(&self, item: Uuid, id: Uuid) -> Result<Option<AlertRow>> {
        self.one::<AlertRow>(
            "SELECT alert_id, type, text, for_user, fires_at, timezone, frequency, period, appointment_id, active FROM item_alerts WHERE item_id = ? AND alert_id = ?",
            (tu(item), id)).await
    }

    /// Saves the alert and queues its first firing (if still in the future).
    pub async fn insert_alert(&self, item: Uuid, a: &AlertRow) -> Result<()> {
        self.exec(
            "INSERT INTO item_alerts (item_id, alert_id, type, text, for_user, fires_at, timezone, frequency, period, appointment_id, active) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (tu(item), a.alert_id, &a.kind, &a.text, a.for_user, a.fires_at, &a.timezone, &a.frequency, &a.period, a.appointment_id, a.active),
        ).await?;
        let now = Utc::now().timestamp_millis();
        let first = if a.fires_at.0 > now { Some(a.fires_at.0) } else { a.next_after(now) };
        if let Some(ms) = first { self.queue_alert(ms, a.alert_id, item, a.for_user).await?; }
        Ok(())
    }

    pub async fn queue_alert(&self, at_ms: i64, alert: Uuid, item: Uuid, user: Uuid) -> Result<()> {
        self.exec("INSERT INTO alerts_due (due_minute, alert_id, item_id, for_user) VALUES (?, ?, ?, ?)",
            (minute_floor(at_ms), alert, tu(item), user)).await?;
        Ok(())
    }

    pub async fn set_alert_active(&self, item: Uuid, alert: Uuid, active: bool) -> Result<()> {
        self.exec("UPDATE item_alerts SET active = ? WHERE item_id = ? AND alert_id = ?", (active, tu(item), alert)).await?;
        Ok(())
    }

    pub async fn delete_alert(&self, item: Uuid, alert: Uuid) -> Result<()> {
        self.exec("DELETE FROM item_alerts WHERE item_id = ? AND alert_id = ?", (tu(item), alert)).await?;
        Ok(())
    }

    /// Alerts queued for one minute (UTC ms at the start of the minute).
    pub async fn alerts_due_at(&self, minute_ms: i64) -> Result<Vec<(Uuid, Uuid, Uuid)>> {
        Ok(self.rows::<(Uuid, CqlTimeuuid, Uuid)>("SELECT alert_id, item_id, for_user FROM alerts_due WHERE due_minute = ?", (minute_floor(minute_ms),))
            .await?.into_iter().map(|r| (r.0, un(r.1), r.2)).collect())
    }

    pub async fn remove_due(&self, minute_ms: i64, alert: Uuid) -> Result<()> {
        self.exec("DELETE FROM alerts_due WHERE due_minute = ? AND alert_id = ?", (minute_floor(minute_ms), alert)).await?;
        Ok(())
    }
}
