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
        };
        assert_eq!(
            serde_json::to_value(support).expect("json")["offered"],
            false
        );
    }
}
