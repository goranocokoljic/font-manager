//! Reading the text on the system clipboard (E54.2, ADR 0025), for the Paste
//! item of the text fields' own menu. The webview can write the clipboard (the
//! menu's Cut and Copy run the webview's own commands), but a page may not read
//! it without asking: WebView2 and WebKitGTK refuse `navigator.clipboard` reads
//! unless the window opted in, and WKWebView shows its own "Paste" bubble
//! first. So the app reads the clipboard natively, the same way on every OS,
//! and hands the text to the field. One implementation per OS:
//!
//! - Windows (`src/windows/clipboard.rs`): `CF_UNICODETEXT` through
//!   `OpenClipboard` / `GetClipboardData`.
//! - macOS (`src/macos/clipboard.rs`): the general pasteboard's
//!   `public.utf8-plain-text` (`NSPasteboardTypeString`).
//! - Linux (`src/linux/clipboard.rs`): GTK's `CLIPBOARD` selection (X11 and
//!   Wayland alike).
//!
//! [`read_text`] runs on the UI thread (GTK needs it there): the app calls it
//! from its main thread. Nothing is written; the clipboard stays as it was.

use std::fmt;

/// Why the clipboard could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardError(pub String);

impl ClipboardError {
    /// An error with this developer detail.
    pub fn new(detail: impl Into<String>) -> Self {
        Self(detail.into())
    }
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ClipboardError {}

/// The text on the system clipboard; empty when it holds no text (an image,
/// files, nothing). Call it on the UI thread.
pub fn read_text() -> Result<String, ClipboardError> {
    #[cfg(windows)]
    return crate::windows::clipboard::read_text();
    #[cfg(target_os = "macos")]
    return crate::macos::clipboard::read_text();
    #[cfg(target_os = "linux")]
    return crate::linux::clipboard::read_text();
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    Err(ClipboardError::new(
        "reading the clipboard is not supported on this system",
    ))
}

/// The cross-process check of [`read_text`] (E54.2): one process puts text on
/// the system clipboard the way another app's Copy does, a second process
/// reads it back. Tests only.
#[doc(hidden)]
pub mod probe {
    use super::ClipboardError;

    /// Puts `text` on the system clipboard and runs `read` (which starts the
    /// second process) while the text is offered. Call it from a test's
    /// thread (it runs the GTK loop on Linux, so only once per process there).
    pub fn offer<T: Send + 'static>(
        text: &str,
        read: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, ClipboardError> {
        #[cfg(windows)]
        return crate::windows::clipboard::offer(text, read);
        #[cfg(target_os = "macos")]
        return crate::macos::clipboard::offer(text, read);
        #[cfg(target_os = "linux")]
        return crate::linux::clipboard::offer(text, read);
        #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
        {
            let _ = (text, read);
            Err(ClipboardError::new(
                "writing the clipboard is not supported on this system",
            ))
        }
    }
}
