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
