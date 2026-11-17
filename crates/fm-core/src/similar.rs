//! Visual similarity (E23.2): how close two faces look, and why.
//!
//! A face is described by a [`Profile`]: its classification, a few metrics
//! from the index, and its glyph fingerprint ([`crate::fingerprint`]) when
//! the scan took one. [`compare`] scores two profiles component by
//! component, each component a closeness in `0.0..=1.0`:
//!
//! | Component | With fingerprints | Without |
//! |-----------|-------------------|---------|
//! | classification | same class, close classes (serif and slab, sans and mono) or not | same |
//! | x-height | `v` over `H`, measured | declared x-height over cap height |
//! | width | mean advance over the x-height | `OS/2` width class |
//! | stroke weight | stem of the `i` over the x-height | `OS/2` weight class |
//! | contrast | thin over thick stroke of the `o` | — |
//! | letter shapes | the reduced drawing, by correlation (by ink for an image) | — |
//!
//! The score is the weighted mean of the components, so it stays in
//! `0.0..=1.0`; a component one face lacks (no class, no fingerprint) counts
//! as half alike ([`UNKNOWN`]). Each component close enough names itself in
//! the reasons ("similar width"). When either face has no fingerprint the
//! pair is compared by metrics and classification only, and the reasons say
//! so.
//!
//! Who is ranked is [`candidate`]'s choice (E54.11): only pairs that both
//! have a fingerprint, and never a symbol face for a text face or the other
//! way round. So a pair compared on less (metrics only) never outranks one
//! compared on everything: it is not ranked at all. A pair scoring below
//! [`MIN_SCORE`] is not returned either.
//! A profile with [`Profile::shapes_only`] (an image of text, E33.1) has
//! its x-height, contrast and stroke weight unknown.
//!
//! Everything is plain `f32` arithmetic over stored values: the same index
//! ranks the same way on every OS.

use fm_api::common::{Classification, Metrics};

use crate::fingerprint::Fingerprint;

/// The reason of the classification component.
pub const SAME_CLASSIFICATION: &str = "same classification";
/// The reason of the x-height component.
pub const MATCHING_X_HEIGHT: &str = "matching x-height";
/// The reason of the width component.
pub const SIMILAR_WIDTH: &str = "similar width";
/// The reason of the contrast component.
pub const SIMILAR_CONTRAST: &str = "similar contrast";
/// The reason of the stroke-weight component.
pub const SIMILAR_STROKE_WEIGHT: &str = "similar stroke weight";
/// The reason of the letter-shape component.
pub const SIMILAR_SHAPES: &str = "similar letter shapes";
/// Added when the pair is compared without fingerprints.
pub const METRICS_ONLY: &str = "compared by metrics and classification only (no fingerprint)";

/// A component this close or closer is named in the reasons.
const REASON_AT: f32 = 0.6;

/// The closeness of a component one face lacks: neither alike nor unlike.
const UNKNOWN: f32 = 0.5;

/// The lowest score a similar face is returned with (E54.11).
///
/// How it was chosen: a score is a weighted mean of closenesses, and
/// [`UNKNOWN`] (0.5) is "neither alike nor unlike". A pair below it is, on
/// balance, more unlike than alike, so calling it similar is wrong whatever
/// the rest of the library holds. Checked on `fixtures/fonts/google-repo`
/// (2026-11-17): for Libre Baskerville every serif scores 0.58 or more, and
/// the faces below 0.5 are sans, scripts (Clicker Script, Miniver) and
/// display faces; for Barlow Bold Italic, Libre Baskerville (0.23) and
/// Clicker Script fall below it (`index::search::similar` tests). E54.13
/// tunes it against agreed pairs.
pub const MIN_SCORE: f32 = 0.5;

/// How much each component counts in the score. Tuned against the agreed
/// pairs (`fixtures/similar/expected.json`, E54.13): with these, every
/// reference ranks each of its "near" faces above each of its "far" faces.
///
/// Classification: was 3.0. The class is a guess (a script read as a sans,
/// a display face as a sans), and at 3.0 a wrong guess outweighed the
/// measures that saw the pair alike (Zeyada and Just Me Again Down Here).
const W_CLASS: f32 = 2.0;
/// x-height: was 1.0. The proportion of small to capital letters is what
/// sets connected scripts (Clicker Script, Engagement) apart from text
/// faces, and it is measured, not guessed.
const W_X_HEIGHT: f32 = 3.5;
/// Width: was 1.5. A condensed face for a wide one is the fault people
/// noticed first (Saira Extra Condensed above Lexend Peta for Lexend Tera).
const W_WIDTH: f32 = 2.5;
/// Stroke weight: was 1.5. The pairs put the light and the black of one
/// design near each other (Lexend Tera, Public Sans): weight counts, but
/// less than the build of the letters.
const W_STROKE: f32 = 1.0;
/// Contrast: was 1.0. Thin and thick strokes tell a Bodoni from a
/// grotesque and a pen script from a marker.
const W_CONTRAST: f32 = 1.5;
/// Letter shapes: was 3.0. The drawing is now compared by correlation
/// ([`shapes`]), which no longer counts weight twice; it stays a strong
/// component, below the x-height and the width it partly repeats.
const W_SHAPES: f32 = 2.0;

