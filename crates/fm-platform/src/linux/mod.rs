//! Linux: the XDG Base Directory specification, and the symbolic-link
//! activator (ADR 0008), and the freedesktop.org trash (E8.3).
//!
//! The activator's logic builds on every Unix target, so its unit tests run
//! on the Linux and macOS runners against a fake fontconfig; only Linux uses it.

#[cfg(unix)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod activator;
#[cfg(unix)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod fontconfig;
pub(crate) mod paths;
#[cfg(target_os = "linux")]
mod trash;
mod tray;

#[cfg(target_os = "linux")]
pub use activator::LinuxActivator;
#[cfg(target_os = "linux")]
pub(crate) use trash::FreedesktopTrash;
#[cfg_attr(not(target_os = "linux"), allow(unused_imports))]
pub(crate) use tray::LinuxTray;
