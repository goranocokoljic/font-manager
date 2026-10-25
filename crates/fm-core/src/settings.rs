//! The settings service over `config.json` ([`crate::config::Config`]).
//!
//! Every call reads the file again, so a hand edit made while the app runs is
//! seen and never overwritten by an older copy; a file that is not valid JSON
//! is reported and left alone (ADR 0007).
//!
//! "Start at login" is not in the file: its state is the OS's login entry
//! ([`fm_platform::Autostart`], ADR 0011), read on every request.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use fm_api::settings::{AppFolders, Settings, SettingsUpdate};
use fm_api::{ApiError, ApiResult, ErrorCode, SettingsService};
use fm_platform::{AppPaths, Autostart};

use crate::config::Config;

/// Reads and writes the settings in the config file.
#[derive(Debug)]
pub struct SettingsStore {
    file: PathBuf,
    folders: AppFolders,
    /// The login entry behind "Start at login"; `None` when not offered.
    autostart: Option<Arc<dyn Autostart>>,
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
            autostart: None,
            lock: Mutex::new(()),
        }
    }

    /// This store with "Start at login" backed by `autostart`.
    pub fn with_autostart(mut self, autostart: Arc<dyn Autostart>) -> Self {
        self.autostart = Some(autostart);
        self
    }

    /// This store with "Start at login" backed by the OS's login entry
    /// (Windows, macOS and Linux; hidden where the OS has none, as in a
    /// Flatpak sandbox).
    pub fn with_system_autostart(self) -> Self {
        self.with_autostart(fm_platform::autostart::system())
    }

    /// The login state as the OS has it; `None` when the option is not
    /// offered or the OS cannot say (logged, and the switch is hidden).
    fn start_at_login(&self) -> Option<bool> {
        let autostart = self.autostart.as_ref()?;
        match autostart.is_enabled() {
            Ok(on) => Some(on),
            Err(e) if e.kind() == std::io::ErrorKind::Unsupported => {
                tracing::debug!(error = %e, "start at login is not offered here");
                None
            }
            Err(e) => {
                tracing::warn!(error = %e, "the login entry could not be read");
                None
            }
        }
    }

    /// Registers or removes the login entry.
    fn set_start_at_login(&self, on: bool) -> ApiResult<()> {
        let Some(autostart) = &self.autostart else {
            return Ok(());
        };
        let result = if on {
            autostart.enable()
        } else {
            autostart.disable().map(|_| ())
        };
        result.map_err(|e| {
            let message = if on {
                "Start at login could not be turned on: the system refused the login entry."
            } else {
                "Start at login could not be turned off: the system refused to remove the login entry."
            };
            let code = match e.kind() {
                std::io::ErrorKind::PermissionDenied => ErrorCode::PermissionDenied,
                std::io::ErrorKind::Unsupported => ErrorCode::Unsupported,
                _ => ErrorCode::Io,
            };
            ApiError::new(code, message).with_detail(e.to_string())
        })
    }
}

impl SettingsStore {
    /// Whether the user turned the MCP server on (E27.1); read from the file
    /// on every call, like every setting.
    pub fn mcp_enabled(&self) -> ApiResult<bool> {
        Ok(Config::load(&self.file)?.mcp_enabled)
    }

    /// Turns the MCP server on or off and saves the choice; the other keys
    /// of the file are kept.
    pub fn set_mcp_enabled(&self, on: bool) -> ApiResult<()> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let mut config = Config::load(&self.file)?;
        if config.mcp_enabled != on {
            config.mcp_enabled = on;
            config.save(&self.file)?;
        }
        Ok(())
    }
}

impl SettingsService for SettingsStore {
    fn get_settings(&self) -> ApiResult<Settings> {
        Ok(Settings {
            start_at_login: self.start_at_login(),
            ..Config::load(&self.file)?.settings
        })
    }

    fn update_settings(&self, req: SettingsUpdate) -> ApiResult<Settings> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let mut config = Config::load(&self.file)?;
        let current = Settings {
            start_at_login: self.start_at_login(),
            ..config.settings
        };
        // Every value is checked before the OS or the file changes.
        let next = current.apply(&req)?;
        if let Some(on) = req.start_at_login
            && current.start_at_login != Some(on)
        {
            self.set_start_at_login(on)?;
        }
        config.settings = Settings {
            start_at_login: None,
            ..next
        };
        config.save(&self.file)?;
        Ok(Settings {
            start_at_login: self.start_at_login(),
            ..config.settings
        })
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
    use fm_platform::MockAutostart;

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
    fn the_mcp_switch_is_off_by_default_and_saved_with_the_other_keys() {
        let dir = TempDir::new("settings-mcp");
        let paths = paths_in(&dir);
        let store = SettingsStore::new(&paths);
        assert!(!store.mcp_enabled().expect("read"), "off by default");

        store
            .update_settings(SettingsUpdate {
                theme: Some(Theme::Dark),
                ..SettingsUpdate::default()
            })
            .expect("updated");
        store.set_mcp_enabled(true).expect("on");
        assert!(store.mcp_enabled().expect("read"));
        // A second store (the next app start) sees it, and the theme stays.
        let next = SettingsStore::new(&paths);
        assert!(next.mcp_enabled().expect("read"));
        assert_eq!(next.get_settings().expect("read").theme, Theme::Dark);
        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(paths.config_file()).expect("file"))
                .expect("json");
        assert_eq!(json["mcpEnabled"], true);

