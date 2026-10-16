//! Where the app keeps its own files on each OS (ADR 0007).
//!
//! Each OS has a pure resolver under `src/{windows,macos,linux}/paths.rs` that
//! reads the environment through a lookup function. The resolvers compile on
//! every host, so the rules for all three platforms are tested everywhere;
//! only [`AppPaths::current`] picks the host's resolver.
//!
//! Nothing here creates a folder. The caller creates the app-owned folders it
//! writes to; the library folder is only a suggestion the user confirms.

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

/// The folder name under the per-user app-data roots (Windows, Linux).
pub(crate) const APP_DIR: &str = "FontManager";
/// The folder name under `~/Library/*` on macOS: the bundle identifier.
pub(crate) const BUNDLE_ID: &str = "org.fontmanager.app";
/// The folder name under `$XDG_*_HOME` on Linux.
pub(crate) const XDG_DIR: &str = "font-manager";
/// The suggested library folder name under the user's home.
pub(crate) const LIBRARY_DIR: &str = "Fonts";

/// An operating system the app runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    /// Windows 10 and later.
    Windows,
    /// macOS.
    MacOs,
    /// Linux desktops (XDG base directories).
    Linux,
}

impl Os {
    /// The OS this binary was built for.
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }
}

/// The per-user folders the app uses. All paths are absolute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    /// Durable app data: the index database, user metadata backups.
    pub data: PathBuf,
    /// The app-data folder that holds `config.json`.
    pub config: PathBuf,
    /// Rebuildable data: thumbnails, catalog metadata.
    pub cache: PathBuf,
    /// Rotating log files.
    pub logs: PathBuf,
    /// The suggested folder for "Copy to library folder" on import. The user
    /// confirms or changes it; the app never creates it uninvited.
    pub library: PathBuf,
}

impl AppPaths {
    /// The folders for the current user on the host OS.
    pub fn current() -> Result<Self, PathError> {
        Self::resolve(Os::current(), |name| std::env::var_os(name))
    }

    /// The folders for `os`, reading environment variables through `env`.
    pub fn resolve(os: Os, env: impl Fn(&str) -> Option<OsString>) -> Result<Self, PathError> {
        let env = Env(&env);
        match os {
            Os::Windows => crate::windows::paths::resolve(&env),
            Os::MacOs => crate::macos::paths::resolve(&env),
            Os::Linux => crate::linux::paths::resolve(&env),
        }
    }

    /// The config file inside [`AppPaths::config`].
    pub fn config_file(&self) -> PathBuf {
        self.config.join("config.json")
    }
}

/// An environment lookup that keeps only usable values: set, non-empty and absolute.
pub(crate) struct Env<'a>(&'a dyn Fn(&str) -> Option<OsString>);

impl Env<'_> {
    /// The variable as an absolute path, or `None` when unset, empty or relative.
    pub(crate) fn dir(&self, name: &str) -> Option<PathBuf> {
        let value = (self.0)(name)?;
        if value.is_empty() {
            return None;
        }
        let path = PathBuf::from(value);
        is_absolute_for_any_os(&path).then_some(path)
    }

    /// Like [`Env::dir`], but a missing value is an error.
    pub(crate) fn required_dir(&self, name: &'static str) -> Result<PathBuf, PathError> {
        self.dir(name).ok_or(PathError::MissingVariable(name))
    }
}

/// Absolute on the host, or a Windows drive or UNC path when resolving the
/// Windows rules on another host (tests run every OS's rules on every OS).
fn is_absolute_for_any_os(path: &std::path::Path) -> bool {
    if path.is_absolute() || path.has_root() {
        return true;
    }
    let s = path.to_string_lossy();
    let b = s.as_bytes();
    let drive = b.len() >= 3
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && (b[2] == b'\\' || b[2] == b'/');
    drive || s.starts_with(r"\\")
}

/// The app folders could not be determined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// A variable the OS always sets for a user session is missing, empty or relative.
    MissingVariable(&'static str),
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingVariable(name) => write!(
                f,
                "cannot find the user's folders: the environment variable {name} is not set to an absolute path"
            ),
        }
    }
}

impl std::error::Error for PathError {}

/// A fake environment for the per-OS resolver tests.
#[cfg(test)]
pub(crate) fn test_env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + use<> {
    let map: std::collections::HashMap<String, OsString> = vars
        .iter()
        .map(|(k, v)| ((*k).to_owned(), OsString::from(*v)))
        .collect();
    move |name| map.get(name).cloned()
}

#[cfg(test)]
mod tests {
    use super::test_env as env;
    use super::*;

    #[test]
    fn current_os_paths_are_absolute_and_distinct() {
        let paths = AppPaths::current().expect("a test session has a home folder");
        for p in [
            &paths.data,
            &paths.config,
            &paths.cache,
            &paths.logs,
            &paths.library,
        ] {
            assert!(p.is_absolute(), "{} is not absolute", p.display());
        }
        assert_ne!(paths.data, paths.cache);
        assert_ne!(paths.logs, paths.library);
        assert!(paths.config_file().starts_with(&paths.config));
    }

    #[test]
    fn relative_and_empty_values_are_ignored() {
        let lookup = env(&[
            ("A", ""),
            ("B", "relative/dir"),
            ("C", "/abs"),
            ("D", r"C:\Users\x"),
            ("E", r"\\server\share"),
            ("F", "C:"),
        ]);
        let e = Env(&lookup);
        assert_eq!(e.dir("A"), None);
        assert_eq!(e.dir("B"), None);
        assert_eq!(e.dir("missing"), None);
        assert_eq!(e.dir("C"), Some(PathBuf::from("/abs")));
        assert_eq!(e.dir("D"), Some(PathBuf::from(r"C:\Users\x")));
        assert_eq!(e.dir("E"), Some(PathBuf::from(r"\\server\share")));
        assert_eq!(e.dir("F"), None);
    }

    #[test]
    fn missing_home_is_an_error_not_a_panic() {
        for os in [Os::Windows, Os::MacOs, Os::Linux] {
            let err = AppPaths::resolve(os, env(&[])).expect_err("no home folder");
            assert!(
                matches!(err, PathError::MissingVariable(_)),
                "{os:?}: {err}"
            );
            assert!(err.to_string().contains("environment variable"));
        }
    }
}
