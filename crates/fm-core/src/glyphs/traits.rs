//! The glyph trait detectors (E49.4): one per trait of the catalog
//! ([`crate::glyph_traits`]), each reading the outline of its glyph at the
//! default instance ([`crate::glyphs::geometry`]).
//!
//! - **Upright first.** An italic is sheared back by its slant before any
//!   measure: the lean of the stem of `l` or `I` as drawn, or the `post`
//!   italic angle when neither is one straight stroke.
//! - **Every trait is detected**, enabled in the catalog or not: a trait
//!   shipped disabled (its detector below the accuracy bar) is stored too,
//!   so turning it on needs no new pass.
//! - **Value and confidence.** A detector answers the value (for a scalar
//!   trait, the chip its numbers fall in, and the numbers) and a confidence
//!   in `0..=1` from how far the measure lies from the boundary between two
//!   values. A measure in the band between them is a low confidence (below
//!   [`CONFIDENT`]), never a guess passed off as a fact.
//! - **No glyph, no answer.** A face that does not map the trait's character,
//!   or whose glyph has no outline, has no detection for that trait. A
//!   glyph whose shape the detector cannot read (no stem where one must be)
//!   is a detection with confidence 0.
//! - **Versions.** Each detector has its own version ([`version`]); a change
//!   to one detector bumps its version, and the trait pass measures that
//!   trait again on every face, and only that trait.
//! - Malformed data is never a panic: a face that cannot be read has no
//!   detections.

mod shape;

use read_fonts::TableProvider;

use crate::glyph_traits::{self, GlyphTrait, TraitKind};
use crate::glyphs::geometry::{GlyphOutline, Point, font_ref};
use crate::glyphs::sfnt;
use shape::{Shape, fit, median};

/// The confidence from which a detection counts as decided: the picker and
/// the search use detections at or above it.
pub const CONFIDENT: f64 = 0.7;

/// The characters the detectors read, in the order of [`DETECTORS`]'s glyphs.
const CHARS: [char; 13] = [
    'R', 'O', 'g', 'a', 'G', 'Q', 't', '1', '7', 'I', 'l', 'M', 'e',
];

/// One detector: the trait it answers, its version and the function.
struct Detector {
    id: &'static str,
    version: u32,
    detect: fn(&Shape) -> Measured,
}

/// Every detector, one per trait of the catalog.
const DETECTORS: [Detector; 13] = [
    Detector {
        id: "R.leg",
        version: 1,
        detect: r_leg,
    },
    Detector {
        id: "O.roundness",
        version: 1,
        detect: o_roundness,
    },
    Detector {
        id: "g.storeys",
        version: 1,
        detect: g_storeys,
    },
    Detector {
        id: "a.storeys",
        version: 1,
        detect: a_storeys,
    },
    Detector {
        id: "G.spur",
        version: 1,
        detect: g_spur,
    },
    Detector {
        id: "Q.tail",
        version: 1,
        detect: q_tail,
    },
    Detector {
        id: "t.terminal",
        version: 1,
        detect: t_terminal,
    },
    Detector {
        id: "one.flag",
        version: 1,
        detect: one_flag,
    },
    Detector {
        id: "seven.bar",
        version: 1,
        detect: seven_bar,
    },
    Detector {
        id: "I.form",
        version: 1,
        detect: i_form,
    },
    Detector {
        id: "l.form",
        version: 1,
        detect: l_form,
    },
    Detector {
        id: "M.sides",
        version: 1,
        detect: m_sides,
    },
    Detector {
        id: "e.bar",
        version: 1,
        detect: e_bar,
    },
];

/// What one detector found on one face.
#[derive(Debug, Clone, PartialEq)]
pub struct Detection {
    /// The trait id (`R.leg`).
    pub trait_id: &'static str,
    /// The value id (`straight`); for a scalar trait, the chip of `scalars`.
    pub value: String,
    /// A scalar trait's measures by id, in catalog order; empty for an enum.
    pub scalars: Vec<(String, f64)>,
    /// How sure the detector is, in `0..=1`.
    pub confidence: f64,
}

/// The detections of one face and how it was measured.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceTraits {
    /// The slant the outlines were sheared back by, in degrees (positive
    /// leans right).
    pub slant_degrees: f64,
    /// Whether the face is variable: then every detection is of its default
    /// instance.
    pub variable: bool,
    /// One entry per detector, in [`trait_ids`] order: `None` when the face
    /// lacks the glyph.
    pub traits: Vec<Option<Detection>>,
}

