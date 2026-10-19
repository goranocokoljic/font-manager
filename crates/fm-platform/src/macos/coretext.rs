//! CoreText and CoreFoundation calls: register and unregister a font file, and
//! the fonts other apps see. Plain `extern "C"` declarations of the documented
//! C API; every owned CF object is released by [`Cf`]'s `Drop`.

use std::collections::HashSet;
use std::ffi::{OsStr, c_char, c_void};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr;

use super::activator::{Registrar, RegistrarError, code, settle_registered, settle_unregistered};
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
    static kCFTypeArrayCallBacks: CFArrayCallBacks;
    fn CFArrayCreate(
        allocator: CFTypeRef,
        values: *const CFTypeRef,
        count: CFIndex,
        callbacks: *const CFArrayCallBacks,
    ) -> CFTypeRef;
    fn CFEqual(a: CFTypeRef, b: CFTypeRef) -> u8;
    fn CFErrorCopyUserInfo(err: CFTypeRef) -> CFTypeRef;
    fn CFDictionaryGetValue(dict: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
}

/// `CFArrayCallBacks`; only its address is used (`kCFTypeArrayCallBacks`).
#[repr(C)]
struct CFArrayCallBacks {
    version: CFIndex,
    retain: *const c_void,
    release: *const c_void,
    copy_description: *const c_void,
    equal: *const c_void,
}

#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    static kCTFontNameAttribute: CFTypeRef;
    static kCTFontURLAttribute: CFTypeRef;
    static kCTFontManagerErrorFontURLsKey: CFTypeRef;
    // Deprecated since macOS 10.15 in favour of the block-based
    // `CTFontManagerRegisterFontURLs`, but still exported and synchronous,
    // with a per-URL error list: one call for a whole request.
    fn CTFontManagerRegisterFontsForURLs(
        urls: CFTypeRef,
        scope: CTFontManagerScope,
        errors: *mut CFTypeRef,
    ) -> bool;
    fn CTFontManagerUnregisterFontsForURLs(
        urls: CFTypeRef,
        scope: CTFontManagerScope,
        errors: *mut CFTypeRef,
    ) -> bool;
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
    fn CTFontManagerCopyAvailablePostScriptNames() -> CFTypeRef;
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

/// Whether any font the OS offers has one of `names` as its PostScript
/// name. `true` when the list cannot be read, so the caller looks closer.
fn any_available(names: &HashSet<&str>) -> bool {
    // SAFETY: the Copy result is owned by `Cf`; its items are borrowed for
    // the loop and type-checked before they are read.
    unsafe {
        let Some(all) = Cf::owned(CTFontManagerCopyAvailablePostScriptNames()) else {
            return true;
        };
        (0..CFArrayGetCount(all.0)).any(|i| {
            let name = CFArrayGetValueAtIndex(all.0, i);
            CFGetTypeID(name) == CFStringGetTypeID()
                && string(name).is_some_and(|n| names.contains(n.as_str()))
        })
    }
}

type BatchCall = unsafe extern "C" fn(CFTypeRef, CTFontManagerScope, *mut CFTypeRef) -> bool;

