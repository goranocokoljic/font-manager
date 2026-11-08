//! The Windows file drag (E44.4, ADR 0022). The shell builds the data object
//! for the files (`SHCreateShellItemArrayFromIDLists` → `BHID_DataObject`):
//! it carries `CF_HDROP` and the shell ID list Explorer prefers, and answers
//! the formats a drop target sets during the drag. "Preferred DropEffect" says
//! copy, and `SHDoDragDrop` allows only the copy operation, so Explorer and
//! every other target copy the files; the originals stay where they are.

use std::ffi::OsString;
use std::mem::ManuallyDrop;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::time::Duration;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::Com::{
    DVASPECT_CONTENT, FORMATETC, IDataObject, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, GetClipboardData, OpenClipboard, RegisterClipboardFormatW,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::{
    CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, IDropSource, OleFlushClipboard, OleInitialize,
    OleSetClipboard, OleUninitialize,
};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    BHID_DataObject, DragQueryFileW, HDROP, ILCreateFromPathW, ILFree, IShellItemArray,
    SHCreateShellItemArrayFromIDLists, SHDoDragDrop,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
use windows::core::{HSTRING, w};

use crate::drag::{DragError, DragSupport, FileDrag, Received, check_files};

/// The only operation the drag allows: the target copies the files.
pub(crate) const ALLOWED: DROPEFFECT = DROPEFFECT_COPY;

/// The channel name [`share`] reports: the clipboard has no names.
const CLIPBOARD: &str = "clipboard";

/// The Windows file drag.
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsFileDrag;

impl FileDrag for WindowsFileDrag {
    fn support(&self) -> DragSupport {
        DragSupport::Available
    }

    fn start(&self, files: &[PathBuf]) -> Result<(), DragError> {
        check_files(files)?;
        let _ole = Ole::init()?;
        let data = data_object(files)?;
        // SAFETY: no arguments; a null window is allowed and becomes `None`.
        let window = unsafe { GetForegroundWindow() };
        let window = (!window.is_invalid()).then_some(window);
        // SAFETY: `data` is a live data object; with no drop source the shell
        // uses its own, which drops on button-up and cancels on Esc. The call
        // runs the drag's modal loop on this (the UI) thread.
        unsafe { SHDoDragDrop(window, &data, None::<&IDropSource>, ALLOWED) }
            .map(|_| ())
            .map_err(|e| DragError::new(format!("the drag did not start: {e}")))
    }
}

/// OLE for this thread, released on drop when this call initialized it.
struct Ole(bool);

impl Ole {
    fn init() -> Result<Self, DragError> {
        // SAFETY: no reserved pointer; S_FALSE (already initialized) is Ok
        // and is balanced by the uninitialize below like S_OK.
        match unsafe { OleInitialize(None) } {
            Ok(()) => Ok(Self(true)),
            Err(e) => Err(DragError::new(format!(
                "OLE is not available on this thread: {e}"
            ))),
        }
    }
}

impl Drop for Ole {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances the successful OleInitialize of `init`.
            unsafe { OleUninitialize() };
        }
    }
}

/// The shell's data object for `files`, saying "copy" to the target.
pub(crate) fn data_object(files: &[PathBuf]) -> Result<IDataObject, DragError> {
    let mut pidls: Vec<*mut ITEMIDLIST> = Vec::with_capacity(files.len());
    let mut unknown = None;
    for file in files {
        let wide = HSTRING::from(file.as_os_str());
        // SAFETY: `wide` is a NUL-terminated wide string that outlives the call.
        let pidl = unsafe { ILCreateFromPathW(&wide) };
        if pidl.is_null() {
            unknown = Some(file);
            break;
        }
        pidls.push(pidl);
    }
    let built = match unknown {
        Some(file) => Err(DragError::new(format!(
            "{} cannot be dragged: the file was not found",
            file.display()
        ))),
        None => build(&pidls),
    };
    for pidl in pidls {
        // SAFETY: each pidl came from ILCreateFromPathW and is freed once.
        unsafe { ILFree(Some(pidl.cast_const())) };
    }
    built
}

fn build(pidls: &[*mut ITEMIDLIST]) -> Result<IDataObject, DragError> {
    let ids: Vec<*const ITEMIDLIST> = pidls.iter().map(|p| p.cast_const()).collect();
    // SAFETY: every entry is a live absolute ID list; the array copies them.
    let items: IShellItemArray = unsafe { SHCreateShellItemArrayFromIDLists(&ids) }
        .map_err(|e| DragError::new(format!("the shell could not list the files: {e}")))?;
    // SAFETY: a live item array; the data object handler takes no bind context.
    let data: IDataObject = unsafe { items.BindToHandler(None, &BHID_DataObject) }
        .map_err(|e| DragError::new(format!("the shell could not build the drag data: {e}")))?;
    set_preferred_copy(&data)?;
    Ok(data)
}

/// The clipboard format id of "Preferred DropEffect".
fn preferred_drop_effect() -> u16 {
    // SAFETY: a static NUL-terminated name.
    let id = unsafe { RegisterClipboardFormatW(w!("Preferred DropEffect")) };
    u16::try_from(id).unwrap_or_default()
}

