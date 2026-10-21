//! Start at login on Linux (ADR 0011): an XDG autostart entry,
//! `$XDG_CONFIG_HOME/autostart/org.fontmanager.app.desktop` (default
//! `~/.config/autostart`), that runs `<program> --autostart` when the user
//! logs in to a desktop that follows the XDG Autostart specification. The
//! user's own folder, so no admin rights.
//!
//! The file is the state: `is_enabled` reads it back. A file this app wrote
//! carries [`MARKER_KEY`]; `disable` deletes only such a file, and `enable`
//! never overwrites a file without it. An entry of ours that already launches
//! this program is left as it is, so a key the user added (a delay, say) stays.
//!
//! Inside a Flatpak sandbox the entry is not written: the host session does
//! not read the sandbox's config folder, and the Background portal needs a
//! D-Bus client the library table does not list. The option is hidden there
//! and Settings → About → Platform notes says why.
//!
//! The logic builds on every OS, so its unit tests run on every CI runner.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::autostart::{AUTOSTART_ARG, Autostart};
use crate::paths::Env;

/// The key that marks an entry this app wrote.
pub const MARKER_KEY: &str = "X-FontManager-Autostart";

/// An autostart entry `<app_id>.desktop` in `dir` that launches `program`.
#[derive(Debug, Clone)]
pub struct LinuxAutostart {
    dir: PathBuf,
    app_id: String,
    /// The `Exec` value: the program, quoted, and the autostart argument.
    exec: String,
}

impl LinuxAutostart {
    /// The entry `app_id` in the autostart folder `dir` that launches
    /// `program` with `--autostart`. A program path that is not UTF-8 or has
    /// a control character cannot be written into a desktop entry.
    pub fn new(dir: &Path, app_id: &str, program: &Path) -> io::Result<Self> {
        let invalid = |why: &str| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("the program path {why}: {}", program.display()),
            )
        };
        let program = program.to_str().ok_or_else(|| invalid("is not UTF-8"))?;
        if program.chars().any(char::is_control) {
            return Err(invalid("has a control character"));
        }
        Ok(Self {
            dir: dir.to_owned(),
            app_id: app_id.to_owned(),
            exec: exec_value(program),
        })
    }

    /// The app's entry in the user's autostart folder:
    /// `$XDG_CONFIG_HOME/autostart`, else `$HOME/.config/autostart`.
    pub(crate) fn for_user(env: &Env<'_>, app_id: &str, program: &Path) -> io::Result<Self> {
        let config = match env.dir("XDG_CONFIG_HOME") {
            Some(config) => config,
            None => env
                .dir("HOME")
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?
                .join(".config"),
        };
        Self::new(&config.join("autostart"), app_id, program)
    }

    /// The desktop-entry file.
    pub fn file(&self) -> PathBuf {
        self.dir.join(format!("{}.desktop", self.app_id))
    }

    /// The desktop entry this app writes.
    pub fn desktop_entry(&self) -> String {
        format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Version=1.0\n\
             Name=Font Manager\n\
             Comment=Starts Font Manager in the tray when you log in\n\
             Exec={exec}\n\
             Terminal=false\n\
             NoDisplay=true\n\
             X-GNOME-Autostart-enabled=true\n\
             {MARKER_KEY}=true\n",
            exec = self.exec,
        )
    }

    fn read(&self) -> io::Result<Option<String>> {
        match fs::read_to_string(self.file()) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            // Not UTF-8: not a file this app wrote.
            Err(e) if e.kind() == io::ErrorKind::InvalidData => Ok(Some(String::new())),
            Err(e) => Err(e),
        }
    }
}

/// The `Exec` value that runs `program` with the autostart argument. The
/// program is always quoted; inside the quotes `"`, `` ` ``, `$` and `\` take
/// a backslash, and then the string escape doubles every backslash and `%`
/// becomes `%%` (Desktop Entry Specification, "The Exec key").
fn exec_value(program: &str) -> String {
    let mut quoted = String::with_capacity(program.len() + 2);
    quoted.push('"');
    for c in program.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    let mut value = String::with_capacity(quoted.len() + AUTOSTART_ARG.len() + 1);
    for c in quoted.chars() {
        match c {
            '\\' => value.push_str("\\\\"),
            '%' => value.push_str("%%"),
            c => value.push(c),
        }
    }
    value.push(' ');
    value.push_str(AUTOSTART_ARG);
    value
}

