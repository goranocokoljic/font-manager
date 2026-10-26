//! Font Manager platform layer.
//!
//! The one place with OS-specific code: the `FontActivator` trait with one
//! implementation per OS under `src/{windows,macos,linux}/`, the app-paths
//! helpers, the file-system calls that differ per OS (symbolic links, the
//! trash), the tray, the login entry, the default browser, how the app was installed (updates),
//! the PDF printer and the IPC transport. Platform differences are
//! documented in an ADR, never hidden in `cfg` branches elsewhere.

pub mod activator;
pub mod autostart;
pub mod browser;
pub mod fs;
pub mod ipc;
mod linux;
mod macos;
pub mod mcp_shim;
pub mod paths;
pub mod pdf;
pub mod process;
pub mod trash;
pub mod tray;
pub mod updater;
mod windows;

pub use activator::{
    ActivationError, ActivationErrorKind, Conflict, ConflictKind, FaceFile, FontActivator,
    MockActivator, Mode, Outcome, OutcomeStatus, Registration,
};
pub use autostart::{AUTOSTART_ARG, Autostart, MockAutostart};
pub use browser::{OpenCommand, OpenUrlError, open_url};
pub use ipc::{IpcCloser, IpcListener, IpcName, IpcStream};
#[cfg(target_os = "linux")]
pub use linux::{LinuxActivator, LinuxAutostart};
#[cfg(target_os = "macos")]
pub use macos::MacosAutostart;
#[cfg(target_os = "macos")]
pub use macos::{LEDGER_FILE, MacosActivator};
pub use paths::{AppPaths, Os, PathError};
pub use pdf::{PdfPrinter, PrintError};
pub use trash::{MockTrash, Trash, TrashError, TrashErrorKind, Trashed};
pub use tray::{MockTray, Tray, TrayKind, TraySupport};
pub use updater::Distribution;
#[cfg(windows)]
pub use windows::{FONTS_KEY, LEDGER_KEY, RUN_KEY, WindowsActivator, WindowsAutostart};
