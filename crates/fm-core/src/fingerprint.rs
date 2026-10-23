//! Glyph fingerprints (E23.1): a short description of how a face looks, for
//! "fonts that look like this one".
//!
//! [`fingerprint`] draws the fixed string [`SAMPLE`] at a normalized x-height
//! ([`Params::x_height_px`], 32 px by default) and reduces the image to a
//! fixed-length vector ([`Params::length`], 64 values by default): the ink
//! of a grid of [`GRID_ROWS`] rows by `length / GRID_ROWS` columns laid over
//! the string, each cell the mean coverage of its pixels in `0.0..=1.0`.
//! The rows span a fixed band around the x-height, so ascenders and the
//! descender fall into their own rows; the columns span the string's advance
//! width, so the vector describes shapes, and the width goes in the metrics.
//!
//! - Glyphs come from the `cmap` and their advances from `hmtx`, without
//!   shaping or kerning: the drawing depends on the outlines only.
//! - A variable face is drawn at its default instance.
//! - Every number comes from integer coverage and IEEE `+ - * /`, so one
//!   file gives the same vector on every OS.
//!
//! A face that does not map every character of [`SAMPLE`] to a glyph with an
//! outline (a symbol or non-Latin face, a bitmap-only face), or whose data is
//! malformed, has no fingerprint. That is never a panic.

use std::panic::{self, AssertUnwindSafe};

use read_fonts::TableProvider;
use swash::FontRef;
use swash::scale::image::Image;
use swash::scale::{Render, ScaleContext, Scaler, Source};
use swash::zeno::{Format, Vector};

use crate::parser::ParsedFile;
use crate::preview::{check_face, sfnt_data};

/// The string every face draws.
pub const SAMPLE: &str = "Hamburgefonstiv";

/// The rows of the reduction grid.
pub const GRID_ROWS: usize = 4;

/// The band the grid rows span, in x-heights above the baseline (top) and
/// below it (bottom): room for ascenders, accents of capitals and the `g`.
const BAND_TOP: f32 = 1.75;
const BAND_BOTTOM: f32 = 0.75;

/// A face whose x-height is less than this share of its em is not measured:
/// the drawing would be huge.
const MIN_X_HEIGHT_EM: f32 = 0.05;

/// The widest drawing, in pixels; a wider one gives no fingerprint.
const MAX_WIDTH_PX: f32 = 8192.0;

/// How a fingerprint is taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Params {
    /// The x-height the sample is drawn at, in pixels.
    pub x_height_px: u16,
    /// The vector length; a multiple of [`GRID_ROWS`].
    pub length: usize,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            x_height_px: 32,
            length: 64,
        }
    }
}

/// What a face looks like: the reduced drawing and a few measures.
#[derive(Debug, Clone, PartialEq)]
pub struct Fingerprint {
    /// The grid's mean coverage, row by row from the top, each in `0.0..=1.0`.
    pub vector: Vec<f32>,
    /// The x-height over the cap height (`v` over `H`).
    pub x_height_ratio: f32,
    /// Stroke contrast in `0.0..=1.0`: one minus the thin stroke of the `o`
    /// over its thick stroke. `0.0` for a monoline face.
    pub contrast: f32,
    /// The stem of the `i` over the x-height.
    pub stroke_width: f32,
    /// `OS/2.usWidthClass` (1–9); 5 (normal) without an `OS/2` table.
    pub width_class: f32,
    /// The mean advance of the sample's glyphs over the x-height: the width
    /// the face measures, whatever its width class declares.
    pub advance_ratio: f32,
}

/// Whether a face's fingerprint was taken, and what it gave.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum FaceFingerprint {
    /// Not taken: the face was not read from its file (it was loaded from the
    /// index), so whatever the index holds stays.
    #[default]
    NotTaken,
    /// Taken; the face has no fingerprint (see the module docs).
    Absent,
    /// Taken.
    Taken(Fingerprint),
}

