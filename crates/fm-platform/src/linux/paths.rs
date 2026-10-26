//! App folders on Linux, after the XDG Base Directory specification.
//!
//! | Folder  | Path |
//! |---------|------|
//! | config  | `$XDG_CONFIG_HOME/font-manager` (default `~/.config`) |
//! | data    | `$XDG_DATA_HOME/font-manager` (default `~/.local/share`) |
//! | cache   | `$XDG_CACHE_HOME/font-manager` (default `~/.cache`) |
//! | logs    | `$XDG_STATE_HOME/font-manager/logs` (default `~/.local/state`) |
//! | library | `~/Fonts` (not `~/.local/share/fonts`, which fontconfig activates) |
//!
//! Relative `XDG_*` values are ignored, as the specification requires.
//!
//! The IPC socket (ADR 0016) is `$XDG_RUNTIME_DIR/font-manager/fm.sock`, or
//! `fm.sock` in the data folder when `XDG_RUNTIME_DIR` is unset or relative.

use std::path::PathBuf;

use crate::ipc::{IpcName, SOCKET_FILE};
use crate::paths::{AppPaths, Env, LIBRARY_DIR, PathError, XDG_DIR};

pub(crate) fn resolve(env: &Env<'_>) -> Result<AppPaths, PathError> {
    let home = env.required_dir("HOME")?;
    let xdg = |var: &str, default: &[&str]| -> PathBuf {
        env.dir(var)
            .unwrap_or_else(|| default.iter().fold(home.clone(), |p, part| p.join(part)))
            .join(XDG_DIR)
    };
    Ok(AppPaths {
        config: xdg("XDG_CONFIG_HOME", &[".config"]),
        data: xdg("XDG_DATA_HOME", &[".local", "share"]),
        cache: xdg("XDG_CACHE_HOME", &[".cache"]),
        logs: xdg("XDG_STATE_HOME", &[".local", "state"]).join("logs"),
        library: home.join(LIBRARY_DIR),
    })
}

pub(crate) fn ipc_name(env: &Env<'_>, paths: &AppPaths) -> IpcName {
    let dir = env
        .dir("XDG_RUNTIME_DIR")
        .map_or_else(|| paths.data.clone(), |run| run.join(XDG_DIR));
    IpcName::Socket(dir.join(SOCKET_FILE))
}

#[cfg(test)]
mod tests {
    use crate::paths::test_env as env;
    use crate::{AppPaths, Os};
    use std::path::PathBuf;

    #[test]
    fn defaults_follow_the_xdg_spec() {
        let p = AppPaths::resolve(Os::Linux, env(&[("HOME", "/home/ada")])).expect("resolves");
        let home = PathBuf::from("/home/ada");
        let state = home.join(".local").join("state").join("font-manager");
        assert_eq!(p.config, home.join(".config").join("font-manager"));
        assert_eq!(
            p.data,
            home.join(".local").join("share").join("font-manager")
        );
        assert_eq!(p.cache, home.join(".cache").join("font-manager"));
        assert_eq!(p.logs, state.join("logs"));
        assert_eq!(p.library, home.join("Fonts"));
    }

    #[test]
    fn honours_absolute_xdg_variables_and_ignores_relative_ones() {
        let p = AppPaths::resolve(
            Os::Linux,
            env(&[
                ("HOME", "/home/ada"),
                ("XDG_CONFIG_HOME", "/cfg"),
                ("XDG_DATA_HOME", "/data"),
                ("XDG_CACHE_HOME", "relative/cache"),
                ("XDG_STATE_HOME", ""),
            ]),
        )
        .expect("resolves");
        let home = PathBuf::from("/home/ada");
        let state = home.join(".local").join("state").join("font-manager");
        assert_eq!(p.config, PathBuf::from("/cfg").join("font-manager"));
        assert_eq!(p.data, PathBuf::from("/data").join("font-manager"));
        assert_eq!(p.cache, home.join(".cache").join("font-manager"));
        assert_eq!(p.logs, state.join("logs"));
    }

    #[test]
    fn never_points_at_the_fontconfig_user_folders() {
        let p = AppPaths::resolve(Os::Linux, env(&[("HOME", "/home/ada")])).expect("resolves");
        let home = PathBuf::from("/home/ada");
        assert_ne!(p.library, home.join(".local").join("share").join("fonts"));
        assert_ne!(p.library, home.join(".fonts"));
    }

    #[test]
    fn the_ipc_socket_is_in_the_runtime_folder_or_the_data_folder() {
        let paths = AppPaths::resolve(Os::Linux, env(&[("HOME", "/home/ada")])).expect("resolves");
        let run = paths.ipc_name_for(
            Os::Linux,
            env(&[("HOME", "/home/ada"), ("XDG_RUNTIME_DIR", "/run/user/1000")]),
        );
        assert_eq!(
            run,
            crate::IpcName::Socket(
                PathBuf::from("/run/user/1000")
                    .join("font-manager")
                    .join("fm.sock")
            )
        );
        for vars in [&[("HOME", "/home/ada")][..], &[("XDG_RUNTIME_DIR", "run")]] {
            assert_eq!(
                paths.ipc_name_for(Os::Linux, env(vars)),
                crate::IpcName::Socket(paths.data.join("fm.sock")),
                "{vars:?}"
            );
        }
    }
}
