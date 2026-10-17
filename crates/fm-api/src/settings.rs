//! Settings service: the user's preferences (`config.json`) and the app folders.

use serde::{Deserialize, Serialize};

use crate::activation::{ActivationMode, ConflictPolicy};
use crate::{ApiError, ApiResult};

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
    /// Light, dark or the OS appearance.
    pub theme: Theme,
    /// Comfortable or compact lists.
    pub density: Density,
    /// The largest size of the preview cache, in megabytes (64–16384).
    pub cache_limit_mb: u32,
    /// Minutes between automatic rescans of the sources; 0 turns them off
    /// (at most 1440, one day).
    pub rescan_interval_minutes: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            activation_mode: ActivationMode::Session,
            on_conflict: ConflictPolicy::KeepExisting,
            theme: Theme::System,
            density: Density::Comfortable,
            cache_limit_mb: 1_024,
            rescan_interval_minutes: 60,
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
        Ok(Self {
            activation_mode: update.activation_mode.unwrap_or(self.activation_mode),
            on_conflict: update.on_conflict.unwrap_or(self.on_conflict),
            theme: update.theme.unwrap_or(self.theme),
            density: update.density.unwrap_or(self.density),
            cache_limit_mb: update.cache_limit_mb.unwrap_or(self.cache_limit_mb),
            rescan_interval_minutes: update
                .rescan_interval_minutes
                .unwrap_or(self.rescan_interval_minutes),
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

/// The user's settings and the app folders.
pub trait SettingsService {
    /// The current settings.
    fn get_settings(&self) -> ApiResult<Settings>;
    /// Changes some settings and saves them; returns all of them.
    fn update_settings(&self, req: SettingsUpdate) -> ApiResult<Settings>;
    /// Where the app keeps its config, data, cache and logs.
    fn app_folders(&self) -> ApiResult<AppFolders>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCode;

    #[test]
    fn defaults_are_session_keep_existing_system_comfortable() {
        let s = Settings::default();
        assert_eq!(s.activation_mode, ActivationMode::Session);
        assert_eq!(s.on_conflict, ConflictPolicy::KeepExisting);
        assert_eq!(s.theme, Theme::System);
        assert_eq!(s.density, Density::Comfortable);
        assert_eq!(s.cache_limit_mb, 1_024);
        assert_eq!(s.rescan_interval_minutes, 60);
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
        assert_eq!(json["cacheLimitMb"], 1_024);
        assert_eq!(json["rescanIntervalMinutes"], 60);
        let partial: Settings = serde_json::from_str(r#"{"density":"compact"}"#).expect("partial");
        assert_eq!(partial.density, Density::Compact);
        assert_eq!(partial.theme, Theme::System);
        let update: SettingsUpdate = serde_json::from_str(r#"{"theme":"light"}"#).expect("update");
        assert_eq!(update.theme, Some(Theme::Light));
        assert_eq!(update.density, None);
    }
}
