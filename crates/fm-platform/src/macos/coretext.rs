//! CoreText and CoreFoundation calls: register and unregister a font file, and
//! the fonts other apps see. Plain `extern "C"` declarations of the documented
//! C API; every owned CF object is released by [`Cf`]'s `Drop`.

use std::collections::HashSet;
use std::ffi::{OsStr, c_char, c_void};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr;

use super::activator::{Registrar, RegistrarError};
use crate::activator::Mode;

type CFTypeRef = *const c_void;
type CFIndex = isize;
type CFTypeID = usize;
type CTFontManagerScope = u32;

/// `kCTFontManagerScopeUser`: the current user, across logins (== `.persistent`).
const SCOPE_USER: CTFontManagerScope = 2;
/// `kCTFontManagerScopeSession`: the current login session, every process in it.
const SCOPE_SESSION: CTFontManagerScope = 3;
const UTF8: u32 = 0x0800_0100; // kCFStringEncodingUTF8
/// `PATH_MAX` on macOS.
const PATH_MAX: usize = 1024;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: CFTypeRef);
    fn CFGetTypeID(cf: CFTypeRef) -> CFTypeID;
    fn CFStringGetTypeID() -> CFTypeID;
    fn CFURLGetTypeID() -> CFTypeID;
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
    fn CFErrorCopyDescription(err: CFTypeRef) -> CFTypeRef;
    fn CFArrayGetCount(array: CFTypeRef) -> CFIndex;
    fn CFArrayGetValueAtIndex(array: CFTypeRef, index: CFIndex) -> CFTypeRef;
    fn CFStringGetLength(s: CFTypeRef) -> CFIndex;
    fn CFStringGetMaximumSizeForEncoding(length: CFIndex, encoding: u32) -> CFIndex;
    fn CFStringGetCString(s: CFTypeRef, buffer: *mut c_char, size: CFIndex, encoding: u32) -> u8;
}

#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    static kCTFontNameAttribute: CFTypeRef;
    static kCTFontURLAttribute: CFTypeRef;
    fn CTFontManagerRegisterFontsForURL(
        url: CFTypeRef,
        scope: CTFontManagerScope,
        error: *mut CFTypeRef,
    ) -> bool;
    fn CTFontManagerUnregisterFontsForURL(
        url: CFTypeRef,
        scope: CTFontManagerScope,
        error: *mut CFTypeRef,
    ) -> bool;
    fn CTFontCollectionCreateFromAvailableFonts(options: CFTypeRef) -> CFTypeRef;
    fn CTFontCollectionCreateMatchingFontDescriptors(collection: CFTypeRef) -> CFTypeRef;
    fn CTFontDescriptorCopyAttribute(descriptor: CFTypeRef, attribute: CFTypeRef) -> CFTypeRef;
}

/// An owned CF object (from a `Create` or `Copy` call), released on drop.
struct Cf(CFTypeRef);

impl Cf {
    /// Takes ownership of `obj`; `None` for NULL. The `Cf` is built only for
    /// a non-NULL `obj`: a `Cf(NULL)` would reach `CFRelease(NULL)` on drop,
    /// which CoreFoundation answers with a trap.
    fn owned(obj: CFTypeRef) -> Option<Self> {
        if obj.is_null() { None } else { Some(Self(obj)) }
    }
}

impl Drop for Cf {
    fn drop(&mut self) {
        // SAFETY: `self.0` is a non-NULL object this process owns one
        // reference to (`Cf::owned` only wraps Create/Copy results).
        unsafe { CFRelease(self.0) }
    }
}

/// The file URL for `path`, or `None` when CoreFoundation refuses it.
fn file_url(path: &Path) -> Option<Cf> {
    let bytes = path.as_os_str().as_bytes();
    let len = CFIndex::try_from(bytes.len()).ok()?;
    // SAFETY: `bytes` is valid for `len` bytes through the call; a NULL
    // allocator is the default one.
    Cf::owned(unsafe {
        CFURLCreateFromFileSystemRepresentation(ptr::null(), bytes.as_ptr(), len, 0)
    })
}

/// The file-system path of a CFURL that CoreText handed out (borrowed).
fn url_path(url: CFTypeRef) -> Option<PathBuf> {
    let mut buffer = vec![0u8; PATH_MAX + 1];
    let max = CFIndex::try_from(buffer.len()).ok()?;
    // SAFETY: `url` is a live CFURL (type-checked by the caller); `buffer`
    // is writable for `max` bytes.
    let ok = unsafe { CFURLGetFileSystemRepresentation(url, 1, buffer.as_mut_ptr(), max) };
    if ok == 0 {
        return None;
    }
    let end = buffer.iter().position(|b| *b == 0)?;
    buffer.truncate(end);
    Some(PathBuf::from(OsStr::from_bytes(&buffer)))
}