/// One CoreText call for every file of `paths`; one result per path. Each
/// error CoreText reports names the URLs it is about
/// (`kCTFontManagerErrorFontURLsKey`); an error that names none is given to
/// every path not yet failed as `code::UNSETTLED`, so a failure is never
/// reported as a success and the caller asks again per file
/// (`settle_registered`, `settle_unregistered`).
fn batch_call(
    call: BatchCall,
    what: &str,
    paths: &[&Path],
    mode: Mode,
) -> Vec<Result<(), RegistrarError>> {
    let mut results: Vec<Result<(), RegistrarError>> = Vec::with_capacity(paths.len());
    let mut urls: Vec<(usize, Cf)> = Vec::with_capacity(paths.len());
    for (i, path) in paths.iter().enumerate() {
        match file_url(path) {
            Some(url) => {
                urls.push((i, url));
                results.push(Ok(()));
            }
            None => results.push(Err(RegistrarError {
                code: super::activator::code::INVALID_FILE_PATH,
                detail: format!("{what}: not a file URL"),
            })),
        }
    }
    if urls.is_empty() {
        return results;
    }
    let values: Vec<CFTypeRef> = urls.iter().map(|(_, u)| u.0).collect();
    let Ok(count) = CFIndex::try_from(values.len()) else {
        return results;
    };
    // SAFETY: `values` holds live CFURLs for the call; the array retains
    // them with the standard callbacks and is owned by `Cf`. `errors` is a
    // local out pointer that receives an owned CFArray of CFErrors or stays
    // NULL; its items and their user-info values are borrowed while it lives.
    unsafe {
        let Some(array) = Cf::owned(CFArrayCreate(
            ptr::null(),
            values.as_ptr(),
            count,
            &raw const kCFTypeArrayCallBacks,
        )) else {
            return results;
        };
        let mut errors: CFTypeRef = ptr::null();
        let ok = call(array.0, scope(mode), &mut errors);
        let errors = Cf::owned(errors);
        let Some(errors) = errors else {
            if !ok {
                for (i, _) in &urls {
                    results[*i] = Err(RegistrarError {
                        code: code::UNSETTLED,
                        detail: format!("{what} failed without an error"),
                    });
                }
            }
            return results;
        };
        for e in 0..CFArrayGetCount(errors.0) {
            let err = CFArrayGetValueAtIndex(errors.0, e);
            let code = CFErrorGetCode(err);
            let description = Cf::owned(CFErrorCopyDescription(err)).and_then(|d| string(d.0));
            let failure = RegistrarError {
                code,
                detail: format!("{what}: {}", description.unwrap_or_default()),
            };
            let info = Cf::owned(CFErrorCopyUserInfo(err));
            let named = info
                .as_ref()
                .map(|d| CFDictionaryGetValue(d.0, kCTFontManagerErrorFontURLsKey))
                .filter(|v| !v.is_null());
            let mut hit = false;
            if let Some(named) = named {
                for n in 0..CFArrayGetCount(named) {
                    let url = CFArrayGetValueAtIndex(named, n);
                    for (i, ours) in &urls {
                        if CFEqual(url, ours.0) != 0 {
                            results[*i] = Err(failure.clone());
                            hit = true;
                        }
                    }
                }
            }
            if !hit {
                let unsettled = RegistrarError {
                    code: code::UNSETTLED,
                    detail: format!("{} (CoreText error {code} names no file)", failure.detail),
                };
                for (i, _) in &urls {
                    if results[*i].is_ok() {
                        results[*i] = Err(unsettled.clone());
                    }
                }
            }
        }
    }
    results
}

fn register_urls(paths: &[&Path], mode: Mode) -> Vec<Result<(), RegistrarError>> {
    batch_call(
        CTFontManagerRegisterFontsForURLs,
        "CTFontManagerRegisterFontsForURLs",
        paths,
        mode,
    )
}

/// Files per parallel registration call: at most [`MAX_WORKERS`] calls and at
/// least [`MIN_PER_CALL`] files each, so a small request stays one call.
fn chunk_sizes(files: usize, workers: usize) -> Vec<usize> {
    let calls = workers
        .clamp(1, MAX_WORKERS)
        .min(files / MIN_PER_CALL)
        .max(1);
    let (base, extra) = (files / calls, files % calls);
    (0..calls)
        .map(|i| base + usize::from(i < extra))
        .filter(|&n| n > 0)
        .collect()
}

