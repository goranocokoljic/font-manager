//! Windows: Known Folders through their environment variables, and the
//! per-user registry activator (ADR 0008), and the Recycle Bin (E8.3).

#[cfg(windows)]
#[allow(unsafe_code)] // Win32 registry and messaging FFI; every block has a SAFETY note.
mod activator;
#[cfg(windows)]
#[allow(unsafe_code)] // DirectWrite COM calls; every block has a SAFETY note.
mod dwrite;
pub(crate) mod paths;
#[cfg(windows)]
#[allow(unsafe_code)] // Win32 registry FFI; every block has a SAFETY note.
mod registry;
#[cfg(windows)]
#[allow(unsafe_code)] // Shell and security FFI; every block has a SAFETY note.
mod trash;

#[cfg(windows)]
pub use activator::{FONTS_KEY, LEDGER_KEY, WindowsActivator};
#[cfg(windows)]
pub(crate) use trash::RecycleBin;
