//! Settings service: the user's preferences (`config.json`) and the app folders.

use serde::{Deserialize, Serialize};

use crate::activation::{ActivationMode, ConflictPolicy};
use crate::{ApiError, ApiResult, ErrorCode};

/// The smallest preview cache, in megabytes.
pub const CACHE_LIMIT_MB_MIN: u32 = 64;
/// The largest preview cache, in megabytes.
pub const CACHE_LIMIT_MB_MAX: u32 = 16_384;
/// The longest time between automatic rescans, in minutes (one day).
pub const RESCAN_INTERVAL_MAX_MINUTES: u32 = 1_440;

/// The colors of the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    /// Follow the OS appearance.
    #[default]
    System,
    /// Always light.
    Light,
    /// Always dark.
    Dark,
}

/// How much space the lists use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Density {
    /// Room to breathe.
    #[default]
    Comfortable,
    /// More rows on screen.
    Compact,
}

/// The language of the UI. Each value but `System` names a message catalog the
/// desktop app ships (`apps/desktop/src/locales/<tag>.json`); a new catalog adds
/// a value here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Language {
    /// Follow the OS language; English when the app has no catalog for it.
    #[default]
    System,
    /// English.
    En,
    /// German.
    De,
}

/// The user's settings. Missing keys take their defaults.
///
/// The TypeScript type marks every field optional because of the defaults;
/// a response always carries all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// What the Activate button does.
    pub activation_mode: ActivationMode,
    /// What activation does when a face clashes with another font.
    pub on_conflict: ConflictPolicy,
    /// Activate again, at start, the faces that were session-active when the
    /// app last ran (off by default: a session activation ends with the app).
    pub reactivate_on_launch: bool,
    /// Light, dark or the OS appearance.
    pub theme: Theme,
    /// Comfortable or compact lists.
    pub density: Density,
    /// The largest size of the preview cache, in megabytes (64–16384).
    pub cache_limit_mb: u32,
    /// Minutes between automatic rescans of the sources; 0 turns them off
    /// (at most 1440, one day).
    pub rescan_interval_minutes: u32,
    /// Closing the main window keeps the app running in the tray (the menu
    /// bar on macOS), so session fonts stay active; off, closing the window
    /// quits. On by default.
    pub keep_in_tray: bool,
    /// Whether the app starts, in the tray, when the user logs in. Read from
    /// the OS's login entry on every request, never stored in `config.json`;
    /// `None` when this system does not offer the option (then the switch
    /// is hidden). Off until the user turns it on.
    #[serde(default)]
    pub start_at_login: Option<bool>,
    /// Check for a new version when the app starts. Off unless the user
    /// turns it on (rule 7: no network call without a user action); where
    /// the app does not update itself the switch is hidden.
    #[serde(default)]
    pub check_updates_automatically: bool,
    /// The language of the UI, the native menus and the tray; `System` by
    /// default.
    #[serde(default)]
    pub language: Language,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            activation_mode: ActivationMode::Session,
            on_conflict: ConflictPolicy::KeepExisting,
            reactivate_on_launch: false,
            theme: Theme::System,
            density: Density::Comfortable,
            cache_limit_mb: 1_024,
            rescan_interval_minutes: 60,
            keep_in_tray: true,
            start_at_login: None,
            check_updates_automatically: false,
            language: Language::System,
        }
    }
}

/// Change some settings; absent fields keep their value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdate {
    /// What the Activate button does.
    #[serde(default)]
    pub activation_mode: Option<ActivationMode>,
    /// What activation does when a face clashes with another font.
    #[serde(default)]
    pub on_conflict: Option<ConflictPolicy>,
    /// Activate the last session's faces again at start.
    #[serde(default)]
    pub reactivate_on_launch: Option<bool>,
    /// Light, dark or the OS appearance.
    #[serde(default)]
    pub theme: Option<Theme>,
    /// Comfortable or compact lists.
    #[serde(default)]
    pub density: Option<Density>,
    /// The largest size of the preview cache, in megabytes.
    #[serde(default)]
    pub cache_limit_mb: Option<u32>,
    /// Minutes between automatic rescans; 0 turns them off.
    #[serde(default)]
    pub rescan_interval_minutes: Option<u32>,
    /// Keep running in the tray when the main window is closed.
    #[serde(default)]
    pub keep_in_tray: Option<bool>,
    /// Register (true) or remove (false) the login entry that starts the app
    /// in the tray. Refused where the settings report `start_at_login: None`.
    #[serde(default)]
    pub start_at_login: Option<bool>,
    /// Check for a new version when the app starts.
    #[serde(default)]
    pub check_updates_automatically: Option<bool>,
    /// The language of the UI.
    #[serde(default)]
    pub language: Option<Language>,
}

