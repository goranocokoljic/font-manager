//! In-app updates (E18.2, ADR 0012): what a check found, how big a step it
//! is, and what an install needs before it may start.
//!
//! The desktop shell checks, downloads and installs through Tauri's updater;
//! the decisions are made here so every caller makes the same ones: whether
//! an offered version is newer and whether it is a major step
//! ([`update_kind`]), and whether an install request is confirmed enough
//! ([`InstallUpdateRequest::check`]).

use serde::{Deserialize, Serialize};

use crate::{ApiError, ApiResult, ErrorCode};

/// How far an update moves the app, by semantic version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum UpdateKind {
    /// Fixes only (1.2.3 → 1.2.4).
    Patch,
    /// New features, nothing removed (1.2.3 → 1.3.0).
    Minor,
    /// May change how the app works (1.2.3 → 2.0.0). Installed only after a
    /// second confirmation. Before 1.0, a new minor version (0.3 → 0.4) is a
    /// major step too: semver gives 0.x no compatibility promise.
    Major,
}

/// An update the release server offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    /// The version running now.
    pub current_version: String,
    /// The version offered.
    pub version: String,
    /// The release notes, as the release wrote them (Markdown).
    #[serde(default)]
    pub notes: Option<String>,
    /// When the release was published (RFC 3339).
    #[serde(default)]
    pub date: Option<String>,
    /// How far the update moves the app.
    pub kind: UpdateKind,
}

impl AvailableUpdate {
    /// The update `version` offered to `current_version`, or `None` when it
    /// is not newer; `InvalidRequest` when either is not a semantic version.
    pub fn new(
        current_version: &str,
        version: &str,
        notes: Option<String>,
        date: Option<String>,
    ) -> ApiResult<Option<Self>> {
        Ok(update_kind(current_version, version)?.map(|kind| Self {
            current_version: current_version.to_owned(),
            version: version.to_owned(),
            notes,
            date,
            kind,
        }))
    }

    /// A major update asks twice before it installs.
    pub fn needs_second_confirmation(&self) -> bool {
        self.kind == UpdateKind::Major
    }

    /// Whether "Install updates when I quit" may install this update (E55.9):
    /// a patch or minor one. A major update (before 1.0 a minor one too, as
    /// [`update_kind`] says) is only announced and asks twice.
    pub fn installs_on_quit(&self) -> bool {
        !self.needs_second_confirmation()
    }
}

/// What a check for updates found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    /// The version running now.
    pub current_version: String,
    /// The newer release, or `None` when this is the latest.
    #[serde(default)]
    pub update: Option<AvailableUpdate>,
}

/// Whether this copy of the app updates itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSupport {
    /// The app offers "Check for updates"; false where something else
    /// installs its updates (Flatpak, a package manager).
    pub offered: bool,
    /// Why not, one sentence for Settings → About → Platform notes; `None`
    /// when offered.
    #[serde(default)]
    pub reason: Option<String>,
    /// The host of the release server a check asks (`github.com`), read from
    /// the endpoint the app was built with, so no text names it by hand.
    /// `None` when the build names no endpoint.
    #[serde(default)]
    pub release_host: Option<String>,
}

/// How often an automatic check runs while the app runs: once a day.
pub const AUTOMATIC_CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;

/// Whether the app checks for updates by itself: the user turned "Check
/// automatically" on and this copy updates itself. Otherwise no timer runs
/// and no request is made (rule 7).
pub fn checks_automatically(settings: &crate::settings::Settings, support: &UpdateSupport) -> bool {
    settings.check_updates_automatically && support.offered
}

/// Whether an update an automatic check finds is downloaded in the
/// background and installed when the user quits (E55.9, ADR 0012): the user
/// turned on "Install updates when I quit", automatic checks run
/// ([`checks_automatically`]) and this copy updates itself. Which updates
/// qualify is [`AvailableUpdate::installs_on_quit`].
pub fn installs_on_quit(settings: &crate::settings::Settings, support: &UpdateSupport) -> bool {
    settings.install_updates_on_quit && checks_automatically(settings, support)
}

/// `Ok` when `update` may be saved by a copy of the app with `support`
/// (`None`: a caller that does not know, such as the CLI's direct link).
/// Turning "Install updates when I quit" on is refused where this copy does
/// not update itself; turning it off is always allowed.
pub fn check_settings_update(
    update: &crate::settings::SettingsUpdate,
    support: Option<&UpdateSupport>,
) -> ApiResult<()> {
    if update.install_updates_on_quit != Some(true) || support.is_some_and(|s| s.offered) {
        return Ok(());
    }
    let reason = support
        .and_then(|s| s.reason.clone())
        .unwrap_or_else(|| "This copy of Font Fu does not update itself.".to_owned());
    Err(ApiError::new(
        ErrorCode::Unsupported,
        format!("Font Fu cannot install updates when you quit here. {reason}"),
    ))
}

