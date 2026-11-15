//! Shows a source folder in the OS file manager (E50.13: the source card's
//! "Show in file manager"). Only an absolute path to an existing folder is
//! handed to the OS, so nothing is read as an option and no file is opened
//! with an app. Each OS builds its own command in
//! `src/{windows,macos,linux}/show_folder.rs`; the commands are plain data,
//! so their unit tests run on every CI runner without opening a window.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::{OpenCommand, Os};

/// Why a folder was not shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShowFolderError {
    /// The path is not absolute or is not an existing folder.
    NotAFolder(String),
    /// The OS command could not be started.
    Launch(String),
}

impl std::fmt::Display for ShowFolderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAFolder(path) => write!(f, "not an existing folder: {path:?}"),
            Self::Launch(detail) => write!(f, "could not start the file manager: {detail}"),
        }
    }
}

impl std::error::Error for ShowFolderError {}

/// The command that shows `folder` on `os`. The path must be absolute on
/// that OS (so it never starts with `-` and no opener reads it as an option);
/// [`show_folder`] also checks that it exists.
pub fn show_folder_command(os: Os, folder: &Path) -> Result<OpenCommand, ShowFolderError> {
    if !looks_absolute(os, folder) {
        return Err(ShowFolderError::NotAFolder(folder.display().to_string()));
    }
    Ok(match os {
        Os::Windows => crate::windows::show_folder_command(folder),
        Os::MacOs => crate::macos::show_folder_command(folder),
        Os::Linux => crate::linux::show_folder_command(folder),
    })
}

/// Absolute on `os`, judged from the text so the tests run on any host: a
/// drive or UNC path (`C:\`, `\\server\share`) on Windows, a path from `/`
/// elsewhere.
fn looks_absolute(os: Os, folder: &Path) -> bool {
    let s = folder.to_string_lossy();
    let b = s.as_bytes();
    match os {
        Os::Windows => {
            s.starts_with(r"\\")
                || (b.len() >= 3
                    && b[0].is_ascii_alphabetic()
                    && b[1] == b':'
                    && matches!(b[2], b'\\' | b'/'))
        }
        Os::MacOs | Os::Linux => s.starts_with('/'),
    }
}

/// Opens `folder` in the file manager (Explorer, Finder, the desktop's file
/// manager). Returns once the OS command started; a thread reaps it.
pub fn show_folder(folder: &Path) -> Result<(), ShowFolderError> {
    if !folder.is_absolute() || !folder.is_dir() {
        return Err(ShowFolderError::NotAFolder(folder.display().to_string()));
    }
    let cmd = show_folder_command(Os::current(), folder)?;
    let mut child = Command::new(&cmd.program)
        .args(&cmd.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| ShowFolderError::Launch(format!("{}: {e}", cmd.program.to_string_lossy())))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;

    fn strings(cmd: &OpenCommand) -> (String, Vec<String>) {
        (
            cmd.program.to_string_lossy().into_owned(),
            cmd.args
                .iter()
                .map(|a| a.to_string_lossy().into_owned())
                .collect(),
        )
    }

    #[test]
    fn windows_opens_the_folder_in_explorer() {
        let folder = Path::new(r"D:\Type\Old, unsorted & more");
        let cmd = show_folder_command(Os::Windows, folder).unwrap();
        assert_eq!(
            strings(&cmd),
            (
                "explorer.exe".into(),
                vec![r"D:\Type\Old, unsorted & more".into()]
            )
        );
        let unc = show_folder_command(Os::Windows, Path::new(r"\\studio\fonts")).unwrap();
        assert_eq!(strings(&unc).1, vec![r"\\studio\fonts".to_owned()]);
    }

    #[test]
    fn macos_runs_open_on_the_folder() {
        let cmd = show_folder_command(Os::MacOs, Path::new("/Users/me/Fonts")).unwrap();
        assert_eq!(
            strings(&cmd),
            ("/usr/bin/open".into(), vec!["/Users/me/Fonts".into()])
        );
    }

    #[test]
    fn linux_runs_xdg_open_on_the_folder() {
        let cmd = show_folder_command(Os::Linux, Path::new("/home/me/Fonts")).unwrap();
        assert_eq!(
            strings(&cmd),
            ("xdg-open".into(), vec!["/home/me/Fonts".into()])
        );
    }

    #[test]
    fn a_relative_path_or_an_option_is_refused_on_every_os() {
        for path in ["", "Fonts", "-a Terminal", "--help", "./x", "C:relative"] {
            for os in [Os::Windows, Os::MacOs, Os::Linux] {
                assert!(
                    matches!(
                        show_folder_command(os, Path::new(path)),
                        Err(ShowFolderError::NotAFolder(_))
                    ),
                    "{os:?} {path:?}"
                );
            }
        }
    }

    #[test]
    fn this_os_builds_its_own_command() {
        let dir = std::env::temp_dir();
        let expected = match Os::current() {
            Os::Windows => "explorer.exe",
            Os::MacOs => "/usr/bin/open",
            Os::Linux => "xdg-open",
        };
        let cmd = show_folder_command(Os::current(), &dir).unwrap();
        assert_eq!(cmd.program, OsString::from(expected));
        assert_eq!(cmd.args.last(), Some(&dir.clone().into_os_string()));
    }

    #[test]
    fn a_missing_folder_a_file_or_a_relative_path_is_not_shown() {
        let missing = std::env::temp_dir().join("fm-show-folder-not-there");
        assert!(matches!(
            show_folder(&missing),
            Err(ShowFolderError::NotAFolder(_))
        ));
        let file = std::env::temp_dir().join("fm-show-folder-a-file.txt");
        std::fs::write(&file, b"x").unwrap();
        assert!(matches!(
            show_folder(&file),
            Err(ShowFolderError::NotAFolder(_))
        ));
        let _ = std::fs::remove_file(&file);
        assert!(matches!(
            show_folder(Path::new("relative")),
            Err(ShowFolderError::NotAFolder(_))
        ));
    }
}
