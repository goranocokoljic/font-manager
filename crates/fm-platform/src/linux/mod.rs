//! Linux: the XDG Base Directory specification, the symbolic-link activator
//! (ADR 0008), the freedesktop.org trash (E8.3), and the XDG autostart entry
//! that starts the app at login (ADR 0011).
//!
//! The activator's logic builds on every Unix target, so its unit tests run
//! on the Linux and macOS runners against a fake fontconfig; only Linux uses it.

#[cfg(unix)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod activator;
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod autostart;
#[cfg(unix)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod fontconfig;
mod open_url;
pub(crate) mod paths;
#[cfg(target_os = "linux")]
mod trash;
mod tray;

#[cfg(target_os = "linux")]
pub use activator::LinuxActivator;
#[cfg(target_os = "linux")]
pub use autostart::LinuxAutostart;
#[cfg_attr(not(target_os = "linux"), allow(unused_imports))]
pub(crate) use autostart::{FLATPAK_INFO, in_flatpak, system_entry};
pub(crate) use open_url::command as open_url_command;
#[cfg(target_os = "linux")]
pub(crate) use trash::FreedesktopTrash;
#[cfg_attr(not(target_os = "linux"), allow(unused_imports))]
pub(crate) use tray::LinuxTray;
