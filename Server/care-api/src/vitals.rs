//! Vitals (Documentation/Vitals_Design.md): the built-in catalogue, the colour rule and the
//! access rules. Pure functions only; storage is in `store/vitals.rs`, HTTP in `api/vitals.rs`.
//!
//! Vitals are deliberately not DataItems: they are small, frequent, per-user measurements with
//! their own tables. A reading is green inside the person's range, yellow up to `margin_pct`
//! outside it and red beyond that. Each person starts with the default range of the vital; only
//! a connected doctor can change it.

use crate::model::Role;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

/// Written with every stored row and sent in every DTO. Readers ignore fields they don't know;
/// bump this only when the meaning of an existing field changes.
pub const SCHEMA_VERSION: i32 = 1;

// ---------------- configuration (Server/config/vitals.json) ----------------

/// The defaults compiled into the server; `VITALS_CONFIG` points at a file that replaces them.
const EMBEDDED: &str = include_str!("../../config/vitals.json");

#[derive(Clone, Debug, Deserialize)]
pub struct Colours {
    /// How far outside the range a value may be and still be yellow (percent of the bound crossed).
    pub yellow_margin_pct: i32,
    /// What a doctor may set the margin to for one patient.
    pub margin_pct_min: i32,
    pub margin_pct_max: i32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Limits {
    pub value_max: f64,
    pub note_max_chars: usize,
    pub custom_label_max_chars: usize,
    pub custom_unit_max_chars: usize,
    pub readings_default_limit: i32,
    pub readings_max_limit: i32,
    /// Most recent alerts looked at for a doctor's dashboard.
    pub doctor_inbox_window: i32,
    /// How long ago a reading may have been taken when it is entered. Optional in the file, so
    /// one written before this setting existed still loads.
    #[serde(default = "default_backdate_hours")]
    pub backdate_hours: i64,
}

fn default_backdate_hours() -> i64 { 12 }

/// Colours a reading by one value as a percentage of another: urine output against water intake.
/// `green_min_pct` or more is green, from `yellow_min_pct` up to that yellow, less is red.
/// The thresholds are left out (`None`) when the type is sent to the person themselves.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RatioRule {
    pub numerator: String,
    pub denominator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub green_min_pct: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yellow_min_pct: Option<f64>,
}

impl RatioRule {
    /// The numerator as a percentage of the denominator; `None` when it can't be worked out.
    pub fn percent(&self, values: &HashMap<String, f64>) -> Option<f64> {
        let (n, d) = (*values.get(&self.numerator)?, *values.get(&self.denominator)?);
        if d > 0.0 { Some(n / d * 100.0) } else { None }
    }

