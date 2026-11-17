//! The Windows clipboard read (E54.2, ADR 0025): `CF_UNICODETEXT`, the text
//! every Windows app's Copy puts there (Windows converts `CF_TEXT` to it).

use std::time::Duration;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    SetClipboardData,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::Win32::System::Ole::CF_UNICODETEXT;

use crate::clipboard::ClipboardError;

fn format() -> u32 {
    u32::from(CF_UNICODETEXT.0)
}

/// Opens the clipboard, retrying while another app holds it (it is a lock).
fn open() -> Result<(), ClipboardError> {
    let mut last = None;
    for _ in 0..20 {
        // SAFETY: no owner window; `close` closes it on every path.
        match unsafe { OpenClipboard(None) } {
            Ok(()) => return Ok(()),
            Err(e) => last = Some(e),
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(ClipboardError::new(format!(
        "the clipboard is held by another app: {}",
        last.map(|e| e.to_string()).unwrap_or_default()
    )))
}

fn close() {
    // SAFETY: `open` opened the clipboard on this thread.
    let _ = unsafe { CloseClipboard() };
}

/// The text on the clipboard; empty when there is none.
pub(crate) fn read_text() -> Result<String, ClipboardError> {
    open()?;
    let text = read_open();
    close();
    text
}

fn read_open() -> Result<String, ClipboardError> {
    // SAFETY: the clipboard is open.
    if unsafe { IsClipboardFormatAvailable(format()) }.is_err() {
        return Ok(String::new());
    }
    // SAFETY: the clipboard is open; the handle belongs to it until it closes.
    let handle = unsafe { GetClipboardData(format()) }
        .map_err(|e| ClipboardError::new(format!("no text on the clipboard: {e}")))?;
    let global = HGLOBAL(handle.0);
    // SAFETY: a global memory handle of the open clipboard, locked and
    // unlocked here; the units read stay inside its size.
    unsafe {
        let units = GlobalSize(global) / 2;
        let data = GlobalLock(global).cast::<u16>();
        if data.is_null() {
            return Err(ClipboardError::new(
                "the clipboard text could not be locked",
            ));
        }
        let all = std::slice::from_raw_parts(data, units);
        let end = all.iter().position(|&u| u == 0).unwrap_or(all.len());
        let text = String::from_utf16_lossy(&all[..end]);
        let _ = GlobalUnlock(global);
        Ok(text)
    }
}

/// Puts `text` on the clipboard as another app's Copy does, then runs `read`.
pub(crate) fn offer<T: Send + 'static>(
    text: &str,
    read: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ClipboardError> {
    let mut units: Vec<u16> = text.encode_utf16().collect();
    units.push(0);
    open()?;
    let written = write_open(&units);
    close();
    written?;
    Ok(read())
}

fn write_open(units: &[u16]) -> Result<(), ClipboardError> {
    let failed = |what: &str, e: windows::core::Error| ClipboardError::new(format!("{what}: {e}"));
    // SAFETY: the clipboard is open; the memory is filled inside its size and
    // handed to the clipboard, which owns it from then on (freed here only
    // when the clipboard refused it).
    unsafe {
        EmptyClipboard().map_err(|e| failed("the clipboard could not be emptied", e))?;
        let global = GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of_val(units))
            .map_err(|e| failed("no memory for the clipboard text", e))?;
        let slot = GlobalLock(global).cast::<u16>();
        if slot.is_null() {
            let _ = GlobalFree(Some(global));
            return Err(ClipboardError::new(
                "the clipboard text could not be locked",
            ));
        }
        std::ptr::copy_nonoverlapping(units.as_ptr(), slot, units.len());
        let _ = GlobalUnlock(global);
        if let Err(e) = SetClipboardData(format(), Some(HANDLE(global.0))) {
            let _ = GlobalFree(Some(global));
            return Err(failed("the clipboard refused the text", e));
        }
    }
    Ok(())
}
