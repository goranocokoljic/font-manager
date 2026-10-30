//! Linux: the XDG Base Directory specification, the symbolic-link activator
//! (ADR 0008), the freedesktop.org trash (E8.3), the XDG autostart entry
//! that starts the app at login (ADR 0011), and the WebKitGTK PDF printer
//! (ADR 0013).
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
// Shared with macOS, which sets the socket's mode its own way.
#[cfg(unix)]
pub(crate) mod ipc;
#[cfg(target_os = "linux")]
#[allow(unsafe_code)] // malloc_trim; the block has a SAFETY note.
mod memory;
mod open_url;
pub(crate) mod paths;
#[cfg(target_os = "linux")]
mod pdf;
#[cfg(target_os = "linux")]
mod process;
#[cfg(target_os = "linux")]
mod trash;
mod tray;

#[cfg(target_os = "linux")]
pub use activator::LinuxActivator;
#[cfg(target_os = "linux")]
pub use autostart::LinuxAutostart;
#[cfg_attr(not(target_os = "linux"), allow(unused_imports))]
pub(crate) use autostart::{FLATPAK_INFO, in_flatpak, system_entry};
#[cfg(target_os = "linux")]
pub(crate) use memory::release_free_memory;
pub(crate) use open_url::command as open_url_command;
#[cfg(target_os = "linux")]
pub use pdf::LinuxPdfPrinter;
#[cfg(target_os = "linux")]
pub(crate) use pdf::run_ui_loop;
#[cfg(target_os = "linux")]
pub(crate) use process::is_running;
#[cfg(target_os = "linux")]
pub(crate) use trash::FreedesktopTrash;
#[cfg_attr(not(target_os = "linux"), allow(unused_imports))]
pub(crate) use tray::LinuxTray;