    pub fn severity(&self, values: &HashMap<String, f64>) -> Severity {
        let (Some(pct), Some(green), Some(yellow)) = (self.percent(values), self.green_min_pct, self.yellow_min_pct) else { return Severity::Green };
        // The same hair of tolerance as the range rule, so exactly 85% is green.
        if pct + 1e-9 >= green { Severity::Green } else if pct + 1e-9 >= yellow { Severity::Yellow } else { Severity::Red }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct VitalDef {
    code: String,
    label: String,
    icon: String,
    #[serde(default)]
    unit: String,
    components: Vec<Component>,
    /// Overrides `colours.yellow_margin_pct` for this vital.
    margin_pct: Option<i32>,
    ratio: Option<RatioRule>,
}

/// The vitals block on the Home screen. The last tile is always "More".
#[derive(Clone, Debug, Deserialize)]
pub struct Home { pub columns: i32, pub rows: i32 }

impl Default for Home {
    fn default() -> Self { Home { columns: 5, rows: 2 } }
}

#[derive(Clone, Debug, Deserialize)]
pub struct VitalsConfig {
    pub colours: Colours,
    /// Optional, so a configuration file written before this section existed still loads.
    #[serde(default)]
    pub home: Home,
    pub limits: Limits,
    /// Icon keys a doctor can choose for a vital they add; the built-ins use the same set.
    pub icons: Vec<String>,
    /// The vitals everyone has, in home-screen order, with the ranges people start with.
    vitals: Vec<VitalDef>,
}

impl VitalsConfig {
    /// Parses and checks a configuration file; the error says what to fix.
    pub fn parse(text: &str) -> Result<Self, String> {
        let cfg: VitalsConfig = serde_json::from_str(text).map_err(|e| format!("not valid: {e}"))?;
        cfg.check()?;
        Ok(cfg)
    }

    fn check(&self) -> Result<(), String> {
        let (c, l) = (&self.colours, &self.limits);
        if c.margin_pct_min < 0 || c.margin_pct_min > c.margin_pct_max { return Err("colours: margin_pct_min must be 0 or more and not above margin_pct_max".into()); }
        if !(c.margin_pct_min..=c.margin_pct_max).contains(&c.yellow_margin_pct) { return Err("colours: yellow_margin_pct must be between margin_pct_min and margin_pct_max".into()); }
        if !(l.value_max > 0.0) || l.note_max_chars == 0 || l.custom_label_max_chars == 0 || l.doctor_inbox_window < 1
            || l.readings_default_limit < 1 || l.readings_max_limit < l.readings_default_limit || !(0..=72).contains(&l.backdate_hours) {
            return Err("limits: every limit must be positive, readings_max_limit at least readings_default_limit, and backdate_hours 0–72".into());
        }
        if !(2..=6).contains(&self.home.columns) || !(1..=4).contains(&self.home.rows) { return Err("home: columns must be 2–6 and rows 1–4".into()); }
        if self.icons.is_empty() { return Err("icons: list at least one icon".into()); }
        let mut codes = HashSet::new();
        for v in &self.vitals {
            let at = |m: &str| format!("vitals \"{}\": {m}", v.code);
            if v.code.is_empty() || v.code.starts_with("c_") || !v.code.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_') {
                return Err(at("code must be lower-case letters, digits or _ and not start with c_ (reserved for doctor-added vitals)"));
            }
            if !codes.insert(&v.code) { return Err(at("code is used twice")); }
            if v.label.trim().is_empty() { return Err(at("label is empty")); }
            if !self.icons.contains(&v.icon) { return Err(at("icon is not in the icons list")); }
            if v.components.is_empty() { return Err(at("needs at least one component")); }
            let mut keys = HashSet::new();
            for comp in &v.components {
                if comp.key.is_empty() || !keys.insert(&comp.key) { return Err(at("component keys must be non-empty and unique")); }
                for n in [comp.low, comp.high].into_iter().flatten() {
                    if !n.is_finite() || !(0.0..=l.value_max).contains(&n) { return Err(at("range values must be between 0 and limits.value_max")); }
                }
                if let (Some(lo), Some(hi)) = (comp.low, comp.high) { if lo > hi { return Err(at("low is above high")); } }
            }
            if let Some(m) = v.margin_pct { if !(c.margin_pct_min..=c.margin_pct_max).contains(&m) { return Err(at("margin_pct is outside the allowed margin")); } }
            // Keys travel as JSON map keys; the iOS app's snake_case conversion would rename others.
            if v.components.iter().any(|comp| !comp.key.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())) {
                return Err(at("component keys must be lower-case letters and digits only"));
            }
            if let Some(r) = &v.ratio {
                if r.numerator == r.denominator || !keys.contains(&r.numerator) || !keys.contains(&r.denominator) {
                    return Err(at("ratio: numerator and denominator must be two different components of this vital"));
                }
                match (r.green_min_pct, r.yellow_min_pct) {
                    (Some(g), Some(y)) if g.is_finite() && y.is_finite() && 0.0 <= y && y <= g => {}
                    _ => return Err(at("ratio: needs green_min_pct and yellow_min_pct, with 0 <= yellow_min_pct <= green_min_pct")),
                }
            }
        }
        Ok(())
    }

