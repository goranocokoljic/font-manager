//! How this copy of the app was installed, which decides whether it updates
//! itself (E18.2, ADR 0012). The release installers (the Windows .exe, the
//! macOS app, the Linux AppImage) do when this user can write the folder they
//! are in; a Flatpak, a system package and a build made by a package manager
//! are updated by that tool instead. A copy in a folder this user cannot
//! write (the per-machine .msi in Program Files, an app in /Applications a
//! standard account cannot change) is not updated in place: that would need
//! admin rights (rule 4).

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

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
    /// A release copy in a folder this user cannot write: installing an
    /// update there would need admin rights (rule 4).
    NeedsAdmin,
}

impl Distribution {
    /// Whether this copy installs its own updates.
    pub fn updates_itself(&self) -> bool {
        matches!(self, Self::Installer | Self::AppImage)
    }
}

/// How the running copy was installed. Asked again on every check and
/// install, so a copy moved to a folder this user cannot write stops
/// offering updates at once.
pub fn distribution() -> Distribution {
    let exe = std::env::current_exe().ok();
    distribution_for(
        Os::current(),
        option_env!("FM_PACKAGED_BY"),
        &|name| std::env::var_os(name),
        crate::autostart::sandboxed(),
        exe.as_deref(),
        &writable_dir,
    )
}

/// How a copy on `os` was installed: `packaged_by` is [`PACKAGED_BY_ENV`]
/// at build time, `env` reads the process environment, `flatpak` says the
/// app runs in a Flatpak sandbox, `exe` is the running program and
/// `writable` says whether this user can create files in a folder.
pub fn distribution_for(
    os: Os,
    packaged_by: Option<&str>,
    env: &dyn Fn(&str) -> Option<OsString>,
    flatpak: bool,
    exe: Option<&Path>,
    writable: &dyn Fn(&Path) -> bool,
) -> Distribution {
    if let Some(tool) = packaged_by.map(str::trim).filter(|t| !t.is_empty()) {
        return Distribution::PackageManager(tool.to_owned());
    }
    let release = match os {
        Os::Windows | Os::MacOs => Distribution::Installer,
        Os::Linux if flatpak => return Distribution::Flatpak,
        Os::Linux if env(APPIMAGE_ENV).is_some_and(|p| !p.is_empty()) => Distribution::AppImage,
        Os::Linux => return Distribution::SystemPackage,
    };
    match update_folders(os, exe, env) {
        Some(folders) if folders.iter().all(|f| writable(f)) => release,
        _ => Distribution::NeedsAdmin,
    }
}

/// The folders an update writes into, all of which this user must be able to
/// write: the program's folder on Windows (the installer replaces it); on
/// macOS the folder holding `Font Manager.app` and the bundle itself (the
/// updater swaps the bundle, and moving a folder to another parent needs
/// write access to it); the AppImage's folder on Linux. `None` when it
/// cannot be told.
pub fn update_folders(
    os: Os,
    exe: Option<&Path>,
    env: &dyn Fn(&str) -> Option<OsString>,
) -> Option<Vec<PathBuf>> {
    match os {
        Os::Windows => Some(vec![exe?.parent()?.to_path_buf()]),
        Os::MacOs => {
            let exe = exe?;
            match exe
                .ancestors()
                .find(|p| p.extension().is_some_and(|e| e == "app"))
            {
                Some(bundle) => Some(vec![bundle.parent()?.to_path_buf(), bundle.to_path_buf()]),
                // An unbundled program (a dev build): its own folder.
                None => Some(vec![exe.parent()?.to_path_buf()]),
            }
        }
        Os::Linux => {
            let appimage = PathBuf::from(env(APPIMAGE_ENV).filter(|p| !p.is_empty())?);
            Some(vec![appimage.parent()?.to_path_buf()])
        }
    }
}

