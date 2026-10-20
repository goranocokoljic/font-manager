//! macOS: the `~/Library` folders, the CoreText activator (ADR 0008), and
//! the Finder trash (E8.3).
//!
//! The activator's logic and its ledger build on every OS, so their unit
//! tests run on every CI runner against a fake CoreText; only the CoreText
//! calls are macOS code.

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod activator;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)] // CoreText / CoreFoundation FFI; every block has a SAFETY note.
mod coretext;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod ledger;
pub(crate) mod paths;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)] // Objective-C runtime and CoreFoundation FFI; every block has a SAFETY note.
mod trash;

#[cfg(target_os = "macos")]
pub use activator::MacosActivator;
#[cfg(target_os = "macos")]
pub use ledger::LEDGER_FILE;
#[cfg(target_os = "macos")]
pub(crate) use trash::FinderTrash;