/// What the app says once about an update installed when the user quit
/// (E55.9): at the next start whether it installed, or while it runs that a
/// package downloaded for it was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UpdateNotice {
    /// The update installed: "Updated to <version>", with a link to what is new.
    #[serde(rename_all = "camelCase")]
    Updated {
        /// The version now running.
        version: String,
        /// The release notes of that version; `None` when the release
        /// server has no page for them.
        #[serde(default)]
        notes_url: Option<String>,
    },
    /// The update was not installed: the earlier version runs as before. The
    /// UI offers the update dialog.
    #[serde(rename_all = "camelCase")]
    NotInstalled {
        /// The version that was not installed.
        version: String,
        /// One user-facing sentence on why.
        message: String,
    },
}

impl UpdateNotice {
    /// What the start after an install of `version` at quit says, with
    /// `running` the version that started: installed when `running` is
    /// `version` or newer, else not installed. `InvalidRequest` when either is
    /// not a semantic version.
    pub fn after_install(
        version: &str,
        running: &str,
        notes_url: Option<String>,
    ) -> ApiResult<Self> {
        Ok(match update_kind(running, version)? {
            None => Self::Updated {
                version: running.to_owned(),
                notes_url,
            },
            Some(_) => Self::NotInstalled {
                version: version.to_owned(),
                message: format!(
                    "Font Fu could not install version {version} when it quit, so version \
                     {running} started as before."
                ),
            },
        })
    }
}

/// Seconds until the next automatic check is due; 0 means now.
///
/// `last_check` is when the last automatic check started in this run of the
/// app, `now` the time now, both in seconds on one clock; `None` means none
/// yet, so the check at start is due at once. A check that failed counts as
/// a check: the next one is a full interval later, never sooner. A `now`
/// before `last_check` (a clock that went back) waits the full interval.
pub fn automatic_check_wait(last_check: Option<u64>, now: u64) -> u64 {
    match last_check {
        None => 0,
        Some(last) => AUTOMATIC_CHECK_INTERVAL_SECS.saturating_sub(now.saturating_sub(last)),
    }
}

/// Install the update a check found. The app quits as it does on Quit
/// (session fonts are deregistered), installs and starts again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InstallUpdateRequest {
    /// The version the user confirmed; must be the one the last check found.
    pub version: String,
    /// The user confirmed a second time that a major update may install.
    #[serde(default)]
    pub confirm_major: bool,
}

impl InstallUpdateRequest {
    /// `Ok` when this request may install `pending`: the same version, and
    /// for a major update the second confirmation. Otherwise nothing is
    /// downloaded or installed.
    pub fn check(&self, pending: &AvailableUpdate) -> ApiResult<()> {
        if self.version != pending.version {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                format!(
                    "Version {} is no longer the update on offer. Check for updates again.",
                    self.version
                ),
            )
            .with_detail(format!("pending update is {}", pending.version)));
        }
        if pending.needs_second_confirmation() && !self.confirm_major {
            return Err(ApiError::invalid(format!(
                "Version {} is a major update. Confirm it a second time to install it.",
                pending.version
            )));
        }
        Ok(())
    }
}

/// How far `offered` moves the app from `current`; `None` when `offered` is
/// not newer. A leading `v` is allowed (`v1.2.0`, as release tags are named).
pub fn update_kind(current: &str, offered: &str) -> ApiResult<Option<UpdateKind>> {
    let current = parse(current)?;
    let offered = parse(offered)?;
    if offered <= current {
        return Ok(None);
    }
    let kind = if offered.major != current.major
        || (current.major == 0 && offered.minor != current.minor)
    {
        UpdateKind::Major
    } else if offered.minor != current.minor {
        UpdateKind::Minor
    } else {
        UpdateKind::Patch
    };
    Ok(Some(kind))
}

