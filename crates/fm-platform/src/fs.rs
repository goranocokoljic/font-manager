//! File-system calls whose shape differs per OS.
//!
//! Symbolic links: Unix has one `symlink` call; Windows has one for folders
//! and one for files, and creating either may need Developer Mode or the
//! "Create symbolic links" privilege. Callers treat an error as "this host
//! cannot make links here", never as a bug.
//!
//! Path bytes: a path that is not valid UTF-8 is raw bytes on Unix and
//! UTF-16 code units (possibly unpaired surrogates) on Windows.
//! [`path_to_bytes`] and [`path_from_bytes`] turn any path into bytes and
//! back on the same OS, so the index can store it exactly.

use std::io;
use std::path::{Path, PathBuf};

/// Creates a symbolic link at `link` that points to the folder `original`.
pub fn symlink_dir(original: &Path, link: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(original, link)
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(original, link)
    }
}

/// Creates a symbolic link at `link` that points to the file `original`.
pub fn symlink_file(original: &Path, link: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(original, link)
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(original, link)
    }
}

/// Writes `contents` to `path` (replacing it) so that only the current user
/// can read or write it (ADR 0015): mode 0600 on macOS and Linux, a
/// protected DACL with the user's SID alone on Windows. The bytes go to a
/// temporary sibling that is private before they are written, then it is
/// renamed over `path`, so no other user ever sees them.
pub fn write_private_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    #[cfg(windows)]
    {
        crate::windows::private_file::write(path, contents)
    }
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let tmp = temp_sibling(path);
        let written = (|| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)?;
            // `mode` applies only when the file is new.
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
            file.write_all(contents)?;
            file.sync_all()?;
            std::fs::rename(&tmp, path)
        })();
        if written.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        written
    }
}

/// Whether only the current user can read `path`, as
/// [`write_private_file`] leaves it: no permission bits for group or others
/// on macOS and Linux, exactly the user's protected DACL on Windows.
pub fn is_private_file(path: &Path) -> io::Result<bool> {
    #[cfg(windows)]
    {
        crate::windows::private_file::is_private(path)
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok(std::fs::metadata(path)?.permissions().mode() & 0o077 == 0)
    }
}

/// `<name>.tmp` next to `path`.
pub(crate) fn temp_sibling(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// The bytes of `path` in this OS's native form: raw bytes on Unix,
/// little-endian UTF-16 code units on Windows. [`path_from_bytes`] reverses it.
pub fn path_to_bytes(path: &Path) -> Vec<u8> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect()
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    }
}

/// The path [`path_to_bytes`] made on this OS. `None` when `bytes` cannot be
/// one (an odd byte count on Windows).
pub fn path_from_bytes(bytes: &[u8]) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        let (units, rest) = bytes.as_chunks::<2>();
        if !rest.is_empty() {
            return None;
        }
        let wide: Vec<u16> = units.iter().map(|u| u16::from_le_bytes(*u)).collect();
        Some(PathBuf::from(std::ffi::OsString::from_wide(&wide)))
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Some(PathBuf::from(std::ffi::OsStr::from_bytes(bytes)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fm-private-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    #[test]
    fn a_private_file_is_readable_by_the_user_only() {
        let dir = scratch("new");
        let path = dir.join("endpoint.json");
        write_private_file(&path, b"{\"token\":\"secret\"}").expect("write");
        assert_eq!(
            std::fs::read(&path).expect("read"),
            b"{\"token\":\"secret\"}"
        );
        assert!(
            is_private_file(&path).expect("check"),
            "{path:?} is not private"
        );
        assert!(
            !temp_sibling(&path).exists(),
            "the temporary file is left behind"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).expect("meta").permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_readable_file_is_replaced_by_a_private_one() {
        let dir = scratch("replace");
        let path = dir.join("endpoint-token");
        std::fs::write(&path, b"old").expect("plain write");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        }
        // A file made the ordinary way inherits the folder's ACL (Windows)
        // or the umask (Unix): it is not private.
        assert!(!is_private_file(&path).expect("check"));
        write_private_file(&path, b"new").expect("write");
        assert_eq!(std::fs::read(&path).expect("read"), b"new");
        assert!(is_private_file(&path).expect("check"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_utf8_path_round_trips_through_bytes() {
        let path = Path::new("fonts/Schrift-ü/a.ttf");
        assert_eq!(path_from_bytes(&path_to_bytes(path)).as_deref(), Some(path));
    }

    /// The same bytes are a non-UTF-8 path on every OS: on Windows they are
    /// `A` and an unpaired surrogate, on Unix `A`, two NULs and a lone 0xD8.
    #[test]
    fn a_non_utf8_path_round_trips_through_bytes() {
        let path = path_from_bytes(&[0x41, 0x00, 0x00, 0xD8]).expect("path");
        assert!(path.to_str().is_none(), "{path:?} should not be UTF-8");
        assert_eq!(path_from_bytes(&path_to_bytes(&path)), Some(path));
    }

    #[test]
    fn bytes_that_are_no_path_on_this_os_are_refused_or_kept() {
        let odd = path_from_bytes(&[0x41, 0x00, 0x42]);
        if cfg!(windows) {
            assert_eq!(odd, None);
        } else {
            assert!(odd.is_some());
        }
    }
}
