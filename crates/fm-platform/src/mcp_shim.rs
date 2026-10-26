//! How an MCP client starts the stdio shim (E27.7): the command a Claude
//! Desktop snippet names.
//!
//! The bundle installs `fm-mcp-stdio` beside the app's executable (Tauri
//! `externalBin`): `<install dir>\fm-mcp-stdio.exe` on Windows,
//! `Font Manager.app/Contents/MacOS/fm-mcp-stdio` on macOS, `usr/bin` on
//! Linux. Inside an AppImage that folder is a mount that exists only while
//! the app runs and changes its name at every start (as for the login entry,
//! ADR 0011), so the command is the AppImage file itself with [`SHIM_ARG`],
//! which makes the app run the shim instead of its window (ADR 0015).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::paths::{Env, Os};
use crate::updater::APPIMAGE_ENV;

/// The file name of the stdio shim without the executable suffix.
pub const SHIM_NAME: &str = "fm-mcp-stdio";

/// The argument that makes the app's own executable run the stdio shim and
/// exit, without a window: what the AppImage command passes.
pub const SHIM_ARG: &str = "--mcp-stdio";

/// A program and its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShimCommand {
    /// An absolute path that stays valid while the app is closed.
    pub program: PathBuf,
    /// The arguments; empty for the shim itself.
    pub args: Vec<String>,
}

/// The command that starts the shim for the app executable `exe` on `os`,
/// reading the environment through `env`; `None` when `exe` has no folder.
pub fn command(os: Os, exe: &Path, env: &dyn Fn(&str) -> Option<OsString>) -> Option<ShimCommand> {
    if os == Os::Linux
        && let Some(appimage) = Env(env).dir(APPIMAGE_ENV)
    {
        return Some(ShimCommand {
            program: appimage,
            args: vec![SHIM_ARG.to_owned()],
        });
    }
    let suffix = if os == Os::Windows { ".exe" } else { "" };
    Some(ShimCommand {
        program: exe.parent()?.join(format!("{SHIM_NAME}{suffix}")),
        args: Vec::new(),
    })
}

/// [`command`] for the running app on this OS.
pub fn current() -> Option<ShimCommand> {
    let exe = std::env::current_exe().ok()?;
    command(Os::current(), &exe, &|name| std::env::var_os(name))
}

/// Whether the app was started to be the shim: its first argument is
/// [`SHIM_ARG`].
pub fn is_shim_launch(args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>) -> bool {
    args.into_iter()
        .nth(1)
        .is_some_and(|a| a.as_ref() == SHIM_ARG)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::test_env;

    fn exe() -> PathBuf {
        PathBuf::from("/opt/Font Manager/fm-app")
    }

    #[test]
    fn the_shim_is_beside_the_executable_with_the_os_suffix() {
        let none = test_env(&[]);
        let windows = command(Os::Windows, &exe(), &none).expect("a folder");
        assert_eq!(
            windows.program,
            PathBuf::from("/opt/Font Manager/fm-mcp-stdio.exe")
        );
        assert!(windows.args.is_empty());
        let macos = command(Os::MacOs, &exe(), &none).expect("a folder");
        assert_eq!(
            macos.program,
            PathBuf::from("/opt/Font Manager/fm-mcp-stdio")
        );
        let linux = command(Os::Linux, &exe(), &none).expect("a folder");
        assert_eq!(
            linux.program,
            PathBuf::from("/opt/Font Manager/fm-mcp-stdio")
        );
        assert_eq!(command(Os::Linux, Path::new(""), &none), None);
    }

    #[test]
    fn inside_an_appimage_the_command_is_the_appimage_with_the_shim_argument() {
        // current_exe is inside the per-start mount; the AppImage file stays.
        let env = test_env(&[("APPIMAGE", "/home/ada/Apps/Font_Manager.AppImage")]);
        let mounted = PathBuf::from("/tmp/.mount_FontMaAbC123/usr/bin/fm-app");
        let linux = command(Os::Linux, &mounted, &env).expect("a command");
        assert_eq!(
            linux.program,
            PathBuf::from("/home/ada/Apps/Font_Manager.AppImage")
        );
        assert_eq!(linux.args, vec![SHIM_ARG.to_owned()]);
        assert!(!linux.program.starts_with("/tmp/.mount_FontMaAbC123"));
    }

    #[test]
    fn an_empty_or_relative_appimage_is_ignored_and_only_linux_reads_it() {
        let mounted = PathBuf::from("/tmp/.mount_x/usr/bin/fm-app");
        for value in ["", "Font_Manager.AppImage"] {
            let env = test_env(&[("APPIMAGE", value)]);
            let linux = command(Os::Linux, &mounted, &env).expect("a command");
            assert_eq!(
                linux.program,
                PathBuf::from("/tmp/.mount_x/usr/bin/fm-mcp-stdio")
            );
        }
        let env = test_env(&[("APPIMAGE", "/home/ada/Font_Manager.AppImage")]);
        let macos = command(Os::MacOs, &exe(), &env).expect("a command");
        assert!(macos.args.is_empty());
    }

    #[test]
    fn a_shim_launch_has_the_shim_argument_first() {
        assert!(is_shim_launch(["fm-app", SHIM_ARG]));
        assert!(!is_shim_launch(["fm-app"]));
        assert!(!is_shim_launch(["fm-app", "--autostart", SHIM_ARG]));
        assert_eq!(SHIM_ARG, "--mcp-stdio");
    }
}
