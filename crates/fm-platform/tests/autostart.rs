//! Start at login against the real OS (E17.2, E17.3, ADR 0011): the entry is
//! written where the OS looks for it at login, a second process (`reg` on
//! Windows, `plutil` on macOS, `desktop-file-validate` on Linux) reads it
//! back after enable, and finds it gone after disable. Each run uses its own
//! entry name (on Linux its own `XDG_CONFIG_HOME`), removed at the end.

// Helpers fail the test by panicking, which clippy's in-test allowance only
// sees inside `#[test]` functions.
#![allow(clippy::expect_used)]

#[cfg(windows)]
mod windows {
    use std::path::Path;
    use std::process::Command;

    use fm_platform::{Autostart, RUN_KEY, WindowsAutostart};

    /// What `reg query` prints for value `name` of the `Run` key; `None` when
    /// the value is not there.
    fn reg_query(name: &str) -> Option<String> {
        let out = Command::new("reg")
            .args(["query", &format!(r"HKCU\{RUN_KEY}"), "/v", name])
            .output()
            .expect("reg runs");
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Deletes the test value even when an assertion fails.
    struct Cleanup<'a>(&'a WindowsAutostart);

    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = self.0.disable();
        }
    }

    #[test]
    fn the_run_value_is_seen_by_another_process_and_removed_again() {
        let name = format!("org.fontmanager.test-autostart-{}", std::process::id());
        let program = Path::new(r"C:\Font Manager Test\fm-app.exe");
        let entry = WindowsAutostart::new(RUN_KEY, &name, program);
        let _cleanup = Cleanup(&entry);
        assert!(reg_query(&name).is_none());

        entry.enable().expect("enabled without admin rights");
        let seen = reg_query(&name).expect("reg sees the value");
        assert!(seen.contains("REG_SZ"), "{seen}");
        assert!(
            seen.contains(r#""C:\Font Manager Test\fm-app.exe" --autostart"#),
            "{seen}"
        );
        assert!(entry.is_enabled().expect("read"));

        assert!(entry.disable().expect("disabled"));
        assert!(reg_query(&name).is_none(), "the value is gone");
        assert!(!entry.is_enabled().expect("read"));
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use fm_platform::{Autostart, MacosAutostart};

    /// `plutil -extract <key> raw` on `file`, a second process reading the agent.
    fn plutil_extract(file: &Path, key: &str) -> Option<String> {
        let out = Command::new("plutil")
            .args(["-extract", key, "raw", "-o", "-"])
            .arg(file)
            .output()
            .expect("plutil runs");
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
    }

    /// Deletes the test agent even when an assertion fails.
    struct Cleanup<'a>(&'a MacosAutostart);

    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = self.0.disable();
        }
    }

    #[test]
    fn the_launch_agent_is_valid_seen_by_another_process_and_removed_again() {
        let home = std::env::var_os("HOME").expect("HOME");
        let agents = PathBuf::from(home).join("Library").join("LaunchAgents");
        let label = format!("org.fontmanager.test-autostart-{}", std::process::id());
        // A harmless program, should a failed run leave the agent behind.
        let entry =
            MacosAutostart::new(&agents, &label, Path::new("/usr/bin/true")).expect("entry");
        let _cleanup = Cleanup(&entry);
        let file = entry.file();
        assert!(!file.exists());

        entry.enable().expect("enabled without admin rights");
        let lint = Command::new("plutil")
            .arg("-lint")
            .arg(&file)
            .output()
            .expect("plutil");
        assert!(
            lint.status.success(),
            "{}",
            String::from_utf8_lossy(&lint.stdout)
        );
        assert_eq!(
            plutil_extract(&file, "Label").as_deref(),
            Some(label.as_str())
        );
        assert_eq!(
            plutil_extract(&file, "ProgramArguments.0").as_deref(),
            Some("/usr/bin/true")
        );
        assert_eq!(
            plutil_extract(&file, "ProgramArguments.1").as_deref(),
            Some("--autostart")
        );
        assert_eq!(plutil_extract(&file, "RunAtLoad").as_deref(), Some("true"));
        assert!(entry.is_enabled().expect("read"));

        assert!(entry.disable().expect("disabled"));
        assert!(!file.exists(), "the agent is gone");
        assert!(!entry.is_enabled().expect("read"));
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    const CHILD: &str = "FM_AUTOSTART_CHILD";
    const CHILD_TEST: &str = "linux::child_switches_the_system_entry";

    /// A fresh `XDG_CONFIG_HOME` under the temp dir, removed on drop.
    struct ConfigHome(PathBuf);

    impl Drop for ConfigHome {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Runs [`child_switches_the_system_entry`] in a new process of this test
    /// binary with `XDG_CONFIG_HOME` set to `config` and `what` to do.
    fn child(config: &Path, what: &str) {
        let out = Command::new(std::env::current_exe().expect("test binary"))
            .args([CHILD_TEST, "--exact", "--ignored", "--nocapture"])
            .env(CHILD, what)
            .env("XDG_CONFIG_HOME", config)
            .env_remove("FLATPAK_ID")
            .env_remove("APPIMAGE")
            .output()
            .expect("the child runs");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success() && stdout.contains("1 passed"),
            "{what}: {stdout}{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// `desktop-file-validate` on `file`; `None` when it is not installed and
    /// `FM_REQUIRE_DESKTOP_FILE_VALIDATE` (set in CI) does not ask for it.
    fn validate(file: &Path) -> Option<String> {
        match Command::new("desktop-file-validate").arg(file).output() {
            Ok(out) => {
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                assert!(out.status.success(), "{text}");
                Some(text)
            }
            Err(e) => {
                assert!(
                    std::env::var_os("FM_REQUIRE_DESKTOP_FILE_VALIDATE")
                        .is_none_or(|v| v.is_empty()),
                    "desktop-file-validate is required: {e}"
                );
                None
            }
        }
    }

    #[test]
    #[ignore = "run as a child process by the tests below"]
    fn child_switches_the_system_entry() {
        let entry = fm_platform::autostart::system();
        match std::env::var(CHILD).as_deref() {
            Ok("on") => entry.enable().expect("enabled without admin rights"),
            Ok("off") => assert!(entry.disable().expect("disabled")),
            Ok("off-none") => assert!(!entry.disable().expect("nothing of ours")),
            Ok("on-refused") => assert_eq!(
                entry.enable().expect_err("not ours").kind(),
                std::io::ErrorKind::AlreadyExists
            ),
            other => panic!("unknown {CHILD}: {other:?}"),
        }
    }

    fn config_home(label: &str) -> ConfigHome {
        let path =
            std::env::temp_dir().join(format!("fm-autostart-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("config home");
        ConfigHome(path)
    }

    #[test]
    fn the_xdg_entry_is_valid_launches_with_autostart_and_is_removed_alone() {
        let home = config_home("cycle");
        let autostart = home.0.join("autostart");
        let file = autostart.join("org.fontmanager.app.desktop");
        fs::create_dir_all(&autostart).expect("autostart folder");
        let other = autostart.join("org.example.other.desktop");
        fs::write(
            &other,
            "[Desktop Entry]
Type=Application
Name=Other
Exec=other
",
        )
        .expect("other entry");

        child(&home.0, "on");
        let text = fs::read_to_string(&file).expect("the entry is there");
        let program = std::env::current_exe().expect("test binary");
        assert!(
            text.contains(&format!(
                "
Exec=\"{}\" --autostart
",
                program.display()
            )),
            "{text}"
        );
        validate(&file);

        child(&home.0, "off");
        assert!(!file.exists(), "the entry is gone");
        assert!(other.exists(), "another entry stays");
        child(&home.0, "off-none");
    }

    #[test]
    fn a_file_the_user_wrote_is_never_overwritten_or_removed() {
        let home = config_home("foreign");
        let autostart = home.0.join("autostart");
        fs::create_dir_all(&autostart).expect("autostart folder");
        let file = autostart.join("org.fontmanager.app.desktop");
        let mine = "[Desktop Entry]
Type=Application
Name=Mine
Exec=fm-app --autostart
";
        fs::write(&file, mine).expect("the user's entry");

        child(&home.0, "on-refused");
        child(&home.0, "off-none");
        assert_eq!(fs::read_to_string(&file).expect("kept"), mine);
    }
}
