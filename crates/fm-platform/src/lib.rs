//! Font Manager platform layer.
//!
//! The one place with OS-specific code: the `FontActivator` trait with one
//! implementation per OS under `src/{windows,macos,linux}/`, the app-paths
//! helpers, the file-system calls that differ per OS (symbolic links, the
//! trash), the tray, the login entry, how the app was installed (updates)
//! and the IPC transport. Platform differences are
//! documented in an ADR, never hidden in `cfg` branches elsewhere.

pub mod activator;
pub mod autostart;
pub mod fs;
mod linux;
mod macos;
pub mod paths;
pub mod trash;
pub mod tray;
pub mod updater;
mod windows;

pub use activator::{
    ActivationError, ActivationErrorKind, Conflict, ConflictKind, FaceFile, FontActivator,
    MockActivator, Mode, Outcome, OutcomeStatus, Registration,
};
pub use autostart::{AUTOSTART_ARG, Autostart, MockAutostart};
#[cfg(target_os = "linux")]
pub use linux::{LinuxActivator, LinuxAutostart};
#[cfg(target_os = "macos")]
pub use macos::MacosAutostart;
#[cfg(target_os = "macos")]
pub use macos::{LEDGER_FILE, MacosActivator};
pub use paths::{AppPaths, Os, PathError};
pub use trash::{MockTrash, Trash, TrashError, TrashErrorKind, Trashed};
pub use tray::{MockTray, Tray, TrayKind, TraySupport};
pub use updater::Distribution;
#[cfg(windows)]
pub use windows::{FONTS_KEY, LEDGER_KEY, RUN_KEY, WindowsActivator, WindowsAutostart};
