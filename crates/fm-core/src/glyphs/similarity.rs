//! Per-glyph similarity (E49.8, ADR 0023): a shape descriptor of one glyph
//! of one face, so the library can be ranked by how close each face's `R`
//! is to a reference `R`.
//!
//! [`describe`] works in three steps:
//!
//! 1. **Raster.** The glyph is drawn at the default instance, unhinted, at
//!    [`SIZE`] px to the em, its outline box centred in a [`SIZE`] ×
//!    [`SIZE`] square: one coverage byte per pixel. Position on the baseline
//!    and the advance do not count, only the drawing.
//! 2. **Edge orientations.** At every pixel the coverage gradient votes, by
//!    its strength, for [`BINS`] unsigned edge orientations (soft: each
//!    orientation gets the squared positive cosine to the edge, so the
//!    votes of one pixel always sum to the same) in a [`CELLS`] × [`CELLS`]
//!    grid of cells (bilinear between the four nearest cell centres): a
//!    histogram of oriented gradients. Edges sit where the strokes are,
//!    whatever their weight, so a bold and a light straight leg read alike
//!    and a straight and a curved leg do not.
//! 3. **Normalise.** The histogram is scaled to unit length, every value
//!    clipped at [`CLIP`] so one long edge does not drown the rest, scaled
//!    to unit length again and stored as bytes, the largest value 255.
//!
//! [`distance`] compares two descriptors by the Euclidean distance of their
//! unit vectors, scaled to `0..=1`: 0 is the same drawing.
//!
//! - Every number comes from integer coverage and IEEE `+ - * / sqrt`, so one
//!   file gives the same descriptor on every OS. No `atan2`.
//! - A face that does not map the character, a glyph without an outline (a
//!   space, a bitmap-only face) or malformed data has no descriptor. That is
//!   never a panic.
//! - [`VERSION`] changes with any change to the steps above: the trait pass
//!   and the on-demand path measure every face again.

use std::cell::RefCell;
use std::panic::{self, AssertUnwindSafe};

use swash::FontRef;
use swash::scale::ScaleContext;
use swash::zeno::{Format, Mask, Origin, Point, Scratch};

use crate::index::{DescribedFace, DescriptorTodo};
use crate::preview::{check_face, sfnt_data};

/// The descriptor's version; part of every stored descriptor.
pub const VERSION: u32 = 1;

/// The key the trait pass stamps a face with once the descriptors of the
/// trait glyphs ([`TRAIT_GLYPHS`]) are stored, next to the trait ids.
pub const STAMP: &str = "glyph.descriptors";

/// The pixels to the em, and the side of the square the glyph is drawn in.
pub const SIZE: usize = 64;

/// The cells of the grid along each side.
pub const CELLS: usize = 6;

/// The edge orientations of each cell, over half a turn.
pub const BINS: usize = 8;

/// The bytes of one descriptor.
pub const LEN: usize = CELLS * CELLS * BINS;

/// The largest share of the unit vector one value keeps.
pub const CLIP: f32 = 0.2;

/// The largest a glyph may be, in ems: the rasterizer allocates its box, so
/// one stray point (malformed data) must not cost gigabytes. Anything
/// outside the square is cut off.
const MAX_EMS: f32 = 4.0;

/// The glyphs the trait pass describes for every face: the trait glyphs of
/// the epic ([`crate::glyphs::geometry::TRAIT_GLYPHS`]) and the `O` the
/// roundness detector reads, so every glyph a trait is read from. Any other
/// is described on demand.
pub const TRAIT_GLYPHS: [char; 16] = [
    'R', 'G', 'Q', 'a', 'g', 'e', 't', 'y', 'M', 'W', '&', '1', '7', 'I', 'l', 'O',
];

/// `cos 2φ` and `sin 2φ` of each orientation `φ = k · 180° / BINS`.
const ORIENTATIONS: [(f32, f32); BINS] = [
    (1.0, 0.0),
    (
        std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
    ),
    (0.0, 1.0),
    (
        -std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
    ),
    (-1.0, 0.0),
    (
        -std::f32::consts::FRAC_1_SQRT_2,
        -std::f32::consts::FRAC_1_SQRT_2,
    ),
    (0.0, -1.0),
    (
        std::f32::consts::FRAC_1_SQRT_2,
        -std::f32::consts::FRAC_1_SQRT_2,
    ),
];

thread_local! {
    /// The outline loader's caches, kept per thread: the trait pass
    /// describes thousands of glyphs.
    static CONTEXT: RefCell<ScaleContext> = RefCell::new(ScaleContext::new());
}

