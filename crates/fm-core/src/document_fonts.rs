//! The fonts a design document names (E32.1): Word and PowerPoint, Sketch,
//! Figma local copies, InDesign, Illustrator and Photoshop.
//!
//! Every reader works on a `Read + Seek` source and reads only the parts it
//! needs (a zip's central directory and a few entries, a Photoshop file's
//! layer records, bounded windows around an XMP packet), so a 1 GB document
//! is not loaded whole. Every length read from the file is bounded before it
//! is used: a truncated, encrypted or malformed document is an [`FmError`]
//! with one plain sentence ([`UNREADABLE`]), never a panic. The document is
//! only opened for reading (rule 2).

mod adobe;
mod fig;
mod office;
mod psd;
mod sketch;
mod xml;
mod zip;

#[cfg(test)]
mod tests;

use std::collections::{BTreeSet, HashSet};
use std::fs::File;
use std::io::{self, BufReader, Read, Seek};
use std::path::Path;

use fm_api::ErrorCode;
use fm_api::FaceId;
use fm_api::activation::{NeededFont, NeededFontMatch};
use fm_api::workflows::{
    DocumentFontCheck, DocumentFontRef, DocumentFontStatus, DocumentFonts, DocumentFontsCheck,
    DocumentFormat,
};

use crate::error::{FmError, FmResult};

/// The sentence the user sees for any document that cannot be read.
pub const UNREADABLE: &str = "This document's fonts could not be read.";

/// The sentence the user sees for a file that is not a document this module
/// reads.
pub const UNSUPPORTED: &str = "Font Manager reads the fonts of .indd, .ai, .psd, .sketch, .fig, .docx and .pptx documents only.";

/// The format of `path`, from its extension (any case); `None` when it is not
/// a document this module reads.
pub fn format_of(path: &Path) -> Option<DocumentFormat> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "docx" => DocumentFormat::Docx,
        "pptx" => DocumentFormat::Pptx,
        "sketch" => DocumentFormat::Sketch,
        "fig" => DocumentFormat::Fig,
        "indd" => DocumentFormat::Indd,
        "ai" => DocumentFormat::Ai,
        "psd" | "psb" => DocumentFormat::Psd,
        _ => return None,
    })
}

/// The fonts the document at `path` names, de-duplicated and sorted. The
/// file is opened read-only and never written, moved or renamed.
pub fn document_fonts(path: &Path) -> FmResult<DocumentFonts> {
    let Some(format) = format_of(path) else {
        return Err(FmError::unsupported(UNSUPPORTED).with_detail(format!(
            "{}: not a document type Font Manager reads",
            path.display()
        )));
    };
    let file = File::open(path).map_err(|e| FmError::io(UNREADABLE, path, &e))?;
    let fonts = read(format, &mut BufReader::new(file)).map_err(|e| {
        let why = e.detail().unwrap_or_default().to_owned();
        e.with_detail(format!("{}: {why}", path.display()))
    })?;
    Ok(DocumentFonts { format, fonts })
}

/// The name `fonts_needed` matches a document's font reference by (E32.2):
/// the PostScript name, the family and the style; documents name no full
/// name.
pub fn needed_font(font: &DocumentFontRef) -> NeededFont {
    NeededFont {
        postscript_name: font.postscript_name.clone(),
        full_name: None,
        family: font.family.clone(),
        style: font.style.clone(),
    }
}

/// Each font of `doc` with its status (E32.2): `matched` is what
/// `fonts_needed` found for [`needed_font`] of the fonts, `active` the faces
/// the app has active. A font without a match is missing; a matched one is
/// active when its face is. `to_activate` lists the faces of the in-library
/// fonts once each, in font order.
pub fn check(
    doc: DocumentFonts,
    matched: &[NeededFontMatch],
    active: &HashSet<FaceId>,
) -> DocumentFontsCheck {
    let mut to_activate = Vec::new();
    let fonts = doc
        .fonts
        .into_iter()
        .map(|font| {
            let needed = needed_font(&font);
            let face = matched.iter().find(|m| m.font == needed).map(|m| m.face);
            let status = match face {
                None => DocumentFontStatus::Missing,
                Some(f) if active.contains(&f) => DocumentFontStatus::Active,
                Some(f) => {
                    if !to_activate.contains(&f) {
                        to_activate.push(f);
                    }
                    DocumentFontStatus::InLibrary
                }
            };
            DocumentFontCheck { font, status, face }
        })
        .collect();
    DocumentFontsCheck {
        format: doc.format,
        fonts,
        to_activate,
    }
}

/// The fonts of a document of `format` in `src`, sorted. Public for the
/// `document_fonts` fuzz target: any bytes give a result, never a panic.
pub fn read<R: Read + Seek>(format: DocumentFormat, src: &mut R) -> FmResult<Vec<DocumentFontRef>> {
    let mut fonts = Fonts::default();
    let done = match format {
        DocumentFormat::Docx => office::docx(src, &mut fonts),
        DocumentFormat::Pptx => office::pptx(src, &mut fonts),
        DocumentFormat::Sketch => sketch::read(src, &mut fonts),
        DocumentFormat::Fig => fig::read(src, &mut fonts),
        DocumentFormat::Indd => adobe::indd(src, &mut fonts),
        DocumentFormat::Ai => adobe::ai(src, &mut fonts),
        DocumentFormat::Psd => psd::read(src, &mut fonts),
    };
    done.map_err(Bad::into_error)?;
    Ok(fonts.finish())
}