/// Takes the fingerprint of every face of `parsed`, read from `data`, the
/// bytes it was parsed from. A face that is not healthy has none.
pub fn fill(data: &[u8], parsed: &mut ParsedFile) {
    let Ok(sfnt) = sfnt_data(data) else {
        for face in &mut parsed.faces {
            face.fingerprint = FaceFingerprint::Absent;
        }
        return;
    };
    for face in &mut parsed.faces {
        face.fingerprint = match face.health {
            fm_api::common::Health::Ok => fingerprint(&sfnt, face.index, Params::default())
                .map_or(FaceFingerprint::Absent, FaceFingerprint::Taken),
            _ => FaceFingerprint::Absent,
        };
    }
}

/// The fingerprint of face `index` of the TrueType or OpenType data `sfnt`,
/// or `None` (see the module docs).
pub fn fingerprint(sfnt: &[u8], index: u32, params: Params) -> Option<Fingerprint> {
    if params.length == 0 || !params.length.is_multiple_of(GRID_ROWS) || params.x_height_px == 0 {
        return None;
    }
    check_face(sfnt, index).ok()?;
    panic::catch_unwind(AssertUnwindSafe(|| take(sfnt, index, params)))
        .ok()
        .flatten()
}

fn take(sfnt: &[u8], index: u32, params: Params) -> Option<Fingerprint> {
    let font = FontRef::from_index(sfnt, index as usize)?;
    let charmap = font.charmap();
    let glyphs: Vec<u16> = SAMPLE.chars().map(|c| charmap.map(c)).collect();
    if glyphs.contains(&0) {
        return None;
    }

    // Measured in font units: the x-height is the top of the flat `v`, the
    // cap height the top of the `H`. Every glyph needs an outline.
    let mut context = ScaleContext::new();
    let mut units = context.builder(font).hint(false).build();
    if !units.has_outlines() {
        return None;
    }
    let mut tops = Vec::with_capacity(glyphs.len());
    for &id in &glyphs {
        let outline = units.scale_outline(id)?;
        if outline.points().is_empty() {
            return None;
        }
        tops.push(outline.bounds().max.y);
    }
    let top = |c: char| SAMPLE.chars().position(|s| s == c).map(|i| tops[i]);
    let (x_height, cap_height) = (top('v')?, top('H')?);
    let upem = f32::from(font.metrics(&[]).units_per_em);
    if upem <= 0.0 || x_height < upem * MIN_X_HEIGHT_EM || cap_height <= 0.0 {
        return None;
    }

    let xh_px = f32::from(params.x_height_px);
    let size = xh_px * upem / x_height;
    let advances = font.glyph_metrics(&[]).scale(size);
    let mut scaler = context.builder(font).size(size).hint(false).build();

    let band_top = (BAND_TOP * xh_px).round() as i32;
    let height = band_top + (BAND_BOTTOM * xh_px).round() as i32;
    let mut pen = 0.0f32;
    let mut placed = Vec::with_capacity(glyphs.len());
    for &id in &glyphs {
        let image = render(&mut scaler, id, pen.fract());
        placed.push((pen.floor() as i32, image));
        pen += advances.advance_width(id);
        if pen > MAX_WIDTH_PX {
            return None;
        }
    }
    let width = pen.ceil() as i32;
    if width <= 0 {
        return None;
    }

    let mut canvas = vec![0u8; (width * height) as usize];
    for (x, image) in placed.iter().filter_map(|(x, i)| Some((x, i.as_ref()?))) {
        let p = image.placement;
        for row in 0..p.height as i32 {
            let cy = band_top - p.top + row;
            if !(0..height).contains(&cy) {
                continue;
            }
            for col in 0..p.width as i32 {
                let cx = x + p.left + col;
                if !(0..width).contains(&cx) {
                    continue;
                }
                let src = image.data[(row * p.width as i32 + col) as usize];
                let dst = &mut canvas[(cy * width + cx) as usize];
                *dst = dst.saturating_add(src);
            }
        }
    }

    let image_of = |c: char| {
        let i = SAMPLE.chars().position(|s| s == c)?;
        placed[i].1.as_ref()
    };
    let stem = image_of('i').map_or(0.0, |i| ink_across(i, xh_px / 2.0));
    let (thick, thin) = image_of('o').map_or((0.0, 0.0), strokes);
    let contrast = if thick > 0.0 {
        1.0 - thin.min(thick) / thick
    } else {
        0.0
    };
    let width_class = read_fonts::FontRef::from_index(sfnt, index)
        .ok()
        .and_then(|f| f.os2().ok())
        .map_or(5.0, |os2| f32::from(os2.us_width_class()));

    Some(Fingerprint {
        vector: reduce(&canvas, width as usize, height as usize, params.length),
        x_height_ratio: x_height / cap_height,
        contrast,
        stroke_width: stem / xh_px,
        width_class,
        advance_ratio: pen / (glyphs.len() as f32 * xh_px),
    })
}

