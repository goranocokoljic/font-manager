//! A delete to the Recycle Bin that can never become a permanent delete.
//!
//! `IFileOperation` with `FOFX_RECYCLEONDELETE` recycles a file, but when
//! the bin cannot take it (the bin is off for the drive, the file is larger
//! than the bin, a `subst` drive) Windows deletes it for good. Before each
//! delete the shell calls the progress sink's `PreDeleteItem` and says, with
//! `TSF_DELETE_RECYCLE_IF_POSSIBLE`, whether this delete goes to the bin. The
//! sink here aborts every delete that does not ([`may_delete`]), so nothing is
//! ever deleted for good and no prompt is shown. `PostDeleteItem` gives the
//! new item in the bin, which is where undo moves the file back from.
//!
//! The sink is a COM object written out by hand (a vtable and a reference
//! count): the `#[implement]` macro of the `windows` crate needs
//! `windows-core` as a direct dependency, which the spec's library table does
//! not list. The operation runs on its own thread in a single-threaded
//! apartment, as `IFileOperation` requires.

use std::ffi::{OsString, c_void};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use windows::Win32::Foundation::{E_ABORT, E_NOINTERFACE, E_POINTER, S_OK};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Shell::{
    FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FOFX_EARLYFAILURE,
    FOFX_RECYCLEONDELETE, FileOperation, IFileOperation, IFileOperationProgressSink,
    IFileOperationProgressSink_Vtbl, IShellItem, SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
    TSF_DELETE_RECYCLE_IF_POSSIBLE,
};
use windows::core::{GUID, HRESULT, IUnknown, IUnknown_Vtbl, Interface, PCWSTR};

use crate::trash::{TrashError, TrashErrorKind};

/// Whether a delete the shell is about to do may go ahead: only when it goes
/// to the Recycle Bin (`TSF_DELETE_RECYCLE_IF_POSSIBLE` in its flags).
pub(super) fn may_delete(flags: u32) -> bool {
    // The constant is a small positive flag; the cast keeps it.
    flags & (TSF_DELETE_RECYCLE_IF_POSSIBLE.0 as u32) != 0
}

/// What the sink saw.
#[derive(Debug, Default)]
struct Seen {
    /// A delete was aborted because it would not have gone to the bin.
    refused: bool,
    /// Where the file is in the bin (the `$R…` file).
    recycled: Option<PathBuf>,
}

/// Moves `path` to the Recycle Bin and returns where it went (`None` when
/// the shell did not say). Never deletes for good: a file the bin cannot
/// take is [`TrashErrorKind::NoTrash`] and stays where it is.
pub(super) fn recycle(path: &Path) -> Result<Option<PathBuf>, TrashError> {
    let path = path.to_path_buf();
    // A fresh thread, so the apartment is ours to choose and to end.
    std::thread::scope(|s| {
        s.spawn(|| recycle_on_sta(&path))
            .join()
            .unwrap_or_else(|_| {
                Err(TrashError::new(
                    TrashErrorKind::Failed,
                    "the Recycle Bin thread stopped",
                ))
            })
    })
}

fn recycle_on_sta(path: &Path) -> Result<Option<PathBuf>, TrashError> {
    let fail = |what: &str, e: windows::core::Error| {
        TrashError::new(
            TrashErrorKind::Failed,
            format!("{what} failed for {}: {e}", path.display()),
        )
    };
    // SAFETY: called once on this new thread, paired with CoUninitialize
    // below when it succeeded.
    let init = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
    if init.is_err() {
        return Err(fail("CoInitializeEx", init.into()));
    }
    let seen = Arc::new(Mutex::new(Seen::default()));
    let result = perform(path, &seen).map_err(|e| fail("IFileOperation", e));
    // SAFETY: every COM object of `perform` is dropped by now.
    unsafe { CoUninitialize() };
    let seen = std::mem::take(&mut *seen.lock().unwrap_or_else(PoisonError::into_inner));
    if seen.refused {
        return Err(TrashError::new(
            TrashErrorKind::NoTrash,
            format!(
                "{}: the Recycle Bin cannot take this file, so it was not moved",
                path.display()
            ),
        ));
    }
    match result? {
        true => Err(TrashError::new(
            TrashErrorKind::Denied,
            format!("the move of {} was cancelled", path.display()),
        )),
        false => Ok(seen.recycled),
    }
}