    /// The vitals everyone has, as people see them before a doctor changes anything.
    pub fn builtin(&self) -> Vec<VitalType> {
        self.vitals.iter().enumerate().map(|(i, v)| VitalType {
            code: v.code.clone(), label: v.label.clone(), icon: v.icon.clone(), unit: v.unit.clone(),
            components: v.components.clone(),
            margin_pct: Some(v.margin_pct.unwrap_or(self.colours.yellow_margin_pct)),
            ratio: v.ratio.clone(),
            custom: false, position: i as i32, schema_version: SCHEMA_VERSION,
        }).collect()
    }
}

/// The active configuration: the file named by `VITALS_CONFIG`, or else the compiled-in defaults.
/// Loaded once. A file that can't be read or fails the checks stops the process with the reason
/// (the servers call this at start-up), rather than running with colours nobody chose.
pub fn config() -> &'static VitalsConfig {
    static CONFIG: std::sync::OnceLock<VitalsConfig> = std::sync::OnceLock::new();
    CONFIG.get_or_init(|| {
        let loaded = match std::env::var("VITALS_CONFIG").ok().filter(|p| !p.trim().is_empty()) {
            Some(path) => std::fs::read_to_string(&path).map_err(|e| format!("can't read {path}: {e}"))
                .and_then(|text| VitalsConfig::parse(&text).map_err(|e| format!("{path}: {e}")))
                .map(|cfg| { tracing::info!("vitals configuration loaded from {path}"); cfg }),
            None => VitalsConfig::parse(EMBEDDED).map_err(|e| format!("built-in defaults: {e}")),
        };
        loaded.unwrap_or_else(|e| panic!("vitals configuration: {e}"))
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Severity { Green, Yellow, Red }

impl Severity {
    pub fn as_str(&self) -> &'static str { match self { Severity::Green => "GREEN", Severity::Yellow => "YELLOW", Severity::Red => "RED" } }
    /// Unknown text (written by a newer server) reads as green rather than failing.
    pub fn parse(s: &str) -> Severity { match s { "RED" => Severity::Red, "YELLOW" => Severity::Yellow, _ => Severity::Green } }
}

/// One side or both may be open (`None`): weight has no default range at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    #[serde(default)]
    pub low: Option<f64>,
    #[serde(default)]
    pub high: Option<f64>,
}

/// Component key ("value", or "sys"/"dia" for blood pressure) → its bounds.
pub type RangeSpec = BTreeMap<String, Bounds>;

/// One number the person enters. Most vitals have a single component called "value".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Component {
    pub key: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<f64>,
}

/// A vital as one person sees it: built-in or added for them by a doctor, with the range that
/// applies to them. Patients get it without the range (`without_ranges`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VitalType {
    pub code: String,
    pub label: String,
    /// A key from [`ICONS`]; each app maps it to its own icon set.
    pub icon: String,
    pub unit: String,
    pub components: Vec<Component>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_pct: Option<i32>,
    /// Set for vitals coloured by one value as a percentage of another (urine).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ratio: Option<RatioRule>,
    /// True when a doctor added it for this person.
    #[serde(default)]
    pub custom: bool,
    pub position: i32,
    #[serde(default = "version")]
    pub schema_version: i32,
}

fn version() -> i32 { SCHEMA_VERSION }

impl VitalType {
    pub fn range(&self) -> RangeSpec {
        self.components.iter().map(|c| (c.key.clone(), Bounds { low: c.low, high: c.high })).collect()
    }

    /// Replaces the default range with the doctor's one for this person.
    pub fn apply_range(&mut self, spec: &RangeSpec, margin_pct: Option<i32>) {
        for c in &mut self.components {
            if let Some(b) = spec.get(&c.key) { c.low = b.low; c.high = b.high; }
        }
        if margin_pct.is_some() { self.margin_pct = margin_pct; }
    }

    /// Ranges are not shown to the person themselves, only the resulting colour.
    pub fn without_ranges(mut self) -> Self {
        for c in &mut self.components { c.low = None; c.high = None; }
        self.margin_pct = None;
        if let Some(r) = &mut self.ratio { r.green_min_pct = None; r.yellow_min_pct = None; }
        self
    }