/// More parallel calls than this gained nothing on the timing spike.
const MAX_WORKERS: usize = 8;
const MIN_PER_CALL: usize = 8;

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

    fn register_all(&self, paths: &[&Path], mode: Mode) -> Vec<Result<(), RegistrarError>> {
        // fontd reads every file it registers, one after another within a
        // call: 100 files took 0.7–1.9 s in one call on a 3-core CI Mac and
        // about half that in parallel calls (E6.6 timing spike).
        let workers = std::thread::available_parallelism().map_or(1, usize::from);
        let parts = chunk_sizes(paths.len(), workers);
        if parts.len() < 2 {
            return settle_registered(self, paths, mode, register_urls(paths, mode));
        }
        let mut slices = Vec::with_capacity(parts.len());
        let mut rest = paths;
        for size in parts {
            let (part, tail) = rest.split_at(size);
            slices.push(part);
            rest = tail;
        }
        let results = std::thread::scope(|s| {
            let handles: Vec<_> = slices
                .iter()
                .map(|part| (part.len(), s.spawn(move || register_urls(part, mode))))
                .collect();
            handles
                .into_iter()
                .flat_map(|(len, handle)| {
                    handle.join().unwrap_or_else(|_| {
                        vec![
                            Err(RegistrarError {
                                code: code::UNSETTLED,
                                detail: "CTFontManagerRegisterFontsForURLs: worker panicked"
                                    .to_owned(),
                            });
                            len
                        ]
                    })
                })
                .collect()
        });
        settle_registered(self, paths, mode, results)
    }

    fn unregister_all(&self, paths: &[&Path], mode: Mode) -> Vec<Result<(), RegistrarError>> {
        let results = batch_call(
            CTFontManagerUnregisterFontsForURLs,
            "CTFontManagerUnregisterFontsForURLs",
            paths,
            mode,
        );
        settle_unregistered(self, paths, mode, results)
    }

    fn fonts_named(&self, names: &HashSet<&str>) -> Result<Vec<(String, PathBuf)>, String> {
        let mut found = Vec::new();
        // Most activations clash with nothing: the list of names alone says
        // so without a descriptor per installed font, which took most of the
        // 100-face activation budget.
        if !any_available(names) {
            return Ok(found);
        }
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

    /// Criterion 1: session mode is `kCTFontManagerScopeSession` (3),
    /// permanent mode is `kCTFontManagerScopeUser` (2, `.persistent`).
    #[test]
    fn each_mode_registers_in_its_own_coretext_scope() {
        assert_eq!(scope(Mode::Session), 3);
        assert_eq!(scope(Mode::Permanent), 2);
    }

    #[test]
    fn a_request_splits_into_ordered_parallel_calls_of_at_least_eight_files() {
        assert_eq!(chunk_sizes(0, 4), Vec::<usize>::new());
        assert_eq!(chunk_sizes(5, 4), vec![5]);
        assert_eq!(chunk_sizes(20, 4), vec![10, 10]);
        assert_eq!(chunk_sizes(100, 3), vec![34, 33, 33]);
        assert_eq!(chunk_sizes(100, 64).len(), MAX_WORKERS);
        assert_eq!(chunk_sizes(100, 0), vec![100]);
        for (files, workers) in [(99, 3), (100, 8), (17, 2), (1000, 16)] {
            assert_eq!(chunk_sizes(files, workers).iter().sum::<usize>(), files);
        }
    }

    /// Parallel calls keep one result per path, in order.
    #[test]
    fn register_all_answers_every_path_in_order() {
        let paths: Vec<PathBuf> = (0..20)
            .map(|i| PathBuf::from(format!("/nonexistent/fm-{i}.ttf")))
            .collect();
        let refs: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
        let results = CoreText.register_all(&refs, Mode::Session);
        assert_eq!(results.len(), paths.len());
        assert!(results.iter().all(Result::is_err), "{results:?}");
    }

    #[test]
    fn a_path_round_trips_through_a_file_url() {
        let path = Path::new("/tmp/fm font/Schrift-ü.ttf");
        let url = file_url(path).expect("a file URL");
        assert_eq!(url_path(url.0).as_deref(), Some(path));
    }
}
