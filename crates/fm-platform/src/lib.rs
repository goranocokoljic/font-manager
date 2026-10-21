//! Font Manager platform layer.
//!
//! The one place with OS-specific code: the `FontActivator` trait with one
//! implementation per OS under `src/{windows,macos,linux}/`, the app-paths
//! helpers, the file-system calls that differ per OS (symbolic links, the
//! trash), the tray and the IPC transport. Platform differences are
//! documented in an ADR, never hidden in `cfg` branches elsewhere.

pub mod activator;
pub mod fs;
mod linux;
mod macos;
pub mod paths;
pub mod trash;
pub mod tray;
mod windows;

pub use activator::{
    ActivationError, ActivationErrorKind, Conflict, ConflictKind, FaceFile, FontActivator,
    MockActivator, Mode, Outcome, OutcomeStatus, Registration,
};
#[cfg(target_os = "linux")]
pub use linux::LinuxActivator;
#[cfg(target_os = "macos")]
pub use macos::{LEDGER_FILE, MacosActivator};
pub use paths::{AppPaths, Os, PathError};
pub use trash::{MockTrash, Trash, TrashError, TrashErrorKind, Trashed};
pub use tray::{MockTray, Tray, TrayKind, TraySupport};
#[cfg(windows)]
pub use windows::{FONTS_KEY, LEDGER_KEY, WindowsActivator};