/// Whether this user can create a file in `dir`: a probe file is created
/// there and removed at once. Permission bits and ACLs are not read, since
/// only the OS can say what they allow this process (and no FFI crate for
/// `access(2)` is in the library table). In the macOS bundle the probe lives
/// for the two calls only; the bundle's seal is checked at launch, when it is
/// gone.
pub fn writable_dir(dir: &Path) -> bool {
    let probe = dir.join(format!(".fm-update-probe-{}", std::process::id()));
    match OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(file) => {
            drop(file);
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
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

    // Forward slashes: Windows reads them as separators, and so does the
    // test on macOS and Linux.
    const WIN_DIR: &str = "C:/Users/u/AppData/Local/Programs/Font Manager";
    const WIN_EXE: &str = "C:/Users/u/AppData/Local/Programs/Font Manager/font-manager.exe";
    const MAC_EXE: &str = "/Applications/Font Manager.app/Contents/MacOS/font-manager";
    const LINUX_EXE: &str = "/tmp/.mount_FontMa/usr/bin/font-manager";

    fn exe(os: Os) -> &'static Path {
        Path::new(match os {
            Os::Windows => WIN_EXE,
            Os::MacOs => MAC_EXE,
            Os::Linux => LINUX_EXE,
        })
    }

    fn yes(_: &Path) -> bool {
        true
    }

    fn no(_: &Path) -> bool {
        false
    }

    fn on(
        os: Os,
        env: &dyn Fn(&str) -> Option<OsString>,
        writable: &dyn Fn(&Path) -> bool,
    ) -> Distribution {
        distribution_for(os, None, env, false, Some(exe(os)), writable)
    }

    #[test]
    fn the_release_installers_and_the_appimage_update_themselves_where_the_user_can_write() {
        assert_eq!(on(Os::Windows, &none, &yes), Distribution::Installer);
        assert_eq!(on(Os::MacOs, &none, &yes), Distribution::Installer);
        assert_eq!(on(Os::Linux, &appimage, &yes), Distribution::AppImage);
        for os in [Os::Windows, Os::MacOs, Os::Linux] {
            assert!(on(os, &appimage, &yes).updates_itself(), "{os:?}");
        }
    }

    #[test]
    fn a_copy_in_a_folder_the_user_cannot_write_does_not_update_itself() {
        // The per-machine .msi in Program Files, an app in /Applications a
        // standard account cannot change, an AppImage in a read-only folder.
        for os in [Os::Windows, Os::MacOs, Os::Linux] {
            let d = on(os, &appimage, &no);
            assert_eq!(d, Distribution::NeedsAdmin, "{os:?}");
            assert!(!d.updates_itself());
        }
        // A program path that cannot be told is not offered either.
        assert_eq!(
            distribution_for(Os::Windows, None, &none, false, None, &yes),
            Distribution::NeedsAdmin
        );
    }

    #[test]
    fn the_folder_checked_is_the_one_the_update_writes_into() {
        let asked = std::cell::RefCell::new(Vec::new());
        let record = |p: &Path| {
            asked.borrow_mut().push(p.to_path_buf());
            true
        };
        on(Os::Windows, &none, &record);
        on(Os::MacOs, &none, &record);
        on(Os::Linux, &appimage, &record);
        assert_eq!(
            asked.into_inner(),
            vec![
                PathBuf::from(WIN_DIR),
                PathBuf::from("/Applications"),
                PathBuf::from("/Applications/Font Manager.app"),
                PathBuf::from("/home/u/Apps"),
            ]
        );
        // An unbundled macOS program (a dev build): its own folder.
        assert_eq!(
            update_folders(
                Os::MacOs,
                Some(Path::new("/src/target/debug/fm-app")),
                &none
            ),
            Some(vec![PathBuf::from("/src/target/debug")])
        );
    }

    #[test]
    fn a_mac_bundle_the_user_cannot_write_in_a_writable_folder_does_not_update_itself() {
        // An admin account can write /Applications, but the bundle belongs to
        // root or another account: swapping it would ask for admin rights.
        let only_parent = |p: &Path| p == Path::new("/Applications");
        assert_eq!(on(Os::MacOs, &none, &only_parent), Distribution::NeedsAdmin);
        let only_bundle = |p: &Path| p == Path::new("/Applications/Font Manager.app");
        assert_eq!(on(Os::MacOs, &none, &only_bundle), Distribution::NeedsAdmin);
    }

    #[test]
    fn writable_dir_creates_and_removes_a_probe_and_fails_where_it_cannot() {
        let dir = std::env::temp_dir().join(format!("fm-probe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        assert!(writable_dir(&dir));
        assert_eq!(
            std::fs::read_dir(&dir).expect("read").count(),
            0,
            "the probe stayed"
        );
        assert!(!writable_dir(&dir.join("missing")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn flatpak_system_packages_and_package_manager_builds_do_not() {
        let linux = Some(exe(Os::Linux));
        let flatpak = distribution_for(Os::Linux, None, &appimage, true, linux, &yes);
        assert_eq!(flatpak, Distribution::Flatpak);
        let deb = on(Os::Linux, &none, &yes);
        assert_eq!(deb, Distribution::SystemPackage);
        let empty = |name: &str| (name == APPIMAGE_ENV).then(OsString::new);
        assert_eq!(on(Os::Linux, &empty, &yes), Distribution::SystemPackage);
        for os in [Os::Windows, Os::MacOs, Os::Linux] {
            let brew = distribution_for(
                os,
                Some(" Homebrew "),
                &appimage,
                false,
                Some(exe(os)),
                &yes,
            );
            assert_eq!(brew, Distribution::PackageManager("Homebrew".into()));
            assert!(!brew.updates_itself());
        }
        assert!(!flatpak.updates_itself());
        assert!(!deb.updates_itself());
        // An empty name is no package manager.
        let windows = Some(exe(Os::Windows));
        assert_eq!(
            distribution_for(Os::Windows, Some(""), &none, false, windows, &yes),
            Distribution::Installer
        );
    }
}