        // A settings update keeps the switch.
        next.update_settings(SettingsUpdate {
            density: Some(Density::Compact),
            ..SettingsUpdate::default()
        })
        .expect("updated");
        assert!(next.mcp_enabled().expect("read"));

        next.set_mcp_enabled(false).expect("off");
        assert!(!store.mcp_enabled().expect("read"));
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

    fn offered(dir: &TempDir) -> (SettingsStore, Arc<MockAutostart>, AppPaths) {
        let paths = paths_in(dir);
        let entry = Arc::new(MockAutostart::new());
        let store =
            SettingsStore::new(&paths).with_autostart(Arc::clone(&entry) as Arc<dyn Autostart>);
        (store, entry, paths)
    }

    #[test]
    fn start_at_login_is_offered_on_every_os_and_hidden_without_an_entry() {
        let dir = TempDir::new("settings-login-gate");
        // Rule 5: Windows, macOS and Linux offer it together (E17.3); reading
        // the entry never changes it.
        let system = SettingsStore::new(&paths_in(&dir)).with_system_autostart();
        let read = system.get_settings().expect("read").start_at_login;
        assert_eq!(
            read.is_some(),
            !fm_platform::autostart::sandboxed(),
            "{read:?}"
        );

        let store = SettingsStore::new(&paths_in(&dir));
        assert_eq!(store.get_settings().expect("read").start_at_login, None);
        let err = store
            .update_settings(SettingsUpdate {
                start_at_login: Some(true),
                ..SettingsUpdate::default()
            })
            .expect_err("not offered");
        assert_eq!(err.code, ErrorCode::Unsupported);

        let (store, _, _) = offered(&dir);
        assert_eq!(
            store.get_settings().expect("read").start_at_login,
            Some(false)
        );
    }

    #[test]
    fn start_at_login_follows_the_os_entry_and_is_never_saved() {
        let dir = TempDir::new("settings-login-os");
        let (store, entry, paths) = offered(&dir);
        let on = store
            .update_settings(SettingsUpdate {
                start_at_login: Some(true),
                ..SettingsUpdate::default()
            })
            .expect("on");
        assert_eq!(on.start_at_login, Some(true));
        assert!(entry.is_enabled().expect("entry"));
        let file = fs::read_to_string(paths.config_file()).expect("saved");
        assert!(!file.contains("startAtLogin"), "{file}");

        // The OS is the truth: an entry removed outside the app reads as off,
        // and a stale value in a hand-edited file is ignored.
        entry.disable().expect("removed elsewhere");
        fs::write(paths.config_file(), r#"{"startAtLogin": true}"#).expect("hand edit");
        assert_eq!(
            store.get_settings().expect("read").start_at_login,
            Some(false)
        );

        entry.enable().expect("on");
        let off = store
            .update_settings(SettingsUpdate {
                start_at_login: Some(false),
                ..SettingsUpdate::default()
            })
            .expect("off");
        assert_eq!(off.start_at_login, Some(false));
        assert!(!entry.is_enabled().expect("entry"));
    }

    #[test]
    fn a_refused_login_entry_is_an_error_and_saves_nothing() {
        let dir = TempDir::new("settings-login-refused");
        let (store, entry, paths) = offered(&dir);
        entry.refuse("blocked by policy");
        let err = store
            .update_settings(SettingsUpdate {
                start_at_login: Some(true),
                theme: Some(Theme::Dark),
                ..SettingsUpdate::default()
            })
            .expect_err("refused");
        assert_eq!(err.code, ErrorCode::PermissionDenied);
        assert!(
            err.message
                .contains("Start at login could not be turned on")
        );
        assert_eq!(err.detail.as_deref(), Some("blocked by policy"));
        assert!(!paths.config_file().exists());
        let now = store.get_settings().expect("read");
        assert_eq!(now.start_at_login, Some(false));
        assert_eq!(now.theme, Theme::System);
    }
}
