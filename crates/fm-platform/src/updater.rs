//! How this copy of the app was installed, which decides whether it updates
//! itself (E18.2, ADR 0012). The release installers (Windows .exe/.msi, the
//! macOS app, the Linux AppImage) do; a Flatpak, a system package and a build
//! made by a package manager are updated by that tool instead.

use std::ffi::OsString;

use crate::Os;

/// The build-time variable a package manager's recipe sets to the tool's
/// name (`FM_PACKAGED_BY=Homebrew cargo build …`); such a build never
/// updates itself.
pub const PACKAGED_BY_ENV: &str = "FM_PACKAGED_BY";

/// The variable the AppImage runtime sets to the AppImage's path.
pub const APPIMAGE_ENV: &str = "APPIMAGE";

/// How this copy of the app was installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Distribution {
    /// A release installer: the Windows .exe or .msi, the macOS app.
    Installer,
    /// The Linux AppImage.
    AppImage,
    /// A Flatpak: Flatpak installs the updates.
    Flatpak,
    /// A Linux system package (.deb, .rpm) not run as an AppImage: the
    /// package manager installs the updates, and the app does not replace
    /// files it cannot write without admin rights (rule 4).
    SystemPackage,
    /// A build a package manager made; the name it gave in
    /// [`PACKAGED_BY_ENV`].
    PackageManager(String),
}

impl Distribution {
    /// Whether this copy installs its own updates.
    pub fn updates_itself(&self) -> bool {
        matches!(self, Self::Installer | Self::AppImage)
    }
}

/// How the running copy was installed.
pub fn distribution() -> Distribution {
    distribution_for(
        Os::current(),
        option_env!("FM_PACKAGED_BY"),
        &|name| std::env::var_os(name),
        crate::autostart::sandboxed(),
    )
}

/// How a copy on `os` was installed: `packaged_by` is [`PACKAGED_BY_ENV`]
/// at build time, `env` reads the process environment, `flatpak` says the
/// app runs in a Flatpak sandbox.
pub fn distribution_for(
    os: Os,
    packaged_by: Option<&str>,
    env: &dyn Fn(&str) -> Option<OsString>,
    flatpak: bool,
) -> Distribution {
    if let Some(tool) = packaged_by.map(str::trim).filter(|t| !t.is_empty()) {
        return Distribution::PackageManager(tool.to_owned());
    }
    match os {
        Os::Windows | Os::MacOs => Distribution::Installer,
        Os::Linux if flatpak => Distribution::Flatpak,
        Os::Linux if env(APPIMAGE_ENV).is_some_and(|p| !p.is_empty()) => Distribution::AppImage,
        Os::Linux => Distribution::SystemPackage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &str) -> Option<OsString> {
        None
    }

    fn appimage(name: &str) -> Option<OsString> {
        (name == APPIMAGE_ENV).then(|| "/home/u/Apps/Font_Manager.AppImage".into())
    }

    #[test]
    fn the_release_installers_and_the_appimage_update_themselves() {
        for (os, env) in [
            (Os::Windows, none as fn(&str) -> Option<OsString>),
            (Os::MacOs, none),
            (Os::Linux, appimage),
        ] {
            let d = distribution_for(os, None, &env, false);
            assert!(d.updates_itself(), "{os:?}: {d:?}");
        }
        assert_eq!(
            distribution_for(Os::Linux, None, &appimage, false),
            Distribution::AppImage
        );
    }

    #[test]
    fn flatpak_system_packages_and_package_manager_builds_do_not() {
        let flatpak = distribution_for(Os::Linux, None, &appimage, true);
        assert_eq!(flatpak, Distribution::Flatpak);
        let deb = distribution_for(Os::Linux, None, &none, false);
        assert_eq!(deb, Distribution::SystemPackage);
        let empty = |name: &str| (name == APPIMAGE_ENV).then(OsString::new);
        assert_eq!(
            distribution_for(Os::Linux, None, &empty, false),
            Distribution::SystemPackage
        );
        for os in [Os::Windows, Os::MacOs, Os::Linux] {
            let brew = distribution_for(os, Some(" Homebrew "), &appimage, false);
            assert_eq!(brew, Distribution::PackageManager("Homebrew".into()));
            assert!(!brew.updates_itself());
        }
        assert!(!flatpak.updates_itself());
        assert!(!deb.updates_itself());
        // An empty name is no package manager.
        assert_eq!(
            distribution_for(Os::Windows, Some(""), &none, false),
            Distribution::Installer
        );
    }
}