    /// Colour of a reading of this vital: the worse of its range colour and, when the vital has
    /// a ratio rule, its ratio colour.
    pub fn severity(&self, values: &HashMap<String, f64>) -> Severity {
        let by_range = severity_of(values, &self.range(), self.margin());
        self.ratio.as_ref().map_or(by_range, |r| by_range.max(r.severity(values)))
    }

    pub fn margin(&self) -> i32 { self.margin_pct.unwrap_or(config().colours.yellow_margin_pct) }
}

/// The vitals everyone has, in home-screen order, with the default ranges people start with
/// (from the configuration). A doctor sets the range for each patient. Adding a vital to the
/// configuration needs no app update.
pub fn builtin() -> Vec<VitalType> { config().builtin() }

/// Colour of one number against its bounds.
pub fn component_severity(v: f64, b: Bounds, margin_pct: i32) -> Severity {
    let margin = margin_pct.max(0) as f64;
    // How far outside, as a percentage of the bound that was crossed.
    let outside = |bound: f64, by: f64| if bound.abs() < f64::EPSILON { f64::INFINITY } else { by / bound.abs() * 100.0 };
    let pct = match (b.low, b.high) {
        (Some(low), _) if v < low => outside(low, low - v),
        (_, Some(high)) if v > high => outside(high, v - high),
        _ => return Severity::Green,
    };
    // A hair of tolerance so that exactly "30% over" stays yellow despite floating point.
    if pct <= margin + 1e-9 { Severity::Yellow } else { Severity::Red }
}

/// Colour of a reading: the worst of its components. Components without bounds are green.
pub fn severity_of(values: &HashMap<String, f64>, range: &RangeSpec, margin_pct: i32) -> Severity {
    values.iter()
        .map(|(k, v)| range.get(k).map_or(Severity::Green, |b| component_severity(*v, *b, margin_pct)))
        .max().unwrap_or(Severity::Green)
}

/// A reading must give every component of the vital, as a sensible number.
pub fn validate_values(t: &VitalType, values: &HashMap<String, f64>) -> Result<(), String> {
    let max = config().limits.value_max;
    for c in &t.components {
        match values.get(&c.key) {
            None => return Err(format!("missing value for {}", c.label)),
            Some(v) if !v.is_finite() || *v < 0.0 || *v > max => return Err(format!("{} must be a number from 0 to {max}", c.label)),
            _ => {}
        }
    }
    if let Some(k) = values.keys().find(|k| !t.components.iter().any(|c| &c.key == *k)) {
        return Err(format!("{} has no value called {k}", t.label));
    }
    // A percentage of nothing can't be coloured.
    if let Some(r) = &t.ratio {
        if r.percent(values).is_none() {
            let name = t.components.iter().find(|c| c.key == r.denominator).map_or(r.denominator.as_str(), |c| c.label.as_str());
            return Err(format!("{name} must be more than 0"));
        }
    }
    Ok(())
}

/// When a reading was taken: now if not given, otherwise at most `backdate_hours` ago and not in
/// the future (a few minutes of clock difference between phone and server are allowed).
/// Readings have no edit operation, so this is the only time a reading ever gets.
pub fn taken_at_millis(given: Option<i64>, now: i64, backdate_hours: i64) -> Result<i64, String> {
    const CLOCK_SKEW_MS: i64 = 5 * 60_000;
    let Some(at) = given else { return Ok(now) };
    if at > now + CLOCK_SKEW_MS { return Err("the time can't be in the future".into()); }
    if at < now - backdate_hours * 3_600_000 { return Err(format!("the time can be at most {backdate_hours} hours ago")); }
    Ok(at.min(now))
}

