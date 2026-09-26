//! Recurring appointments and alerts (Documentation/DataItem_Design.md section 3.7).
//!
//! A series is stored as a rule (first date/time + frequency + optional period) plus per-visit
//! exceptions; visits are calculated when needed. Pure functions, unit-tested below.

use chrono::{Days, Months, NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Frequency { Daily, Weekly, Biweekly, Monthly, Quarterly }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Period { OneMonth, TwoMonths, ThreeMonths, SixMonths }

impl Period {
    pub fn months(self) -> u32 {
        match self { Period::OneMonth => 1, Period::TwoMonths => 2, Period::ThreeMonths => 3, Period::SixMonths => 6 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recurrence {
    pub frequency: Frequency,
    #[serde(default)]
    pub period: Option<Period>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExceptionAction { Cancelled, Moved }

/// A change to one visit of a series, identified by the visit's original date.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisitException {
    pub date: NaiveDate,
    pub action: ExceptionAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_date: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "opt_hhmm")]
    pub new_time: Option<NaiveTime>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Visit {
    /// The date the rule produced (identifies the visit even after it was moved).
    pub original_date: NaiveDate,
    pub date: NaiveDate,
    pub time: NaiveTime,
    pub cancelled: bool,
}

/// The n-th date of the rule (n = 0 is the first visit). Month steps count from the first
/// date, so a series on the 31st lands on each month's last day without drifting.
pub fn nth_date(start: NaiveDate, f: Frequency, n: u32) -> Option<NaiveDate> {
    match f {
        Frequency::Daily => start.checked_add_days(Days::new(n as u64)),
        Frequency::Weekly => start.checked_add_days(Days::new(7 * n as u64)),
        Frequency::Biweekly => start.checked_add_days(Days::new(14 * n as u64)),
        Frequency::Monthly => start.checked_add_months(Months::new(n)),
        Frequency::Quarterly => start.checked_add_months(Months::new(3 * n)),
    }
}

/// Exclusive end date of a series with a period, or None when it runs until cancelled.
pub fn end_date(start: NaiveDate, r: &Recurrence) -> Option<NaiveDate> {
    r.period.and_then(|p| start.checked_add_months(Months::new(p.months())))
}

/// Number of visits in a series with a period (None when it runs until cancelled).
pub fn visit_count(start: NaiveDate, r: &Recurrence) -> Option<u32> {
    let end = end_date(start, r)?;
    let mut n = 0;
    while let Some(d) = nth_date(start, r.frequency, n) {
        if d >= end { break; }
        n += 1;
    }
    Some(n)
}

/// Rules 3 and 4 of section 3.7.
pub fn validate(start: NaiveDate, r: Option<&Recurrence>) -> Result<(), String> {
    let Some(r) = r else { return Ok(()) };
    if r.frequency == Frequency::Daily && r.period.is_none() {
        return Err("daily repeats need a period (1, 2, 3 or 6 months)".into());
    }
    if let Some(n) = visit_count(start, r) {
        if n <= 1 {
            return Err("this repeat and period give only one visit; book a single appointment instead".into());
        }
    }
    Ok(())
}

/// Visits with a date in [from, until], at most `max`, exceptions applied.
pub fn visits(
    start: NaiveDate,
    time: NaiveTime,
    r: Option<&Recurrence>,
    exceptions: &[VisitException],
    from: NaiveDate,
    until: NaiveDate,
    max: usize,
) -> Vec<Visit> {
    let mut out = Vec::new();
    let mut push = |orig: NaiveDate| {
        let ex = exceptions.iter().find(|e| e.date == orig);
        let v = match ex {
            Some(e) if e.action == ExceptionAction::Cancelled => Visit { original_date: orig, date: orig, time, cancelled: true },
            Some(e) => Visit { original_date: orig, date: e.new_date.unwrap_or(orig), time: e.new_time.unwrap_or(time), cancelled: false },
            None => Visit { original_date: orig, date: orig, time, cancelled: false },
        };
        if v.date >= from && v.date <= until { out.push(v); }
    };
    match r {
        None => push(start),
        Some(r) => {
            let end = end_date(start, r);
            let mut n = 0;
            while let Some(d) = nth_date(start, r.frequency, n) {
                if end.map_or(false, |e| d >= e) || d > until.checked_add_days(Days::new(400)).unwrap_or(until) { break; }
                push(d);
                n += 1;
                if n > 2000 { break; } // safety for "until cancelled" daily-like loops
            }
        }
    }
    out.sort_by_key(|v| (v.date, v.time));
    out.truncate(max);
    out
}

pub fn parse_date(s: &str) -> Option<NaiveDate> { NaiveDate::parse_from_str(s, "%Y-%m-%d").ok() }
pub fn parse_time(s: &str) -> Option<NaiveTime> { NaiveTime::parse_from_str(s, "%H:%M").ok() }
pub fn fmt_date(d: NaiveDate) -> String { d.format("%Y-%m-%d").to_string() }
pub fn fmt_time(t: NaiveTime) -> String { t.format("%H:%M").to_string() }

mod opt_hhmm {
    use chrono::NaiveTime;
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(t: &Option<NaiveTime>, s: S) -> Result<S::Ok, S::Error> {
        match t { Some(t) => s.serialize_str(&t.format("%H:%M").to_string()), None => s.serialize_none() }
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<NaiveTime>, D::Error> {
        let v: Option<String> = Option::deserialize(d)?;
        v.map(|s| NaiveTime::parse_from_str(&s, "%H:%M").map_err(serde::de::Error::custom)).transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Frequency::*;
    use Period::*;

    fn d(s: &str) -> NaiveDate { parse_date(s).unwrap() }
    fn rule(f: Frequency, p: Option<Period>) -> Recurrence { Recurrence { frequency: f, period: p } }

    /// The visit-count table in DataItem_Design.md 3.7 (start 1 Oct 2026).
    #[test]
    fn visit_count_table() {
        let start = d("2026-10-01");
        let expect = [
            (Daily, [31, 61, 92, 182]),
            (Weekly, [5, 9, 14, 26]),
            (Biweekly, [3, 5, 7, 13]),
            (Monthly, [1, 2, 3, 6]),
            (Quarterly, [1, 1, 1, 2]),
        ];
        for (f, counts) in expect {
            for (p, n) in [OneMonth, TwoMonths, ThreeMonths, SixMonths].into_iter().zip(counts) {
                assert_eq!(visit_count(start, &rule(f, Some(p))), Some(n), "{f:?} {p:?}");
            }
        }
    }

    #[test]
    fn single_visit_combinations_are_rejected() {
        let start = d("2026-10-01");
        assert!(validate(start, Some(&rule(Monthly, Some(OneMonth)))).is_err());
        for p in [OneMonth, TwoMonths, ThreeMonths] {
            assert!(validate(start, Some(&rule(Quarterly, Some(p)))).is_err());
        }
        assert!(validate(start, Some(&rule(Quarterly, Some(SixMonths)))).is_ok());
        assert!(validate(start, Some(&rule(Daily, None))).is_err());
        assert!(validate(start, Some(&rule(Weekly, None))).is_ok());
        assert!(validate(start, None).is_ok());
    }

    #[test]
    fn month_end_uses_last_day() {
        let start = d("2026-01-31");
        let v = visits(start, parse_time("09:00").unwrap(), Some(&rule(Monthly, Some(SixMonths))), &[], start, d("2026-12-31"), 10);
        let dates: Vec<String> = v.iter().map(|v| fmt_date(v.date)).collect();
        assert_eq!(dates, ["2026-01-31", "2026-02-28", "2026-03-31", "2026-04-30", "2026-05-31", "2026-06-30"]);
    }

    #[test]
    fn exceptions_move_and_cancel_single_visits() {
        let start = d("2026-10-05");
        let t = parse_time("11:30").unwrap();
        let ex = vec![
            VisitException { date: d("2026-10-19"), action: ExceptionAction::Moved, new_date: Some(d("2026-10-20")), new_time: parse_time("10:00") },
            VisitException { date: d("2026-11-02"), action: ExceptionAction::Cancelled, new_date: None, new_time: None },
        ];
        let v = visits(start, t, Some(&rule(Biweekly, Some(ThreeMonths))), &ex, start, d("2026-12-31"), 20);
        assert_eq!(v.len(), 7);
        assert_eq!((fmt_date(v[1].date), fmt_time(v[1].time)), ("2026-10-20".to_string(), "10:00".to_string()));
        assert!(v[2].cancelled && v[2].original_date == d("2026-11-02"));
    }

    #[test]
    fn until_cancelled_is_limited_by_window() {
        let start = d("2026-10-01");
        let v = visits(start, parse_time("08:00").unwrap(), Some(&rule(Weekly, None)), &[], start, d("2026-10-31"), 100);
        assert_eq!(v.len(), 5);
    }
}
