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

use std::cell::RefCell;
use std::panic::{self, AssertUnwindSafe};

use read_fonts::TableProvider;
use swash::FontRef;
use swash::scale::ScaleContext;
use swash::scale::outline::Outline;
use swash::zeno::{Bounds, Format, Mask, Origin, Point, Scratch, Vector};

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

/// The widest and tallest one glyph may be, in x-heights. The rasterizer
/// allocates a glyph's whole box before it draws, so one stray outline point
/// (malformed data) would otherwise cost gigabytes or abort the scan; a real
/// Latin letter is a few x-heights at most.
const MAX_GLYPH_X_HEIGHTS: f32 = 16.0;

thread_local! {
    /// The outline loader's caches, kept per scan thread: a scan takes
    /// thousands of fingerprints.
    static CONTEXT: RefCell<ScaleContext> = RefCell::new(ScaleContext::new());
}

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
    fill_sfnt(&sfnt_data(data).unwrap_or_default(), parsed);
}

/// [`fill`] from the TrueType or OpenType bytes the faces were parsed from
/// (the font inside a WOFF or WOFF2 file; empty when it cannot be read).
pub(crate) fn fill_sfnt(sfnt: &[u8], parsed: &mut ParsedFile) {
    for face in &mut parsed.faces {
        face.fingerprint = match face.health {
            fm_api::common::Health::Ok => fingerprint(sfnt, face.index, Params::default())
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

    // Each outline is loaded once, in font units, and scaled for drawing.
    // Measured in font units: the x-height is the top of the flat `v`, the
    // cap height the top of the `H`. Every glyph needs an outline.
    let outlines = CONTEXT.with(|context| {
        let mut context = context.try_borrow_mut().ok()?;
        let mut units = context.builder(font).hint(false).build();
        if !units.has_outlines() {
            return None;
        }
        glyphs
            .iter()
            .map(|&id| units.scale_outline(id).filter(|o| !o.points().is_empty()))
            .collect::<Option<Vec<_>>>()
    })?;
    let boxes: Vec<_> = outlines.iter().map(|o| o.bounds()).collect();
    let top = |c: char| SAMPLE.chars().position(|s| s == c).map(|i| boxes[i].max.y);
    let (x_height, cap_height) = (top('v')?, top('H')?);
    let upem = f32::from(font.metrics(&[]).units_per_em);
    if upem <= 0.0 || x_height < upem * MIN_X_HEIGHT_EM || cap_height <= 0.0 {
        return None;
    }

    let xh_px = f32::from(params.x_height_px);
    let size = xh_px * upem / x_height;
    let max_glyph = MAX_GLYPH_X_HEIGHTS * xh_px;
    let scale = size / upem;
    let fits = |b: &Bounds| {
        (b.max.x - b.min.x) * scale <= max_glyph && (b.max.y - b.min.y) * scale <= max_glyph
    };
    if !boxes.iter().all(fits) {
        return None;
    }
    let advances = font.glyph_metrics(&[]).scale(size);
    let band_top = (BAND_TOP * xh_px).round();
    let height = (band_top + (BAND_BOTTOM * xh_px).round()) as usize;
    let mut pens = Vec::with_capacity(glyphs.len());
    let mut pen = 0.0f32;
    for &id in &glyphs {
        pens.push(pen);
        pen += advances.advance_width(id);
        if pen.is_nan() || pen > MAX_WIDTH_PX {
            return None;
        }
    }
    let width = pen.ceil() as usize;
    if width == 0 {
        return None;
    }

    let mut scratch = Scratch::new();
    let mut canvas = vec![0u8; width * height];
    for (outline, &pen) in outlines.iter().zip(&pens) {
        if let Some(glyph) = Image::render(&mut scratch, outline, scale, pen.fract()) {
            glyph.add_to(&mut canvas, width, pen.floor() as i32, band_top as i32);
        }
    }

    let mut image_of = |c: char| {
        let i = SAMPLE.chars().position(|s| s == c)?;
        Image::render(&mut scratch, &outlines[i], scale, 0.0)
    };
    let stem = image_of('i').map_or(0.0, |i| ink_across(&i, xh_px / 2.0));
    let (thick, thin) = image_of('o').map_or((0.0, 0.0), |o| strokes(&o));
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
        vector: reduce(&canvas, width, height, params.length),
        x_height_ratio: x_height / cap_height,
        contrast,
        stroke_width: stem / xh_px,
        width_class,
        advance_ratio: pen / (glyphs.len() as f32 * xh_px),
    })
}