/// The shape descriptor of one glyph: [`LEN`] bytes, the largest 255.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descriptor {
    bytes: Vec<u8>,
}

impl Descriptor {
    /// A stored descriptor; `None` when it is not [`LEN`] bytes.
    pub fn from_bytes(bytes: Vec<u8>) -> Option<Self> {
        (bytes.len() == LEN).then_some(Self { bytes })
    }

    /// The bytes, as the index stores them.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The squared length of the byte vector.
    fn norm2(&self) -> u64 {
        self.bytes
            .iter()
            .map(|&b| u64::from(b) * u64::from(b))
            .sum()
    }
}

/// A descriptor with its length, for comparing one against many.
#[derive(Debug, Clone)]
pub struct Prepared {
    descriptor: Descriptor,
    norm2: u64,
}

impl Prepared {
    /// `descriptor` ready to compare.
    pub fn new(descriptor: Descriptor) -> Self {
        let norm2 = descriptor.norm2();
        Self { descriptor, norm2 }
    }

    /// The distance to `other` ([`distance`]).
    pub fn distance(&self, other: &Self) -> f32 {
        if self.norm2 == 0 || other.norm2 == 0 {
            return if self.norm2 == other.norm2 { 0.0 } else { 1.0 };
        }
        let dot: u64 = self
            .descriptor
            .bytes
            .iter()
            .zip(&other.descriptor.bytes)
            .map(|(&a, &b)| u64::from(a) * u64::from(b))
            .sum();
        // Each squared length is below 2^25, so their product is exact in an
        // f64 and the root of a square is exact: a drawing is at distance 0
        // from itself.
        let lengths = ((self.norm2 as f64) * (other.norm2 as f64)).sqrt();
        let cos = (dot as f64 / lengths).min(1.0);
        // Both vectors are non-negative: their unit distance is at most √2.
        ((2.0 - 2.0 * cos).max(0.0).sqrt() / std::f64::consts::SQRT_2) as f32
    }
}

/// How far apart two drawings are: the Euclidean distance of the unit
/// vectors of `a` and `b`, over √2, in `0..=1`; 0 is the same drawing.
pub fn distance(a: &Descriptor, b: &Descriptor) -> f32 {
    Prepared::new(a.clone()).distance(&Prepared::new(b.clone()))
}

/// The descriptor of character `code_point` of face `index` of the font
/// file `data` (TrueType, OpenType, a collection, WOFF or WOFF2), or `None`
/// (see the module docs).
pub fn describe(data: &[u8], index: u32, code_point: u32) -> Option<Descriptor> {
    describe_many(data, index, &[code_point])?.pop().flatten()
}

/// The descriptors of `code_points` of face `index` of the font file
/// `data`, in that order; `None` for a character the face does not draw.
/// `None` as a whole when the face cannot be read.
pub fn describe_many(
    data: &[u8],
    index: u32,
    code_points: &[u32],
) -> Option<Vec<Option<Descriptor>>> {
    let sfnt = sfnt_data(data).ok()?;
    check_face(&sfnt, index).ok()?;
    panic::catch_unwind(AssertUnwindSafe(|| {
        let font = FontRef::from_index(&sfnt, index as usize)?;
        let upem = f32::from(font.metrics(&[]).units_per_em);
        if upem <= 0.0 {
            return None;
        }
        let charmap = font.charmap();
        CONTEXT.with(|context| {
            let mut context = context.try_borrow_mut().ok()?;
            let mut scaler = context.builder(font).hint(false).build();
            let mut scratch = Scratch::new();
            Some(
                code_points
                    .iter()
                    .map(|&cp| {
                        let ch = char::from_u32(cp)?;
                        let gid = charmap.map(ch);
                        if gid == 0 {
                            return None;
                        }
                        let outline = scaler.scale_outline(gid)?;
                        let canvas = raster(&mut scratch, &outline, upem)?;
                        Some(from_canvas(&canvas))
                    })
                    .collect(),
            )
        })
    }))
    .ok()
    .flatten()
}

/// Describes `code_point` for each face of `todo` from its file, on the
/// disk threads; a face whose file cannot be read now is left out (and
/// tried again next time), a face that cannot be read as a font gets no
/// descriptor.
pub fn describe_todo(todo: &[DescriptorTodo], code_point: u32) -> Vec<DescribedFace> {
    let described = crate::parallel::map_ordered_io(
        todo,
        |t| {
            let data = match std::fs::read(&t.path) {
                Ok(data) => data,
                Err(e) => {
                    tracing::debug!(path = %t.path, error = %e, "glyph descriptor: file not read");
                    return None;
                }
            };
            let descriptor = describe(&data, t.collection_index, code_point);
            Some(DescribedFace {
                face: t.face,
                file_hash: t.file_hash.clone(),
                glyphs: vec![(code_point, descriptor)],
            })
        },
        &mut |_| {},
    );
    described.into_iter().flatten().collect()
}