impl Settings {
    /// These settings with the update applied, or `InvalidRequest` when a
    /// value is out of range (then nothing changes). Every implementation of
    /// [`SettingsService::update_settings`] uses this, so the rules are one.
    pub fn apply(&self, update: &SettingsUpdate) -> ApiResult<Self> {
        if let Some(mb) = update.cache_limit_mb
            && !(CACHE_LIMIT_MB_MIN..=CACHE_LIMIT_MB_MAX).contains(&mb)
        {
            return Err(ApiError::invalid(format!(
                "The cache size must be between {CACHE_LIMIT_MB_MIN} and {CACHE_LIMIT_MB_MAX} MB."
            )));
        }
        if let Some(minutes) = update.rescan_interval_minutes
            && minutes > RESCAN_INTERVAL_MAX_MINUTES
        {
            return Err(ApiError::invalid(format!(
                "The rescan interval must be between 0 (off) and {RESCAN_INTERVAL_MAX_MINUTES} minutes."
            )));
        }
        if update.start_at_login.is_some() && self.start_at_login.is_none() {
            return Err(ApiError::new(
                ErrorCode::Unsupported,
                "Start at login is not available on this system.",
            ));
        }
        Ok(Self {
            activation_mode: update.activation_mode.unwrap_or(self.activation_mode),
            on_conflict: update.on_conflict.unwrap_or(self.on_conflict),
            reactivate_on_launch: update
                .reactivate_on_launch
                .unwrap_or(self.reactivate_on_launch),
            theme: update.theme.unwrap_or(self.theme),
            density: update.density.unwrap_or(self.density),
            cache_limit_mb: update.cache_limit_mb.unwrap_or(self.cache_limit_mb),
            rescan_interval_minutes: update
                .rescan_interval_minutes
                .unwrap_or(self.rescan_interval_minutes),
            keep_in_tray: update.keep_in_tray.unwrap_or(self.keep_in_tray),
            start_at_login: update.start_at_login.or(self.start_at_login),
            check_updates_automatically: update
                .check_updates_automatically
                .unwrap_or(self.check_updates_automatically),
            language: update.language.unwrap_or(self.language),
        })
    }
}

/// The per-user folders the app keeps its own files in (ADR 0007).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppFolders {
    /// Holds `config.json`.
    pub config: String,
    /// Holds the index database.
    pub data: String,
    /// Holds the preview cache.
    pub cache: String,
    /// Holds the daily log files.
    pub logs: String,
}

/// The local API endpoint (ADR 0015): where it listens and the token every
/// request must carry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EndpointInfo {
    /// `http://127.0.0.1:<port>`; `None` while the endpoint is not running
    /// (the app has not started it yet, or the caller is the CLI's direct link).
    #[serde(default)]
    pub url: Option<String>,
    /// The bearer token: 64 lowercase hex digits (32 random bytes).
    pub token: String,
}

/// The user's settings, the app folders and the local API endpoint.
pub trait SettingsService {
    /// The current settings.
    fn get_settings(&self) -> ApiResult<Settings>;
    /// Changes some settings and saves them; returns all of them.
    fn update_settings(&self, req: SettingsUpdate) -> ApiResult<Settings>;
    /// Where the app keeps its config, data, cache and logs.
    fn app_folders(&self) -> ApiResult<AppFolders>;
    /// The local API endpoint's address and token. `Unsupported` where there
    /// is no data folder to keep the token in.
    fn endpoint_info(&self) -> ApiResult<EndpointInfo> {
        Err(endpoint_unsupported())
    }
    /// Makes a new token, keeps it and returns it; the old one fails at once.
    fn regenerate_endpoint_token(&self) -> ApiResult<EndpointInfo> {
        Err(endpoint_unsupported())
    }
}

