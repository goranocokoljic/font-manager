//! "Install updates when I quit" (E55.9, ADR 0012): the update an automatic
//! check downloaded in the background, kept in the app's cache folder until
//! the user quits, and what the next start says about the install.
//!
//! The shell downloads and installs through Tauri's updater; this file keeps
//! the files those steps leave between them, so each step reads the same:
//!
//! - `<cache>/updates/update.pkg` and `update.json`: the package of one
//!   version, written only after its signature was checked (the shell checks
//!   it again before the install). A newer package replaces it.
//! - `<data>/update-install.json`: written just before the install starts at
//!   quit. The next start reads it once ([`QuitInstall::resolve_at_start`]):
//!   the version that starts says whether the install worked.
//! - `<data>/update-notice.json`: what the UI says once
//!   ([`QuitInstall::take_notice`]).
//!
//! Every file here is the app's own; none is a user file (rule 2).

use std::fs;
use std::path::{Path, PathBuf};

use fm_api::settings::Settings;
use fm_api::update::{AvailableUpdate, UpdateNotice, UpdateSupport, installs_on_quit};
use fm_platform::AppPaths;
use serde::{Deserialize, Serialize};

use crate::{FmError, FmResult};

/// What an update an automatic check found asks for while the app runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoundStep {
    /// Nothing: the setting is off, the copy does not update itself, the
    /// update is major (only announced), or its package is kept already.
    Nothing,
    /// Download the package in the background and keep it for the quit.
    Download,
}

/// What an update `found` by an automatic check asks for: a download when
/// "Install updates when I quit" applies ([`installs_on_quit`] with
/// `settings` and `support`, the folder probe just made) and `found` is a
/// patch or minor update whose package is not kept already (`staged`, the
/// version kept).
pub fn found_step(
    settings: &Settings,
    support: &UpdateSupport,
    found: &AvailableUpdate,
    staged: Option<&str>,
) -> FoundStep {
    if installs_on_quit(settings, support)
        && found.installs_on_quit()
        && staged != Some(found.version.as_str())
    {
        FoundStep::Download
    } else {
        FoundStep::Nothing
    }
}

/// What the app does with a kept package as it quits, after the quit path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitStep {
    /// Nothing now: no package is kept, or the quit was not the user's (a
    /// logout, a shutdown, the restart after a confirmed install). A kept
    /// package waits for the next quit.
    Keep,
    /// Delete the kept package: the setting was turned off, the copy no
    /// longer updates itself (the folder probe failed), or the update on
    /// offer is another or a major one.
    Discard,
    /// Install the kept package of this version.
    Install(String),
}

/// What the quit does with the package of version `staged`: installs it
/// only when the user quit the app (`user_quit`: Quit in the menu or the
/// tray, or the last window closed), "Install updates when I quit" still
/// applies (`settings`, `support` with the folder probe made now), and the
/// update on offer (`pending`, the last check's) is that version and a patch
/// or minor one.
pub fn exit_step(
    user_quit: bool,
    settings: &Settings,
    support: &UpdateSupport,
    pending: Option<&AvailableUpdate>,
    staged: Option<&str>,
) -> ExitStep {
    let Some(staged) = staged else {
        return ExitStep::Keep;
    };
    if !user_quit {
        return ExitStep::Keep;
    }
    match pending {
        Some(p)
            if installs_on_quit(settings, support)
                && p.installs_on_quit()
                && p.version == staged =>
        {
            ExitStep::Install(staged.to_owned())
        }
        _ => ExitStep::Discard,
    }
}

/// The version downloaded and waiting to install when the user quits, as
/// Settings → About shows it (E55.10): the package of `staged` would install
/// at the user's next quit ([`exit_step`] with the same `settings`,
/// `support` and `pending`). `None` when nothing waits or the kept package
/// no longer applies.
pub fn waiting_version(
    settings: &Settings,
    support: &UpdateSupport,
    pending: Option<&AvailableUpdate>,
    staged: Option<&str>,
) -> Option<String> {
    match exit_step(true, settings, support, pending, staged) {
        ExitStep::Install(version) => Some(version),
        ExitStep::Keep | ExitStep::Discard => None,
    }
}

