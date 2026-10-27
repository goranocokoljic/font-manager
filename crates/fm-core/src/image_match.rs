//! Find by image (E33.1): the glyphs of a screenshot reduced to the glyph
//! fingerprint of E23.1, so the library can be ranked against it with the
//! E23.2 distance ([`crate::similar::compare`]).
//!
//! [`fingerprint_png`] works in four steps:
//!
//! 1. **Ink.** The PNG ([`crate::preview::png_decode`]) becomes one coverage
//!    byte per pixel. Transparent pixels are laid over white; when that
//!    leaves no ink (white text on a transparent background), over black.
//!    The background is the median of the border pixels: on a light one the
//!    ink is what is darker, on a dark one what is lighter, so dark-on-light
//!    and light-on-dark read the same. A border that is not mostly that one
//!    grey (a photo, noise, a gradient) is no background, and no text. Ink is
//!    scaled so the strongest pixel is full coverage.
//! 2. **Glyphs.** Pixels of at least half coverage are grouped into
//!    8-connected components. A small component right above a larger one
//!    that it overlaps horizontally (the dot of an `i` or `j`, an accent)
//!    joins it. A component much smaller than the text's glyph height is a
//!    speck and is dropped, as is one mostly inside a larger one. The boxes are sorted into lines, top to bottom,
//!    and each line left to right: reading order for left-to-right text.
//! 3. **Line.** The line with the most glyphs is measured.
//! 4. **Fingerprint.** The same grid as E23.1 is laid over the line: four
//!    rows over the band from 1.75 x-heights above the baseline to 0.75
//!    below, `length / 4` columns over the line's advance width (its ink
//!    plus one mean gap between glyphs, the side bearings). When the line
//!    has one glyph per character of [`fingerprint::SAMPLE`], the measures
//!    are taken from the same letters as E23.1 (the `v` and `H` for the
//!    x-height ratio, the `i` for the stem, the `o` for contrast). Other
//!    text (or the sample with joined letters, a script) has its x-height
//!    from the rows where most of its ink is, and only its drawing and width
//!    are compared ([`ImageFingerprint::shapes_only`]); fewer glyphs than
//!    the sample has letters are taken as the sample with joined letters.
//!
//! An image that is blank, shows no text (fewer than [`MIN_GLYPHS`] glyphs
//! on its longest line) or text under [`MIN_X_HEIGHT_PX`] is a plain error.
//! Everything is integer and IEEE `f32` arithmetic: one image gives the same
//! fingerprint on every OS. The image is only read.

use std::fs;
use std::io::Read as _;
use std::path::Path;

use crate::fingerprint::{self, Fingerprint, GRID_ROWS, Params};
use crate::preview::png_decode::{self, Pixels};
use crate::{FmError, FmResult};

/// The fewest glyphs a line needs to be text.
pub const MIN_GLYPHS: usize = 3;

/// The smallest x-height that is measured, in pixels.
pub const MIN_X_HEIGHT_PX: f32 = 6.0;

/// The largest file that is read: a 4096 px square RGBA PNG stored without
/// compression is 64 MiB and a bit.
const MAX_FILE_BYTES: u64 = 65 * 1024 * 1024;

/// The least difference between background and ink, out of 255, for the
/// image to have any ink.
const MIN_CONTRAST: u8 = 40;

/// The least share of the border within half of [`MIN_CONTRAST`] of the
/// background: a plain background.
const PLAIN_BORDER: f32 = 0.6;

/// A pixel of at least this coverage belongs to a glyph.
const INK_AT: u8 = 128;

/// A component whose box is smaller than this share of the glyph height on
/// both sides is a speck.
const SPECK: f32 = 0.3;

/// A component counts towards the glyph height when its ink is at least this
/// share of the largest component's.
const BIG_INK: f32 = 0.1;

/// An image with more components than this shows no text, only noise.
const MAX_COMPONENTS: usize = 10_000;

/// The widest vertical gap between a dot and the glyph under it, in glyph
/// heights.
const DOT_GAP: f32 = 0.6;

/// A row with at least this share of the fullest row's ink is in the
/// x-height band (text that is not the sample).
const X_BAND: f32 = 0.4;

/// The band of the fingerprint grid, in x-heights (as E23.1).
const BAND_TOP: f32 = 1.75;
const BAND_BOTTOM: f32 = 0.75;

