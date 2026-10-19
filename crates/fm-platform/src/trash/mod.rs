//! The OS trash: move a file there, and put it back (E8.3).
//!
//! Rule 2: the app never deletes a user's file. Removing a copy means the
//! OS trash, and every move can be undone: [`Trash::trash`] returns where
//! the file went ([`Trashed`]), and [`Trash::restore`] moves it back.
//!
//! | OS | Trash | Where a file goes |
//! |----|-------|-------------------|
//! | Windows | Recycle Bin (`IFileOperation`, recycle only) | `<drive>\$Recycle.Bin\<SID>\$R…`, with its `$I…` record |
//! | macOS | `NSFileManager trashItemAtURL` | `~/.Trash` or the volume's `.Trashes/<uid>` |
//! | Linux | freedesktop.org Trash specification | `$XDG_DATA_HOME/Trash/files` or the volume's `.Trash-<uid>/files`, with a `.trashinfo` |
//!
//! Restoring is the same on every OS: the file is renamed back to its old
//! path and the OS record (`$I…`, `.trashinfo`) is removed. It is refused
//! when a file is at the old path again (never overwritten) or when the
//! file is no longer in the trash (the user emptied it).
//!
//! Windows deletes a file for good when the Recycle Bin cannot take it (no
//! bin on the drive, the bin turned off or too small); such a file is
//! refused before anything moves.

mod mock;

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use mock::MockTrash;

/// The trash of this OS. Any other OS is [`TrashErrorKind::Unsupported`].
pub fn system() -> Result<Arc<dyn Trash>, TrashError> {
    #[cfg(windows)]
    {
        Ok(Arc::new(crate::windows::RecycleBin))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(Arc::new(crate::macos::FinderTrash))
    }
    #[cfg(target_os = "linux")]
    {
        Ok(Arc::new(crate::linux::FreedesktopTrash::from_env()?))
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        Err(TrashError::new(
            TrashErrorKind::Unsupported,
            "the trash is implemented for Windows, macOS and Linux only",
        ))
    }
}

/// Moves files to the OS trash and back.
pub trait Trash: Send + Sync + fmt::Debug {
    /// Moves the file at `path` to the trash. Never deletes it: when the OS
    /// cannot keep it in a trash, the call fails and the file stays.
    fn trash(&self, path: &Path) -> Result<Trashed, TrashError>;

    /// Moves a file [`Trash::trash`] moved back to where it was.
    fn restore(&self, item: &Trashed) -> Result<(), TrashError> {
        restore_by_rename(item)
    }
}

/// Where [`Trash::trash`] put a file. The core stores it to undo the move.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Trashed {
    /// Where the file was.
    pub original: PathBuf,
    /// Where it is now, inside the trash.
    pub location: PathBuf,
    /// The OS record of the move (`$I…` on Windows, `.trashinfo` on Linux),
    /// removed when the file is put back. macOS keeps none the app can see.
    pub info: Option<PathBuf>,
}

/// Why a file could not be moved to the trash or back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrashErrorKind {
    /// The file is not there.
    NotFound,
    /// The file's drive or volume has no trash.
    NoTrash,
    /// Restore: a file is at the old path again.
    Occupied,
    /// Restore: the file is no longer in the trash.
    Emptied,
    /// The OS refused (permissions, the file is open).
    Denied,
    /// This OS has no trash support.
    Unsupported,
    /// Anything else.
    Failed,
}

/// A failed trash or restore: a kind and a developer detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashError {
    /// What went wrong.
    pub kind: TrashErrorKind,
    /// The underlying error, a path.
    pub detail: String,
}

impl TrashError {
    /// An error of `kind` with the developer detail `detail`.
    pub fn new(kind: TrashErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    /// An I/O error on `path`, its kind mapped.
    pub fn io(e: &io::Error, path: &Path) -> Self {
        let kind = match e.kind() {
            io::ErrorKind::NotFound => TrashErrorKind::NotFound,
            io::ErrorKind::PermissionDenied => TrashErrorKind::Denied,
            _ => TrashErrorKind::Failed,
        };
        Self::new(kind, format!("{}: {e}", path.display()))
    }

    /// One sentence for the user.
    pub fn message(&self) -> &'static str {
        match self.kind {
            TrashErrorKind::NotFound => "The file is not there any more.",
            TrashErrorKind::NoTrash => {
                "This drive has no trash, so the file was not moved. Remove it by hand if you want."
            }
            TrashErrorKind::Occupied => {
                "Another file is at this path now, so the copy was left in the trash."
            }
            TrashErrorKind::Emptied => "The file is no longer in the trash.",
            TrashErrorKind::Denied => "The system did not allow the move.",
            TrashErrorKind::Unsupported => "This system has no trash the app can use.",
            TrashErrorKind::Failed => "The file could not be moved.",
        }
    }
}

impl fmt::Display for TrashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.message(), self.detail)
    }
}

impl std::error::Error for TrashError {}

/// Puts a trashed file back by renaming it, then removes the OS record.
/// Refuses to overwrite a file at the old path.
pub(crate) fn restore_by_rename(item: &Trashed) -> Result<(), TrashError> {
    if fs::symlink_metadata(&item.original).is_ok() {
        return Err(TrashError::new(
            TrashErrorKind::Occupied,
            item.original.display().to_string(),
        ));
    }
    if fs::symlink_metadata(&item.location).is_err() {
        return Err(TrashError::new(
            TrashErrorKind::Emptied,
            item.location.display().to_string(),
        ));
    }
    fs::rename(&item.location, &item.original).map_err(|e| {
        let mut err = TrashError::io(&e, &item.original);
        if err.kind == TrashErrorKind::NotFound {
            // The folder the file was in is gone.
            err.kind = TrashErrorKind::Failed;
        }
        err
    })?;
    if let Some(info) = &item.info {
        // The file is back; a record left over only shows as a broken
        // entry in the trash, so a failure here is not the user's problem.
        let _ = fs::remove_file(info);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