const PACKAGE: &str = "update.pkg";
const PACKAGE_INFO: &str = "update.json";
const INSTALL_MARKER: &str = "update-install.json";
const NOTICE: &str = "update-notice.json";

/// Which version a staged package is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageInfo {
    version: String,
}

/// An install at quit that started: the version that ran and the one it
/// installs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallMarker {
    from_version: String,
    version: String,
    #[serde(default)]
    notes_url: Option<String>,
}

/// The files of "Install updates when I quit" in the app folders.
#[derive(Debug, Clone)]
pub struct QuitInstall {
    /// `<cache>/updates`: the downloaded package.
    packages: PathBuf,
    /// `<data>`: the install marker and the notice.
    data: PathBuf,
}

impl QuitInstall {
    /// The files under the app folders `paths`.
    pub fn new(paths: &AppPaths) -> Self {
        Self {
            packages: paths.cache.join("updates"),
            data: paths.data.clone(),
        }
    }

    /// The folder the downloaded package is kept in.
    pub fn package_dir(&self) -> &Path {
        &self.packages
    }

    /// Keeps `bytes`, the verified package of `version`, for the install at
    /// quit; it replaces any package kept before.
    pub fn stage(&self, version: &str, bytes: &[u8]) -> FmResult<()> {
        let failed = "The downloaded update could not be saved.";
        self.discard();
        fs::create_dir_all(&self.packages).map_err(|e| FmError::io(failed, &self.packages, &e))?;
        write_atomic(&self.packages.join(PACKAGE), bytes, failed)?;
        let info = serde_json::to_vec(&PackageInfo {
            version: version.to_owned(),
        })
        .map_err(|e| FmError::internal(e.to_string()))?;
        write_atomic(&self.packages.join(PACKAGE_INFO), &info, failed)
    }

    /// The version of the package kept for the install at quit.
    pub fn staged_version(&self) -> Option<String> {
        let info = fs::read(self.packages.join(PACKAGE_INFO)).ok()?;
        serde_json::from_slice::<PackageInfo>(&info)
            .ok()
            .map(|i| i.version)
    }

    /// The bytes of the package kept for `version`; `None` when none is kept,
    /// or it is of another version.
    pub fn staged(&self, version: &str) -> Option<Vec<u8>> {
        if self.staged_version().as_deref() != Some(version) {
            return None;
        }
        fs::read(self.packages.join(PACKAGE)).ok()
    }

    /// Deletes the kept package: refused, installed, out of date, or the
    /// setting was turned off.
    pub fn discard(&self) {
        for name in [PACKAGE_INFO, PACKAGE] {
            let path = self.packages.join(name);
            if let Err(e) = fs::remove_file(&path)
                && e.kind() != std::io::ErrorKind::NotFound
            {
                tracing::warn!(error = %e, path = %path.display(), "update package not deleted");
            }
        }
    }

    /// Records, just before the installer runs, that `from_version`
    /// installs `version`; the next start reads it.
    pub fn begin_install(
        &self,
        from_version: &str,
        version: &str,
        notes_url: Option<String>,
    ) -> FmResult<()> {
        let failed = "The update could not be prepared.";
        fs::create_dir_all(&self.data).map_err(|e| FmError::io(failed, &self.data, &e))?;
        let marker = serde_json::to_vec(&InstallMarker {
            from_version: from_version.to_owned(),
            version: version.to_owned(),
            notes_url,
        })
        .map_err(|e| FmError::internal(e.to_string()))?;
        write_atomic(&self.data.join(INSTALL_MARKER), &marker, failed)
    }