fn endpoint_unsupported() -> ApiError {
    ApiError::new(
        ErrorCode::Unsupported,
        "The local API is not available here.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_session_keep_existing_system_comfortable() {
        let s = Settings::default();
        assert_eq!(s.activation_mode, ActivationMode::Session);
        assert_eq!(s.on_conflict, ConflictPolicy::KeepExisting);
        assert!(!s.reactivate_on_launch);
        assert_eq!(s.theme, Theme::System);
        assert_eq!(s.density, Density::Comfortable);
        assert_eq!(s.cache_limit_mb, 1_024);
        assert_eq!(s.rescan_interval_minutes, 60);
        assert!(s.keep_in_tray);
        assert_eq!(s.start_at_login, None);
        assert!(!s.check_updates_automatically);
    }

    #[test]
    fn automatic_update_checks_are_off_for_old_config_files_and_can_be_turned_on() {
        // A config.json written before the field existed never checks by itself.
        let old: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).expect("old config");
        assert!(!old.check_updates_automatically);
        let json = serde_json::to_value(Settings::default()).expect("json");
        assert_eq!(json["checkUpdatesAutomatically"], false);
        let on: SettingsUpdate =
            serde_json::from_str(r#"{"checkUpdatesAutomatically":true}"#).expect("update");
        let next = Settings::default().apply(&on).expect("valid");
        assert!(next.check_updates_automatically);
        let unchanged = next.apply(&SettingsUpdate::default()).expect("valid");
        assert!(unchanged.check_updates_automatically);
    }

    #[test]
    fn start_at_login_is_never_written_and_refused_where_not_offered() {
        // Not offered: null on the wire.
        let json = serde_json::to_value(Settings::default()).expect("json");
        assert_eq!(json["startAtLogin"], serde_json::Value::Null);
        let on: SettingsUpdate = serde_json::from_str(r#"{"startAtLogin":true}"#).expect("update");
        let e = Settings::default().apply(&on).expect_err("not offered");
        assert_eq!(e.code, ErrorCode::Unsupported);

        // Offered (the service filled it from the OS): the update applies.
        let offered = Settings {
            start_at_login: Some(false),
            ..Settings::default()
        };
        assert_eq!(
            serde_json::to_value(offered).expect("json")["startAtLogin"],
            false
        );
        assert_eq!(
            offered.apply(&on).expect("valid").start_at_login,
            Some(true)
        );
        let unchanged = offered.apply(&SettingsUpdate::default()).expect("valid");
        assert_eq!(unchanged.start_at_login, Some(false));
    }

    #[test]
    fn language_is_system_for_old_config_files_and_can_be_chosen() {
        // A config.json written before the field existed follows the OS.
        let old: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).expect("old config");
        assert_eq!(old.language, Language::System);
        assert_eq!(Settings::default().language, Language::System);
        let json = serde_json::to_value(Settings::default()).expect("json");
        assert_eq!(json["language"], "system");
        let de: SettingsUpdate = serde_json::from_str(r#"{"language":"de"}"#).expect("update");
        let next = Settings::default().apply(&de).expect("valid");
        assert_eq!(next.language, Language::De);
        assert_eq!(serde_json::to_value(next).expect("json")["language"], "de");
        let unchanged = next.apply(&SettingsUpdate::default()).expect("valid");
        assert_eq!(unchanged.language, Language::De);
        let bad: Result<SettingsUpdate, _> = serde_json::from_str(r#"{"language":"xx"}"#);
        assert!(bad.is_err(), "an unknown language is refused");
    }

    #[test]
    fn keep_in_tray_defaults_on_for_old_config_files_and_can_be_turned_off() {
        // A config.json written before the field existed keeps the app in the tray.
        let old: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).expect("old config");
        assert!(old.keep_in_tray);
        let json = serde_json::to_value(Settings::default()).expect("json");
        assert_eq!(json["keepInTray"], true);
        let off: SettingsUpdate = serde_json::from_str(r#"{"keepInTray":false}"#).expect("update");
        let next = Settings::default().apply(&off).expect("valid");
        assert!(!next.keep_in_tray);
        let unchanged = next.apply(&SettingsUpdate::default()).expect("valid");
        assert!(!unchanged.keep_in_tray);
    }

    #[test]
    fn endpoint_info_is_camel_case_and_the_url_may_be_absent() {
        let info = EndpointInfo {
            url: Some("http://127.0.0.1:4242".into()),
            token: "ab".repeat(32),
        };
        let json = serde_json::to_value(&info).expect("json");
        assert_eq!(json["url"], "http://127.0.0.1:4242");
        assert_eq!(json["token"], "ab".repeat(32));
        let stopped: EndpointInfo = serde_json::from_str(r#"{"token":"00"}"#).expect("parse");
        assert_eq!(stopped.url, None);
    }

    #[test]
    fn the_mock_regenerates_a_new_64_digit_token() {
        let api = crate::mock::MockApi::new();
        let first = api.endpoint_info().expect("info");
        assert_eq!(first.token.len(), 64);
        assert!(first.token.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(
            api.endpoint_info().expect("info"),
            first,
            "stable until regenerated"
        );
        let next = api.regenerate_endpoint_token().expect("regenerate");
        assert_ne!(next.token, first.token);
        assert_eq!(next.token.len(), 64);
        assert_eq!(api.endpoint_info().expect("info"), next);
    }

    #[test]
    fn apply_changes_only_the_given_fields() {
        let next = Settings::default()
            .apply(&SettingsUpdate {
                theme: Some(Theme::Dark),
                cache_limit_mb: Some(CACHE_LIMIT_MB_MIN),
                rescan_interval_minutes: Some(0),
                ..SettingsUpdate::default()
            })
            .expect("valid");
        assert_eq!(next.theme, Theme::Dark);
        assert_eq!(next.cache_limit_mb, CACHE_LIMIT_MB_MIN);
        assert_eq!(next.rescan_interval_minutes, 0);
        assert_eq!(next.density, Density::Comfortable);
        assert_eq!(next.activation_mode, ActivationMode::Session);
    }

    #[test]
    fn apply_rejects_out_of_range_values() {
        let s = Settings::default();
        for update in [
            SettingsUpdate {
                cache_limit_mb: Some(CACHE_LIMIT_MB_MIN - 1),
                ..SettingsUpdate::default()
            },
            SettingsUpdate {
                cache_limit_mb: Some(CACHE_LIMIT_MB_MAX + 1),
                theme: Some(Theme::Dark),
                ..SettingsUpdate::default()
            },
            SettingsUpdate {
                rescan_interval_minutes: Some(RESCAN_INTERVAL_MAX_MINUTES + 1),
                ..SettingsUpdate::default()
            },
        ] {
            let err = s.apply(&update).expect_err("out of range");
            assert_eq!(err.code, ErrorCode::InvalidRequest);
        }
    }

    #[test]
    fn wire_format_is_camel_case_with_defaults_for_missing_keys() {
        let json = serde_json::to_value(Settings::default()).expect("json");
        assert_eq!(json["activationMode"], "session");
        assert_eq!(json["onConflict"], "keepExisting");
        assert_eq!(json["reactivateOnLaunch"], false);
        assert_eq!(json["cacheLimitMb"], 1_024);
        assert_eq!(json["rescanIntervalMinutes"], 60);
        let partial: Settings = serde_json::from_str(r#"{"density":"compact"}"#).expect("partial");
        assert_eq!(partial.density, Density::Compact);
        assert_eq!(partial.theme, Theme::System);
        let update: SettingsUpdate = serde_json::from_str(r#"{"theme":"light"}"#).expect("update");
        assert_eq!(update.theme, Some(Theme::Light));
        assert_eq!(update.density, None);
        let on: SettingsUpdate =
            serde_json::from_str(r#"{"reactivateOnLaunch":true}"#).expect("update");
        let next = Settings::default().apply(&on).expect("valid");
        assert!(next.reactivate_on_launch);
        assert_eq!(next.theme, Theme::System);
    }
}
