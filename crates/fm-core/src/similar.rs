//! Visual similarity (E23.2): how close two faces look, and why.
//!
//! A face is described by a [`Profile`]: its classification, a few metrics
//! from the index, and its glyph fingerprint ([`crate::fingerprint`]) when
//! the scan took one. [`compare`] scores two profiles component by
//! component, each component a closeness in `0.0..=1.0`:
//!
//! | Component | With fingerprints | Without |
//! |-----------|-------------------|---------|
//! | classification | same class or not | same |
//! | x-height | `v` over `H`, measured | declared x-height over cap height |
//! | width | mean advance over the x-height | `OS/2` width class |
//! | stroke weight | stem of the `i` over the x-height | `OS/2` weight class |
//! | contrast | thin over thick stroke of the `o` | — |
//! | letter shapes | the reduced drawing | — |
//!
//! The score is the weighted mean of the components, so it stays in
//! `0.0..=1.0`; a component one face lacks (no class, no fingerprint) counts
//! as half alike, so a pair compared on little never outranks a pair compared
//! on everything. Each component close enough names itself in the reasons
//! ("similar width"). When either face has no fingerprint the pair is
//! compared by metrics and classification only, and the reasons say so.
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

/// The closeness of a component one face lacks: neither alike nor unlike,
/// so a pair that is compared on less never outranks one compared on all.
const UNKNOWN: f32 = 0.5;

/// How much each component counts in the score.
const W_CLASS: f32 = 3.0;
const W_X_HEIGHT: f32 = 1.0;
const W_WIDTH: f32 = 1.5;
const W_STROKE: f32 = 1.5;
const W_CONTRAST: f32 = 1.0;
const W_SHAPES: f32 = 3.0;

/// The difference at which a component's closeness reaches zero.
/// x-height over cap height: a typical face lies between 0.6 and 0.8.
const TOL_X_HEIGHT: f32 = 0.12;
/// Relative difference of the advance ratio and the stem ratio.
const TOL_WIDTH: f32 = 0.25;
const TOL_STROKE: f32 = 0.5;
/// Width classes (1–9) and weight classes (100–900).
const TOL_WIDTH_CLASS: f32 = 3.0;
const TOL_WEIGHT_CLASS: f32 = 400.0;
/// Contrast, in `0.0..=1.0`.
const TOL_CONTRAST: f32 = 0.3;
/// The mean absolute difference of two drawings' cells.
const TOL_SHAPES: f32 = 0.12;

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

/// Scores `b` against `a` (symmetric).
pub fn compare(a: &Profile, b: &Profile) -> Similarity {
    let class = a
        .classification
        .zip(b.classification)
        .map(|(x, y)| if x == y { 1.0 } else { 0.0 });
    let both = a.fingerprint.as_ref().zip(b.fingerprint.as_ref());
    let (x_height, width, contrast, stroke, shape) = match both {
        Some((x, y)) => (
            Some(closeness(x.x_height_ratio, y.x_height_ratio, TOL_X_HEIGHT)),
            relative(x.advance_ratio, y.advance_ratio, TOL_WIDTH),
            Some(closeness(x.contrast, y.contrast, TOL_CONTRAST)),
            relative(x.stroke_width, y.stroke_width, TOL_STROKE),
            Some(shapes(&x.vector, &y.vector)),
        ),
        None => (
            a.x_height_ratio
                .zip(b.x_height_ratio)
                .map(|(x, y)| closeness(x, y, TOL_X_HEIGHT)),
            closeness(f32::from(a.width), f32::from(b.width), TOL_WIDTH_CLASS),
            None,
            closeness(f32::from(a.weight), f32::from(b.weight), TOL_WEIGHT_CLASS),
            None,
        ),
    };
    let parts = [
        (W_CLASS, class, SAME_CLASSIFICATION),
        (W_X_HEIGHT, x_height, MATCHING_X_HEIGHT),
        (W_WIDTH, Some(width), SIMILAR_WIDTH),
        (W_CONTRAST, contrast, SIMILAR_CONTRAST),
        (W_STROKE, Some(stroke), SIMILAR_STROKE_WEIGHT),
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

/// [`closeness`] of two drawings by the mean absolute difference of their
/// cells; drawings of different lengths (other parameters) do not compare.
fn shapes(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let total: f32 = a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum();
    closeness(total / a.len() as f32, 0.0, TOL_SHAPES)
}

#[cfg(test)]
mod tests;