    /// At start, once: when the last quit began an install, what this start
    /// says about it (`running` is the version that started). The marker
    /// and the package are deleted and the notice is kept for
    /// [`Self::take_notice`]. `None` when the last quit installed nothing.
    pub fn resolve_at_start(&self, running: &str) -> Option<UpdateNotice> {
        // A package no newer than this version is of no use any more (the
        // user installed the update another way).
        if let Some(kept) = self.staged_version()
            && !matches!(fm_api::update::update_kind(running, &kept), Ok(Some(_)))
        {
            self.discard();
        }
        let path = self.data.join(INSTALL_MARKER);
        let read = fs::read(&path).ok()?;
        remove(&path);
        self.discard();
        let marker = match serde_json::from_slice::<InstallMarker>(&read) {
            Ok(marker) => marker,
            Err(e) => {
                tracing::warn!(error = %e, "the record of the last update install is unreadable");
                return None;
            }
        };
        let notice = match UpdateNotice::after_install(&marker.version, running, marker.notes_url) {
            Ok(notice) => notice,
            Err(e) => {
                tracing::warn!(error = %e, "the record of the last update install is unreadable");
                return None;
            }
        };
        match &notice {
            UpdateNotice::Updated { version, .. } => {
                tracing::info!(from = %marker.from_version, %version, "updated when the app quit");
            }
            UpdateNotice::NotInstalled { version, .. } => {
                tracing::warn!(%version, %running, "the update at the last quit did not install");
            }
        }
        self.record_notice(&notice);
        Some(notice)
    }

    /// Keeps `notice` for the UI to show once; it replaces an older one.
    pub fn record_notice(&self, notice: &UpdateNotice) {
        let written = serde_json::to_vec(notice)
            .map_err(|e| FmError::internal(e.to_string()))
            .and_then(|json| {
                fs::create_dir_all(&self.data).map_err(|e| {
                    FmError::io("The update notice could not be saved.", &self.data, &e)
                })?;
                write_atomic(
                    &self.data.join(NOTICE),
                    &json,
                    "The update notice could not be saved.",
                )
            });
        if let Err(e) = written {
            tracing::warn!(error = %e, "the update notice was not saved");
        }
    }

    /// The notice to show, once: it is deleted as it is read.
    pub fn take_notice(&self) -> Option<UpdateNotice> {
        let path = self.data.join(NOTICE);
        let read = fs::read(&path).ok()?;
        remove(&path);
        serde_json::from_slice(&read).ok()
    }
}

fn remove(path: &Path) {
    if let Err(e) = fs::remove_file(path)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(error = %e, path = %path.display(), "not deleted");
    }
}

