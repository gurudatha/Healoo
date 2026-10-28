//! Server settings from `Server/config/server.json` (Documentation/Configuration.md): the limits
//! and defaults that used to be constants in the code. Deployment settings (addresses, secrets,
//! modes) stay in environment variables, see `config.rs`; vitals have their own file, see
//! `vitals.rs`.
//!
//! The file is compiled in as the defaults; `SERVER_CONFIG` names a file that replaces them.
//! Every setting is optional in the file, so one written for an older server still loads.

use serde::Deserialize;
use std::time::Duration;

const EMBEDDED: &str = include_str!("../../config/server.json");
const MB: i64 = 1024 * 1024;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Uploads {
    /// Files per item, and per upload request (doc 4.5).
    pub max_attachments: usize,
    pub image_max_mb: i64,
    pub pdf_max_mb: i64,
    /// Longest side of the thumbnails the worker makes.
    pub thumbnail_px: u32,
    /// How long a download or upload link stays valid (doc 7.4).
    pub download_link_minutes: u64,
}

impl Default for Uploads {
    fn default() -> Self { Uploads { max_attachments: 20, image_max_mb: 10, pdf_max_mb: 25, thumbnail_px: 320, download_link_minutes: 10 } }
}

impl Uploads {
    pub fn image_max_bytes(&self) -> i64 { self.image_max_mb * MB }
    pub fn pdf_max_bytes(&self) -> i64 { self.pdf_max_mb * MB }
    pub fn link_ttl(&self) -> Duration { Duration::from_secs(self.download_link_minutes * 60) }
    /// Body limit for the file endpoint: the largest allowed file plus room for the request itself.
    pub fn file_body_bytes(&self) -> usize { (self.image_max_mb.max(self.pdf_max_mb) + 1) as usize * MB as usize }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Messages { pub max_chars: usize }

impl Default for Messages {
    fn default() -> Self { Messages { max_chars: 4000 } }
}

/// Per-user request limit (doc 4.5).
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct RateLimit { pub per_second: f64, pub burst: f64 }

impl Default for RateLimit {
    fn default() -> Self { RateLimit { per_second: 20.0, burst: 40.0 } }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Appointments {
    /// Used when an appointment or alert doesn't name a time zone (IANA name).
    pub default_timezone: String,
    /// How far ahead visits are kept ready for the calendar.
    pub calendar_days: i64,
    /// How many upcoming visits an appointment lists.
    pub upcoming_visits: usize,
}

impl Default for Appointments {
    fn default() -> Self { Appointments { default_timezone: "Asia/Kolkata".into(), calendar_days: 183, upcoming_visits: 12 } }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Realtime { pub max_sockets_per_user: usize }

impl Default for Realtime {
    fn default() -> Self { Realtime { max_sockets_per_user: 5 } }
}

/// How many administrators there may be (Administration_Design.md section 12).
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Administrators {
    /// Platform administrators (the dashboard), in total.
    pub max_platform_admins: usize,
    /// Hospital administrators, per hospital.
    pub max_admins_per_hospital: usize,
}

impl Default for Administrators {
    fn default() -> Self { Administrators { max_platform_admins: 3, max_admins_per_hospital: 2 } }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub uploads: Uploads,
    pub messages: Messages,
    pub rate_limit: RateLimit,
    pub appointments: Appointments,
    pub realtime: Realtime,
    pub administrators: Administrators,
}

impl Settings {
    /// Parses and checks a settings file; the error says what to fix.
    pub fn parse(text: &str) -> Result<Self, String> {
        let s: Settings = serde_json::from_str(text).map_err(|e| format!("not valid: {e}"))?;
        s.check()?;
        Ok(s)
    }

    fn check(&self) -> Result<(), String> {
        let u = &self.uploads;
        if !(1..=100).contains(&u.max_attachments) { return Err("uploads: max_attachments must be 1–100".into()); }
        if !(1..=200).contains(&u.image_max_mb) || !(1..=200).contains(&u.pdf_max_mb) { return Err("uploads: image_max_mb and pdf_max_mb must be 1–200".into()); }
        if !(64..=2048).contains(&u.thumbnail_px) { return Err("uploads: thumbnail_px must be 64–2048".into()); }
        if !(1..=1440).contains(&u.download_link_minutes) { return Err("uploads: download_link_minutes must be 1–1440".into()); }
        if !(1..=100_000).contains(&self.messages.max_chars) { return Err("messages: max_chars must be 1–100000".into()); }
        let r = &self.rate_limit;
        if !(r.per_second > 0.0 && r.per_second.is_finite()) || !(r.burst >= r.per_second && r.burst.is_finite()) {
            return Err("rate_limit: per_second must be more than 0 and burst at least per_second".into());
        }
        let a = &self.appointments;
        if a.default_timezone.parse::<chrono_tz::Tz>().is_err() { return Err("appointments: default_timezone must be an IANA name such as Asia/Kolkata".into()); }
        if !(1..=732).contains(&a.calendar_days) { return Err("appointments: calendar_days must be 1–732".into()); }
        if !(1..=100).contains(&a.upcoming_visits) { return Err("appointments: upcoming_visits must be 1–100".into()); }
        if !(1..=50).contains(&self.realtime.max_sockets_per_user) { return Err("realtime: max_sockets_per_user must be 1–50".into()); }
        let ad = &self.administrators;
        if !(1..=100).contains(&ad.max_platform_admins) || !(1..=100).contains(&ad.max_admins_per_hospital) {
            return Err("administrators: max_platform_admins and max_admins_per_hospital must be 1–100".into());
        }
        Ok(())
    }
}

/// The active settings: the file named by `SERVER_CONFIG`, or else the compiled-in defaults.
/// Loaded once. A file that can't be read or fails the checks stops the process with the reason
/// (the servers call this at start-up), rather than running with limits nobody chose.
pub fn settings() -> &'static Settings {
    static SETTINGS: std::sync::OnceLock<Settings> = std::sync::OnceLock::new();
    SETTINGS.get_or_init(|| {
        let loaded = match std::env::var("SERVER_CONFIG").ok().filter(|p| !p.trim().is_empty()) {
            Some(path) => std::fs::read_to_string(&path).map_err(|e| format!("can't read {path}: {e}"))
                .and_then(|text| Settings::parse(&text).map_err(|e| format!("{path}: {e}")))
                .map(|s| { tracing::info!("server settings loaded from {path}"); s }),
            None => Settings::parse(EMBEDDED).map_err(|e| format!("built-in defaults: {e}")),
        };
        loaded.unwrap_or_else(|e| panic!("server settings: {e}"))
    })
}

/// The time zone used when none is given (appointments.default_timezone).
pub fn default_tz() -> &'static str { &settings().appointments.default_timezone }

#[cfg(test)]
mod tests {
    //! Documentation/Configuration.md, tests S1–S4.
    use super::*;

