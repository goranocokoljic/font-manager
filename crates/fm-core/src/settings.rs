//! The settings service over `config.json` ([`crate::config::Config`]).
//!
//! Every call reads the file again, so a hand edit made while the app runs is
//! seen and never overwritten by an older copy; a file that is not valid JSON
//! is reported and left alone (ADR 0007).

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use fm_api::settings::{AppFolders, Settings, SettingsUpdate};
use fm_api::{ApiResult, SettingsService};
use fm_platform::AppPaths;

use crate::config::Config;

/// Reads and writes the settings in the config file.
#[derive(Debug)]
pub struct SettingsStore {
    file: PathBuf,
    folders: AppFolders,
    /// Serializes read-change-write, so two updates never lose one another.
    lock: Mutex<()>,
}

impl SettingsStore {
    /// The store for these app folders.
    pub fn new(paths: &AppPaths) -> Self {
        let show = |p: &std::path::Path| p.display().to_string();
        Self {
            file: paths.config_file(),
            folders: AppFolders {
                config: show(&paths.config),
                data: show(&paths.data),
                cache: show(&paths.cache),
                logs: show(&paths.logs),
            },
            lock: Mutex::new(()),
        }
    }
}

impl SettingsService for SettingsStore {
    fn get_settings(&self) -> ApiResult<Settings> {
        Ok(Config::load(&self.file)?.settings)
    }

    fn update_settings(&self, req: SettingsUpdate) -> ApiResult<Settings> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let mut config = Config::load(&self.file)?;
        config.settings = config.settings.apply(&req)?;
        config.save(&self.file)?;
        Ok(config.settings)
    }

    fn app_folders(&self) -> ApiResult<AppFolders> {
        Ok(self.folders.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use fm_api::ErrorCode;
    use fm_api::activation::ActivationMode;
    use fm_api::settings::{Density, Theme};

    use super::*;
    use crate::config::LogLevel;
    use crate::test_support::TempDir;

    fn paths_in(dir: &TempDir) -> AppPaths {
        let root = dir.path();
        AppPaths {
            data: root.join("data"),
            config: root.join("config"),
            cache: root.join("cache"),
            logs: root.join("logs"),
            library: root.join("Fonts"),
        }
    }

    #[test]
    fn a_missing_file_gives_the_defaults() {
        let dir = TempDir::new("settings-missing");
        let store = SettingsStore::new(&paths_in(&dir));
        assert_eq!(store.get_settings().expect("defaults"), Settings::default());
    }

    #[test]
    fn an_update_is_saved_and_keeps_the_other_keys() {
        let dir = TempDir::new("settings-update");
        let paths = paths_in(&dir);
        fs::create_dir_all(&paths.config).expect("dir");
        fs::write(
            paths.config_file(),
            r#"{"logLevel": "debug", "telemetry": true, "density": "compact"}"#,
        )
        .expect("seed");
        let store = SettingsStore::new(&paths);
        let next = store
            .update_settings(SettingsUpdate {
                theme: Some(Theme::Light),
                activation_mode: Some(ActivationMode::Permanent),
                ..SettingsUpdate::default()
            })
            .expect("updated");
        assert_eq!(next.theme, Theme::Light);
        assert_eq!(next.density, Density::Compact);

        let on_disk = Config::load(&paths.config_file()).expect("loaded");
        assert_eq!(on_disk.settings, next);
        assert_eq!(on_disk.log_level, LogLevel::Debug);
        assert!(on_disk.telemetry);
        // A second store (the next app start) sees the change.
        assert_eq!(
            SettingsStore::new(&paths).get_settings().expect("read"),
            next
        );
    }

    #[test]
    fn a_hand_edit_made_while_running_is_not_lost() {
        let dir = TempDir::new("settings-hand-edit");
        let paths = paths_in(&dir);
        let store = SettingsStore::new(&paths);
        store
            .update_settings(SettingsUpdate {
                theme: Some(Theme::Dark),
                ..SettingsUpdate::default()
            })
            .expect("first");
        fs::write(
            paths.config_file(),
            r#"{"theme": "dark", "telemetry": true}"#,
        )
        .expect("edit");
        store
            .update_settings(SettingsUpdate {
                cache_limit_mb: Some(512),
                ..SettingsUpdate::default()
            })
            .expect("second");
        let on_disk = Config::load(&paths.config_file()).expect("loaded");
        assert!(on_disk.telemetry);
        assert_eq!(on_disk.settings.cache_limit_mb, 512);
    }

    #[test]
    fn a_rejected_value_writes_nothing() {
        let dir = TempDir::new("settings-rejected");
        let paths = paths_in(&dir);
        let store = SettingsStore::new(&paths);
        let err = store
            .update_settings(SettingsUpdate {
                rescan_interval_minutes: Some(100_000),
                ..SettingsUpdate::default()
            })
            .expect_err("out of range");
        assert_eq!(err.code, ErrorCode::InvalidRequest);
        assert!(!paths.config_file().exists());
    }

    #[test]
    fn a_malformed_file_is_reported_and_left_alone() {
        let dir = TempDir::new("settings-malformed");
        let paths = paths_in(&dir);
        fs::create_dir_all(&paths.config).expect("dir");
        fs::write(paths.config_file(), "{ not json").expect("seed");
        let store = SettingsStore::new(&paths);
        assert_eq!(
            store.get_settings().expect_err("malformed").code,
            ErrorCode::InvalidRequest
        );
        let err = store
            .update_settings(SettingsUpdate {
                theme: Some(Theme::Dark),
                ..SettingsUpdate::default()
            })
            .expect_err("malformed");
        assert_eq!(err.code, ErrorCode::InvalidRequest);
        assert_eq!(
            fs::read_to_string(paths.config_file()).expect("read"),
            "{ not json"
        );
    }

    #[test]
    fn app_folders_are_the_resolved_paths_including_non_ascii() {
        let dir = TempDir::new("settings-folders-ünïcødé-字体");
        let paths = paths_in(&dir);
        let folders = SettingsStore::new(&paths).app_folders().expect("folders");
        assert_eq!(folders.logs, paths.logs.display().to_string());
        assert_eq!(folders.config, paths.config.display().to_string());
        assert!(folders.logs.contains("字体"));
    }
}