/// The classification closeness of two classes that are close (serif and
/// slab, sans and mono): neither the same nor unrelated. Until E54.13 a
/// slab was as far from a serif as a script was.
const CLOSE_CLASSES: f32 = 0.5;

/// The difference at which a component's closeness reaches zero.
/// x-height over cap height, a typical face lies between 0.6 and 0.8: was
/// 0.12, which made a 0.07 difference (Zeyada and Just Me Again Down Here)
/// count as half unlike; 0.35 grades the whole range.
const TOL_X_HEIGHT: f32 = 0.35;
/// Relative difference of the advance ratio: was 0.25. Sibling widths of
/// the pairs differ by 0.1 or less (Lexend Tera and Lexend Peta: 0.09), a
/// condensed face against a normal one by 0.3 or more.
const TOL_WIDTH: f32 = 0.17;
/// Relative difference of the stem ratio: was 0.5. A light and a heavy of
/// one design differ by about 0.5 (Lexend Tera and its ExtraLight) and are near (see [`W_STROKE`]).
const TOL_STROKE: f32 = 0.85;
/// Width classes (1–9) and weight classes (100–900), for a pair without
/// fingerprints (not ranked, E54.11). Unchanged.
const TOL_WIDTH_CLASS: f32 = 3.0;
const TOL_WEIGHT_CLASS: f32 = 400.0;
/// Contrast, in `0.0..=1.0`: was 0.3. A text serif against a didone
/// differs by about 0.2 and is near (Libre Baskerville and Libre Bodoni).
const TOL_CONTRAST: f32 = 0.55;
/// One minus the correlation of two drawings' cells (was a mean absolute
/// difference of 0.12): drawings that correlate less than 0.9 do not look
/// alike.
const TOL_SHAPES: f32 = 0.1;
/// The mean absolute difference of two drawings' cells at which an image
/// of text (a shapes-only profile, E33.1) is unlike a face. Unchanged: an
/// image has no stroke measure, so its ink is the only sign of weight, and
/// the thin strokes of a script image correlate less (0.85 for Clicker
/// Script) than their ink differs (0.04).
const TOL_INK: f32 = 0.12;

/// What [`compare`] looks at in one face.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Profile {
    /// Design class; `None` when nothing says.
    pub classification: Option<Classification>,
    /// `OS/2.usWeightClass`.
    pub weight: u16,
    /// `OS/2.usWidthClass`.
    pub width: u16,
    /// Declared (or measured) x-height over cap height; `None` without both.
    pub x_height_ratio: Option<f32>,
    /// The glyph fingerprint; `None` when the face has none.
    pub fingerprint: Option<Fingerprint>,
    /// Only the fingerprint's drawing and width are known (an image of text
    /// that is not the sample, E33.1): its x-height, contrast and stroke
    /// count as unknown.
    pub shapes_only: bool,
}

impl Profile {
    /// A profile from the index's values.
    pub fn new(
        classification: Option<Classification>,
        weight: u16,
        width: u16,
        metrics: Option<&Metrics>,
        fingerprint: Option<Fingerprint>,
    ) -> Self {
        let x_height_ratio = metrics.and_then(|m| match (m.x_height, m.cap_height) {
            (Some(x), Some(cap)) if x > 0 && cap > 0 => Some(f32::from(x) / f32::from(cap)),
            _ => None,
        });
        Self {
            classification,
            weight,
            width,
            x_height_ratio,
            fingerprint,
            shapes_only: false,
        }
    }
}

/// How close two faces look.
#[derive(Debug, Clone, PartialEq)]
pub struct Similarity {
    /// `0.0..=1.0`; higher is closer.
    pub score: f32,
    /// The components that match, in plain words, in the order of the table
    /// in the module docs; [`METRICS_ONLY`] last when it applies.
    pub reasons: Vec<&'static str>,
}

/// Whether `b` is ranked as similar to the reference `a` (E54.11): both
/// have a glyph fingerprint, and both or neither are symbol faces (icons,
/// dingbats). A pair compared on metrics alone, or a symbol face for a text
/// face, is no candidate.
pub fn candidate(a: &Profile, b: &Profile) -> bool {
    a.fingerprint.is_some() && b.fingerprint.is_some() && is_symbol(a) == is_symbol(b)
}

/// A symbol, icon or dingbat face.
pub fn is_symbol(p: &Profile) -> bool {
    p.classification == Some(Classification::Symbol)
}