/// Why a document could not be read: the code and the developer detail. The
/// user always sees [`UNREADABLE`].
#[derive(Debug)]
pub(crate) struct Bad {
    code: ErrorCode,
    detail: String,
}

impl Bad {
    /// The bytes do not have the structure the format needs.
    fn malformed(detail: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::InvalidRequest,
            detail: detail.into(),
        }
    }

    /// The document is well formed but uses something this module cannot
    /// read (encryption, a compression without a decoder here).
    fn unsupported(detail: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Unsupported,
            detail: detail.into(),
        }
    }

    fn into_error(self) -> FmError {
        FmError::new(self.code, UNREADABLE).with_detail(self.detail)
    }
}

impl From<io::Error> for Bad {
    /// Reading past the end is a truncated document; anything else the
    /// source refused is reported the same way, with the OS error as detail.
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            Self::malformed("the document ends early (truncated)")
        } else {
            Self::malformed(format!("read failed: {e}"))
        }
    }
}

type Res<T> = Result<T, Bad>;

/// The references found so far, de-duplicated by the whole triple.
#[derive(Debug, Default)]
pub(crate) struct Fonts(BTreeSet<DocumentFontRef>);

impl Fonts {
    /// Adds one reference; blank parts are dropped, and a reference without
    /// any part is ignored.
    fn add(&mut self, postscript_name: Option<&str>, family: Option<&str>, style: Option<&str>) {
        let clean = |s: Option<&str>| {
            s.map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        let r = DocumentFontRef {
            postscript_name: clean(postscript_name),
            family: clean(family),
            style: clean(style),
        };
        if r.postscript_name.is_some() || r.family.is_some() || r.style.is_some() {
            self.0.insert(r);
        }
    }

    /// The sorted list. A reference that names only a PostScript name (or
    /// only a family) is dropped when a fuller one carries the same name:
    /// a PDF `/BaseFont` next to the XMP entry of the same font is one font.
    fn finish(self) -> Vec<DocumentFontRef> {
        let fuller_ps: BTreeSet<&str> = self
            .0
            .iter()
            .filter(|r| r.family.is_some() || r.style.is_some())
            .filter_map(|r| r.postscript_name.as_deref())
            .collect();
        let fuller_family: BTreeSet<&str> = self
            .0
            .iter()
            .filter(|r| r.postscript_name.is_some() || r.style.is_some())
            .filter_map(|r| r.family.as_deref())
            .collect();
        self.0
            .iter()
            .filter(|r| match (&r.postscript_name, &r.family, &r.style) {
                (Some(ps), None, None) => !fuller_ps.contains(ps.as_str()),
                (None, Some(f), None) => !fuller_family.contains(f.as_str()),
                _ => true,
            })
            .cloned()
            .collect()
    }

    fn postscript(&mut self, name: &str) {
        self.add(Some(name), None, None);
    }

    fn family(&mut self, name: &str) {
        self.add(None, Some(name), None);
    }
}

/// Reads up to `max` bytes from `src`; a longer input is refused as `what`
/// too large instead of growing without bound.
fn read_capped(src: &mut impl Read, max: u64, what: &str) -> Res<Vec<u8>> {
    let mut out = Vec::new();
    src.take(max.saturating_add(1)).read_to_end(&mut out)?;
    if out.len() as u64 > max {
        return Err(Bad::malformed(format!("{what} is larger than {max} bytes")));
    }
    Ok(out)
}

/// Exactly `len` bytes, refused above `max` before anything is allocated.
fn read_exact_capped(src: &mut impl Read, len: u64, max: u64, what: &str) -> Res<Vec<u8>> {
    if len > max {
        return Err(Bad::malformed(format!("{what} claims {len} bytes")));
    }
    let mut out = Vec::new();
    src.take(len).read_to_end(&mut out)?;
    if (out.len() as u64) < len {
        return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
    }
    Ok(out)
}

/// The text of an XML part: UTF-8 (BOM dropped) or UTF-16 with a BOM.
fn xml_text(bytes: &[u8]) -> Res<String> {
    let utf16 = |be: bool, rest: &[u8]| {
        let units = rest.as_chunks::<2>().0.iter().map(|&c| {
            if be {
                u16::from_be_bytes(c)
            } else {
                u16::from_le_bytes(c)
            }
        });
        char::decode_utf16(units)
            .collect::<Result<String, _>>()
            .map_err(|_| Bad::malformed("XML is not valid UTF-16"))
    };
    match bytes {
        [0xEF, 0xBB, 0xBF, rest @ ..] => {
            String::from_utf8(rest.to_vec()).map_err(|_| Bad::malformed("XML is not valid UTF-8"))
        }
        [0xFE, 0xFF, rest @ ..] => utf16(true, rest),
        [0xFF, 0xFE, rest @ ..] => utf16(false, rest),
        _ => {
            String::from_utf8(bytes.to_vec()).map_err(|_| Bad::malformed("XML is not valid UTF-8"))
        }
    }
}
