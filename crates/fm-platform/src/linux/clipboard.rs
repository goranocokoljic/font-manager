//! The Linux clipboard read (E54.2, ADR 0025): GTK's `CLIPBOARD` selection,
//! the one every app's Copy and Ctrl+V use (not the middle-click `PRIMARY`
//! selection). GTK speaks X11 selections and the Wayland data device alike.

use std::time::Duration;

use gtk::{gdk, glib};

use crate::clipboard::ClipboardError;

fn clipboard() -> Result<gtk::Clipboard, ClipboardError> {
    gtk::init().map_err(|e| ClipboardError::new(format!("GTK did not start: {e}")))?;
    Ok(gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD))
}

/// The text on the clipboard; empty when there is none. Runs GTK's loop until
/// the owner answers, so call it on the GTK thread.
pub(crate) fn read_text() -> Result<String, ClipboardError> {
    Ok(clipboard()?
        .wait_for_text()
        .map(|s| s.to_string())
        .unwrap_or_default())
}

/// Takes the clipboard with `text` as another app's Copy does and runs `read`
/// on a worker thread, running GTK's loop meanwhile to answer it.
pub(crate) fn offer<T: Send + 'static>(
    text: &str,
    read: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ClipboardError> {
    let clipboard = clipboard()?;
    clipboard.set_text(text);
    let ctx = glib::MainContext::default();
    let worker = std::thread::spawn(read);
    while !worker.is_finished() {
        if !ctx.iteration(false) {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    match worker.join() {
        Ok(value) => Ok(value),
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