/// The trait ids with a detector, in detector order.
pub fn trait_ids() -> impl Iterator<Item = &'static str> {
    DETECTORS.iter().map(|d| d.id)
}

/// The version of the detector of `trait_id`; `None` for a trait without one.
pub fn version(trait_id: &str) -> Option<u32> {
    DETECTORS
        .iter()
        .find(|d| d.id == trait_id)
        .map(|d| d.version)
}

/// Every trait with its detector version, in detector order.
pub fn versions() -> impl Iterator<Item = (&'static str, u32)> {
    DETECTORS.iter().map(|d| (d.id, d.version))
}

/// Runs every detector on face `collection_index` of the font file `data`
/// (TrueType, OpenType, a collection, WOFF or WOFF2). `None` when the face
/// cannot be read.
pub fn detect(data: &[u8], collection_index: u32) -> Option<FaceTraits> {
    detect_only(data, collection_index, &|_| true)
}

/// [`detect`] for the traits `wanted` accepts; the others are `None`.
pub fn detect_only(
    data: &[u8],
    collection_index: u32,
    wanted: &dyn Fn(&str) -> bool,
) -> Option<FaceTraits> {
    let (slant_degrees, variable, raw) = measure(data, collection_index, wanted)?;
    Some(FaceTraits {
        slant_degrees,
        variable,
        traits: raw
            .into_iter()
            .map(|r| r.map(|(id, m)| finish(id, m)))
            .collect(),
    })
}

/// Runs every detector `wanted` accepts: the slant, whether the face is
/// variable and the detectors' raw answers.
pub(crate) fn measure(
    data: &[u8],
    collection_index: u32,
    wanted: &dyn Fn(&str) -> bool,
) -> Option<RawTraits> {
    let sfnt = sfnt(data).ok()?;
    let font = font_ref(&sfnt, collection_index)?;
    let variable = font.fvar().is_ok_and(|f| f.axis_count() > 0);
    let mut chars: Vec<char> = CHARS.to_vec();
    chars.extend(['l', 'I']);
    let mut outlines = GlyphOutline::load_many(&font, &chars)?;
    let stem_outlines = [outlines.pop().flatten(), outlines.pop().flatten()];
    let stated = font
        .post()
        .ok()
        .map(|p| f64::from(p.italic_angle().to_f32()))
        .filter(|a| a.is_finite() && a.abs() >= 0.5 && a.abs() <= 45.0)
        // The `post` angle is counter-clockwise from vertical: an italic
        // leaning right has a negative one.
        .map(|a| -a);
    // The drawn stem is the truth; a stated angle is often rounded or
    // missing.
    let slant_degrees = stem_outlines
        .iter()
        .flatten()
        .find_map(stem_slant)
        .or(stated)
        .unwrap_or(0.0);
    let traits = DETECTORS
        .iter()
        .zip(outlines)
        .map(|(d, outline)| {
            if !wanted(d.id) {
                return None;
            }
            let shape = Shape::new(outline?, slant_degrees)?;
            Some((d.id, (d.detect)(&shape)))
        })
        .collect();
    Some((slant_degrees, variable, traits))
}

