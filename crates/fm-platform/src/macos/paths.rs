//! App folders on macOS.
//!
//! | Folder  | Path |
//! |---------|------|
//! | config  | `~/Library/Application Support/org.fontmanager.app` |
//! | data    | `~/Library/Application Support/org.fontmanager.app` |
//! | cache   | `~/Library/Caches/org.fontmanager.app` |
//! | logs    | `~/Library/Logs/org.fontmanager.app` |
//! | library | `~/Fonts` (not `~/Library/Fonts`, which the OS activates) |
//!
//! The IPC socket (ADR 0016) is `fm.sock` in the data folder.

use crate::ipc::{IpcName, SOCKET_FILE};
use crate::paths::{AppPaths, BUNDLE_ID, Env, LIBRARY_DIR, PathError};

pub(crate) fn resolve(env: &Env<'_>) -> Result<AppPaths, PathError> {
    let home = env.required_dir("HOME")?;
    let library = home.join("Library");
    let support = library.join("Application Support").join(BUNDLE_ID);
    Ok(AppPaths {
        config: support.clone(),
        data: support,
        cache: library.join("Caches").join(BUNDLE_ID),
        logs: library.join("Logs").join(BUNDLE_ID),
        library: home.join(LIBRARY_DIR),
    })
}

/// The folders macOS reads fonts from: the system, local and network
/// libraries and the user's `~/Library/Fonts`.
pub(crate) fn font_folders(env: &Env<'_>) -> Vec<std::path::PathBuf> {
    let mut folders: Vec<std::path::PathBuf> = [
        "/System/Library/Fonts",
        "/Library/Fonts",
        "/Network/Library/Fonts",
    ]
    .into_iter()
    .map(std::path::PathBuf::from)
    .collect();
    folders.extend(env.dir("HOME").map(|h| h.join("Library").join("Fonts")));
    folders
}

pub(crate) fn ipc_name(paths: &AppPaths) -> IpcName {
    IpcName::Socket(paths.data.join(SOCKET_FILE))
}

#[cfg(test)]
mod tests {
    use crate::paths::test_env as env;
    use crate::{AppPaths, Os};
    use std::path::PathBuf;

    #[test]
    fn uses_the_library_folders() {
        let p = AppPaths::resolve(Os::MacOs, env(&[("HOME", "/Users/ada")])).expect("resolves");
        let lib = PathBuf::from("/Users/ada").join("Library");
        let support = lib.join("Application Support").join("org.fontmanager.app");
        assert_eq!(p.config, support);
        assert_eq!(p.data, support);
        assert_eq!(p.cache, lib.join("Caches").join("org.fontmanager.app"));
        assert_eq!(p.logs, lib.join("Logs").join("org.fontmanager.app"));
        assert_eq!(p.library, PathBuf::from("/Users/ada").join("Fonts"));
    }

    #[test]
    fn never_points_at_the_os_font_folders() {
        let p = AppPaths::resolve(Os::MacOs, env(&[("HOME", "/Users/ada")])).expect("resolves");
        assert_ne!(
            p.library,
            PathBuf::from("/Users/ada").join("Library").join("Fonts")
        );
    }

    #[test]
    fn the_ipc_socket_is_in_the_data_folder() {
        let paths = AppPaths::resolve(Os::MacOs, env(&[("HOME", "/Users/ada")])).expect("resolves");
        let name = paths.ipc_name_for(Os::MacOs, env(&[("XDG_RUNTIME_DIR", "/run/user/501")]));
        assert_eq!(name, crate::IpcName::Socket(paths.data.join("fm.sock")));
    }
}