/// A borrowed CFString as Rust text.
fn string(s: CFTypeRef) -> Option<String> {
    // SAFETY: `s` is a live CFString (type-checked or documented as one by
    // the caller); `buffer` is writable for `size` bytes.
    unsafe {
        let size = CFStringGetMaximumSizeForEncoding(CFStringGetLength(s), UTF8).checked_add(1)?;
        let mut buffer = vec![0u8; usize::try_from(size).ok()?];
        if CFStringGetCString(s, buffer.as_mut_ptr().cast(), size, UTF8) == 0 {
            return None;
        }
        let end = buffer.iter().position(|b| *b == 0)?;
        buffer.truncate(end);
        String::from_utf8(buffer).ok()
    }
}

/// The code and description of a CFError this process owns.
fn take_error(err: CFTypeRef, what: &str) -> RegistrarError {
    let Some(err) = Cf::owned(err) else {
        return RegistrarError {
            code: 0,
            detail: format!("{what} failed without an error"),
        };
    };
    // SAFETY: `err.0` is a live CFError; the description is a Copy result
    // owned by `Cf`.
    let (code, description) = unsafe {
        let code = CFErrorGetCode(err.0);
        let description = Cf::owned(CFErrorCopyDescription(err.0)).and_then(|d| string(d.0));
        (code, description)
    };
    RegistrarError {
        code,
        detail: format!("{what}: {}", description.unwrap_or_default()),
    }
}

fn scope(mode: Mode) -> CTFontManagerScope {
    match mode {
        Mode::Session => SCOPE_SESSION,
        Mode::Permanent => SCOPE_USER,
    }
}

type ManagerCall = unsafe extern "C" fn(CFTypeRef, CTFontManagerScope, *mut CFTypeRef) -> bool;

fn manager_call(
    call: ManagerCall,
    what: &str,
    path: &Path,
    mode: Mode,
) -> Result<(), RegistrarError> {
    let Some(url) = file_url(path) else {
        return Err(RegistrarError {
            code: super::activator::code::INVALID_FILE_PATH,
            detail: format!("{what}: not a file URL"),
        });
    };
    let mut err: CFTypeRef = ptr::null();
    // SAFETY: `url` is a live CFURL; `err` is a local out pointer that
    // receives an owned CFError or stays NULL.
    let ok = unsafe { call(url.0, scope(mode), &mut err) };
    if ok {
        // Success leaves `err` NULL; release one if CoreText set it anyway.
        drop(Cf::owned(err));
        Ok(())
    } else {
        Err(take_error(err, what))
    }
}

/// The CoreText registrar the app uses.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CoreText;

impl Registrar for CoreText {
    fn register(&self, path: &Path, mode: Mode) -> Result<(), RegistrarError> {
        manager_call(
            CTFontManagerRegisterFontsForURL,
            "CTFontManagerRegisterFontsForURL",
            path,
            mode,
        )
    }

    fn unregister(&self, path: &Path, mode: Mode) -> Result<(), RegistrarError> {
        manager_call(
            CTFontManagerUnregisterFontsForURL,
            "CTFontManagerUnregisterFontsForURL",
            path,
            mode,
        )
    }

    fn fonts_named(&self, names: &HashSet<&str>) -> Result<Vec<(String, PathBuf)>, String> {
        let mut found = Vec::new();
        // SAFETY: Create/Copy results are owned by `Cf`; array items and the
        // attribute keys are borrowed for the duration of the loop; every
        // attribute value is type-checked before it is read.
        unsafe {
            let collection = Cf::owned(CTFontCollectionCreateFromAvailableFonts(ptr::null()))
                .ok_or("no font collection")?;
            let Some(descriptors) =
                Cf::owned(CTFontCollectionCreateMatchingFontDescriptors(collection.0))
            else {
                return Ok(found);
            };
            for i in 0..CFArrayGetCount(descriptors.0) {
                let descriptor = CFArrayGetValueAtIndex(descriptors.0, i);
                let Some(name) = Cf::owned(CTFontDescriptorCopyAttribute(
                    descriptor,
                    kCTFontNameAttribute,
                )) else {
                    continue;
                };
                if CFGetTypeID(name.0) != CFStringGetTypeID() {
                    continue;
                }
                let Some(name) = string(name.0).filter(|n| names.contains(n.as_str())) else {
                    continue;
                };
                let Some(url) = Cf::owned(CTFontDescriptorCopyAttribute(
                    descriptor,
                    kCTFontURLAttribute,
                )) else {
                    continue;
                };
                if CFGetTypeID(url.0) != CFURLGetTypeID() {
                    continue;
                }
                if let Some(path) = url_path(url.0) {
                    found.push((name, path));
                }
            }
        }
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A NULL result must never be released: `CFRelease(NULL)` traps (it did,
    /// after every successful registration, before `Cf::owned` checked first).
    #[test]
    fn a_null_object_is_not_owned_and_not_released() {
        assert!(Cf::owned(ptr::null()).is_none());
    }

    #[test]
    fn a_path_round_trips_through_a_file_url() {
        let path = Path::new("/tmp/fm font/Schrift-ü.ttf");
        let url = file_url(path).expect("a file URL");
        assert_eq!(url_path(url.0).as_deref(), Some(path));
    }
}