/// What [`measure`] answers.
type RawTraits = (f64, bool, Vec<Option<(&'static str, Measured)>>);

/// A detection from a detector's raw answer; a scalar trait takes the chip
/// of its numbers from the catalog.
fn finish(trait_id: &'static str, m: Measured) -> Detection {
    let catalog = glyph_traits::find(trait_id).ok().flatten();
    let measured: Vec<(&str, f64)> = m.scalars.iter().map(|(k, v)| (*k, *v)).collect();
    let (value, confidence) = match catalog {
        Some(t) if t.kind == TraitKind::Scalar => match t.chip(&measured) {
            Some(chip) => (
                chip.id.clone(),
                m.confidence
                    .min(scalar_confidence(t, chip, &measured, &m.bands)),
            ),
            None => (m.value.to_owned(), 0.0),
        },
        _ => (m.value.to_owned(), m.confidence),
    };
    Detection {
        trait_id,
        value,
        scalars: m
            .scalars
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
        confidence: confidence.clamp(0.0, 1.0),
    }
}

/// The confidence of a scalar chip: the least distance of a measure from a
/// threshold that decided it (the chip's own and those of every value
/// before it, which did not hold), in units of the measure's band.
/// [`CONFIDENT`] one band away, 1 at two and a half.
fn scalar_confidence(
    t: &GlyphTrait,
    chip: &glyph_traits::TraitValue,
    measured: &[(&str, f64)],
    bands: &[(&str, f64)],
) -> f64 {
    let mut least = f64::INFINITY;
    for value in &t.values {
        for th in &value.when {
            let Some((_, x)) = measured.iter().find(|(id, _)| *id == th.measure) else {
                continue;
            };
            let band = bands
                .iter()
                .find(|(id, _)| *id == th.measure)
                .map_or(f64::INFINITY, |(_, b)| *b);
            for bound in [th.min, th.max].into_iter().flatten() {
                least = least.min((x - bound).abs() / band);
            }
        }
        if value.id == chip.id {
            break;
        }
    }
    if least.is_infinite() {
        1.0
    } else if least < 1.0 {
        0.3 + 0.4 * least
    } else {
        CONFIDENT + (1.0 - CONFIDENT) * ((least - 1.0) / 1.5).min(1.0)
    }
}

/// The lean of a stem glyph (`l`, `I`) in degrees, positive to the right:
/// the line through the middles of its inked runs between a quarter and
/// three quarters of its height; 0 below 1.5°. `None` when it leans more
/// than 30°, or is not one stroke there.
fn stem_slant(outline: &GlyphOutline) -> Option<f64> {
    let shape = Shape::new(outline.clone(), 0.0)?;
    let mut points = Vec::new();
    for i in 0..=10 {
        let y = shape.y_at(0.25 + 0.05 * f64::from(i));
        let row = shape.row(y);
        if row.len() != 1 {
            return None;
        }
        points.push((y, (row[0].0 + row[0].1) / 2.0));
    }
    let (dx_dy, _) = fit(&points, shape.height())?;
    let deg = dx_dy.atan().to_degrees();
    match deg.abs() {
        a if a < 1.5 => Some(0.0),
        a if a <= 30.0 => Some(deg),
        _ => None,
    }
}

/// A detector's answer before the catalog's chip.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Measured {
    pub(crate) value: &'static str,
    pub(crate) confidence: f64,
    pub(crate) scalars: Vec<(&'static str, f64)>,
    /// The detector's inner numbers, for the accuracy report only.
    pub(crate) debug: Vec<(&'static str, f64)>,
    /// A scalar trait: per measure, the distance from a threshold within
    /// which a number does not decide the chip.
    pub(crate) bands: Vec<(&'static str, f64)>,
}

impl Measured {
    fn new(value: &'static str, confidence: f64) -> Self {
        Self {
            value,
            confidence,
            scalars: Vec::new(),
            debug: Vec::new(),
            bands: Vec::new(),
        }
    }

    /// A shape the detector could not read.
    fn unread(value: &'static str) -> Self {
        Self::new(value, 0.0)
    }
}

/// Picks `low` below `lo` and `high` above `hi`. The confidence is
/// [`CONFIDENT`] at the threshold and rises to 1 one band width beyond it;
/// between the thresholds the nearer value wins below [`CONFIDENT`].
fn split(x: f64, lo: f64, hi: f64, low: &'static str, high: &'static str) -> Measured {
    if !x.is_finite() {
        return Measured::unread(low);
    }
    let band = (hi - lo).max(f64::EPSILON);
    if x <= lo {
        Measured::new(
            low,
            CONFIDENT + (1.0 - CONFIDENT) * ((lo - x) / band).min(1.0),
        )
    } else if x >= hi {
        Measured::new(
            high,
            CONFIDENT + (1.0 - CONFIDENT) * ((x - hi) / band).min(1.0),
        )
    } else {
        let mid = (lo + hi) / 2.0;
        let lean = ((x - mid).abs() / (band / 2.0)).min(1.0);
        let confidence = 0.3 + 0.35 * lean;
        Measured::new(if x < mid { low } else { high }, confidence)
    }
}

/// The confidence of a ramp: [`CONFIDENT`] at `at`, 1 at `full`.
fn ramp(x: f64, at: f64, full: f64) -> f64 {
    let t = ((x - at) / (full - at)).clamp(0.0, 1.0);
    CONFIDENT + (1.0 - CONFIDENT) * t
}

/// The stem of an upright stroke glyph at fraction `f` of its height: the
/// widest inked run there.
fn stem_at(s: &Shape, f: f64) -> Option<(f64, f64)> {
    s.row(s.y_at(f))
        .into_iter()
        .max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0)))
}

// ---------------------------------------------------------------- R.leg

/// The leg's centre line between the foot and the bowl: straight when it
/// stays near its chord, curved when its lower part bends too, both when
/// only the upper part bends.
fn r_leg(s: &Shape) -> Measured {
    let h = s.height();
    let mut mids = Vec::new();
    for i in 0..=14 {
        let f = 0.08 + 0.025 * f64::from(i);
        let y = s.y_at(f);
        let row = s.row(y);
        if row.len() < 2 {
            continue;
        }
        let leg = row[row.len() - 1];
        mids.push((y, (leg.0 + leg.1) / 2.0));
    }
    if mids.len() < 8 {
        return Measured::unread("straight");
    }
    let Some((_, all)) = fit(&mids, h) else {
        return Measured::unread("straight");
    };
    let lower = &mids[..mids.len() * 3 / 5];
    let low = fit(lower, h).map_or(1.0, |f| f.1);
    let mut m = if all < 0.025 {
        Measured::new("straight", ramp(-all, -0.025, -0.008))
    } else if all > 0.04 {
        if low < 0.012 {
            Measured::new(
                "both",
                ramp(-low, -0.012, -0.004).min(ramp(all, 0.04, 0.07)),
            )
        } else {
            Measured::new("curved", ramp(low, 0.012, 0.03).min(ramp(all, 0.04, 0.07)))
        }
    } else {
        Measured::new("straight", 0.4)
    };
    m.debug = vec![("all", all), ("low", low)];
    m
}

// ---------------------------------------------------------- O.roundness

/// Width over height of the outer contour, and how far it bulges past the
/// ellipse of its box near the diagonals, over the height. The catalog's
/// thresholds pick the chip; [`finish`] takes the confidence from how far
/// the numbers lie from the thresholds that decide it.
fn o_roundness(s: &Shape) -> Measured {
    let b = s.bbox;
    let (w, h) = (b.width(), b.height());
    let (cx, cy) = ((b.x_min + b.x_max) / 2.0, (b.y_min + b.y_max) / 2.0);
    let (rx, ry) = (w / 2.0, h / 2.0);
    // The outer outline: in each of 72 directions from the centre, the
    // farthest ink along the ray, in units of the box ellipse's radius.
    let (mut corner, mut corners) = (0.0_f64, 0.0_f64);
    for k in 0..72 {
        let a = f64::from(k) * std::f64::consts::TAU / 72.0;
        // Only within 20° of a diagonal of the box.
        if (20.0..70.0).contains(&(a.to_degrees() - 45.0).rem_euclid(90.0)) {
            continue;
        }
        let (sin, cos) = a.sin_cos();
        let steps = 400;
        let hit = (0..=steps)
            .rev()
            .map(|i| f64::from(i) / f64::from(steps) * 1.2)
            .find(|r| s.inked(Point::new(cx + r * rx * cos, cy + r * ry * sin)));
        if let Some(r) = hit {
            let len = (rx * cos).hypot(ry * sin);
            corner += (r - 1.0) * len / h;
            corners += 1.0;
        }
    }
    if corners < 12.0 {
        return Measured::unread("round");
    }
    Measured {
        value: "round",
        confidence: 1.0,
        scalars: vec![("roundness", w / h), ("corners", corner / corners)],
        debug: Vec::new(),
        bands: vec![("roundness", 0.04), ("corners", 0.004)],
    }
}

// ---------------------------------------------------- g.storeys, a.storeys

/// Two counters, one above the other, are a double-storey `g`; one counter
/// above the baseline with a descending tail is a single-storey one.
fn g_storeys(s: &Shape) -> Measured {
    let counters = s.outline.counters();
    let h = s.height();
    match counters.len() {
        0 => {
            // An open lower loop and an open bowl: undecided.
            Measured::new("single", 0.3)
        }
        1 => {
            let c = counters[0].bbox;
            // A single-storey bowl sits on the x-height band, above the
            // baseline; a closed lower loop alone (open upper bowl) is rare.
            let above = (c.y_min - s.bbox.y_min) / h;
            let mut m = split(above, 0.2, 0.3, "double", "single");
            // An open lower loop below a bowl is still double: ink below
            // the bowl that spans to the left of it again.
            if m.value == "single" && open_lower_loop(s, c.y_min, c.x_min) {
                m = Measured::new("double", 0.6);
            }
            m
        }
        _ => {
            let mut cs: Vec<_> = counters.iter().map(|c| c.bbox).collect();
            cs.sort_by(|a, b| b.y_max.total_cmp(&a.y_max));
            let gap = (cs[0].y_min - cs[1].y_max) / h;
            if gap > -0.05 {
                Measured::new("double", 0.95)
            } else {
                Measured::new("double", 0.5)
            }
        }
    }
}

/// Whether ink below the bowl (under `bowl_bottom`) holds two strokes on one
/// row that reach left of the bowl's counter: the sides of an open loop.
fn open_lower_loop(s: &Shape, bowl_bottom: f64, counter_left: f64) -> bool {
    let low = s.bbox.y_min;
    let span = bowl_bottom - low;
    (1..8).any(|i| {
        let y = low + span * f64::from(i) / 8.0;
        let row = s.row(y);
        row.len() >= 2 && row[0].1 < counter_left
    })
}

/// The top of the bowl's counter over the glyph's height: a single-storey
/// `a` has its counter up to the top, a double-storey one a stroke arching
/// over it.
fn a_storeys(s: &Shape) -> Measured {
    let counters = s.outline.counters();
    let Some(c) = counters
        .iter()
        .max_by(|a, b| a.area.total_cmp(&b.area))
        .map(|c| c.bbox)
    else {
        return Measured::unread("double");
    };
    let top = (c.y_max - s.bbox.y_min) / s.height();
    let mut m = split(top, 0.7, 0.78, "double", "single");
    m.debug = vec![("top", top)];
    m
}

// ---------------------------------------------------------------- G.spur

/// The right edge of the `G` below the bar: a spur or a vertical stroke keeps
/// it upright, a bowl without one curves it in towards the baseline.
fn g_spur(s: &Shape) -> Measured {
    let right = |f: f64| s.row(s.y_at(f)).last().map(|r| r.1);
    let (Some(hi), Some(lo)) = (right(0.38), right(0.12)) else {
        return Measured::unread("none");
    };
    let pull = (hi - lo) / s.width();
    let mut m = split(pull, 0.05, 0.14, "spur", "none");
    m.debug = vec![("pull", pull)];
    m
}

// ---------------------------------------------------------------- Q.tail

/// How far the ink reaches right of and below the bowl, the bowl found at
/// the counter's middle height and left of centre.
fn q_tail(s: &Shape) -> Measured {
    let counters = s.outline.counters();
    let Some(c) = counters
        .iter()
        .max_by(|a, b| a.area.total_cmp(&b.area))
        .map(|c| c.bbox)
    else {
        return Measured::unread("crossing");
    };
    let mid_y = (c.y_min + c.y_max) / 2.0;
    let row = s.row(mid_y);
    let (Some(first), Some(last)) = (row.first(), row.last()) else {
        return Measured::unread("crossing");
    };
    let (left, right) = (first.0, last.1);
    let bw = right - left;
    let probe = left + 0.3 * bw;
    let Some(bottom) = s.column(probe).first().map(|r| r.0) else {
        return Measured::unread("crossing");
    };
    let bh = s.bbox.y_max - bottom;
    let out_right = (s.bbox.x_max - right) / bw;
    let out_down = (bottom - s.bbox.y_min) / bh;
    let reach = out_right.max(0.0) + out_down.max(0.0);
    let low_row = s.row(s.y_at(0.03));
    let low_x = match (low_row.first(), low_row.last()) {
        (Some(a), Some(b)) => ((a.0 + b.1) / 2.0 - left) / bw,
        _ => -1.0,
    };
    let (ccx, ccy) = ((c.x_min + c.x_max) / 2.0, (c.y_min + c.y_max) / 2.0);
    let (crx, cry) = ((c.x_max - c.x_min) * 0.45, (c.y_max - c.y_min) * 0.45);
    let (mut inked, mut all) = (0.0_f64, 0.0_f64);
    for i in 0..12 {
        for j in 0..12 {
            let u = f64::from(i) / 11.0;
            let v = -f64::from(j) / 11.0;
            if u * u + v * v > 1.0 {
                continue;
            }
            all += 1.0;
            if s.inked(Point::new(ccx + u * crx, ccy + v * cry)) {
                inked += 1.0;
            }
        }
    }
    let enters = inked / all;
    let mut m = if reach < 0.06 {
        Measured::new("inside", ramp(-reach, -0.06, -0.02))
    } else if reach > 0.35 {
        Measured::new("swash", ramp(reach, 0.35, 0.5))
    } else if (0.1..0.28).contains(&reach) {
        Measured::new("crossing", 0.75)
    } else {
        Measured::new("crossing", 0.5)
    };
    m.debug = vec![
        ("right", out_right),
        ("down", out_down),
        ("lowx", low_x),
        ("enters", enters),
    ];
    m
}

// ------------------------------------------------------------ t.terminal

/// The slope of the top of the `t`'s stem across its width.
fn t_terminal(s: &Shape) -> Measured {
    let Some(stem) = stem_at(s, 0.45) else {
        return Measured::unread("flat");
    };
    let sw = stem.1 - stem.0;
    let near_top = s.y_at(0.8);
    let mut points = Vec::new();
    for i in 1..=9 {
        let x = stem.0 + sw * f64::from(i) / 10.0;
        if let Some(top) = s.column(x).last().filter(|r| r.1 >= near_top) {
            points.push((x, top.1));
        }
    }
    let Some((slope, _)) = fit(&points, sw) else {
        return Measured::unread("flat");
    };
    let deg = slope.atan().to_degrees().abs();
    let mut m = split(deg, 8.0, 14.0, "flat", "angled");
    m.debug = vec![("deg", deg)];
    m
}

// -------------------------------------------------------------- one.flag

/// How far the ink of the upper part reaches left of the stem, in stem
/// widths.
fn one_flag(s: &Shape) -> Measured {
    let Some(stem) = stem_at(s, 0.45) else {
        return Measured::unread("none");
    };
    let sw = (stem.1 - stem.0).max(1.0);
    let mut reach: f64 = 0.0;
    for i in 0..=10 {
        let f = 0.62 + 0.03 * f64::from(i);
        if let Some(first) = s.row(s.y_at(f)).first() {
            reach = reach.max((stem.0 - first.0) / sw);
        }
    }
    let mut m = split(reach, 0.4, 0.9, "none", "flag");
    m.debug = vec![("reach", reach)];
    m
}

// ------------------------------------------------------------- seven.bar

/// The widest inked run across the stem against the stem's usual width.
fn seven_bar(s: &Shape) -> Measured {
    let mut widths = Vec::new();
    for i in 0..=16 {
        let f = 0.15 + 0.03 * f64::from(i);
        let row = s.row(s.y_at(f));
        let total: f64 = row.iter().map(|r| r.1 - r.0).sum();
        if total > 0.0 {
            widths.push(total);
        }
    }
    let widest = widths.iter().copied().fold(0.0, f64::max);
    let Some(usual) = median(&mut widths) else {
        return Measured::unread("none");
    };
    let ratio = widest / usual.max(1.0);
    let mut m = split(ratio, 1.6, 2.2, "none", "bar");
    m.debug = vec![("ratio", ratio)];
    m
}

// ---------------------------------------------------------------- I.form

/// The ends of the `I` against its stem: as wide as the stem is plain;
/// wider is serifs, or bars when the ends are wide, square and thick.
fn i_form(s: &Shape) -> Measured {
    let Some(stem) = stem_at(s, 0.5) else {
        return Measured::unread("plain");
    };
    let sw = (stem.1 - stem.0).max(1.0);
    let width_at = |f: f64| {
        let row = s.row(s.y_at(f));
        match (row.first(), row.last()) {
            (Some(a), Some(b)) => b.1 - a.0,
            _ => 0.0,
        }
    };
    let end = |fs: [f64; 5]| fs.into_iter().map(width_at).fold(0.0, f64::max);
    let top = end([0.995, 0.99, 0.985, 0.98, 0.97]);
    let bottom = end([0.005, 0.01, 0.015, 0.02, 0.03]);
    let ends = top.min(bottom) / sw;
    if ends < 1.3 {
        let mut m = Measured::new("plain", ramp(-ends, -1.3, -1.1));
        m.debug = vec![("ends", ends)];
        return m;
    }
    // Bars are as wide at their inner edge as at the end (no bracket, no
    // taper) and thick; serifs taper or are thin. The end's thickness is
    // the height over which the top stays wider than 1.3 stems.
    let mut thick = 0.0;
    for i in 1..=40 {
        let f = 1.0 - 0.005 * f64::from(i);
        if width_at(f) / sw > 1.3 {
            thick = 1.0 - f;
        } else {
            break;
        }
    }
    let inner = width_at(1.0 - thick * 0.8) / sw;
    let outer = width_at(1.0 - thick * 0.2) / sw;
    let square = if outer > 0.0 { inner / outer } else { 0.0 };
    let mut m = if thick <= 0.0 {
        // The widest row is a hairline: undecided.
        Measured::new("serifed", 0.5)
    } else if square < 0.9 {
        Measured::new("serifed", ramp(-square, -0.9, -0.75))
    } else if square >= 0.95 && thick >= 0.08 {
        Measured::new("barred", ramp(thick, 0.08, 0.12))
    } else if thick < 0.045 {
        Measured::new("serifed", ramp(-thick, -0.045, -0.03))
    } else {
        Measured::new(if thick >= 0.065 { "barred" } else { "serifed" }, 0.5)
    };
    if ends < 2.2 && m.value == "barred" {
        m.confidence = m.confidence.min(0.5);
    }
    m.debug = vec![("ends", ends), ("square", square), ("thick", thick)];
    m
}

// ---------------------------------------------------------------- l.form

/// How far the foot of the `l` reaches right of the stem against left of
/// it: a tail turns right only.
fn l_form(s: &Shape) -> Measured {
    let Some(stem) = stem_at(s, 0.55) else {
        return Measured::unread("plain");
    };
    let sw = (stem.1 - stem.0).max(1.0);
    let (mut right, mut left): (f64, f64) = (0.0, 0.0);
    for i in 0..=6 {
        let f = 0.01 + 0.02 * f64::from(i);
        let row = s.row(s.y_at(f));
        if let (Some(a), Some(b)) = (row.first(), row.last()) {
            right = right.max((b.1 - stem.1) / sw);
            left = left.max((stem.0 - a.0) / sw);
        }
    }
    let tail = right - left.max(0.0);
    let mut m = split(tail, 0.35, 0.7, "plain", "tailed");
    m.debug = vec![("right", right), ("left", left)];
    m
}

// ---------------------------------------------------------------- M.sides

/// How far the outer edges spread from the top to the foot, together, over
/// the height: the sum does not change with the slant.
fn m_sides(s: &Shape) -> Measured {
    let edge = |f: f64| {
        let row = s.row(s.y_at(f));
        Some((row.first()?.0, row.last()?.1))
    };
    let (Some(top), Some(foot)) = (edge(0.82), edge(0.18)) else {
        return Measured::unread("vertical");
    };
    let spread = ((top.0 - foot.0) + (foot.1 - top.1)) / s.height();
    let mut m = split(spread, 0.05, 0.1, "vertical", "splayed");
    m.debug = vec![("spread", spread)];
    m
}

// ----------------------------------------------------------------- e.bar

/// The slope of the bar: the bottom edge of the eye, the counter above it,
/// across the middle of the eye.
fn e_bar(s: &Shape) -> Measured {
    let counters = s.outline.counters();
    let Some(c) = counters
        .iter()
        .max_by(|a, b| a.area.total_cmp(&b.area))
        .map(|c| c.bbox)
    else {
        return Measured::unread("angled");
    };
    let cw = c.x_max - c.x_min;
    let cy = (c.y_min + c.y_max) / 2.0;
    let mut points = Vec::new();
    for i in 2..=8 {
        let x = c.x_min + cw * f64::from(i) / 10.0;
        let col = s.column(x);
        // The bar is the run just below the counter's middle.
        if let Some(bar) = col.iter().rev().find(|r| r.1 <= cy) {
            points.push((x, bar.1));
        }
    }
    let Some((slope, _)) = fit(&points, cw) else {
        return Measured::unread("angled");
    };
    let deg = slope.atan().to_degrees().abs();
    let mut m = split(deg, 5.0, 9.0, "horizontal", "angled");
    m.debug = vec![("deg", deg)];
    m
}

#[cfg(test)]
mod tests;
