//! The Finder trash: `-[NSFileManager trashItemAtURL:resultingItemURL:error:]`.
//!
//! The call moves the file to the trash of its volume (`~/.Trash`, or
//! `.Trashes/<uid>` on another volume) and says where it went, so undo is a
//! rename back. It fails, and moves nothing, on a volume without a trash.
//!
//! Foundation is reached through the Objective-C runtime's C functions
//! (`objc_getClass`, `sel_registerName`, `objc_msgSend`); `NSURL` and
//! `NSError` are toll-free bridged with `CFURL` and `CFError`, so the
//! CoreFoundation C API builds and reads them.

use std::ffi::{OsStr, c_char, c_void};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr;

use crate::trash::{Trash, TrashError, TrashErrorKind, Trashed};

type Id = *mut c_void;
type Sel = *mut c_void;
type CFTypeRef = *const c_void;
type CFIndex = isize;

/// `PATH_MAX` on macOS.
const PATH_MAX: usize = 1024;
/// `NSFileNoSuchFileError`.
const NO_SUCH_FILE: CFIndex = 4;
/// `NSFileWriteNoPermissionError`.
const NO_PERMISSION: CFIndex = 513;
/// `NSFeatureUnsupportedError`: the volume has no trash.
const UNSUPPORTED: CFIndex = 3328;

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Sel;
    // Called through a cast to the exact signature of each message.
    fn objc_msgSend();
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}

// NSFileManager lives in Foundation; linking it makes the class known.
#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: CFTypeRef);
    fn CFURLCreateFromFileSystemRepresentation(
        allocator: CFTypeRef,
        buffer: *const u8,
        len: CFIndex,
        is_directory: u8,
    ) -> CFTypeRef;
    fn CFURLGetFileSystemRepresentation(
        url: CFTypeRef,
        resolve_against_base: u8,
        buffer: *mut u8,
        max_len: CFIndex,
    ) -> u8;
    fn CFErrorGetCode(err: CFTypeRef) -> CFIndex;
}

/// The Finder trash of the user running the app.
#[derive(Debug, Default)]
pub struct FinderTrash;

impl Trash for FinderTrash {
    fn trash(&self, path: &Path) -> Result<Trashed, TrashError> {
        let path = std::path::absolute(path).map_err(|e| TrashError::io(&e, path))?;
        std::fs::symlink_metadata(&path).map_err(|e| TrashError::io(&e, &path))?;
        let location = trash_item(&path)?;
        Ok(Trashed {
            original: path,
            location,
            info: None,
        })
    }
}

/// Sends `path` to the trash and returns where it went.
fn trash_item(path: &Path) -> Result<PathBuf, TrashError> {
    let bytes = path.as_os_str().as_bytes();
    let len = CFIndex::try_from(bytes.len())
        .map_err(|_| TrashError::new(TrashErrorKind::Failed, "path too long"))?;
    // SAFETY: `bytes` is valid for `len` bytes; the URL is released below.
    let url =
        unsafe { CFURLCreateFromFileSystemRepresentation(ptr::null(), bytes.as_ptr(), len, 0) };
    if url.is_null() {
        return Err(TrashError::new(
            TrashErrorKind::Failed,
            format!("no URL for {}", path.display()),
        ));
    }
    // SAFETY: the pool is popped once below; every autoreleased object of
    // the call (the resulting URL, the error) is read before that.
    let pool = unsafe { objc_autoreleasePoolPush() };
    let result = send_trash(url, path);
    // SAFETY: `pool` came from the push above; `url` was created above.
    unsafe {
        objc_autoreleasePoolPop(pool);
        CFRelease(url);
    }
    result
}

fn send_trash(url: CFTypeRef, path: &Path) -> Result<PathBuf, TrashError> {
    type DefaultManager = unsafe extern "C" fn(Id, Sel) -> Id;
    type TrashItem = unsafe extern "C" fn(Id, Sel, CFTypeRef, *mut Id, *mut Id) -> i8;
    // SAFETY: literal NUL-terminated names; the runtime returns the class
    // and selectors (registering a selector never fails).
    let (class, default_manager, trash_item) = unsafe {
        (
            objc_getClass(c"NSFileManager".as_ptr()),
            sel_registerName(c"defaultManager".as_ptr()),
            sel_registerName(c"trashItemAtURL:resultingItemURL:error:".as_ptr()),
        )
    };
    if class.is_null() {
        return Err(TrashError::new(
            TrashErrorKind::Unsupported,
            "NSFileManager is not available",
        ));
    }
    // SAFETY: `objc_msgSend` is called through the exact signature of each
    // message: `+defaultManager` takes no argument and returns an object;
    // `-trashItemAtURL:resultingItemURL:error:` takes an NSURL (a CFURL,
    // toll-free bridged) and two out pointers and returns a BOOL.
    let (manager, ok, resulting, error) = unsafe {
        let send0: DefaultManager = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        let send3: TrashItem = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        let manager = send0(class, default_manager);
        let mut resulting: Id = ptr::null_mut();
        let mut error: Id = ptr::null_mut();
        let ok = if manager.is_null() {
            0
        } else {
            send3(manager, trash_item, url, &mut resulting, &mut error)
        };
        (manager, ok, resulting, error)
    };
    if manager.is_null() {
        return Err(TrashError::new(
            TrashErrorKind::Unsupported,
            "no default NSFileManager",
        ));
    }
    if ok == 0 {
        // SAFETY: on failure `error` is an autoreleased NSError (a CFError)
        // or NULL; it is only read here, inside the pool.
        let code = if error.is_null() {
            0
        } else {
            unsafe { CFErrorGetCode(error.cast_const()) }
        };
        let kind = match code {
            NO_SUCH_FILE => TrashErrorKind::NotFound,
            NO_PERMISSION => TrashErrorKind::Denied,
            UNSUPPORTED => TrashErrorKind::NoTrash,
            _ => TrashErrorKind::Failed,
        };
        return Err(TrashError::new(
            kind,
            format!(
                "trashItemAtURL failed with code {code} for {}",
                path.display()
            ),
        ));
    }
    if resulting.is_null() {
        return Err(TrashError::new(
            TrashErrorKind::Failed,
            format!("the trash did not say where {} went", path.display()),
        ));
    }
    let mut buf = vec![0u8; PATH_MAX];
    // SAFETY: `resulting` is an autoreleased NSURL (a CFURL) read inside the
    // pool; `buf` is writable for `PATH_MAX` bytes.
    let got = unsafe {
        CFURLGetFileSystemRepresentation(
            resulting.cast_const(),
            1,
            buf.as_mut_ptr(),
            CFIndex::try_from(PATH_MAX).unwrap_or(CFIndex::MAX),
        )
    };
    if got == 0 {
        return Err(TrashError::new(
            TrashErrorKind::Failed,
            "the trash location is not a file path",
        ));
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Ok(PathBuf::from(OsStr::from_bytes(&buf[..end])))
}
