//! Font Manager platform layer.
//!
//! The one place with OS-specific code: the `FontActivator` trait with one
//! implementation per OS under `src/{windows,macos,linux}/`, the app-paths
//! helpers, the file-system calls that differ per OS (symbolic links), the
//! tray and the IPC transport. Platform differences are
//! documented in an ADR, never hidden in `cfg` branches elsewhere.

pub mod activator;
pub mod fs;
mod linux;
mod macos;
pub mod paths;
mod windows;

pub use activator::{
    ActivationError, ActivationErrorKind, Conflict, ConflictKind, FaceFile, FontActivator,
    MockActivator, Mode, Outcome, OutcomeStatus, Registration,
};
pub use paths::{AppPaths, Os, PathError};
#[cfg(windows)]
pub use windows::{FONTS_KEY, LEDGER_KEY, WindowsActivator};