/// One glyph's box in the image, in pixels; `right` and `bottom` are
/// exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphBox {
    /// Leftmost column.
    pub left: u32,
    /// Top row.
    pub top: u32,
    /// One past the rightmost column.
    pub right: u32,
    /// One past the bottom row.
    pub bottom: u32,
    /// Pixels of ink.
    pub ink: u32,
}

impl GlyphBox {
    fn width(&self) -> u32 {
        self.right - self.left
    }

    fn height(&self) -> u32 {
        self.bottom - self.top
    }

    fn join(&mut self, other: &Self) {
        self.left = self.left.min(other.left);
        self.top = self.top.min(other.top);
        self.right = self.right.max(other.right);
        self.bottom = self.bottom.max(other.bottom);
        self.ink += other.ink;
    }
}

/// What an image looks like: the E23.1 fingerprint of its text.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageFingerprint {
    /// The fingerprint. Without [`Self::shapes_only`] every measure is
    /// taken from the sample's letters.
    pub fingerprint: Fingerprint,
    /// The text is not one glyph per letter of [`fingerprint::SAMPLE`]: only
    /// the drawing and the width are measured, the x-height ratio, stroke
    /// and contrast are `0.0` and must not be compared.
    pub shapes_only: bool,
}

/// Ink coverage, one byte per pixel (`255` full ink), row by row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coverage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height` bytes.
    pub ink: Vec<u8>,
}

impl Coverage {
    fn at(&self, x: u32, y: u32) -> u8 {
        self.ink[y as usize * self.width as usize + x as usize]
    }
}

fn no_text() -> FmError {
    FmError::invalid("No text was found in this image.")
}

/// The fingerprint of the text in the PNG file at `path`; the file is only
/// read.
pub fn fingerprint_file(path: &Path) -> FmResult<ImageFingerprint> {
    let unreadable = "This image could not be read.";
    let file = fs::File::open(path).map_err(|e| FmError::io(unreadable, path, &e))?;
    let mut data = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut data)
        .map_err(|e| FmError::io(unreadable, path, &e))?;
    if data.len() as u64 > MAX_FILE_BYTES {
        return Err(
            FmError::invalid("The image file is too large; crop it to the text.")
                .with_detail(path.display().to_string()),
        );
    }
    fingerprint_png(&data).map_err(|e| match e.detail() {
        Some(_) => e,
        None => e.with_detail(path.display().to_string()),
    })
}

/// The fingerprint of the text in PNG data (see the module docs).
pub fn fingerprint_png(data: &[u8]) -> FmResult<ImageFingerprint> {
    let pixels = png_decode::decode(data)?;
    let coverage = coverage(&pixels)?;
    let lines = lines(&coverage);
    let line = lines
        .iter()
        .max_by_key(|l| l.len())
        .filter(|l| l.len() >= MIN_GLYPHS)
        .ok_or_else(no_text)?;
    measure(&coverage, line, Params::default())
}

/// Step 1: the ink of `pixels` (see the module docs). A blank image is an
/// error.
pub fn coverage(pixels: &Pixels) -> FmResult<Coverage> {
    let over = |background: u32| -> Vec<u8> {
        match &pixels.alpha {
            Some(alpha) => pixels
                .grey
                .iter()
                .zip(alpha)
                .map(|(&g, &a)| {
                    let (g, a) = (u32::from(g), u32::from(a));
                    ((g * a + background * (255 - a) + 127) / 255) as u8
                })
                .collect(),
            None => pixels.grey.clone(),
        }
    };
    let (width, height) = (pixels.width, pixels.height);
    let on_white = ink(&over(255), width, height);
    let ink = match (on_white, &pixels.alpha) {
        (Some(ink), _) => ink,
        (None, Some(_)) => ink(&over(0), width, height).ok_or_else(no_text)?,
        (None, None) => return Err(no_text()),
    };
    Ok(Coverage { width, height, ink })
}