/// A range names only components the vital has, with low ≤ high; the margin stays within the
/// configured bounds.
pub fn validate_range(t: &VitalType, spec: &RangeSpec, margin_pct: Option<i32>) -> Result<(), String> {
    let (c, max) = (&config().colours, config().limits.value_max);
    if let Some(m) = margin_pct {
        if !(c.margin_pct_min..=c.margin_pct_max).contains(&m) { return Err(format!("margin_pct must be {}–{}", c.margin_pct_min, c.margin_pct_max)); }
    }
    for (k, b) in spec {
        if !t.components.iter().any(|c| &c.key == k) { return Err(format!("{} has no value called {k}", t.label)); }
        for v in [b.low, b.high].into_iter().flatten() {
            if !v.is_finite() || !(0.0..=max).contains(&v) { return Err(format!("range values must be numbers from 0 to {max}")); }
        }
        if let (Some(l), Some(h)) = (b.low, b.high) { if l > h { return Err("the low end of a range can't be above the high end".into()); } }
    }
    Ok(())
}

/// Code for a doctor-added vital: "c_" + the label's letters and digits.
pub fn custom_code(label: &str) -> Option<String> {
    let slug: String = label.trim().to_lowercase().chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect::<String>()
        .split('_').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("_");
    if slug.is_empty() { None } else { Some(format!("c_{}", slug.chars().take(40).collect::<String>())) }
}

// ---------------- access ----------------

/// A person reads their own vitals; a doctor reads those of a patient in their contacts.
pub fn may_read(owner: Uuid, who: Uuid, who_roles: &HashSet<Role>, connected: bool) -> bool {
    owner == who || (who_roles.contains(&Role::Doctor) && connected)
}

/// Readings are entered by the person themselves.
pub fn may_record(owner: Uuid, who: Uuid) -> bool { owner == who }

/// Only a doctor connected to the patient sets ranges or adds vitals for them — never the
/// patient, and never a doctor for themselves.
pub fn may_configure(owner: Uuid, who: Uuid, who_roles: &HashSet<Role>, connected: bool) -> bool {
    owner != who && who_roles.contains(&Role::Doctor) && connected
}

#[cfg(test)]
mod tests {
    //! Tests V1–V10 of Documentation/Vitals_Design.md section 8.
    use super::*;

    fn vals(pairs: &[(&str, f64)]) -> HashMap<String, f64> { pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect() }
    fn bounds(low: f64, high: f64) -> Bounds { Bounds { low: Some(low), high: Some(high) } }
    fn roles(r: &[Role]) -> HashSet<Role> { r.iter().copied().collect() }
    fn get(code: &str) -> VitalType { builtin().into_iter().find(|t| t.code == code).unwrap() }

    #[test]
    fn v1_inside_range_is_green_including_the_ends() {
        for v in [80.0, 100.0, 120.0] { assert_eq!(component_severity(v, bounds(80.0, 120.0), 30), Severity::Green, "{v}"); }
    }

    #[test]
    fn v2_up_to_30_percent_outside_is_yellow() {
        let b = bounds(80.0, 120.0);
        assert_eq!(component_severity(121.0, b, 30), Severity::Yellow);
        assert_eq!(component_severity(156.0, b, 30), Severity::Yellow); // exactly 30% over 120
        assert_eq!(component_severity(79.0, b, 30), Severity::Yellow);
        assert_eq!(component_severity(56.0, b, 30), Severity::Yellow);  // exactly 30% under 80
    }

    #[test]
    fn v3_beyond_the_margin_is_red() {
        let b = bounds(80.0, 120.0);
        assert_eq!(component_severity(156.1, b, 30), Severity::Red);
        assert_eq!(component_severity(55.9, b, 30), Severity::Red);
        assert_eq!(component_severity(300.0, b, 30), Severity::Red);
    }

    #[test]
    fn v4_the_doctor_can_change_the_margin() {
        let b = bounds(80.0, 120.0);
        assert_eq!(component_severity(140.0, b, 10), Severity::Red);    // 16.7% over, margin 10
        assert_eq!(component_severity(140.0, b, 50), Severity::Yellow);
        assert_eq!(component_severity(121.0, b, 0), Severity::Red);     // no yellow band at all
    }

