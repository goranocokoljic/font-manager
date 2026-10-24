//! Windows: Known Folders through their environment variables, and the
//! per-user registry activator (ADR 0008), the Recycle Bin (E8.3), and the
//! `Run` value that starts the app at login (ADR 0011).

#[cfg(windows)]
#[allow(unsafe_code)] // Win32 registry and messaging FFI; every block has a SAFETY note.
mod activator;
#[cfg(windows)]
mod autostart;
#[cfg(windows)]
#[allow(unsafe_code)] // DirectWrite COM calls; every block has a SAFETY note.
mod dwrite;
#[cfg(windows)]
#[allow(unsafe_code)] // IFileOperation and a hand-written COM sink; every block has a SAFETY note.
mod file_op;
mod open_url;
pub(crate) mod paths;
#[cfg(windows)]
#[allow(unsafe_code)] // Win32 registry FFI; every block has a SAFETY note.
mod registry;
#[cfg(windows)]
#[allow(unsafe_code)] // Shell and security FFI; every block has a SAFETY note.
mod trash;
mod tray;

#[cfg(windows)]
pub use activator::{FONTS_KEY, LEDGER_KEY, WindowsActivator};
#[cfg(windows)]
pub use autostart::{RUN_KEY, WindowsAutostart};
pub(crate) use open_url::command as open_url_command;
#[cfg(windows)]
pub(crate) use trash::RecycleBin;
#[cfg_attr(not(windows), allow(unused_imports))]
pub(crate) use tray::WindowsTray;
