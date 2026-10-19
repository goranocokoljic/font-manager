//! DirectWrite: the fonts other apps see. WebView2, browsers and Office render
//! with DirectWrite, and its system collection includes the per-user fonts
//! under `HKCU` (ADR 0008), so it is the view [`super::WindowsActivator::conflicts`]
//! checks against.

use std::collections::HashSet;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_ISOLATED, DWRITE_FONT_PROPERTY_ID_POSTSCRIPT_NAME, DWriteCreateFactory,
    IDWriteFactory3, IDWriteFontFile, IDWriteFontSet, IDWriteLocalFontFileLoader,
    IDWriteLocalizedStrings,
};
use windows::core::{BOOL, Interface};

/// Every font DirectWrite knows whose PostScript name is in `names`, with the
/// local file it comes from. A font that cannot be read is skipped; `Err`
/// only when DirectWrite itself is not there.
///
/// It reads the system font set (Windows 10+), whose properties come from
/// the font cache: no font file is opened to learn a name, so a check stays
/// well inside the activation budget on a system with thousands of fonts.
pub(crate) fn fonts_named(names: &HashSet<&str>) -> windows::core::Result<Vec<(String, PathBuf)>> {
    let mut found = Vec::new();
    if names.is_empty() {
        return Ok(found);
    }
    // SAFETY: plain COM calls on interfaces the calls above returned; every
    // out pointer is a local that lives through the call.
    unsafe {
        // An isolated factory does not reuse a font set cached before the
        // latest activation.
        let factory: IDWriteFactory3 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_ISOLATED)?;
        let set = factory.GetSystemFontSet()?;
        for i in 0..set.GetFontCount() {
            let Some(name) = postscript_name(&set, i) else {
                continue;
            };
            if !names.contains(name.as_str()) {
                continue;
            }
            let Some(path) = file_path(&set, i) else {
                continue;
            };
            if !found.iter().any(|(n, p)| *n == name && *p == path) {
                found.push((name, path));
            }
        }
    }
    Ok(found)
}

/// The PostScript name (name ID 6) of entry `i` of `set`, if it has one.
///
/// # Safety
/// `set` is a live DirectWrite font set and `i` is below its count.
unsafe fn postscript_name(set: &IDWriteFontSet, i: u32) -> Option<String> {
    let mut strings: Option<IDWriteLocalizedStrings> = None;
    let mut exists = BOOL::default();
    // SAFETY: the out pointers are locals that live through the calls; the
    // buffer has room for the length DirectWrite reports plus the NUL.
    unsafe {
        set.GetPropertyValues3(
            i,
            DWRITE_FONT_PROPERTY_ID_POSTSCRIPT_NAME,
            &mut exists,
            &mut strings,
        )
        .ok()?;
        let strings = strings.filter(|_| exists.as_bool())?;
        if strings.GetCount() == 0 {
            return None;
        }
        let len = strings.GetStringLength(0).ok()? as usize;
        let mut buf = vec![0u16; len + 1];
        strings.GetString(0, &mut buf).ok()?;
        Some(String::from_utf16_lossy(&buf[..len]))
    }
}

/// The local file of entry `i` of `set`. Fonts from other loaders (memory,
/// the downloadable-font cache) have no path and are left out.
///
/// # Safety
/// `set` is a live DirectWrite font set and `i` is below its count.
unsafe fn file_path(set: &IDWriteFontSet, i: u32) -> Option<PathBuf> {
    // SAFETY: plain COM calls; see `local_path`.
    unsafe {
        let file = set.GetFontFaceReference(i).ok()?.GetFontFile().ok()?;
        local_path(&file)
    }
}

/// The path of a font file from the local loader; `None` for other loaders.
///
/// # Safety
/// `file` is a live DirectWrite font file.
unsafe fn local_path(file: &IDWriteFontFile) -> Option<PathBuf> {
    // SAFETY: the reference key stays valid while `file` lives; the buffer
    // has room for the length the loader reports plus the NUL.
    unsafe {
        let mut key = std::ptr::null_mut();
        let mut key_size = 0u32;
        file.GetReferenceKey(&mut key, &mut key_size).ok()?;
        let local = file
            .GetLoader()
            .ok()?
            .cast::<IDWriteLocalFontFileLoader>()
            .ok()?;
        let len = local.GetFilePathLengthFromKey(key, key_size).ok()?;
        let mut buf = vec![0u16; len as usize + 1];
        local.GetFilePathFromKey(key, key_size, &mut buf).ok()?;
        Some(PathBuf::from(OsString::from_wide(&buf[..len as usize])))
    }
}