/// Runs the delete; `Ok(true)` when the shell reports it aborted.
fn perform(path: &Path, seen: &Arc<Mutex<Seen>>) -> windows::core::Result<bool> {
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: plain COM calls on interfaces this function owns; `wide` is
    // NUL-terminated and outlives the call that reads it; the sink lives
    // until the operation released it (reference counted).
    unsafe {
        let op: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL)?;
        op.SetOperationFlags(
            FOF_ALLOWUNDO
                | FOFX_RECYCLEONDELETE
                | FOF_NOCONFIRMATION
                | FOF_SILENT
                | FOF_NOERRORUI
                | FOFX_EARLYFAILURE,
        )?;
        let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)?;
        let sink = Sink::create(Arc::clone(seen));
        let cookie = op.Advise(&sink)?;
        op.DeleteItem(&item, None)?;
        let performed = op.PerformOperations();
        let _ = op.Unadvise(cookie);
        let refused = seen.lock().unwrap_or_else(PoisonError::into_inner).refused;
        if refused {
            // The abort is the expected way out; the caller reports it.
            return Ok(true);
        }
        performed?;
        Ok(op.GetAnyOperationsAborted()?.as_bool())
    }
}

/// The progress sink: a COM object with a reference count and what it saw.
#[repr(C)]
struct Sink {
    vtable: *const IFileOperationProgressSink_Vtbl,
    refs: AtomicU32,
    seen: Arc<Mutex<Seen>>,
}

impl Sink {
    /// A new sink, owned by the returned interface (one reference).
    fn create(seen: Arc<Mutex<Seen>>) -> IFileOperationProgressSink {
        let raw = Box::into_raw(Box::new(Sink {
            vtable: &VTABLE,
            refs: AtomicU32::new(1),
            seen,
        }));
        // SAFETY: `raw` is a live object whose first field is the vtable of
        // this interface; the interface takes over its one reference.
        unsafe { IFileOperationProgressSink::from_raw(raw.cast()) }
    }

    /// # Safety
    /// `this` is a pointer [`Sink::create`] made, still alive.
    unsafe fn get<'a>(this: *mut c_void) -> &'a Sink {
        // SAFETY: the caller's contract.
        unsafe { &*this.cast::<Sink>() }
    }

    fn seen(&self) -> std::sync::MutexGuard<'_, Seen> {
        self.seen.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

static VTABLE: IFileOperationProgressSink_Vtbl = IFileOperationProgressSink_Vtbl {
    base__: IUnknown_Vtbl {
        QueryInterface: query_interface,
        AddRef: add_ref,
        Release: release,
    },
    StartOperations: ok,
    FinishOperations: ok_finish,
    PreRenameItem: ok_pre_name,
    PostRenameItem: ok_post_rename,
    PreMoveItem: ok_pre_move,
    PostMoveItem: ok_post_move,
    PreCopyItem: ok_pre_move,
    PostCopyItem: ok_post_move,
    PreDeleteItem: pre_delete,
    PostDeleteItem: post_delete,
    PreNewItem: ok_pre_name,
    PostNewItem: ok_post_new,
    UpdateProgress: ok_progress,
    ResetTimer: ok,
    PauseTimer: ok,
    ResumeTimer: ok,
};

unsafe extern "system" fn query_interface(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() || iid.is_null() {
        return E_POINTER;
    }
    // SAFETY: both pointers were checked; COM passes a valid IID.
    unsafe {
        if *iid == IUnknown::IID || *iid == IFileOperationProgressSink::IID {
            add_ref(this);
            *out = this;
            S_OK
        } else {
            *out = std::ptr::null_mut();
            E_NOINTERFACE
        }
    }
}

unsafe extern "system" fn add_ref(this: *mut c_void) -> u32 {
    // SAFETY: COM calls AddRef on a live object only.
    unsafe { Sink::get(this) }
        .refs
        .fetch_add(1, Ordering::Relaxed)
        + 1
}

unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    // SAFETY: COM calls Release on a live object only.
    let left = unsafe { Sink::get(this) }
        .refs
        .fetch_sub(1, Ordering::AcqRel)
        - 1;
    if left == 0 {
        // SAFETY: the last reference is gone; the object came from Box::into_raw.
        drop(unsafe { Box::from_raw(this.cast::<Sink>()) });
    }
    left
}

unsafe extern "system" fn pre_delete(this: *mut c_void, flags: u32, _item: *mut c_void) -> HRESULT {
    if may_delete(flags) {
        return S_OK;
    }
    // SAFETY: the operation calls its sink while it holds a reference.
    unsafe { Sink::get(this) }.seen().refused = true;
    E_ABORT
}

