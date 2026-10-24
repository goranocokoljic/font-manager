//! macOS: the `~/Library` folders, the CoreText activator (ADR 0008), the
//! Finder trash (E8.3), the launch agent that starts the app at login
//! (ADR 0011), and the WKWebView PDF printer (ADR 0013).
//!
//! The activator's logic and its ledger build on every OS, so their unit
//! tests run on every CI runner against a fake CoreText; only the CoreText
//! calls are macOS code.

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod activator;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod autostart;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)] // CoreText / CoreFoundation FFI; every block has a SAFETY note.
mod coretext;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod ledger;
mod open_url;
pub(crate) mod paths;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)] // AppKit and WebKit FFI; every block has a SAFETY note.
mod pdf;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)] // Objective-C runtime and CoreFoundation FFI; every block has a SAFETY note.
mod trash;
mod tray;

#[cfg(target_os = "macos")]
pub use activator::MacosActivator;
#[cfg(target_os = "macos")]
pub use autostart::MacosAutostart;
#[cfg(target_os = "macos")]
pub use ledger::LEDGER_FILE;
pub(crate) use open_url::command as open_url_command;
#[cfg(target_os = "macos")]
pub use pdf::MacosPdfPrinter;
#[cfg(target_os = "macos")]
pub(crate) use pdf::run_ui_loop;
#[cfg(target_os = "macos")]
pub(crate) use trash::FinderTrash;
#[cfg_attr(not(target_os = "macos"), allow(unused_imports))]
pub(crate) use tray::MacosTray;