/// Sets "Preferred DropEffect" on `data` to copy: Explorer then copies even
/// where it would move by default (the same drive).
fn set_preferred_copy(data: &IDataObject) -> Result<(), DragError> {
    let fail = |e: windows::core::Error| DragError::new(format!("the drag data refused copy: {e}"));
    let format = FORMATETC {
        cfFormat: preferred_drop_effect(),
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };
    // SAFETY: a movable block of one u32, written while locked; on success
    // the data object owns it (fRelease), on failure it is freed here.
    unsafe {
        let global = GlobalAlloc(GMEM_MOVEABLE, size_of::<u32>()).map_err(fail)?;
        let slot = GlobalLock(global).cast::<u32>();
        if slot.is_null() {
            let _ = GlobalFree(Some(global));
            return Err(DragError::new("the drag data refused copy: no memory"));
        }
        slot.write_unaligned(ALLOWED.0);
        let _ = GlobalUnlock(global);
        let medium = STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: global },
            pUnkForRelease: ManuallyDrop::new(None),
        };
        if let Err(e) = data.SetData(&format, &medium, true) {
            let _ = GlobalFree(Some(global));
            return Err(fail(e));
        }
    }
    Ok(())
}

/// Puts the drag's data object for `files` on the clipboard, renders it so it
/// outlives this process's object, and runs `read`.
pub(crate) fn share<T>(files: &[PathBuf], read: impl FnOnce(String) -> T) -> Result<T, DragError> {
    let _ole = Ole::init()?;
    let data = data_object(files)?;
    let mut last = None;
    for _ in 0..20 {
        // SAFETY: a live data object; the clipboard takes a reference.
        match unsafe { OleSetClipboard(&data) } {
            Ok(()) => {
                last = None;
                break;
            }
            Err(e) => {
                last = Some(e);
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    if let Some(e) = last {
        return Err(DragError::new(format!("the clipboard is busy: {e}")));
    }
    // SAFETY: renders every format of the object onto the clipboard.
    unsafe { OleFlushClipboard() }
        .map_err(|e| DragError::new(format!("the clipboard did not take the data: {e}")))?;
    Ok(read(CLIPBOARD.to_owned()))
}

/// Reads the files and the preferred drop effect from the clipboard.
pub(crate) fn receive(channel: &str) -> Result<Received, DragError> {
    if channel != CLIPBOARD {
        return Err(DragError::new(format!("unknown channel {channel}")));
    }
    open_clipboard()?;
    let read = read_clipboard();
    // SAFETY: the clipboard was opened above by this thread.
    let _ = unsafe { CloseClipboard() };
    read
}

fn open_clipboard() -> Result<(), DragError> {
    let mut last = None;
    for _ in 0..20 {
        // SAFETY: no owner window.
        match unsafe { OpenClipboard(None) } {
            Ok(()) => return Ok(()),
            Err(e) => last = Some(e),
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(DragError::new(format!(
        "the clipboard is busy: {}",
        last.map(|e| e.to_string()).unwrap_or_default()
    )))
}

fn read_clipboard() -> Result<Received, DragError> {
    // SAFETY: the clipboard is open; the handle stays owned by the clipboard.
    let handle = unsafe { GetClipboardData(u32::from(CF_HDROP.0)) }
        .map_err(|e| DragError::new(format!("no files on the clipboard: {e}")))?;
    let drop = HDROP(handle.0);
    // SAFETY: a valid HDROP; index u32::MAX asks for the count.
    let count = unsafe { DragQueryFileW(drop, u32::MAX, None) };
    let mut paths = Vec::new();
    for i in 0..count {
        // SAFETY: `i` is in range; a null buffer asks for the length.
        let len = unsafe { DragQueryFileW(drop, i, None) } as usize;
        let mut buf = vec![0u16; len + 1];
        // SAFETY: the buffer holds the name and its NUL.
        let got = unsafe { DragQueryFileW(drop, i, Some(&mut buf)) } as usize;
        paths.push(PathBuf::from(OsString::from_wide(&buf[..got.min(len)])));
    }
    Ok(Received {
        paths,
        copy: read_preferred_effect().map(|e| e == ALLOWED.0),
    })
}

fn read_preferred_effect() -> Option<u32> {
    // SAFETY: the clipboard is open; the block is only read while locked.
    unsafe {
        let handle: HANDLE = GetClipboardData(u32::from(preferred_drop_effect())).ok()?;
        let global = HGLOBAL(handle.0);
        let slot = GlobalLock(global).cast::<u32>();
        if slot.is_null() {
            return None;
        }
        let effect = slot.read_unaligned();
        let _ = GlobalUnlock(global);
        Some(effect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_copy_operation_is_allowed() {
        assert_eq!(ALLOWED, DROPEFFECT_COPY);
    }

    #[test]
    fn a_missing_file_is_refused_by_name() {
        let gone = std::env::temp_dir().join(format!("fm-drag-gone-{}\\a.ttf", std::process::id()));
        let _ole = Ole::init().expect("ole");
        let e = data_object(std::slice::from_ref(&gone)).expect_err("missing");
        assert!(e.0.contains(&gone.display().to_string()), "{e}");
    }
}