/// Coverage of the greys `grey` against the border's median, or `None`
/// when nothing differs from it by [`MIN_CONTRAST`].
fn ink(grey: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    let (w, h) = (width as usize, height as usize);
    let mut histogram = [0u32; 256];
    for x in 0..w {
        histogram[usize::from(grey[x])] += 1;
        if h > 1 {
            histogram[usize::from(grey[(h - 1) * w + x])] += 1;
        }
    }
    for y in 1..h.saturating_sub(1) {
        histogram[usize::from(grey[y * w])] += 1;
        if w > 1 {
            histogram[usize::from(grey[y * w + w - 1])] += 1;
        }
    }
    let half = histogram.iter().sum::<u32>().div_ceil(2);
    let mut seen = 0;
    let background = histogram
        .iter()
        .position(|&n| {
            seen += n;
            seen >= half
        })
        .unwrap_or(255) as u8;
    // Text sits on a plain background: most of the border is close to it.
    // A photo, noise or a gradient has none.
    let near: u32 = histogram
        .iter()
        .enumerate()
        .filter(|&(g, _)| g.abs_diff(usize::from(background)) <= usize::from(MIN_CONTRAST / 2))
        .map(|(_, &n)| n)
        .sum();
    if (near as f32) < PLAIN_BORDER * histogram.iter().sum::<u32>() as f32 {
        return None;
    }
    let light = background >= 128;
    let diff = |g: u8| {
        if light {
            background.saturating_sub(g)
        } else {
            g.saturating_sub(background)
        }
    };
    let strongest = grey.iter().map(|&g| diff(g)).max().unwrap_or(0);
    if strongest < MIN_CONTRAST {
        return None;
    }
    let range = u32::from(strongest);
    Some(
        grey.iter()
            .map(|&g| (u32::from(diff(g)) * 255 / range) as u8)
            .collect(),
    )
}

/// Steps 2: the glyph boxes of `coverage`, line by line from the top, each
/// line left to right.
pub fn lines(coverage: &Coverage) -> Vec<Vec<GlyphBox>> {
    let mut boxes = components(coverage);
    if boxes.len() > MAX_COMPONENTS {
        // Noise or a photo, not a line of text; joining would be quadratic.
        return Vec::new();
    }
    let Some(max_ink) = boxes.iter().map(|b| b.ink).max() else {
        return Vec::new();
    };
    let mut heights: Vec<u32> = boxes
        .iter()
        .filter(|b| b.ink as f32 >= BIG_INK * max_ink as f32)
        .map(GlyphBox::height)
        .collect();
    heights.sort_unstable();
    let glyph_height = heights[heights.len() / 2] as f32;

    join_dots(&mut boxes, glyph_height);
    let speck = SPECK * glyph_height;
    boxes.retain(|b| (b.width() as f32) >= speck || (b.height() as f32) >= speck);
    // Glyphs sit side by side; a box mostly inside a larger one is a blot
    // in a shape (noise, a photo), not a glyph.
    let inside = |b: &GlyphBox| {
        boxes.iter().any(|o| {
            let area = |g: &GlyphBox| u64::from(g.width()) * u64::from(g.height());
            let w = b.right.min(o.right).saturating_sub(b.left.max(o.left));
            let h = b.bottom.min(o.bottom).saturating_sub(b.top.max(o.top));
            area(o) > area(b) && 2 * u64::from(w) * u64::from(h) >= area(b)
        })
    };
    let outside: Vec<bool> = boxes.iter().map(|b| !inside(b)).collect();
    let mut keep = outside.into_iter();
    boxes.retain(|_| keep.next().unwrap_or(true));

    // Lines: by vertical centre, a box joins the line its centre falls in.
    boxes.sort_by_key(|b| (b.top + b.bottom, b.left));
    let mut lines: Vec<(u32, u32, Vec<GlyphBox>)> = Vec::new();
    for b in boxes {
        let centre = (b.top + b.bottom) / 2;
        match lines
            .iter_mut()
            .find(|(top, bottom, _)| (*top..*bottom).contains(&centre))
        {
            Some((top, bottom, line)) => {
                *top = (*top).min(b.top);
                *bottom = (*bottom).max(b.bottom);
                line.push(b);
            }
            None => lines.push((b.top, b.bottom, vec![b])),
        }
    }
    lines.sort_by_key(|(top, bottom, _)| top + bottom);
    lines
        .into_iter()
        .map(|(_, _, mut line)| {
            line.sort_by_key(|b| (b.left, b.top));
            line
        })
        .collect()
}