/// Writes `data` through a temporary file, so a crash never leaves half a
/// package or record in place.
pub(super) fn write_atomic(path: &Path, data: &[u8], failed: &str) -> FmResult<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, data).map_err(|e| FmError::io(failed, &tmp, &e))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        FmError::io(failed, path, &e)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Home {
        root: PathBuf,
        store: QuitInstall,
    }

    impl Home {
        fn new(label: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("fm-on-quit-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            let store = QuitInstall::new(&AppPaths::under(&root));
            Self { root, store }
        }
    }

    impl Drop for Home {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn a_staged_package_is_kept_for_its_version_only() {
        let home = Home::new("stage");
        let s = &home.store;
        assert_eq!(s.staged_version(), None);
        s.stage("1.5.0", b"package").expect("staged");
        assert!(s.package_dir().starts_with(&home.root));
        assert_eq!(s.staged_version().as_deref(), Some("1.5.0"));
        assert_eq!(s.staged("1.5.0").as_deref(), Some(&b"package"[..]));
        assert_eq!(s.staged("1.5.1"), None);
        // A newer download replaces it.
        s.stage("1.5.1", b"newer").expect("staged");
        assert_eq!(s.staged("1.5.0"), None);
        assert_eq!(s.staged("1.5.1").as_deref(), Some(&b"newer"[..]));
        s.discard();
        assert_eq!(s.staged_version(), None);
        assert!(!s.package_dir().join(PACKAGE).exists());
        // Discarding nothing is fine.
        s.discard();
    }

    #[test]
    fn the_start_after_an_install_that_worked_says_updated_once() {
        let home = Home::new("updated");
        let s = &home.store;
        s.stage("1.5.0", b"package").expect("staged");
        s.begin_install(
            "1.4.0",
            "1.5.0",
            Some("https://example.invalid/v1.5.0".into()),
        )
        .expect("marker");
        let notice = s.resolve_at_start("1.5.0").expect("a notice");
        assert_eq!(
            notice,
            UpdateNotice::Updated {
                version: "1.5.0".into(),
                notes_url: Some("https://example.invalid/v1.5.0".into())
            }
        );
        assert_eq!(s.staged_version(), None, "the installed package is deleted");
        assert_eq!(s.resolve_at_start("1.5.0"), None, "read once");
        assert_eq!(s.take_notice(), Some(notice));
        assert_eq!(s.take_notice(), None, "shown once");
    }

    #[test]
    fn the_start_after_a_failed_install_says_so_once() {
        let home = Home::new("failed");
        let s = &home.store;
        s.stage("1.5.0", b"package").expect("staged");
        s.begin_install("1.4.0", "1.5.0", None).expect("marker");
        // The earlier version starts again.
        let Some(UpdateNotice::NotInstalled { version, message }) = s.resolve_at_start("1.4.0")
        else {
            panic!("not a failure");
        };
        assert_eq!(version, "1.5.0");
        assert!(message.contains("1.4.0"));
        assert_eq!(s.staged_version(), None);
        assert!(matches!(
            s.take_notice(),
            Some(UpdateNotice::NotInstalled { .. })
        ));
        assert_eq!(s.take_notice(), None);
    }

    #[test]
    fn a_start_without_an_install_says_nothing() {
        let home = Home::new("nothing");
        assert_eq!(home.store.resolve_at_start("1.4.0"), None);
        assert_eq!(home.store.take_notice(), None);
    }

    #[test]
    fn an_unreadable_install_record_is_dropped_without_a_notice() {
        let home = Home::new("garbled");
        fs::create_dir_all(&home.store.data).expect("data");
        fs::write(home.store.data.join(INSTALL_MARKER), b"\xff not json").expect("write");
        assert_eq!(home.store.resolve_at_start("1.4.0"), None);
        assert!(!home.store.data.join(INSTALL_MARKER).exists());
    }

    #[test]
    fn a_package_kept_for_an_older_or_the_running_version_is_deleted_at_start() {
        let home = Home::new("stale");
        home.store.stage("1.5.0", b"package").expect("staged");
        assert_eq!(home.store.resolve_at_start("1.4.0"), None);
        assert_eq!(
            home.store.staged_version().as_deref(),
            Some("1.5.0"),
            "still newer"
        );
        home.store.resolve_at_start("1.5.0");
        assert_eq!(home.store.staged_version(), None);
    }

    fn settings(check: bool, on_quit: bool) -> Settings {
        Settings {
            check_updates_automatically: check,
            install_updates_on_quit: on_quit,
            ..Settings::default()
        }
    }

    fn support(offered: bool) -> UpdateSupport {
        UpdateSupport {
            offered,
            reason: None,
            release_host: None,
        }
    }

    fn update(current: &str, version: &str) -> AvailableUpdate {
        AvailableUpdate::new(current, version, None, None)
            .expect("valid")
            .expect("newer")
    }

    #[test]
    fn only_a_patch_or_minor_update_with_the_setting_on_is_downloaded() {
        let on = settings(true, true);
        let minor = update("1.4.0", "1.5.0");
        assert_eq!(
            found_step(&on, &support(true), &minor, None),
            FoundStep::Download
        );
        assert_eq!(
            found_step(
                &on,
                &support(true),
                &update("1.4.0", "1.4.1"),
                Some("1.4.0")
            ),
            FoundStep::Download
        );
        // Already kept.
        assert_eq!(
            found_step(&on, &support(true), &minor, Some("1.5.0")),
            FoundStep::Nothing
        );
        // A major update is only announced (E55.8); before 1.0 a minor one too.
        assert_eq!(
            found_step(&on, &support(true), &update("1.4.0", "2.0.0"), None),
            FoundStep::Nothing
        );
        assert_eq!(
            found_step(&on, &support(true), &update("0.4.0", "0.5.0"), None),
            FoundStep::Nothing
        );
        // The setting off, automatic checks off, a copy that cannot update
        // itself (the folder probe failed): nothing is downloaded.
        for (s, sup) in [
            (settings(true, false), support(true)),
            (settings(false, true), support(true)),
            (on.clone(), support(false)),
        ] {
            assert_eq!(found_step(&s, &sup, &minor, None), FoundStep::Nothing);
        }
    }

    #[test]
    fn the_kept_package_installs_only_at_a_quit_by_the_user() {
        let on = settings(true, true);
        let minor = update("1.4.0", "1.5.0");
        let install = exit_step(true, &on, &support(true), Some(&minor), Some("1.5.0"));
        assert_eq!(install, ExitStep::Install("1.5.0".into()));
        // A logout, a shutdown, the restart after a confirmed install: the
        // package waits for the next quit.
        assert_eq!(
            exit_step(false, &on, &support(true), Some(&minor), Some("1.5.0")),
            ExitStep::Keep
        );
        // Nothing kept.
        assert_eq!(
            exit_step(true, &on, &support(true), Some(&minor), None),
            ExitStep::Keep
        );
    }

    #[test]
    fn a_package_waits_only_while_the_next_quit_would_install_it() {
        let on = settings(true, true);
        let minor = update("1.4.0", "1.5.0");
        assert_eq!(
            waiting_version(&on, &support(true), Some(&minor), Some("1.5.0")).as_deref(),
            Some("1.5.0")
        );
        let other = update("1.4.0", "1.5.1");
        let major = update("1.4.0", "2.0.0");
        for (s, sup, pending, staged) in [
            // Nothing downloaded yet.
            (on.clone(), support(true), Some(&minor), None),
            // The level is "Tell me", or off.
            (
                settings(true, false),
                support(true),
                Some(&minor),
                Some("1.5.0"),
            ),
            (
                settings(false, true),
                support(true),
                Some(&minor),
                Some("1.5.0"),
            ),
            // The copy no longer updates itself.
            (on.clone(), support(false), Some(&minor), Some("1.5.0")),
            // The offer is gone, another version, or major.
            (on.clone(), support(true), None, Some("1.5.0")),
            (on.clone(), support(true), Some(&other), Some("1.5.0")),
            (on.clone(), support(true), Some(&major), Some("2.0.0")),
        ] {
            assert_eq!(waiting_version(&s, &sup, pending, staged), None);
        }
    }

    #[test]
    fn a_kept_package_that_no_longer_applies_is_deleted_at_the_quit() {
        let on = settings(true, true);
        let minor = update("1.4.0", "1.5.0");
        for (s, sup, pending) in [
            // The setting turned off, automatic checks off.
            (settings(true, false), support(true), Some(&minor)),
            (settings(false, true), support(true), Some(&minor)),
            // The folder probe at quit failed: no admin rights are asked.
            (on.clone(), support(false), Some(&minor)),
            // The offer is gone or is another version.
            (on.clone(), support(true), None),
        ] {
            assert_eq!(
                exit_step(true, &s, &sup, pending, Some("1.5.0")),
                ExitStep::Discard
            );
        }
        let other = update("1.4.0", "1.5.1");
        assert_eq!(
            exit_step(true, &on, &support(true), Some(&other), Some("1.5.0")),
            ExitStep::Discard
        );
        let major = update("1.4.0", "2.0.0");
        assert_eq!(
            exit_step(true, &on, &support(true), Some(&major), Some("2.0.0")),
            ExitStep::Discard
        );
    }

    #[test]
    fn a_refused_package_notice_is_shown_once() {
        let home = Home::new("refused");
        let notice = UpdateNotice::NotInstalled {
            version: "1.5.0".into(),
            message: "not signed".into(),
        };
        home.store.record_notice(&notice);
        assert_eq!(home.store.take_notice(), Some(notice));
        assert_eq!(home.store.take_notice(), None);
    }
}