/// The value of `key` in the `[Desktop Entry]` group of `text`.
fn entry_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
        } else if in_entry
            && let Some((k, v)) = line.split_once('=')
            && k.trim_end() == key
        {
            return Some(v.trim_start());
        }
    }
    None
}

/// Whether `text` is an entry this app wrote.
fn is_ours(text: &str) -> bool {
    entry_value(text, MARKER_KEY) == Some("true")
}

impl Autostart for LinuxAutostart {
    fn enable(&self) -> io::Result<()> {
        if let Some(text) = self.read()? {
            if !is_ours(&text) {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "{} exists and was not written by Font Manager",
                        self.file().display()
                    ),
                ));
            }
            if self.is_enabled()? {
                // Ours and on: keep what the user added to it.
                return Ok(());
            }
        }
        fs::create_dir_all(&self.dir)?;
        // A sibling file without `.desktop`, which the session ignores, renamed
        // over, so the entry is never half written.
        let tmp = self.dir.join(format!(".{}.desktop.tmp", self.app_id));
        fs::write(&tmp, self.desktop_entry())?;
        fs::rename(&tmp, self.file()).inspect_err(|_| {
            let _ = fs::remove_file(&tmp);
        })
    }

    fn disable(&self) -> io::Result<bool> {
        match self.read()? {
            Some(text) if is_ours(&text) => {
                fs::remove_file(self.file())?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn is_enabled(&self) -> io::Result<bool> {
        Ok(self.read()?.is_some_and(|text| {
            is_ours(&text)
                && entry_value(&text, "Exec") == Some(self.exec.as_str())
                && entry_value(&text, "Hidden") != Some("true")
                && entry_value(&text, "X-GNOME-Autostart-enabled") != Some("false")
        }))
    }
}

/// The file every Flatpak sandbox has at its root.
pub(crate) const FLATPAK_INFO: &str = "/.flatpak-info";

/// The login entry of the running app, or why there is none: inside a
/// Flatpak (`flatpak_info`: [`FLATPAK_INFO`] exists) there is none; else
/// the user's autostart entry launching [`program`] of `current_exe`.
pub(crate) fn system_entry(
    env: &Env<'_>,
    flatpak_info: bool,
    current_exe: PathBuf,
) -> Result<LinuxAutostart, String> {
    if in_flatpak(env.0, flatpak_info) {
        return Err("start at login is not available inside the Flatpak sandbox".into());
    }
    let program = program(env, current_exe);
    LinuxAutostart::for_user(env, crate::paths::BUNDLE_ID, &program).map_err(|e| e.to_string())
}

/// Whether the app runs inside a Flatpak sandbox: `FLATPAK_ID` is set or
/// `/.flatpak-info` exists.
pub(crate) fn in_flatpak(env: &dyn Fn(&str) -> Option<OsString>, info_file: bool) -> bool {
    info_file || env("FLATPAK_ID").is_some_and(|id| !id.is_empty())
}

/// The program the entry launches: the AppImage file itself when the app runs
/// from one (`$APPIMAGE`), since `current_exe` is inside a mount that changes
/// at every start; else the running program.
pub(crate) fn program(env: &Env<'_>, current_exe: PathBuf) -> PathBuf {
    env.dir("APPIMAGE").unwrap_or(current_exe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::test_env;

    /// A fresh folder under the temp dir, removed on drop.
    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("fm-platform-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("temp dir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const APP_ID: &str = "org.fontmanager.app";

    fn entry(dir: &Path, program: &str) -> LinuxAutostart {
        LinuxAutostart::new(dir, APP_ID, Path::new(program)).expect("entry")
    }

    #[test]
    fn the_entry_launches_the_program_with_the_argument_at_login() {
        let dir = TestDir::new("xdg-entry");
        let text = entry(dir.path(), "/opt/Font Manager/fm-app").desktop_entry();
        assert!(text.starts_with("[Desktop Entry]\n"), "{text}");
        assert!(text.contains("\nType=Application\n"), "{text}");
        assert!(
            text.contains("\nExec=\"/opt/Font Manager/fm-app\" --autostart\n"),
            "{text}"
        );
        assert!(text.contains("\nX-FontManager-Autostart=true\n"), "{text}");
    }

    #[test]
    fn exec_quotes_and_escapes_reserved_characters() {
        assert_eq!(
            exec_value("/usr/bin/fm-app"),
            r#""/usr/bin/fm-app" --autostart"#
        );
        // `$` and `"` take one backslash, which the string escape doubles;
        // `%` is doubled for the field codes.
        assert_eq!(
            exec_value(r#"/opt/a$b"c\d 100%"#),
            r#""/opt/a\\$b\\"c\\\\d 100%%" --autostart"#
        );
        let dir = TestDir::new("xdg-control");
        let e = LinuxAutostart::new(dir.path(), APP_ID, Path::new("/opt/a\nb"))
            .expect_err("control character");
        assert_eq!(e.kind(), io::ErrorKind::InvalidInput);
    }

    #[cfg(unix)]
    #[test]
    fn a_program_path_that_is_not_utf8_is_refused() {
        use std::os::unix::ffi::OsStrExt;
        let dir = TestDir::new("xdg-utf8");
        let path = Path::new(std::ffi::OsStr::from_bytes(b"/opt/\xff/fm-app"));
        let e = LinuxAutostart::new(dir.path(), APP_ID, path).expect_err("not UTF-8");
        assert_eq!(e.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn the_folder_follows_xdg_config_home_and_defaults_to_dot_config() {
        let program = Path::new("/usr/bin/fm-app");
        let at = |vars: &[(&str, &str)]| {
            let get = test_env(vars);
            LinuxAutostart::for_user(&Env(&get), APP_ID, program).map(|e| e.file())
        };
        assert_eq!(
            at(&[("HOME", "/home/ada"), ("XDG_CONFIG_HOME", "/cfg")]).expect("xdg"),
            Path::new("/cfg/autostart/org.fontmanager.app.desktop")
        );
        assert_eq!(
            at(&[("HOME", "/home/ada"), ("XDG_CONFIG_HOME", "rel")]).expect("relative ignored"),
            Path::new("/home/ada/.config/autostart/org.fontmanager.app.desktop")
        );
        assert_eq!(
            at(&[]).expect_err("no home").kind(),
            io::ErrorKind::NotFound
        );
    }

    #[test]
    fn enable_and_disable_write_and_remove_only_that_file() {
        let dir = TestDir::new("xdg-cycle");
        let autostart = dir.path().join("autostart");
        let entry = entry(&autostart, "/usr/bin/fm-app");
        assert!(!entry.is_enabled().expect("read"));
        assert!(!entry.disable().expect("nothing yet"));
        entry.enable().expect("on");
        let other = autostart.join("other.desktop");
        fs::write(&other, "[Desktop Entry]\nType=Application\n").expect("other");
        assert!(entry.is_enabled().expect("read"));
        assert_eq!(
            fs::read_to_string(entry.file()).expect("file"),
            entry.desktop_entry()
        );
        assert!(entry.disable().expect("off"));
        assert!(!entry.file().exists());
        assert!(other.exists(), "another entry stays");
        assert!(autostart.is_dir(), "the folder stays");
        assert!(!entry.is_enabled().expect("read"));
    }

    #[test]
    fn a_file_the_app_did_not_write_is_never_overwritten_or_removed() {
        let dir = TestDir::new("xdg-foreign");
        let entry = entry(dir.path(), "/usr/bin/fm-app");
        // The user's own entry of the same name, even one that runs this program.
        let foreign = "[Desktop Entry]\nType=Application\nName=Mine\n\
                       Exec=\"/usr/bin/fm-app\" --autostart\n";
        fs::write(entry.file(), foreign).expect("seed");
        assert!(!entry.is_enabled().expect("read"));
        assert!(!entry.disable().expect("not ours"));
        let e = entry.enable().expect_err("never overwrite a foreign file");
        assert_eq!(e.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(entry.file()).expect("kept"), foreign);

        // A marker outside [Desktop Entry] does not make it ours.
        let other_group =
            "[Desktop Entry]\nType=Application\n[Other]\nX-FontManager-Autostart=true\n";
        fs::write(entry.file(), other_group).expect("seed");
        assert!(!entry.disable().expect("not ours"));
        assert!(entry.file().exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_file_that_is_not_utf8_is_not_ours() {
        let dir = TestDir::new("xdg-bytes");
        let entry = entry(dir.path(), "/usr/bin/fm-app");
        fs::write(entry.file(), b"[Desktop Entry]\n\xff\n").expect("seed");
        assert!(!entry.is_enabled().expect("read"));
        assert!(!entry.disable().expect("not ours"));
        assert!(entry.enable().is_err());
        assert!(entry.file().exists());
    }

    #[test]
    fn an_entry_of_ours_the_user_edited_is_kept_when_it_is_on() {
        let dir = TestDir::new("xdg-edited");
        let entry = entry(dir.path(), "/usr/bin/fm-app");
        entry.enable().expect("on");
        let edited = format!("{}X-GNOME-Autostart-Delay=10\n", entry.desktop_entry());
        fs::write(entry.file(), &edited).expect("edit");
        assert!(entry.is_enabled().expect("still on"));
        entry.enable().expect("on again");
        assert_eq!(fs::read_to_string(entry.file()).expect("kept"), edited);

        // Turned off in the desktop's startup settings: off here; turning it on
        // writes the entry again.
        let hidden = format!("{}Hidden=true\n", entry.desktop_entry());
        fs::write(entry.file(), hidden).expect("hide");
        assert!(!entry.is_enabled().expect("off"));
        entry.enable().expect("on");
        assert!(entry.is_enabled().expect("on"));
    }

    #[test]
    fn an_entry_for_another_copy_of_the_app_is_replaced_and_removable() {
        let dir = TestDir::new("xdg-moved");
        entry(dir.path(), "/old/fm-app").enable().expect("old on");
        let new = entry(dir.path(), "/new/fm-app");
        assert!(!new.is_enabled().expect("read"));
        new.enable().expect("rewritten");
        assert!(new.is_enabled().expect("read"));
        assert!(new.disable().expect("off"));
    }

    #[test]
    fn a_config_home_that_is_a_file_fails_and_writes_nothing() {
        let dir = TestDir::new("xdg-blocked");
        let blocker = dir.path().join("autostart");
        fs::write(&blocker, "not a folder").expect("seed");
        let entry = entry(&blocker, "/usr/bin/fm-app");
        assert!(entry.enable().is_err());
        assert_eq!(fs::read_to_string(&blocker).expect("kept"), "not a folder");
    }

    #[test]
    fn the_system_entry_is_none_inside_a_flatpak_and_the_xdg_entry_outside() {
        let exe = PathBuf::from("/tmp/.mount_fmXYZ/usr/bin/fm-app");
        let at = |vars: &[(&str, &str)], info: bool| {
            let get = test_env(vars);
            system_entry(&Env(&get), info, exe.clone())
        };
        let home = [("HOME", "/home/ada")];
        let e = at(&[("HOME", "/home/ada"), ("FLATPAK_ID", APP_ID)], false)
            .expect_err("no entry in a Flatpak");
        assert!(e.contains("Flatpak"), "{e}");
        assert!(at(&home, true).is_err(), "the info file alone is a Flatpak");

        let entry = at(
            &[
                ("HOME", "/home/ada"),
                ("XDG_CONFIG_HOME", "/cfg"),
                ("APPIMAGE", "/apps/fm.AppImage"),
            ],
            false,
        )
        .expect("an entry outside a Flatpak");
        assert_eq!(
            entry.file(),
            Path::new("/cfg/autostart/org.fontmanager.app.desktop")
        );
        assert!(
            entry.desktop_entry().contains(
                "
Exec=\"/apps/fm.AppImage\" --autostart
"
            ),
            "the AppImage file, not its mount"
        );
        let plain = at(&home, false).expect("entry");
        assert!(
            plain
                .desktop_entry()
                .contains(&format!("Exec=\"{}\"", exe.display()))
        );
    }

    #[test]
    fn flatpak_and_appimage_are_detected() {
        let none = test_env(&[]);
        assert!(!in_flatpak(&none, false));
        assert!(in_flatpak(&none, true));
        assert!(in_flatpak(&test_env(&[("FLATPAK_ID", APP_ID)]), false));
        assert!(!in_flatpak(&test_env(&[("FLATPAK_ID", "")]), false));

        let exe = PathBuf::from("/tmp/.mount_fmXYZ/usr/bin/fm-app");
        assert_eq!(program(&Env(&none), exe.clone()), exe);
        let appimage = test_env(&[("APPIMAGE", "/home/ada/Apps/Font_Manager.AppImage")]);
        assert_eq!(
            program(&Env(&appimage), exe),
            Path::new("/home/ada/Apps/Font_Manager.AppImage")
        );
    }
}
