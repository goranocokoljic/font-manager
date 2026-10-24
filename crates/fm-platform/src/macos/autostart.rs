//! Start at login on macOS (ADR 0011): a per-user launch agent,
//! `~/Library/LaunchAgents/org.fontmanager.app.autostart.plist`, that runs
//! `<program> --autostart` once at login (`RunAtLoad`) in the user's GUI
//! session. macOS lists it under System Settings → General → Login Items.
//! The user's own folder, so no admin rights.
//!
//! The file is the state: `is_enabled` reads it back. `disable` deletes it
//! only when it is a launch agent this app wrote (its label and the
//! `--autostart` argument), never another file.
//!
//! The logic builds on every OS, so its unit tests run on every CI runner.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::autostart::{AUTOSTART_ARG, Autostart};

/// The label of the launch agent; the file is `<label>.plist`.
pub const AGENT_LABEL: &str = "org.fontmanager.app.autostart";

/// A launch agent `<label>.plist` in `dir` that launches `program`.
#[derive(Debug, Clone)]
pub struct MacosAutostart {
    dir: PathBuf,
    label: String,
    program: String,
}

impl MacosAutostart {
    /// The agent `label` in `dir` (normally `~/Library/LaunchAgents`) that
    /// launches `program` with `--autostart`. A program path that is not
    /// UTF-8 cannot be written into a property list.
    pub fn new(dir: &Path, label: &str, program: &Path) -> io::Result<Self> {
        let program = program.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("the program path is not UTF-8: {}", program.display()),
            )
        })?;
        Ok(Self {
            dir: dir.to_owned(),
            label: label.to_owned(),
            program: program.to_owned(),
        })
    }

    /// The app's agent in the user's `~/Library/LaunchAgents`.
    pub fn for_user(program: &Path) -> io::Result<Self> {
        let home = std::env::var_os("HOME")
            .filter(|h| !h.is_empty())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
        let dir = PathBuf::from(home).join("Library").join("LaunchAgents");
        Self::new(&dir, AGENT_LABEL, program)
    }

    /// The property-list file of the agent.
    pub fn file(&self) -> PathBuf {
        self.dir.join(format!("{}.plist", self.label))
    }

    /// The property list this entry writes.
    pub fn plist(&self) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{label}</string>
	<key>ProgramArguments</key>
	<array>
		<string>{program}</string>
		<string>{arg}</string>
	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>LimitLoadToSessionType</key>
	<string>Aqua</string>
	<key>ProcessType</key>
	<string>Interactive</string>
</dict>
</plist>
"#,
            label = escape(&self.label),
            program = escape(&self.program),
            arg = AUTOSTART_ARG,
        )
    }

    /// Whether `text` is a launch agent this app wrote: our label and the
    /// autostart argument.
    fn is_ours(&self, text: &str) -> bool {
        text.contains(&format!("<string>{}</string>", escape(&self.label)))
            && text.contains(&format!("<string>{AUTOSTART_ARG}</string>"))
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

/// `s` with the five XML special characters escaped.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

impl Autostart for MacosAutostart {
    fn enable(&self) -> io::Result<()> {
        if let Some(text) = self.read()?
            && !self.is_ours(&text)
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "{} exists and was not written by Font Manager",
                    self.file().display()
                ),
            ));
        }
        fs::create_dir_all(&self.dir)?;
        // Write a sibling file and rename it over, so the agent is never half written.
        let tmp = self.dir.join(format!(".{}.plist.tmp", self.label));
        fs::write(&tmp, self.plist())?;
        fs::rename(&tmp, self.file()).inspect_err(|_| {
            let _ = fs::remove_file(&tmp);
        })
    }

    fn disable(&self) -> io::Result<bool> {
        match self.read()? {
            Some(text) if self.is_ours(&text) => {
                fs::remove_file(self.file())?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn is_enabled(&self) -> io::Result<bool> {
        Ok(self.read()?.is_some_and(|text| text == self.plist()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn entry(dir: &Path, program: &str) -> MacosAutostart {
        MacosAutostart::new(dir, AGENT_LABEL, Path::new(program)).expect("entry")
    }

    #[test]
    fn the_plist_launches_the_program_with_the_argument_at_login() {
        let dir = TestDir::new("agent-plist");
        let plist = entry(
            dir.path(),
            "/Applications/Font & Co.app/Contents/MacOS/fm-app",
        )
        .plist();
        assert!(plist.contains("<string>org.fontmanager.app.autostart</string>"));
        assert!(plist.contains(
            "<string>/Applications/Font &amp; Co.app/Contents/MacOS/fm-app</string>\n\t\t<string>--autostart</string>"
        ));
        assert!(plist.contains("<key>RunAtLoad</key>\n\t<true/>"));
    }

    #[test]
    fn enable_and_disable_write_and_remove_the_agent() {
        let dir = TestDir::new("agent-cycle");
        let agents = dir.path().join("LaunchAgents");
        let entry = entry(
            &agents,
            "/Applications/Font Manager.app/Contents/MacOS/fm-app",
        );
        assert!(!entry.is_enabled().expect("read"));
        assert!(!entry.disable().expect("nothing yet"));
        entry.enable().expect("on");
        assert!(entry.is_enabled().expect("read"));
        assert_eq!(
            fs::read_to_string(entry.file()).expect("file"),
            entry.plist()
        );
        assert!(entry.disable().expect("off"));
        assert!(!entry.file().exists());
        assert!(!entry.is_enabled().expect("read"));
    }

    #[test]
    fn a_file_the_app_did_not_write_is_left_alone() {
        let dir = TestDir::new("agent-foreign");
        let entry = entry(
            dir.path(),
            "/Applications/Font Manager.app/Contents/MacOS/fm-app",
        );
        let foreign = "<plist><dict><key>Label</key><string>something.else</string></dict></plist>";
        fs::write(entry.file(), foreign).expect("seed");
        let other = dir.path().join("com.other.agent.plist");
        fs::write(&other, "other").expect("other");

        assert!(!entry.is_enabled().expect("read"));
        assert!(!entry.disable().expect("not ours"));
        let e = entry.enable().expect_err("never overwrite a foreign file");
        assert_eq!(e.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(entry.file()).expect("kept"), foreign);
        assert_eq!(fs::read_to_string(&other).expect("kept"), "other");
    }

    #[test]
    fn an_agent_for_another_copy_of_the_app_is_replaced_and_removable() {
        let dir = TestDir::new("agent-moved");
        let old = entry(dir.path(), "/Old/fm-app");
        old.enable().expect("old on");
        let new = entry(dir.path(), "/New/fm-app");
        assert!(!new.is_enabled().expect("read"));
        new.enable().expect("rewritten");
        assert!(new.is_enabled().expect("read"));
        assert!(new.disable().expect("off"));
    }
}
