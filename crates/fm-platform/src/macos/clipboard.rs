//! The macOS clipboard read (E54.2, ADR 0025): the general pasteboard's
//! `NSPasteboardTypeString` (`public.utf8-plain-text`), the text every Mac
//! app's Copy puts there. AppKit reads it directly, without WKWebView's
//! "Paste" bubble.

use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::NSString;

use crate::clipboard::ClipboardError;

/// The text on the general pasteboard; empty when there is none.
pub(crate) fn read_text() -> Result<String, ClipboardError> {
    let pasteboard = NSPasteboard::generalPasteboard();
    // SAFETY: a constant AppKit defines for the life of the process.
    let kind = unsafe { NSPasteboardTypeString };
    Ok(pasteboard
        .stringForType(kind)
        .map(|s| s.to_string())
        .unwrap_or_default())
}

/// Puts `text` on the general pasteboard as another app's Copy does, then
/// runs `read`.
pub(crate) fn offer<T: Send + 'static>(
    text: &str,
    read: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ClipboardError> {
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    // SAFETY: a constant AppKit defines for the life of the process.
    let kind = unsafe { NSPasteboardTypeString };
    if !pasteboard.setString_forType(&NSString::from_str(text), kind) {
        return Err(ClipboardError::new("the pasteboard refused the text"));
    }
    Ok(read())
}