fn parse(version: &str) -> ApiResult<semver::Version> {
    let text = version.trim();
    semver::Version::parse(text.strip_prefix('v').unwrap_or(text)).map_err(|e| {
        ApiError::invalid(format!("{version:?} is not a version number."))
            .with_detail(e.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(current: &str, offered: &str) -> Option<UpdateKind> {
        update_kind(current, offered).expect("valid versions")
    }

    #[test]
    fn patch_minor_and_major_steps_are_told_apart() {
        assert_eq!(kind("1.2.3", "1.2.4"), Some(UpdateKind::Patch));
        assert_eq!(kind("1.2.3", "1.3.0"), Some(UpdateKind::Minor));
        assert_eq!(kind("1.2.3", "1.10.0"), Some(UpdateKind::Minor));
        assert_eq!(kind("1.2.3", "2.0.0"), Some(UpdateKind::Major));
        assert_eq!(kind("1.9.9", "3.0.1"), Some(UpdateKind::Major));
        assert_eq!(kind("v1.2.3", "v2.0.0"), Some(UpdateKind::Major));
    }

    #[test]
    fn before_one_point_oh_a_minor_step_is_major() {
        assert_eq!(kind("0.1.0", "0.1.1"), Some(UpdateKind::Patch));
        assert_eq!(kind("0.1.0", "0.2.0"), Some(UpdateKind::Major));
        assert_eq!(kind("0.9.0", "1.0.0"), Some(UpdateKind::Major));
    }

    #[test]
    fn the_same_or_an_older_version_is_no_update() {
        assert_eq!(kind("1.2.3", "1.2.3"), None);
        assert_eq!(kind("1.2.3", "1.2.2"), None);
        assert_eq!(kind("2.0.0", "1.9.9"), None);
        // A pre-release of the running version is older than it.
        assert_eq!(kind("1.2.3", "1.2.3-beta.1"), None);
        assert_eq!(kind("1.2.3-beta.1", "1.2.3"), Some(UpdateKind::Patch));
    }

    #[test]
    fn a_version_that_is_not_semver_is_refused() {
        for (current, offered) in [("1.2", "1.3.0"), ("1.2.3", "latest"), ("", "1.0.0")] {
            let e = update_kind(current, offered).expect_err("not semver");
            assert_eq!(e.code, ErrorCode::InvalidRequest);
        }
    }

    fn pending(current: &str, version: &str) -> AvailableUpdate {
        AvailableUpdate::new(current, version, Some("Notes".into()), None)
            .expect("valid")
            .expect("newer")
    }

    #[test]
    fn a_major_update_installs_only_after_the_second_confirmation() {
        let major = pending("1.4.0", "2.0.0");
        assert!(major.needs_second_confirmation());
        let once = InstallUpdateRequest {
            version: "2.0.0".into(),
            confirm_major: false,
        };
        assert_eq!(
            once.check(&major).expect_err("needs a second yes").code,
            ErrorCode::InvalidRequest
        );
        let twice = InstallUpdateRequest {
            confirm_major: true,
            ..once
        };
        assert!(twice.check(&major).is_ok());
    }

    #[test]
    fn a_minor_update_installs_after_one_confirmation() {
        let minor = pending("1.4.0", "1.5.0");
        assert!(!minor.needs_second_confirmation());
        let req: InstallUpdateRequest =
            serde_json::from_str(r#"{"version":"1.5.0"}"#).expect("confirmMajor defaults");
        assert!(!req.confirm_major);
        assert!(req.check(&minor).is_ok());
    }

    #[test]
    fn an_install_for_another_version_is_refused() {
        let minor = pending("1.4.0", "1.5.0");
        let req = InstallUpdateRequest {
            version: "1.6.0".into(),
            confirm_major: true,
        };
        assert_eq!(
            req.check(&minor).expect_err("stale").code,
            ErrorCode::Conflict
        );
    }

    #[test]
    fn no_update_is_built_for_an_older_release() {
        assert_eq!(AvailableUpdate::new("1.4.0", "1.4.0", None, None), Ok(None));
    }

    #[test]
    fn wire_format_is_camel_case() {
        let json = serde_json::to_value(pending("1.4.0", "2.0.0")).expect("json");
        assert_eq!(json["currentVersion"], "1.4.0");
        assert_eq!(json["kind"], "major");
        let support = UpdateSupport {
            offered: false,
            reason: Some("Flatpak".into()),
            release_host: Some("github.com".into()),
        };
        let json = serde_json::to_value(support).expect("json");
        assert_eq!(json["offered"], false);
        assert_eq!(json["releaseHost"], "github.com");
    }

    #[test]
    fn support_written_before_the_release_host_still_reads() {
        let old: UpdateSupport =
            serde_json::from_str(r#"{"offered":true,"reason":null}"#).expect("old support");
        assert_eq!(old.release_host, None);
    }

    #[test]
    fn the_check_at_start_is_due_at_once() {
        assert_eq!(automatic_check_wait(None, 0), 0);
        assert_eq!(automatic_check_wait(None, 5_000_000), 0);
    }

    #[test]
    fn the_next_check_is_due_a_day_after_the_last() {
        let day = AUTOMATIC_CHECK_INTERVAL_SECS;
        assert_eq!(day, 86_400);
        assert_eq!(automatic_check_wait(Some(100), 100), day);
        assert_eq!(automatic_check_wait(Some(100), 100 + 3_600), day - 3_600);
        assert_eq!(automatic_check_wait(Some(100), 100 + day - 1), 1);
        assert_eq!(automatic_check_wait(Some(100), 100 + day), 0);
        assert_eq!(automatic_check_wait(Some(100), 100 + 3 * day), 0);
    }

    #[test]
    fn a_clock_that_went_back_waits_the_full_day() {
        assert_eq!(
            automatic_check_wait(Some(10_000), 50),
            AUTOMATIC_CHECK_INTERVAL_SECS
        );
    }

    fn support(offered: bool) -> UpdateSupport {
        UpdateSupport {
            offered,
            reason: (!offered).then(|| "Flatpak installs the updates of this copy.".to_owned()),
            release_host: Some("github.com".into()),
        }
    }

    #[test]
    fn only_patch_and_minor_updates_install_on_quit() {
        assert!(pending("1.4.0", "1.4.1").installs_on_quit());
        assert!(pending("1.4.0", "1.5.0").installs_on_quit());
        assert!(!pending("1.4.0", "2.0.0").installs_on_quit());
        // Before 1.0 a minor step is major: announced, never installed at quit.
        assert!(pending("0.3.0", "0.3.1").installs_on_quit());
        assert!(!pending("0.3.0", "0.4.0").installs_on_quit());
    }

    #[test]
    fn install_on_quit_needs_the_setting_the_automatic_check_and_a_copy_that_updates_itself() {
        let off = crate::settings::Settings::default();
        let on = crate::settings::Settings {
            check_updates_automatically: true,
            install_updates_on_quit: true,
            ..off.clone()
        };
        let no_checks = crate::settings::Settings {
            check_updates_automatically: false,
            ..on.clone()
        };
        assert!(installs_on_quit(&on, &support(true)));
        assert!(!installs_on_quit(&on, &support(false)));
        assert!(!installs_on_quit(&no_checks, &support(true)));
        assert!(!installs_on_quit(&off, &support(true)));
    }

    #[test]
    fn turning_install_on_quit_on_is_refused_where_the_copy_does_not_update_itself() {
        let on = crate::settings::SettingsUpdate {
            install_updates_on_quit: Some(true),
            ..Default::default()
        };
        let off = crate::settings::SettingsUpdate {
            install_updates_on_quit: Some(false),
            ..Default::default()
        };
        assert!(check_settings_update(&on, Some(&support(true))).is_ok());
        let e = check_settings_update(&on, Some(&support(false))).expect_err("refused");
        assert_eq!(e.code, ErrorCode::Unsupported);
        assert!(e.message.contains("Flatpak"), "{e:?}");
        assert_eq!(
            check_settings_update(&on, None).expect_err("unknown").code,
            ErrorCode::Unsupported
        );
        assert!(check_settings_update(&off, Some(&support(false))).is_ok());
        assert!(check_settings_update(&off, None).is_ok());
        assert!(check_settings_update(&Default::default(), None).is_ok());
    }

    #[test]
    fn the_start_after_an_install_says_whether_it_installed() {
        let url = Some("https://example.invalid/notes".to_owned());
        assert_eq!(
            UpdateNotice::after_install("1.5.0", "1.5.0", url.clone()).expect("valid"),
            UpdateNotice::Updated {
                version: "1.5.0".into(),
                notes_url: url.clone()
            }
        );
        let failed = UpdateNotice::after_install("1.5.0", "1.4.0", url).expect("valid");
        let UpdateNotice::NotInstalled { version, message } = failed else {
            panic!("not a failure: {failed:?}");
        };
        assert_eq!(version, "1.5.0");
        assert!(message.contains("1.4.0"), "{message}");
        assert!(UpdateNotice::after_install("x", "1.4.0", None).is_err());
    }

    #[test]
    fn an_update_notice_is_tagged_camel_case() {
        let json = serde_json::to_value(UpdateNotice::Updated {
            version: "1.5.0".into(),
            notes_url: None,
        })
        .expect("json");
        assert_eq!(json["type"], "updated");
        assert_eq!(json["notesUrl"], serde_json::Value::Null);
        let json = serde_json::to_value(UpdateNotice::NotInstalled {
            version: "1.5.0".into(),
            message: "m".into(),
        })
        .expect("json");
        assert_eq!(json["type"], "notInstalled");
    }

    #[test]
    fn only_a_copy_that_updates_itself_with_the_setting_on_checks_by_itself() {
        let offered = UpdateSupport {
            offered: true,
            reason: None,
            release_host: Some("github.com".into()),
        };
        let managed = UpdateSupport {
            offered: false,
            reason: Some("Flatpak".into()),
            release_host: Some("github.com".into()),
        };
        let off = crate::settings::Settings::default();
        let on = crate::settings::Settings {
            check_updates_automatically: true,
            ..off.clone()
        };
        assert!(checks_automatically(&on, &offered));
        assert!(!checks_automatically(&off, &offered));
        assert!(!checks_automatically(&on, &managed));
        assert!(!checks_automatically(&off, &managed));
    }
}