/// The outline (font units) drawn at [`SIZE`] px to the em, its box centred
/// in the square: [`SIZE`]² coverage bytes, row by row from the top. `None`
/// for an empty or absurdly large outline.
fn raster(
    scratch: &mut Scratch,
    outline: &swash::scale::outline::Outline,
    upem: f32,
) -> Option<Vec<u8>> {
    if outline.points().is_empty() {
        return None;
    }
    let b = outline.bounds();
    let scale = SIZE as f32 / upem;
    let (w, h) = ((b.max.x - b.min.x) * scale, (b.max.y - b.min.y) * scale);
    let max = MAX_EMS * SIZE as f32;
    if !(w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 || w > max || h > max {
        return None;
    }
    let side = SIZE as f32;
    let (ox, oy) = ((side - w) / 2.0, (side - h) / 2.0);
    let points: Vec<Point> = outline
        .points()
        .iter()
        .map(|p| Point::new((p.x - b.min.x) * scale + ox, (p.y - b.min.y) * scale + oy))
        .collect();
    let (data, _) = Mask::with_scratch((&points[..], outline.verbs()), scratch)
        .format(Format::Alpha)
        .origin(Origin::BottomLeft)
        .size(SIZE as u32, SIZE as u32)
        .render();
    (data.len() == SIZE * SIZE && data.iter().any(|&c| c > 0)).then_some(data)
}

/// The descriptor of a [`SIZE`]² coverage canvas (steps 2 and 3).
fn from_canvas(canvas: &[u8]) -> Descriptor {
    let at = |x: isize, y: isize| -> f32 {
        if x < 0 || y < 0 || x >= SIZE as isize || y >= SIZE as isize {
            0.0
        } else {
            f32::from(canvas[y as usize * SIZE + x as usize]) / 255.0
        }
    };
    let cell = SIZE as f32 / CELLS as f32;
    let mut hist = vec![0f32; LEN];
    for y in 0..SIZE as isize {
        for x in 0..SIZE as isize {
            let gx = at(x + 1, y) - at(x - 1, y);
            let gy = at(x, y + 1) - at(x, y - 1);
            let m2 = gx * gx + gy * gy;
            if m2 == 0.0 {
                continue;
            }
            let m = m2.sqrt();
            let (cos2, sin2) = ((gx * gx - gy * gy) / m2, 2.0 * gx * gy / m2);
            // The cell centres either side of the pixel centre, and the share
            // of each.
            let (u, v) = ((x as f32 + 0.5) / cell - 0.5, (y as f32 + 0.5) / cell - 0.5);
            let (cu, cv) = (u.floor(), v.floor());
            let (fu, fv) = (u - cu, v - cv);
            for (k, &(c, s)) in ORIENTATIONS.iter().enumerate() {
                let along = (cos2 * c + sin2 * s).max(0.0);
                let vote = along * along * m;
                if vote == 0.0 {
                    continue;
                }
                for (dv, wv) in [(0.0, 1.0 - fv), (1.0, fv)] {
                    let row = cv + dv;
                    if row < 0.0 || row >= CELLS as f32 || wv == 0.0 {
                        continue;
                    }
                    for (du, wu) in [(0.0, 1.0 - fu), (1.0, fu)] {
                        let col = cu + du;
                        if col < 0.0 || col >= CELLS as f32 || wu == 0.0 {
                            continue;
                        }
                        let i = (row as usize * CELLS + col as usize) * BINS + k;
                        hist[i] += vote * wu * wv;
                    }
                }
            }
        }
    }
    normalise(&mut hist);
    for v in &mut hist {
        *v = v.min(CLIP);
    }
    normalise(&mut hist);
    let max = hist.iter().copied().fold(0.0f32, f32::max);
    let bytes = hist
        .iter()
        .map(|&v| {
            if max > 0.0 {
                (v / max * 255.0).round().clamp(0.0, 255.0) as u8
            } else {
                0
            }
        })
        .collect();
    Descriptor { bytes }
}

/// Scales `v` to unit length; a zero vector stays zero.
fn normalise(v: &mut [f32]) {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        for x in v {
            *x /= n;
        }
    }
}

#[cfg(test)]
mod tests;