/// The 8-connected components of the pixels of at least [`INK_AT`].
fn components(coverage: &Coverage) -> Vec<GlyphBox> {
    let (w, h) = (coverage.width, coverage.height);
    let mut seen = vec![false; coverage.ink.len()];
    let mut stack = Vec::new();
    let mut out = Vec::new();
    for start in 0..coverage.ink.len() {
        if seen[start] || coverage.ink[start] < INK_AT {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        let (x, y) = ((start % w as usize) as u32, (start / w as usize) as u32);
        let mut b = GlyphBox {
            left: x,
            top: y,
            right: x + 1,
            bottom: y + 1,
            ink: 0,
        };
        while let Some(at) = stack.pop() {
            let (x, y) = ((at % w as usize) as u32, (at / w as usize) as u32);
            b.join(&GlyphBox {
                left: x,
                top: y,
                right: x + 1,
                bottom: y + 1,
                ink: 1,
            });
            for ny in y.saturating_sub(1)..(y + 2).min(h) {
                for nx in x.saturating_sub(1)..(x + 2).min(w) {
                    let n = ny as usize * w as usize + nx as usize;
                    if !seen[n] && coverage.ink[n] >= INK_AT {
                        seen[n] = true;
                        stack.push(n);
                    }
                }
            }
        }
        out.push(b);
        if out.len() > MAX_COMPONENTS {
            break;
        }
    }
    out
}

/// Joins each small box that sits right above a larger one it overlaps
/// horizontally (a dot, an accent) to the nearest such box.
fn join_dots(boxes: &mut Vec<GlyphBox>, glyph_height: f32) {
    let max_gap = DOT_GAP * glyph_height;
    // Smallest first, so a dot joins before its stem could join anything.
    boxes.sort_by_key(|b| (b.ink, b.left, b.top));
    let mut gone = vec![false; boxes.len()];
    for i in 0..boxes.len() {
        let dot = boxes[i];
        let under = boxes
            .iter()
            .enumerate()
            .filter(|&(j, b)| {
                let overlap = dot.right.min(b.right).saturating_sub(dot.left.max(b.left));
                j != i
                    && !gone[j]
                    && b.ink > dot.ink
                    && dot.bottom <= b.top
                    && ((b.top - dot.bottom) as f32) <= max_gap
                    && 2 * overlap >= dot.width().min(b.width())
            })
            .min_by_key(|&(_, b)| (b.top - dot.bottom, b.left))
            .map(|(j, _)| j);
        if let Some(j) = under {
            boxes[j].join(&dot);
            gone[i] = true;
        }
    }
    let mut keep = gone.iter().map(|g| !g);
    boxes.retain(|_| keep.next().unwrap_or(true));
}

/// Step 3 and 4: the fingerprint of `line` (reading order) in `coverage`.
fn measure(coverage: &Coverage, line: &[GlyphBox], params: Params) -> FmResult<ImageFingerprint> {
    let sample_len = fingerprint::SAMPLE.chars().count();
    let letters = line.len() == sample_len;
    let glyph = |c: char| {
        let i = fingerprint::SAMPLE.chars().position(|s| s == c)?;
        line.get(i).filter(|_| letters)
    };
    let (baseline, x_height) = match glyph('v') {
        Some(v) => {
            let mut bottoms: Vec<u32> = line.iter().map(|b| b.bottom).collect();
            bottoms.sort_unstable();
            let baseline = bottoms[bottoms.len() / 2] as f32;
            (baseline, baseline - v.top as f32)
        }
        None => x_band(coverage, line),
    };
    if x_height.is_nan() || x_height < MIN_X_HEIGHT_PX {
        return Err(FmError::invalid(
            "The text in this image is too small; use a larger screenshot.",
        )
        .with_detail(format!("x-height {x_height} px")));
    }

    // The sample's letters, measured as E23.1 measures them; 0 for other
    // text, whose measures are not compared.
    let x_height_ratio = glyph('H').map_or(0.0, |h| x_height / (baseline - h.top as f32));
    let stem_row = (baseline - x_height / 2.0).round().max(0.0) as u32;
    let stroke = glyph('i').map_or(0.0, |i| row_ink(coverage, i, stem_row));
    let (thick, thin) = glyph('o').map_or((0.0, 0.0), |o| strokes(coverage, o));
    let contrast = if thick > 0.0 {
        1.0 - thin.min(thick) / thick
    } else {
        0.0
    };

    // The advance width: the ink plus one mean gap (the side bearings).
    let first = line.first().ok_or_else(no_text)?;
    let gaps: f32 = line
        .windows(2)
        .map(|p| p[1].left as f32 - p[0].right as f32)
        .sum();
    let gap = (gaps / (line.len() - 1) as f32).max(0.0);
    let left = first.left as f32 - gap / 2.0;
    let right = line.iter().map(|b| b.right).max().unwrap_or(first.right) as f32 + gap / 2.0;
    let advance = right - left;

    let vector = reduce(
        coverage,
        (left, right),
        (
            baseline - BAND_TOP * x_height,
            baseline + BAND_BOTTOM * x_height,
        ),
        params.length,
    );
    Ok(ImageFingerprint {
        fingerprint: Fingerprint {
            vector,
            x_height_ratio,
            contrast,
            stroke_width: stroke / x_height,
            width_class: 5.0,
            // Fewer glyphs than the sample has letters: joined letters (a
            // script), taken as the sample.
            advance_ratio: advance / (line.len().max(sample_len) as f32 * x_height),
        },
        shapes_only: !letters,
    })
}

/// The baseline and x-height of `line` from its ink per row: the rows of
/// at least [`X_BAND`] of the fullest row's ink are the x-height band, where
/// every lowercase letter has ink, whether or not the letters are joined.
fn x_band(coverage: &Coverage, line: &[GlyphBox]) -> (f32, f32) {
    let top = line.iter().map(|b| b.top).min().unwrap_or(0);
    let bottom = line.iter().map(|b| b.bottom).max().unwrap_or(0);
    let left = line.iter().map(|b| b.left).min().unwrap_or(0);
    let right = line.iter().map(|b| b.right).max().unwrap_or(0);
    let rows: Vec<u64> = (top..bottom)
        .map(|y| (left..right).map(|x| u64::from(coverage.at(x, y))).sum())
        .collect();
    let peak = rows.iter().copied().max().unwrap_or(0) as f32;
    let full = |r: &u64| *r as f32 >= X_BAND * peak && peak > 0.0;
    let first = rows.iter().position(full).unwrap_or(0);
    let last = rows.iter().rposition(full).unwrap_or(0);
    let baseline = (top as usize + last + 1) as f32;
    (baseline, (last + 1 - first) as f32)
}

/// The ink of row `y` inside `b`, in pixels of full ink.
fn row_ink(coverage: &Coverage, b: &GlyphBox, y: u32) -> f32 {
    if !(b.top..b.bottom).contains(&y) {
        return 0.0;
    }
    (b.left..b.right)
        .map(|x| f32::from(coverage.at(x, y)))
        .sum::<f32>()
        / 255.0
}

/// The thick and thin strokes of a round glyph, as E23.1 measures them: the
/// ink of the left half of its middle row and of the top half of its middle
/// column, thicker first.
fn strokes(coverage: &Coverage, b: &GlyphBox) -> (f32, f32) {
    let (mid_x, mid_y) = (b.left + b.width() / 2, b.top + b.height() / 2);
    let side: f32 = (b.left..mid_x)
        .map(|x| f32::from(coverage.at(x, mid_y)))
        .sum::<f32>()
        / 255.0;
    let top: f32 = (b.top..mid_y)
        .map(|y| f32::from(coverage.at(mid_x, y)))
        .sum::<f32>()
        / 255.0;
    (side.max(top), side.min(top))
}

/// The mean coverage of a [`GRID_ROWS`] × `length / GRID_ROWS` grid over
/// the columns `x` and rows `y` (pixel edges, fractional), row by row. A
/// pixel counts in the cell its centre falls in; the grid's area outside the
/// image is blank.
fn reduce(coverage: &Coverage, x: (f32, f32), y: (f32, f32), length: usize) -> Vec<f32> {
    let cols = length / GRID_ROWS;
    let (cell_w, cell_h) = ((x.1 - x.0) / cols as f32, (y.1 - y.0) / GRID_ROWS as f32);
    let mut sums = vec![0u64; length];
    if cell_w > 0.0 && cell_h > 0.0 {
        let rows = (y.0.floor().max(0.0) as u32)..(y.1.ceil().max(0.0) as u32).min(coverage.height);
        let columns =
            (x.0.floor().max(0.0) as u32)..(x.1.ceil().max(0.0) as u32).min(coverage.width);
        for py in rows {
            let r = ((py as f32 + 0.5 - y.0) / cell_h).floor();
            if !(0.0..GRID_ROWS as f32).contains(&r) {
                continue;
            }
            for px in columns.clone() {
                let c = ((px as f32 + 0.5 - x.0) / cell_w).floor();
                if (0.0..cols as f32).contains(&c) {
                    sums[r as usize * cols + c as usize] += u64::from(coverage.at(px, py));
                }
            }
        }
    }
    let area = f64::from(cell_w * cell_h) * 255.0;
    sums.into_iter()
        .map(|s| {
            if area > 0.0 {
                ((s as f64 / area) as f32).min(1.0)
            } else {
                0.0
            }
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests;
