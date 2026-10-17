//! The user's settings, stored as `config.json` in the platform app-data
//! folder ([`fm_platform::AppPaths::config`]).
//!
//! Unknown keys are ignored and missing keys take their defaults, so an older
//! or newer app can read the file. A file that is not valid JSON is reported,
//! never overwritten: the user may have edited it by hand.

use std::fs;
use std::io;
use std::path::Path;

use fm_api::settings::Settings;
use fm_api::{ApiError, ErrorCode};
use serde::{Deserialize, Serialize};

/// The app's settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    /// The least severe level written to the log file.
    pub log_level: LogLevel,
    /// Anonymous usage statistics. Off unless the user turns it on (rule 7).
    pub telemetry: bool,
    /// What the Settings window edits, stored at the top level of the file.
    #[serde(flatten)]
    pub settings: Settings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            log_level: LogLevel::Info,
            telemetry: false,
            settings: Settings::default(),
        }
    }
}

/// How much the log file records, from least to most.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Failures only.
    Error,
    /// Failures and things that look wrong.
    Warn,
    /// Normal operation: start, scans, activations.
    Info,
    /// Detail for a bug report.
    Debug,
    /// Everything.
    Trace,
}

impl Config {
    /// Reads the config file. A missing file gives the defaults.
    pub fn load(path: &Path) -> Result<Self, ApiError> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(io_error("The settings file could not be read.", path, &e)),
        };
        serde_json::from_str(&text).map_err(|e| {
            ApiError::new(
                ErrorCode::InvalidRequest,
                "The settings file is not valid. Fix or remove it to use the defaults.",
            )
            .with_detail(format!("{}: {e}", path.display()))
        })
    }

    /// Reads the config file and writes the defaults when it does not exist
    /// yet, so the user finds the file in the app-data folder.
    pub fn load_or_create(path: &Path) -> Result<Self, ApiError> {
        if path.exists() {
            return Self::load(path);
        }
        let config = Self::default();
        config.save(path)?;
        Ok(config)
    }

    /// Writes the config file, creating its folder. The write goes to a
    /// temporary file first, so a crash never leaves a half-written file.
    pub fn save(&self, path: &Path) -> Result<(), ApiError> {
        let failed = "The settings could not be saved.";
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| io_error(failed, dir, &e))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| ApiError::new(ErrorCode::Internal, failed).with_detail(e.to_string()))?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json + "\n").map_err(|e| io_error(failed, &tmp, &e))?;
        fs::rename(&tmp, path).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            io_error(failed, path, &e)
        })
    }
}

fn io_error(message: &str, path: &Path, e: &io::Error) -> ApiError {
    ApiError::new(ErrorCode::Io, message).with_detail(format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn missing_file_gives_defaults_with_telemetry_off() {
        let dir = TempDir::new("config-missing");
        let config = Config::load(&dir.path().join("config.json")).expect("defaults");
        assert_eq!(config, Config::default());
        assert!(!config.telemetry);
        assert_eq!(config.log_level, LogLevel::Info);
    }

    #[test]
    fn save_then_load_round_trips_and_creates_the_folder() {
        let dir = TempDir::new("config-roundtrip");
        let path = dir.path().join("nested").join("config.json");
        let config = Config {
            log_level: LogLevel::Debug,
            telemetry: true,
            settings: Settings {
                cache_limit_mb: 256,
                ..Settings::default()
            },
        };
        config.save(&path).expect("saved");
        assert_eq!(Config::load(&path).expect("loaded"), config);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn load_or_create_writes_the_defaults_once() {
        let dir = TempDir::new("config-create");
        let path = dir.path().join("config.json");
        assert_eq!(
            Config::load_or_create(&path).expect("created"),
            Config::default()
        );
        assert!(path.is_file());
        fs::write(&path, r#"{"telemetry": true}"#).expect("edit");
        assert!(Config::load_or_create(&path).expect("kept").telemetry);
    }

    #[test]
    fn unknown_and_missing_keys_are_tolerated() {
        let dir = TempDir::new("config-partial");
        let path = dir.path().join("config.json");
        fs::write(&path, r#"{"logLevel": "warn", "fromTheFuture": 1}"#).expect("write");
        let config = Config::load(&path).expect("loaded");
        assert_eq!(config.log_level, LogLevel::Warn);
        assert!(!config.telemetry);
        assert_eq!(config.settings, Settings::default());
    }

    #[test]
    fn settings_sit_at_the_top_level_of_the_file() {
        let dir = TempDir::new("config-flat");
        let path = dir.path().join("config.json");
        fs::write(
            &path,
            r#"{"theme": "dark", "cacheLimitMb": 128, "telemetry": true}"#,
        )
        .expect("write");
        let config = Config::load(&path).expect("loaded");
        assert_eq!(config.settings.theme, fm_api::settings::Theme::Dark);
        assert_eq!(config.settings.cache_limit_mb, 128);
        assert!(config.telemetry);
        config.save(&path).expect("saved");
        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("json");
        assert_eq!(json["theme"], "dark");
        assert_eq!(json["rescanIntervalMinutes"], 60);
        assert!(json.get("settings").is_none());
    }

    #[test]
    fn malformed_file_is_an_error_and_is_left_alone() {
        let dir = TempDir::new("config-malformed");
        let path = dir.path().join("config.json");
        fs::write(&path, "{ not json").expect("write");
        let err = Config::load_or_create(&path).expect_err("malformed");
        assert_eq!(err.code, ErrorCode::InvalidRequest);
        assert!(err.detail.as_deref().unwrap_or("").contains("config.json"));
        assert_eq!(fs::read_to_string(&path).expect("read"), "{ not json");
    }

    #[test]
    fn unwritable_folder_is_an_io_error() {
        let dir = TempDir::new("config-blocked");
        let blocker = dir.path().join("not-a-folder");
        fs::write(&blocker, "").expect("write");
        let err = Config::default()
            .save(&blocker.join("config.json"))
            .expect_err("parent is a file");
        assert_eq!(err.code, ErrorCode::Io);
    }

    #[test]
    fn non_ascii_paths_work() {
        let dir = TempDir::new("config-ünïcødé-字体");
        let path = dir.path().join("Ünïcødé 字体").join("config.json");
        Config::default().save(&path).expect("saved");
        assert_eq!(Config::load(&path).expect("loaded"), Config::default());
    }
}