unsafe extern "system" fn post_delete(
    this: *mut c_void,
    _flags: u32,
    _item: *mut c_void,
    result: HRESULT,
    created: *mut c_void,
) -> HRESULT {
    if result.is_err() || created.is_null() {
        return S_OK;
    }
    // SAFETY: `created` is a live IShellItem for the duration of the call;
    // it is borrowed, not released. The display name is freed below.
    let recycled = unsafe {
        IShellItem::from_raw_borrowed(&created).and_then(|item| {
            let name = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
            let units = name.as_wide().to_vec();
            CoTaskMemFree(Some(name.0.cast_const().cast()));
            Some(PathBuf::from(OsString::from_wide(&units)))
        })
    };
    // SAFETY: the operation calls its sink while it holds a reference.
    unsafe { Sink::get(this) }.seen().recycled = recycled;
    S_OK
}

unsafe extern "system" fn ok(_this: *mut c_void) -> HRESULT {
    S_OK
}

unsafe extern "system" fn ok_finish(_this: *mut c_void, _result: HRESULT) -> HRESULT {
    S_OK
}

unsafe extern "system" fn ok_pre_name(
    _this: *mut c_void,
    _flags: u32,
    _item: *mut c_void,
    _name: PCWSTR,
) -> HRESULT {
    S_OK
}

unsafe extern "system" fn ok_post_rename(
    _this: *mut c_void,
    _flags: u32,
    _item: *mut c_void,
    _name: PCWSTR,
    _result: HRESULT,
    _created: *mut c_void,
) -> HRESULT {
    S_OK
}

unsafe extern "system" fn ok_pre_move(
    _this: *mut c_void,
    _flags: u32,
    _item: *mut c_void,
    _folder: *mut c_void,
    _name: PCWSTR,
) -> HRESULT {
    S_OK
}

unsafe extern "system" fn ok_post_move(
    _this: *mut c_void,
    _flags: u32,
    _item: *mut c_void,
    _folder: *mut c_void,
    _name: PCWSTR,
    _result: HRESULT,
    _created: *mut c_void,
) -> HRESULT {
    S_OK
}

#[allow(clippy::too_many_arguments)] // the shape of the COM method
unsafe extern "system" fn ok_post_new(
    _this: *mut c_void,
    _flags: u32,
    _folder: *mut c_void,
    _name: PCWSTR,
    _template: PCWSTR,
    _attributes: u32,
    _result: HRESULT,
    _created: *mut c_void,
) -> HRESULT {
    S_OK
}

unsafe extern "system" fn ok_progress(_this: *mut c_void, _total: u32, _done: u32) -> HRESULT {
    S_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_delete_to_the_bin_may_go_ahead() {
        let recycle = TSF_DELETE_RECYCLE_IF_POSSIBLE.0 as u32;
        assert!(may_delete(recycle));
        assert!(may_delete(recycle | 0x1));
        // A permanent delete: the flag is missing.
        assert!(!may_delete(0));
        assert!(!may_delete(0x1 | 0x4));
    }

    #[test]
    fn the_sink_counts_its_references_and_answers_for_its_interfaces() {
        let sink = Sink::create(Arc::new(Mutex::new(Seen::default())));
        let unknown: IUnknown = sink.cast().expect("IUnknown");
        drop(unknown);
        assert!(sink.cast::<IShellItem>().is_err(), "no other interface");
        let raw = sink.as_raw();
        // SAFETY: `raw` is the live sink; one AddRef is undone by one Release.
        let after = unsafe {
            add_ref(raw);
            release(raw)
        };
        assert_eq!(after, 1, "the interface still holds its one reference");
    }

    #[test]
    fn a_permanent_delete_is_refused_and_recorded() {
        let seen = Arc::new(Mutex::new(Seen::default()));
        let sink = Sink::create(Arc::clone(&seen));
        // SAFETY: `sink` is live for both calls.
        let (refused, allowed) = unsafe {
            (
                pre_delete(sink.as_raw(), 0, std::ptr::null_mut()),
                pre_delete(
                    sink.as_raw(),
                    TSF_DELETE_RECYCLE_IF_POSSIBLE.0 as u32,
                    std::ptr::null_mut(),
                ),
            )
        };
        assert_eq!(refused, E_ABORT);
        assert_eq!(allowed, S_OK);
        assert!(seen.lock().expect("seen").refused);
    }
}