/// Scores `b` against `a` (symmetric).
pub fn compare(a: &Profile, b: &Profile) -> Similarity {
    let class = a
        .classification
        .zip(b.classification)
        .map(|(x, y)| classes(x, y));
    let both = a.fingerprint.as_ref().zip(b.fingerprint.as_ref());
    let measured = !(a.shapes_only || b.shapes_only);
    let (x_height, width, contrast, stroke, shape) = match both {
        Some((x, y)) => (
            measured.then(|| closeness(x.x_height_ratio, y.x_height_ratio, TOL_X_HEIGHT)),
            relative(x.advance_ratio, y.advance_ratio, TOL_WIDTH),
            measured.then(|| closeness(x.contrast, y.contrast, TOL_CONTRAST)),
            measured.then(|| relative(x.stroke_width, y.stroke_width, TOL_STROKE)),
            Some(if measured {
                shapes(&x.vector, &y.vector)
            } else {
                ink(&x.vector, &y.vector)
            }),
        ),
        None => (
            a.x_height_ratio
                .zip(b.x_height_ratio)
                .map(|(x, y)| closeness(x, y, TOL_X_HEIGHT)),
            closeness(f32::from(a.width), f32::from(b.width), TOL_WIDTH_CLASS),
            None,
            Some(closeness(
                f32::from(a.weight),
                f32::from(b.weight),
                TOL_WEIGHT_CLASS,
            )),
            None,
        ),
    };
    let parts = [
        (W_CLASS, class, SAME_CLASSIFICATION),
        (W_X_HEIGHT, x_height, MATCHING_X_HEIGHT),
        (W_WIDTH, Some(width), SIMILAR_WIDTH),
        (W_CONTRAST, contrast, SIMILAR_CONTRAST),
        (W_STROKE, stroke, SIMILAR_STROKE_WEIGHT),
        (W_SHAPES, shape, SIMILAR_SHAPES),
    ];
    let total: f32 = parts.iter().map(|(w, _, _)| w).sum();
    let score = parts
        .iter()
        .map(|(w, s, _)| w * s.unwrap_or(UNKNOWN))
        .sum::<f32>()
        / total;
    let mut reasons: Vec<&'static str> = parts
        .iter()
        .filter(|(_, s, _)| s.is_some_and(|s| s >= REASON_AT))
        .map(|(_, _, r)| *r)
        .collect();
    if both.is_none() {
        reasons.push(METRICS_ONLY);
    }
    Similarity {
        score: score.clamp(0.0, 1.0),
        reasons,
    }
}

/// `1.0` for equal values, falling to `0.0` at a difference of `tol`; `0.0`
/// for a value that is not a number.
fn closeness(a: f32, b: f32, tol: f32) -> f32 {
    let d = (a - b).abs() / tol;
    if d.is_nan() { 0.0 } else { (1.0 - d).max(0.0) }
}

/// [`closeness`] of the difference relative to the larger value.
fn relative(a: f32, b: f32, tol: f32) -> f32 {
    let larger = a.abs().max(b.abs());
    if larger == 0.0 {
        return 1.0;
    }
    closeness(a / larger, b / larger, tol)
}

/// The classification closeness of two classes: 1.0 for the same class,
/// [`CLOSE_CLASSES`] for two that are close, 0.0 for the rest.
fn classes(a: Classification, b: Classification) -> f32 {
    use Classification::{Mono, Sans, Serif, Slab};
    match (a, b) {
        _ if a == b => 1.0,
        (Serif, Slab) | (Slab, Serif) | (Sans, Mono) | (Mono, Sans) => CLOSE_CLASSES,
        _ => 0.0,
    }
}

/// [`closeness`] of two drawings by the mean absolute difference of their
/// cells; drawings of different lengths (other parameters) do not compare.
fn ink(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let total: f32 = a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum();
    closeness(total / a.len() as f32, 0.0, TOL_INK)
}

/// The summed squared deviation below which a drawing counts as without
/// variation: rounding leaves a little on an even drawing, and a real one
/// (64 cells of letters and white) has about 1.0.
const MIN_VARIATION: f32 = 1e-6;

/// [`closeness`] of two drawings by the correlation of their cells (E54.13):
/// where the ink is, not how much of it. A bold and a light face of one
/// design correlate highly; a mean difference of the cells called them
/// unlike. Drawings of different lengths (other parameters) do not compare;
/// a drawing with no variation correlates only with an equal one.
fn shapes(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let n = a.len() as f32;
    let mean_a = a.iter().sum::<f32>() / n;
    let mean_b = b.iter().sum::<f32>() / n;
    let (mut ab, mut aa, mut bb) = (0.0f32, 0.0f32, 0.0f32);
    for (x, y) in a.iter().zip(b) {
        let (x, y) = (x - mean_a, y - mean_b);
        ab += x * y;
        aa += x * x;
        bb += y * y;
    }
    let r = if aa > MIN_VARIATION && bb > MIN_VARIATION {
        ab / (aa * bb).sqrt()
    } else if a == b {
        1.0
    } else {
        0.0
    };
    closeness(1.0 - r, 0.0, TOL_SHAPES)
}

#[cfg(test)]
mod tests;