    #[test]
    fn s1_the_shipped_file_holds_the_values_the_code_used_to_have() {
        let s = Settings::parse(EMBEDDED).unwrap();
        assert_eq!((s.uploads.max_attachments, s.uploads.image_max_bytes(), s.uploads.pdf_max_bytes()), (20, 10 * MB, 25 * MB));
        assert_eq!((s.uploads.thumbnail_px, s.uploads.link_ttl()), (320, Duration::from_secs(600)));
        assert_eq!(s.uploads.file_body_bytes(), 26 * 1024 * 1024);
        assert_eq!(s.messages.max_chars, 4000);
        assert_eq!((s.administrators.max_platform_admins, s.administrators.max_admins_per_hospital), (3, 2));
        assert_eq!((s.rate_limit.per_second, s.rate_limit.burst), (20.0, 40.0));
        assert_eq!((s.appointments.default_timezone.as_str(), s.appointments.calendar_days, s.appointments.upcoming_visits), ("Asia/Kolkata", 183, 12));
        assert_eq!(s.realtime.max_sockets_per_user, 5);
        // And the compiled-in defaults say the same, so a missing setting changes nothing.
        let d = Settings::default();
        assert_eq!((d.uploads.max_attachments, d.uploads.image_max_mb, d.uploads.pdf_max_mb, d.messages.max_chars), (20, 10, 25, 4000));
        assert_eq!(default_tz(), "Asia/Kolkata");
    }

    #[test]
    fn s2_editing_the_file_changes_the_limits() {
        let s = Settings::parse(&EMBEDDED.replace("\"image_max_mb\": 10", "\"image_max_mb\": 5").replace("\"max_chars\": 4000", "\"max_chars\": 500")).unwrap();
        assert_eq!((s.uploads.image_max_bytes(), s.messages.max_chars), (5 * MB, 500));
        assert_eq!(s.uploads.file_body_bytes(), 26 * 1024 * 1024);   // still sized for the 25 MB PDFs
    }

    #[test]
    fn s3_a_partial_or_older_file_keeps_defaults_for_the_rest() {
        let s = Settings::parse(r#"{ "uploads": { "pdf_max_mb": 50 }, "future_section": { "x": 1 } }"#).unwrap();
        assert_eq!((s.uploads.pdf_max_mb, s.uploads.image_max_mb, s.uploads.max_attachments), (50, 10, 20));
        assert_eq!((s.rate_limit.per_second, s.appointments.calendar_days), (20.0, 183));
        assert!(Settings::parse("{}").is_ok());
    }

    #[test]
    fn s4_a_broken_file_is_refused_with_the_reason() {
        let bad = |from: &str, to: &str| {
            assert!(EMBEDDED.contains(from), "test is out of date: {from}");
            Settings::parse(&EMBEDDED.replace(from, to)).unwrap_err()
        };
        assert!(bad("\"default_timezone\": \"Asia/Kolkata\"", "\"default_timezone\": \"India\"").contains("default_timezone"));
        assert!(bad("\"image_max_mb\": 10", "\"image_max_mb\": 0").contains("image_max_mb"));
        assert!(bad("\"burst\": 40", "\"burst\": 5").contains("burst"));
        assert!(bad("\"max_attachments\": 20", "\"max_attachments\": 5000").contains("max_attachments"));
        assert!(bad("\"max_sockets_per_user\": 5", "\"max_sockets_per_user\": 0").contains("max_sockets_per_user"));
        assert!(bad("\"max_platform_admins\": 3", "\"max_platform_admins\": 0").contains("max_platform_admins"));
        assert!(bad("\"max_admins_per_hospital\": 2", "\"max_admins_per_hospital\": 500").contains("max_admins_per_hospital"));
        assert!(Settings::parse("{ nope").unwrap_err().contains("not valid"));
    }
}