/// One glyph drawn with swash's rasterizer (zeno): one byte of coverage per
/// pixel, row by row from the top.
struct Image {
    data: Vec<u8>,
    width: usize,
    height: usize,
    /// Pixels from the pen to the left column.
    left: i32,
    /// Pixels from the baseline up to the top row.
    top: i32,
}

impl Image {
    /// `outline` (font units) at `scale`, moved right by `offset` (a pixel
    /// fraction): what swash's `Render` draws from the same outline, without
    /// loading the outline again.
    fn render(scratch: &mut Scratch, outline: &Outline, scale: f32, offset: f32) -> Option<Self> {
        let points: Vec<Point> = outline
            .points()
            .iter()
            .map(|p| Point::new(p.x * scale, p.y * scale))
            .collect();
        let offset = Vector::new(offset, 0.0);
        let (data, p) = Mask::with_scratch((&points[..], outline.verbs()), scratch)
            .format(Format::Alpha)
            .origin(Origin::BottomLeft)
            .offset(offset)
            .render_offset(offset)
            // Sizes the mask first, as swash does: without it the
            // placement's top is computed from a height of zero.
            .inspect(|_, _, _| {})
            .render();
        (p.width > 0 && p.height > 0).then_some(Self {
            data,
            width: p.width as usize,
            height: p.height as usize,
            left: p.left,
            top: p.top,
        })
    }

    /// Adds the glyph's coverage to a `width`-wide canvas, with the pen at
    /// column `x` and the baseline at row `baseline`; what falls outside is
    /// cut off.
    fn add_to(&self, canvas: &mut [u8], width: usize, x: i32, baseline: i32) {
        let (w, cw) = (self.width as i32, width as i32);
        let left = x + self.left;
        let (c0, c1) = ((-left).clamp(0, w), (cw - left).clamp(0, w));
        if c0 >= c1 {
            return;
        }
        let rows = canvas.len() as i32 / cw.max(1);
        for row in 0..self.height as i32 {
            let cy = baseline - self.top + row;
            if !(0..rows).contains(&cy) {
                continue;
            }
            let src = &self.data[(row * w + c0) as usize..(row * w + c1) as usize];
            let at = (cy * cw + left + c0) as usize;
            for (dst, &src) in canvas[at..at + src.len()].iter_mut().zip(src) {
                *dst = dst.saturating_add(src);
            }
        }
    }
}

/// The ink of the image row at `y` pixels above the baseline, in pixels.
fn ink_across(image: &Image, y: f32) -> f32 {
    let row = image.top - y.round() as i32;
    if !(0..image.height as i32).contains(&row) {
        return 0.0;
    }
    let start = row as usize * image.width;
    sum(&image.data[start..start + image.width])
}

/// The thick and thin strokes of a round glyph, in pixels: the ink of the
/// left half of its middle row (the side stroke) and of the top half of its
/// middle column (the top stroke), thicker first.
fn strokes(image: &Image) -> (f32, f32) {
    let (w, h) = (image.width, image.height);
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
                // A row of one cell is at most `MAX_WIDTH_PX` bytes: no u32
                // overflow.
                total += u64::from(
                    canvas[y * width + x0..y * width + x1]
                        .iter()
                        .map(|&v| u32::from(v))
                        .sum::<u32>(),
                );
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