/// The glyph drawn as coverage, shifted right by `offset` (a pixel fraction).
fn render(scaler: &mut Scaler<'_>, id: u16, offset: f32) -> Option<Image> {
    Render::new(&[Source::Outline])
        .format(Format::Alpha)
        .offset(Vector::new(offset, 0.0))
        .render(scaler, id)
        .filter(|i| i.placement.width > 0 && i.placement.height > 0)
}

/// The ink of the image row at `y` pixels above the baseline, in pixels.
fn ink_across(image: &Image, y: f32) -> f32 {
    let p = image.placement;
    let row = p.top - y.round() as i32;
    if !(0..p.height as i32).contains(&row) {
        return 0.0;
    }
    let start = row as usize * p.width as usize;
    sum(&image.data[start..start + p.width as usize])
}

/// The thick and thin strokes of a round glyph, in pixels: the ink of the
/// left half of its middle row (the side stroke) and of the top half of its
/// middle column (the top stroke), thicker first.
fn strokes(image: &Image) -> (f32, f32) {
    let (w, h) = (
        image.placement.width as usize,
        image.placement.height as usize,
    );
    let row = &image.data[(h / 2) * w..(h / 2) * w + w / 2];
    let column: Vec<u8> = (0..h / 2).map(|y| image.data[y * w + w / 2]).collect();
    let (side, top) = (sum(row), sum(&column));
    (side.max(top), side.min(top))
}

/// Coverage bytes as pixels of full ink.
fn sum(coverage: &[u8]) -> f32 {
    let total: u32 = coverage.iter().map(|&c| u32::from(c)).sum();
    total as f32 / 255.0
}

/// The mean coverage of a `GRID_ROWS` × `length / GRID_ROWS` grid over the
/// `width` × `height` canvas, row by row. Cell edges are whole pixels; a
/// cell narrower than a pixel takes the pixel it starts in.
fn reduce(canvas: &[u8], width: usize, height: usize, length: usize) -> Vec<f32> {
    let cols = length / GRID_ROWS;
    let edge = |i: usize, n: usize, total: usize| i * total / n;
    let mut out = Vec::with_capacity(length);
    for r in 0..GRID_ROWS {
        let (y0, y1) = (edge(r, GRID_ROWS, height), edge(r + 1, GRID_ROWS, height));
        let y1 = y1.max(y0 + 1).min(height);
        for c in 0..cols {
            let (x0, x1) = (edge(c, cols, width), edge(c + 1, cols, width));
            let x1 = x1.max(x0 + 1).min(width);
            let mut total: u64 = 0;
            for y in y0..y1 {
                total += canvas[y * width + x0..y * width + x1]
                    .iter()
                    .map(|&v| u64::from(v))
                    .sum::<u64>();
            }
            let cells = ((y1 - y0) * (x1 - x0)) as f64 * 255.0;
            out.push((total as f64 / cells) as f32);
        }
    }
    out
}

/// The vector as little-endian `f32` bytes, as the index stores it.
pub fn vector_to_bytes(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// A stored vector back; `None` when the length is not a whole number of
/// `f32`s.
pub fn vector_from_bytes(bytes: &[u8]) -> Option<Vec<f32>> {
    let (chunks, rest) = bytes.as_chunks::<4>();
    rest.is_empty()
        .then(|| chunks.iter().map(|b| f32::from_le_bytes(*b)).collect())
}

#[cfg(test)]
mod tests;
