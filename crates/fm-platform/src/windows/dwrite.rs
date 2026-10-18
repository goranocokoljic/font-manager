//! DirectWrite: the fonts other apps see. WebView2, browsers and Office render
//! with DirectWrite, and its system collection includes the per-user fonts
//! under `HKCU` (ADR 0008), so it is the view [`super::WindowsActivator::conflicts`]
//! checks against.

use std::collections::HashSet;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_ISOLATED, DWRITE_FONT_SIMULATIONS_NONE,
    DWRITE_INFORMATIONAL_STRING_POSTSCRIPT_NAME, DWriteCreateFactory, IDWriteFactory, IDWriteFont,
    IDWriteFontCollection, IDWriteFontFile, IDWriteLocalFontFileLoader, IDWriteLocalizedStrings,
};
use windows::core::{BOOL, Interface};

/// Every font DirectWrite knows whose PostScript name is in `names`, with the
/// local file it comes from. A font that cannot be read is skipped; `Err`
/// only when DirectWrite itself is not there.
pub(crate) fn fonts_named(names: &HashSet<&str>) -> windows::core::Result<Vec<(String, PathBuf)>> {
    let mut found = Vec::new();
    if names.is_empty() {
        return Ok(found);
    }
    // SAFETY: plain COM calls on interfaces the calls above returned; every
    // out pointer is a local that lives through the call.
    unsafe {
        // An isolated factory does not reuse a collection cached before the
        // latest activation.
        let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_ISOLATED)?;
        let mut collection: Option<IDWriteFontCollection> = None;
        factory.GetSystemFontCollection(&mut collection, true)?;
        let Some(collection) = collection else {
            return Ok(found);
        };
        for f in 0..collection.GetFontFamilyCount() {
            let Ok(family) = collection.GetFontFamily(f) else {
                continue;
            };
            for i in 0..family.GetFontCount() {
                let Ok(font) = family.GetFont(i) else {
                    continue;
                };
                if font.GetSimulations() != DWRITE_FONT_SIMULATIONS_NONE {
                    continue;
                }
                let Some(name) = postscript_name(&font) else {
                    continue;
                };
                if !names.contains(name.as_str()) {
                    continue;
                }
                for path in file_paths(&font) {
                    if !found.iter().any(|(n, p)| *n == name && *p == path) {
                        found.push((name.clone(), path));
                    }
                }
            }
        }
    }
    Ok(found)
}

/// The font's PostScript name (name ID 6), if it has one.
///
/// # Safety
/// `font` is a live DirectWrite font.
unsafe fn postscript_name(font: &IDWriteFont) -> Option<String> {
    let mut strings: Option<IDWriteLocalizedStrings> = None;
    let mut exists = BOOL::default();
    // SAFETY: the out pointers are locals that live through the calls; the
    // buffer has room for the length DirectWrite reports plus the NUL.
    unsafe {
        font.GetInformationalStrings(
            DWRITE_INFORMATIONAL_STRING_POSTSCRIPT_NAME,
            &mut strings,
            &mut exists,
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

/// The local files behind `font`. Fonts from other loaders (memory, the
/// downloadable-font cache) have no path and are left out.
///
/// # Safety
/// `font` is a live DirectWrite font.
unsafe fn file_paths(font: &IDWriteFont) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    // SAFETY: `GetFiles` is called first for the count, then with a buffer of
    // that many slots; the reference key stays valid while `file` lives.
    unsafe {
        let Ok(face) = font.CreateFontFace() else {
            return paths;
        };
        let mut count = 0u32;
        if face.GetFiles(&mut count, None).is_err() {
            return paths;
        }
        let mut files: Vec<Option<IDWriteFontFile>> = vec![None; count as usize];
        if face.GetFiles(&mut count, Some(files.as_mut_ptr())).is_err() {
            return paths;
        }
        for file in files.into_iter().flatten() {
            let mut key = std::ptr::null_mut();
            let mut key_size = 0u32;
            if file.GetReferenceKey(&mut key, &mut key_size).is_err() {
                continue;
            }
            let Ok(loader) = file.GetLoader() else {
                continue;
            };
            let Ok(local) = loader.cast::<IDWriteLocalFontFileLoader>() else {
                continue;
            };
            let Ok(len) = local.GetFilePathLengthFromKey(key, key_size) else {
                continue;
            };
            let mut buf = vec![0u16; len as usize + 1];
            if local.GetFilePathFromKey(key, key_size, &mut buf).is_ok() {
                paths.push(PathBuf::from(OsString::from_wide(&buf[..len as usize])));
            }
        }
    }
    paths
}
