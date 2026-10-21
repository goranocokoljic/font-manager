//! Whether this copy of the app offers "Check for updates" (E18.2, ADR 0012),
//! and the platform note that says why not.

use fm_api::update::UpdateSupport;
use fm_platform::Distribution;

/// What Settings shows for a copy installed as `distribution`: the update
/// section for the release installers and the AppImage; a platform note
/// naming the tool that installs updates for every other copy.
pub fn support(distribution: &Distribution) -> UpdateSupport {
    let reason = match distribution {
        Distribution::Installer | Distribution::AppImage => None,
        Distribution::Flatpak => Some(
            "Flatpak installs the updates of this copy (flatpak update, or your software \
             center), so Check for updates is not shown."
                .to_owned(),
        ),
        Distribution::SystemPackage => Some(
            "Your system's package manager installs the updates of this copy, so Check for \
             updates is not shown. The AppImage updates itself."
                .to_owned(),
        ),
        Distribution::PackageManager(tool) => Some(format!(
            "{tool} installs the updates of this copy, so Check for updates is not shown."
        )),
        Distribution::NeedsAdmin => Some(
            "This copy is in a folder your account cannot change (installed for all users), \
             and Font Manager never asks for administrator rights, so Check for updates is \
             not shown. Install new versions from the releases page; the per-user installer \
             (.exe on Windows) updates itself."
                .to_owned(),
        ),
    };
    UpdateSupport {
        offered: reason.is_none(),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installers_and_the_appimage_offer_updates_without_a_note() {
        for d in [Distribution::Installer, Distribution::AppImage] {
            assert_eq!(
                support(&d),
                UpdateSupport {
                    offered: true,
                    reason: None
                }
            );
        }
    }

    #[test]
    fn other_copies_hide_updates_and_name_the_tool_that_installs_them() {
        let flatpak = support(&Distribution::Flatpak);
        assert!(!flatpak.offered);
        assert!(
            flatpak
                .reason
                .as_deref()
                .is_some_and(|r| r.contains("Flatpak"))
        );
        let deb = support(&Distribution::SystemPackage);
        assert!(!deb.offered);
        assert!(
            deb.reason
                .as_deref()
                .is_some_and(|r| r.contains("package manager"))
        );
        let admin = support(&Distribution::NeedsAdmin);
        assert!(!admin.offered);
        assert!(
            admin
                .reason
                .as_deref()
                .is_some_and(|r| r.contains("never asks for administrator rights"))
        );
        let brew = support(&Distribution::PackageManager("Homebrew".into()));
        assert!(!brew.offered);
        assert!(
            brew.reason
                .as_deref()
                .is_some_and(|r| r.starts_with("Homebrew "))
        );
    }
}
