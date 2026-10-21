//! Start at login against the real OS (E17.2, ADR 0011): the entry is
//! written where the OS looks for it at login, a second process (`reg` on
//! Windows, `plutil` on macOS) reads it back after enable, and finds it
//! gone after disable. Each run uses its own entry name, removed at the end.
//! Linux has no entry until E17.3.

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

#[cfg(not(any(windows, target_os = "macos")))]
#[test]
fn start_at_login_waits_for_e17_3_on_this_os() {
    let err = fm_platform::autostart::system()
        .is_enabled()
        .expect_err("not yet");
    assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
}