    #[test]
    fn v5_blood_pressure_takes_the_worse_of_its_two_values() {
        let bp = get("bp");
        assert_eq!(severity_of(&vals(&[("sys", 118.0), ("dia", 78.0)]), &bp.range(), 30), Severity::Green);
        assert_eq!(severity_of(&vals(&[("sys", 118.0), ("dia", 95.0)]), &bp.range(), 30), Severity::Yellow);
        assert_eq!(severity_of(&vals(&[("sys", 190.0), ("dia", 95.0)]), &bp.range(), 30), Severity::Red);
    }

    #[test]
    fn v6_open_ranges_and_no_range() {
        assert_eq!(severity_of(&vals(&[("value", 72.5)]), &get("weight").range(), 30), Severity::Green);
        let only_low = Bounds { low: Some(95.0), high: None };
        assert_eq!(component_severity(100.0, only_low, 30), Severity::Green);
        assert_eq!(component_severity(90.0, only_low, 30), Severity::Yellow);
        assert_eq!(component_severity(5.0, Bounds { low: None, high: Some(0.0) }, 30), Severity::Red); // nothing is "30% of zero"
    }

    #[test]
    fn v7_a_persons_own_range_replaces_the_default() {
        // One person's sugar range is 80–160, another's the default 80–120: 150 is green for the first only.
        let default = get("sugar");
        let mut own = default.clone();
        own.apply_range(&[("value".to_string(), bounds(80.0, 160.0))].into(), None);
        let reading = vals(&[("value", 150.0)]);
        assert_eq!(severity_of(&reading, &own.range(), own.margin()), Severity::Green);
        assert_eq!(severity_of(&reading, &default.range(), default.margin()), Severity::Yellow);
    }

    #[test]
    fn v8_readings_and_ranges_are_validated() {
        let bp = get("bp");
        assert!(validate_values(&bp, &vals(&[("sys", 120.0), ("dia", 80.0)])).is_ok());
        assert!(validate_values(&bp, &vals(&[("sys", 120.0)])).is_err());                       // diastolic missing
        assert!(validate_values(&bp, &vals(&[("sys", 120.0), ("dia", -1.0)])).is_err());
        assert!(validate_values(&bp, &vals(&[("sys", 120.0), ("dia", 80.0), ("x", 1.0)])).is_err());
        assert!(validate_values(&get("sugar"), &vals(&[("value", f64::NAN)])).is_err());
        assert!(validate_range(&bp, &[("sys".to_string(), bounds(110.0, 100.0))].into(), None).is_err());
        assert!(validate_range(&bp, &[("pulse".to_string(), bounds(1.0, 2.0))].into(), None).is_err());
        assert!(validate_range(&bp, &[("sys".to_string(), bounds(100.0, 110.0))].into(), Some(201)).is_err());
        assert!(validate_range(&bp, &[("sys".to_string(), bounds(100.0, 110.0))].into(), Some(20)).is_ok());
    }

    #[test]
    fn v9_only_a_connected_doctor_reads_or_configures_someone_elses_vitals() {
        let (patient, other) = (Uuid::new_v4(), Uuid::new_v4());
        assert!(may_read(patient, patient, &roles(&[Role::Patient]), false));
        assert!(may_read(patient, other, &roles(&[Role::Doctor]), true));
        assert!(!may_read(patient, other, &roles(&[Role::Doctor]), false));
        for r in [Role::Patient, Role::Lab, Role::Hospital, Role::Assistant, Role::Administrator] {
            assert!(!may_read(patient, other, &roles(&[r]), true), "{r:?}");
            assert!(!may_configure(patient, other, &roles(&[r]), true), "{r:?}");
        }
        assert!(may_configure(patient, other, &roles(&[Role::Doctor]), true));
        assert!(!may_configure(patient, other, &roles(&[Role::Doctor]), false));
        assert!(!may_configure(patient, patient, &roles(&[Role::Patient]), true));   // not the patient
        assert!(!may_configure(other, other, &roles(&[Role::Doctor]), true));        // not for themselves
        assert!(may_record(patient, patient) && !may_record(patient, other));
    }

