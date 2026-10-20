//! One glyph's outline as SVG (E11.5).
//!
//! - **Outline**: skrifa draws the glyph unhinted in font units, from `glyf`
//!   (quadratic, composites resolved) or `CFF`/`CFF2` (cubic), at the
//!   requested variation coordinates. Unlisted axes stay at their defaults;
//!   a tag the face has no axis for is ignored.
//! - **Frame**: the SVG is upright: font y grows up, SVG y grows down, so
//!   every point is drawn at `ascender - y`. The viewBox is the advance width
//!   by ascender to descender, all at the same location.
//! - **Errors**: a character the face does not map is `NotFound`; malformed
//!   font data is `Unsupported` with the reason in the detail, never a panic.
//!   A glyph with no outline (a space) is an SVG with an empty path.
//! - **No writes**: the SVG is returned as text. Saving it is the caller's
//!   job, to a path the user picks (rule 2).

use std::fmt::Write as _;
use std::path::Path;

use fm_api::export::GlyphSvgRequest;
use read_fonts::types::Tag;
use skrifa::instance::Size;
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider};

use crate::error::{FmError, FmResult};
use crate::glyphs::{draws, face, num_glyphs, sfnt};
use crate::preview::tag;

/// Reads the font file at `path` (face `collection_index` of a collection)
/// and answers [`glyph_svg`] for it.
pub fn glyph_svg_file(
    path: &Path,
    collection_index: u32,
    req: &GlyphSvgRequest,
) -> FmResult<String> {
    let data = std::fs::read(path)
        .map_err(|e| FmError::io("The font file could not be read.", path, &e))?;
    glyph_svg(&data, collection_index, req)
}

/// The outline of character `req.code_point` of face `collection_index` in
/// the font file `data` (TrueType, OpenType, a collection, WOFF or WOFF2) as
/// SVG text.
pub fn glyph_svg(data: &[u8], collection_index: u32, req: &GlyphSvgRequest) -> FmResult<String> {
    let settings = req
        .variations
        .iter()
        .map(|v| Ok((Tag::new(&tag(&v.tag)?), v.value)))
        .collect::<FmResult<Vec<_>>>()?;
    if req.variations.iter().any(|v| !v.value.is_finite()) {
        return Err(FmError::invalid("An axis value must be a number."));
    }
    let sfnt = sfnt(data).map_err(malformed)?;
    let font = face(&sfnt, collection_index).map_err(malformed)?;
    let count = num_glyphs(&font).map_err(malformed)?;
    let code_point = req.code_point;
    let gid = font
        .charmap()
        .map(code_point)
        .filter(|&gid| draws(gid, count))
        .ok_or_else(|| FmError::not_found(format_args!("Character U+{code_point:04X}")))?;

    let location = font.axes().location(settings);
    let outline = font
        .outline_glyphs()
        .get(gid)
        .ok_or_else(|| FmError::unsupported("This font has no outlines to export."))?;
    let (ascender, descender) = frame(&font, &location);
    let mut pen = SvgPath {
        d: String::new(),
        ascender,
    };
    outline
        .draw(
            DrawSettings::unhinted(Size::unscaled(), &location),
            &mut pen,
        )
        .map_err(|e| malformed(format!("glyph {}: {e}", gid.to_u32())))?;
    let advance = font
        .glyph_metrics(Size::unscaled(), &location)
        .advance_width(gid)
        .unwrap_or(0.0)
        .max(0.0);
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\">\
         <path d=\"{}\"/></svg>",
        num(advance),
        num(ascender - descender),
        pen.d
    ))
}

/// The ascender and (negative) descender at `location`. A font whose
/// ascender is not above its descender gets the em from the baseline up, so
/// the viewBox is never empty.
fn frame(font: &FontRef<'_>, location: &skrifa::instance::Location) -> (f32, f32) {
    let metrics = font.metrics(Size::unscaled(), location);
    if metrics.ascent > metrics.descent {
        (metrics.ascent, metrics.descent)
    } else {
        (f32::from(metrics.units_per_em), 0.0)
    }
}

fn malformed(detail: impl Into<String>) -> FmError {
    FmError::unsupported("This glyph cannot be exported: the font data is malformed.")
        .with_detail(detail)
}

/// SVG path data in font units, y flipped about the ascender.
struct SvgPath {
    d: String,
    ascender: f32,
}

impl SvgPath {
    fn command(&mut self, op: char, points: &[(f32, f32)]) {
        self.d.push(op);
        for (i, &(x, y)) in points.iter().enumerate() {
            if i > 0 {
                self.d.push(' ');
            }
            // Writing to a String cannot fail.
            let _ = write!(self.d, "{} {}", num(x), num(self.ascender - y));
        }
    }
}

impl OutlinePen for SvgPath {
    fn move_to(&mut self, x: f32, y: f32) {
        self.command('M', &[(x, y)]);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.command('L', &[(x, y)]);
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.command('Q', &[(cx0, cy0), (x, y)]);
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.command('C', &[(cx0, cy0), (cx1, cy1), (x, y)]);
    }

    fn close(&mut self) {
        self.d.push('Z');
    }
}

/// A coordinate with at most two decimals and no trailing zeros (`12`,
/// `12.5`, `-0.25`).
fn num(v: f32) -> String {
    let r = (v * 100.0).round() / 100.0;
    // `-0` reads as noise in a path.
    format!("{}", if r == 0.0 { 0.0 } else { r })
}

#[cfg(test)]
mod tests;
