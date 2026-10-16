//! File-system calls whose shape differs per OS.
//!
//! Symbolic links: Unix has one `symlink` call; Windows has one for folders
//! and one for files, and creating either may need Developer Mode or the
//! "Create symbolic links" privilege. Callers treat an error as "this host
//! cannot make links here", never as a bug.

use std::io;
use std::path::Path;

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