    #[test]
    fn v10_catalogue_is_well_formed_and_tolerant_of_newer_data() {
        let all = builtin();
        let codes: HashSet<&str> = all.iter().map(|t| t.code.as_str()).collect();
        assert_eq!(codes.len(), all.len());
        assert_eq!(all.len(), 9); // nine tiles + "More" make two rows of five
        for t in &all {
            assert!(config().icons.contains(&t.icon), "{}", t.icon);
            assert!(validate_range(t, &t.range(), t.margin_pct).is_ok());
            assert!(t.clone().without_ranges().components.iter().all(|c| c.low.is_none() && c.high.is_none()));
        }
        assert_eq!(custom_code("  Peak Flow (L/min) "), Some("c_peak_flow_l_min".into()));
        assert_eq!(custom_code("???"), None);
        // Fields added by a newer server or app are ignored; missing optional ones get defaults.
        let newer = r#"{"code":"x","label":"X","icon":"other","unit":"u","position":3,"future_field":{"a":1},
                        "components":[{"key":"value","label":"X","low":1,"precision":2}]}"#;
        let t: VitalType = serde_json::from_str(newer).unwrap();
        assert_eq!((t.schema_version, t.custom, t.components[0].high), (SCHEMA_VERSION, false, None));
        assert_eq!(Severity::parse("PURPLE"), Severity::Green);
    }

    #[test]
    fn v11_configuration_file_sets_colours_and_defaults() {
        // The shipped file: yellow is 30% and BP starts at 90–120 / 60–80.
        assert_eq!(config().colours.yellow_margin_pct, 30);
        assert_eq!(get("bp").range()["sys"], bounds(90.0, 120.0));
        // A changed file changes behaviour without code: 10% margin, sugar 70–100.
        let edited = EMBEDDED.replace("\"yellow_margin_pct\": 30", "\"yellow_margin_pct\": 10")
            .replace("\"label\": \"Sugar\", \"low\": 80, \"high\": 120", "\"label\": \"Sugar\", \"low\": 70, \"high\": 100");
        let cfg = VitalsConfig::parse(&edited).unwrap();
        let sugar = cfg.builtin().into_iter().find(|t| t.code == "sugar").unwrap();
        assert_eq!((sugar.margin_pct, sugar.range()["value"]), (Some(10), bounds(70.0, 100.0)));
        assert_eq!(severity_of(&vals(&[("value", 115.0)]), &sugar.range(), sugar.margin()), Severity::Red);    // 15% over 100
        assert_eq!(severity_of(&vals(&[("value", 115.0)]), &get("sugar").range(), 30), Severity::Green);
    }

    #[test]
    fn v13_urine_is_coloured_by_output_as_a_percentage_of_intake() {
        let urine = get("urine");
        assert_eq!(urine.components.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(), ["intake", "output"]);
        let colour = |intake: f64, output: f64| urine.severity(&vals(&[("intake", intake), ("output", output)]));
        assert_eq!(colour(1000.0, 900.0), Severity::Green);     // 90%
        assert_eq!(colour(1000.0, 850.0), Severity::Green);     // exactly 85%
        assert_eq!(colour(1000.0, 849.0), Severity::Yellow);
        assert_eq!(colour(1000.0, 700.0), Severity::Yellow);    // exactly 70%
        assert_eq!(colour(1000.0, 699.0), Severity::Red);
        assert_eq!(colour(1000.0, 0.0), Severity::Red);
        assert_eq!(colour(1000.0, 1200.0), Severity::Green);    // more out than in is still "85% or more"
        // No intake: a percentage can't be worked out, so the reading is refused rather than coloured.
        assert!(validate_values(&urine, &vals(&[("intake", 0.0), ("output", 300.0)])).unwrap_err().contains("Water intake"));
        assert!(validate_values(&urine, &vals(&[("intake", 1000.0), ("output", 300.0)])).is_ok());
        // The thresholds come from the configuration file, and the patient never receives them.
        let cfg = VitalsConfig::parse(&EMBEDDED.replace("\"green_min_pct\": 85, \"yellow_min_pct\": 70", "\"green_min_pct\": 95, \"yellow_min_pct\": 90")).unwrap();
        let strict = cfg.builtin().into_iter().find(|t| t.code == "urine").unwrap();
        assert_eq!(strict.severity(&vals(&[("intake", 1000.0), ("output", 900.0)])), Severity::Yellow);
        assert_eq!(strict.severity(&vals(&[("intake", 1000.0), ("output", 880.0)])), Severity::Red);
        let shown = urine.clone().without_ranges().ratio.unwrap();
        assert_eq!((shown.green_min_pct, shown.yellow_min_pct, shown.numerator.as_str()), (None, None, "output"));
        // A doctor's absolute range on a component still counts: the worse colour wins.
        let mut limited = urine.clone();
        limited.apply_range(&[("output".to_string(), bounds(800.0, 2000.0))].into(), None);
        assert_eq!(limited.severity(&vals(&[("intake", 500.0), ("output", 450.0)])), Severity::Red);   // 90% but far below 800
    }

    #[test]
    fn v14_time_is_now_or_within_the_last_12_hours() {
        let (now, hour) = (1_800_000_000_000_i64, 3_600_000_i64);
        assert_eq!(taken_at_millis(None, now, 12), Ok(now));
        assert_eq!(taken_at_millis(Some(now - 3 * hour), now, 12), Ok(now - 3 * hour));
        assert_eq!(taken_at_millis(Some(now - 12 * hour), now, 12), Ok(now - 12 * hour));     // exactly 12 hours
        assert!(taken_at_millis(Some(now - 12 * hour - 1), now, 12).unwrap_err().contains("12 hours"));
        assert!(taken_at_millis(Some(now + hour), now, 12).unwrap_err().contains("future"));
        assert_eq!(taken_at_millis(Some(now + 60_000), now, 12), Ok(now));                    // phone clock a minute ahead
        assert_eq!(config().limits.backdate_hours, 12);
        assert!(taken_at_millis(Some(now - 2 * hour), now, 1).is_err());                      // the limit is configurable
    }

    #[test]
    fn v12_a_broken_configuration_is_refused_with_the_reason() {
        let bad = |from: &str, to: &str| {
            assert!(EMBEDDED.contains(from), "test is out of date: {from}");
            VitalsConfig::parse(&EMBEDDED.replace(from, to)).unwrap_err()
        };
        assert!(bad("\"yellow_margin_pct\": 30", "\"yellow_margin_pct\": 500").contains("yellow_margin_pct"));
        assert!(bad("\"low\": 90, \"high\": 120", "\"low\": 130, \"high\": 120").contains("low is above high"));
        assert!(bad("\"code\": \"sugar\"", "\"code\": \"bp\"").contains("used twice"));
        assert!(bad("\"icon\": \"sleep\"", "\"icon\": \"moon\"").contains("icons list"));
        assert!(bad("\"code\": \"urine\"", "\"code\": \"c_urine\"").contains("c_"));
        assert!(bad("\"columns\": 5", "\"columns\": 12").contains("home"));
        assert!(bad("\"green_min_pct\": 85", "\"green_min_pct\": 60").contains("ratio"));          // green below yellow
        assert!(bad("\"numerator\": \"output\"", "\"numerator\": \"volume\"").contains("ratio"));   // not a component
        assert!(bad("\"backdate_hours\": 12", "\"backdate_hours\": 500").contains("backdate_hours"));
        // A file from before the "home" section existed still loads, with 5 x 2.
        let older = serde_json::to_string(&{
            let mut v: serde_json::Value = serde_json::from_str(EMBEDDED).unwrap();
            v.as_object_mut().unwrap().remove("home");
            v
        }).unwrap();
        let cfg = VitalsConfig::parse(&older).unwrap();
        assert_eq!((cfg.home.columns, cfg.home.rows), (5, 2));
        assert!(VitalsConfig::parse("{ not json").unwrap_err().contains("not valid"));
    }
}
