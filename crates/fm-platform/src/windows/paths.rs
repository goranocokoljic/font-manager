//! App folders on Windows.
//!
//! | Folder  | Path |
//! |---------|------|
//! | config  | `%APPDATA%\FontManager` (roams with the profile) |
//! | data    | `%LOCALAPPDATA%\FontManager\data` |
//! | cache   | `%LOCALAPPDATA%\FontManager\cache` |
//! | logs    | `%LOCALAPPDATA%\FontManager\logs` |
//! | library | `%USERPROFILE%\Fonts` (not the OS per-user font folder) |

//!
//! The IPC pipe (ADR 0016) is `\\.\pipe\org.fontmanager.app-<%USERNAME%>`.

use crate::ipc::{IpcName, pipe_name};
use crate::paths::{APP_DIR, AppPaths, Env, LIBRARY_DIR, PathError};

pub(crate) fn resolve(env: &Env<'_>) -> Result<AppPaths, PathError> {
    let profile = env.required_dir("USERPROFILE")?;
    let roaming = env
        .dir("APPDATA")
        .unwrap_or_else(|| profile.join("AppData").join("Roaming"));
    let local = env
        .dir("LOCALAPPDATA")
        .unwrap_or_else(|| profile.join("AppData").join("Local"))
        .join(APP_DIR);
    Ok(AppPaths {
        config: roaming.join(APP_DIR),
        data: local.join("data"),
        cache: local.join("cache"),
        logs: local.join("logs"),
        library: profile.join(LIBRARY_DIR),
    })
}

pub(crate) fn ipc_name(env: &Env<'_>) -> IpcName {
    let user = (env.0)("USERNAME").unwrap_or_default();
    IpcName::Pipe(pipe_name(&user.to_string_lossy()))
}

#[cfg(test)]
mod tests {
    use crate::paths::test_env as env;
    use crate::{AppPaths, Os};
    use std::path::PathBuf;

    #[test]
    fn uses_known_folder_variables() {
        let p = AppPaths::resolve(
            Os::Windows,
            env(&[
                ("USERPROFILE", r"C:\Users\ada"),
                ("APPDATA", r"C:\Users\ada\AppData\Roaming"),
                ("LOCALAPPDATA", r"D:\Local"),
            ]),
        )
        .expect("resolves");
        let local = PathBuf::from(r"D:\Local").join("FontManager");
        assert_eq!(
            p.config,
            PathBuf::from(r"C:\Users\ada\AppData\Roaming").join("FontManager")
        );
        assert_eq!(p.data, local.join("data"));
        assert_eq!(p.cache, local.join("cache"));
        assert_eq!(p.logs, local.join("logs"));
        assert_eq!(p.library, PathBuf::from(r"C:\Users\ada").join("Fonts"));
    }

    #[test]
    fn falls_back_to_the_profile_when_appdata_is_unset() {
        let p = AppPaths::resolve(Os::Windows, env(&[("USERPROFILE", r"C:\Users\ada")]))
            .expect("resolves");
        let app_data = PathBuf::from(r"C:\Users\ada").join("AppData");
        assert_eq!(p.config, app_data.join("Roaming").join("FontManager"));
        assert_eq!(
            p.logs,
            app_data.join("Local").join("FontManager").join("logs")
        );
    }

    #[test]
    fn never_points_at_the_os_font_folders() {
        let p = AppPaths::resolve(
            Os::Windows,
            env(&[
                ("USERPROFILE", r"C:\Users\ada"),
                ("LOCALAPPDATA", r"C:\Users\ada\AppData\Local"),
            ]),
        )
        .expect("resolves");
        let lib = p
            .library
            .to_string_lossy()
            .to_lowercase()
            .replace('/', "\\");
        assert!(!lib.contains(r"microsoft\windows\fonts"));
        assert!(!lib.starts_with(r"c:\windows"));
    }

    #[test]
    fn the_ipc_pipe_is_named_after_the_user() {
        let paths = AppPaths::under(std::path::Path::new(r"C:\x"));
        let name = paths.ipc_name_for(Os::Windows, env(&[("USERNAME", "ada")]));
        assert_eq!(
            name,
            crate::IpcName::Pipe(r"\\.\pipe\org.fontmanager.app-ada".into())
        );
        let nobody = paths.ipc_name_for(Os::Windows, env(&[]));
        assert_eq!(
            nobody,
            crate::IpcName::Pipe(r"\\.\pipe\org.fontmanager.app-user".into())
        );
    }
}
