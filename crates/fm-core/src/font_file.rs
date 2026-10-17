//! The bytes of a font file for the webview (E4.1).
//!
//! The list renders text with the webview's own engine: the app serves each
//! face's file over a URI scheme, by face id, and the webview registers it as
//! an `@font-face`. This module reads the file and names its media type from
//! the first bytes, so a renamed or mislabeled file is still served right and a
//! file the webview cannot load is refused with a clear error.

use std::fs;
use std::path::Path;

use crate::{FmError, FmResult};

/// A font file, ready to be sent to the webview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFile {
    /// The whole file.
    pub bytes: Vec<u8>,
    /// Media type (`font/ttf`, `font/otf`, `font/woff`, `font/woff2`,
    /// `font/collection`).
    pub mime: &'static str,
}

/// The media type of a font file from its first four bytes, or `None` when
/// they are not a format a webview can load (Type 1, a truncated file, not a
/// font at all).
pub fn font_mime(bytes: &[u8]) -> Option<&'static str> {
    match bytes.get(..4)? {
        [0x00, 0x01, 0x00, 0x00] | b"true" => Some("font/ttf"),
        b"OTTO" => Some("font/otf"),
        b"wOFF" => Some("font/woff"),
        b"wOF2" => Some("font/woff2"),
        b"ttcf" => Some("font/collection"),
        _ => None,
    }
}

/// Reads the font file at `path`. Never writes or moves it.
///
/// A missing file is [`fm_api::ErrorCode::NotFound`], a refused one
/// [`fm_api::ErrorCode::PermissionDenied`], and a file that is not a
/// TrueType, OpenType, WOFF or WOFF2 font (or collection)
/// [`fm_api::ErrorCode::Unsupported`].
pub fn read_font_file(path: &Path) -> FmResult<FontFile> {
    let bytes =
        fs::read(path).map_err(|e| FmError::io("The font file could not be read.", path, &e))?;
    let mime = font_mime(&bytes).ok_or_else(|| {
        FmError::unsupported("This file is not a font the preview can show.")
            .with_detail(path.display().to_string())
    })?;
    Ok(FontFile { bytes, mime })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use fm_api::ErrorCode;

    fn variable_fixture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf")
    }

    #[test]
    fn mime_follows_the_magic_bytes() {
        assert_eq!(font_mime(&[0, 1, 0, 0, 9]), Some("font/ttf"));
        assert_eq!(font_mime(b"true...."), Some("font/ttf"));
        assert_eq!(font_mime(b"OTTO"), Some("font/otf"));
        assert_eq!(font_mime(b"wOFF"), Some("font/woff"));
        assert_eq!(font_mime(b"wOF2"), Some("font/woff2"));
        assert_eq!(font_mime(b"ttcf"), Some("font/collection"));
        assert_eq!(font_mime(b"%!PS-AdobeFont"), None);
        assert_eq!(font_mime(b"OTT"), None);
        assert_eq!(font_mime(&[]), None);
    }

    #[test]
    fn reads_a_fixture_font_with_its_type() {
        let path = variable_fixture();
        let file = read_font_file(&path).unwrap();
        assert_eq!(file.mime, "font/ttf");
        assert_eq!(file.bytes, fs::read(&path).unwrap());
    }

    #[test]
    fn a_missing_file_is_not_found() {
        let dir = TempDir::new("font-file-missing");
        let err = read_font_file(&dir.path().join("gone.ttf")).unwrap_err();
        assert_eq!(err.code(), ErrorCode::NotFound);
        assert!(err.detail().unwrap().contains("gone.ttf"));
    }

    #[test]
    fn a_file_that_is_not_a_font_is_unsupported() {
        let dir = TempDir::new("font-file-junk");
        for (name, bytes) in [("notes.ttf", &b"hello world"[..]), ("empty.otf", &b""[..])] {
            let path = dir.path().join(name);
            fs::write(&path, bytes).unwrap();
            let err = read_font_file(&path).unwrap_err();
            assert_eq!(err.code(), ErrorCode::Unsupported, "{name}");
        }
    }

    #[test]
    fn a_renamed_file_is_typed_by_its_bytes() {
        let dir = TempDir::new("font-file-renamed");
        let path = dir.path().join("Schrift-ü.woff");
        fs::copy(variable_fixture(), &path).unwrap();
        assert_eq!(read_font_file(&path).unwrap().mime, "font/ttf");
    }
}
