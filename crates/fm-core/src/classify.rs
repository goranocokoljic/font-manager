//! Design classification: serif, sans, slab, mono, script, display, symbol.
//!
//! [`classify`] first reads the class the font declares about itself: the
//! IBM family class (`OS/2.sFamilyClass`), the PANOSE numbers (`OS/2.panose`)
//! and the fixed-pitch flag (`post.isFixedPitch`). Many fonts leave these
//! blank. Then heuristics measure the outlines of a few Latin letters
//! ([`Measures`]): pixel or split strokes make a display face, equal advances
//! a mono, joining strokes that leave the glyph box or wobbly hand-drawn
//! stems a script, extreme stems or unicase letters a display face, feet
//! wider than the stem a serif, and thick unbracketed feet with little stroke
//! contrast a slab. Everything else is a sans. Every guess has a confidence
//! in `0.0..=1.0`.
//!
//! A face without the capital `H` and `I` (a Khmer-only face, an icon font)
//! cannot be measured; it falls back to the words of its family name ("Sans",
//! "Mono") and, without those, to no class at all. Malformed outlines are not
//! an error: a glyph that cannot be drawn counts as missing.

use fm_api::common::Classification;
use read_fonts::{FontRef, TableProvider};
use serde::{Deserialize, Serialize};
use skrifa::charmap::Charmap;
use skrifa::instance::{LocationRef, Size};
use skrifa::metrics::GlyphMetrics;
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{GlyphId, MetadataProvider, OutlineGlyphCollection};

/// A design class and how sure the classifier is of it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassGuess {
    /// The class.
    pub class: Classification,
    /// How sure, from `0.0` to `1.0`. Declared classes are `0.8` and more;
    /// outline guesses range from `0.5` (borderline) to `0.9`.
    pub confidence: f32,
    /// Where the class comes from.
    pub source: ClassSource,
}

/// Where a [`ClassGuess`] comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClassSource {
    /// The font's own `OS/2` family class, PANOSE or fixed-pitch flag.
    Declared,
    /// Measurements of the glyph outlines.
    Outlines,
    /// The characters the font maps: a third or more are symbols.
    Coverage,
    /// Words of the family name, when the outlines cannot be measured.
    Name,
}

/// Classifies one face: the declared class, else a symbol font by its
/// characters, else the outline heuristics, else the words of `family` (its
/// family name). `code_points` are the characters its `cmap` maps.
///
/// A class read from the PANOSE serif style alone is refined by the
/// outlines: that digit tells serif from sans and nothing else, so a face it
/// calls "sans" whose outlines are pixels, unicase or hand-drawn is a
/// display or script face.
pub fn classify(
    font: &FontRef<'_>,
    family: Option<&str>,
    code_points: &[u32],
) -> Option<ClassGuess> {
    match declared(font) {
        Some(d) if d.serif_style_only => Some(
            from_outlines(font)
                .filter(|o| matches!(o.class, Classification::Display | Classification::Script))
                .unwrap_or(d.guess),
        ),
        Some(d) => Some(d.guess),
        None => from_coverage(code_points)
            .or_else(|| from_outlines(font))
            .or_else(|| family.and_then(from_name)),
    }
}

/// Symbol blocks: arrows to miscellaneous symbols and arrows, private use,
/// musical notations, and the pictographs and emoji of the supplementary
/// planes.
const SYMBOL_BLOCKS: [std::ops::RangeInclusive<u32>; 6] = [
    0x2190..=0x2BFF,
    0xE000..=0xF8FF,
    0x1CF00..=0x1CFCF,
    0x1D000..=0x1D24F,
    0x1F000..=0x1FAFF,
    0xF0000..=0x10FFFF,
];

/// A symbol font: a third of its characters or more are in
/// [`SYMBOL_BLOCKS`]. (A notation font carries the Latin text it is set
/// with; a text font with arrows and dingbats stays well under a third.)
pub(crate) fn from_coverage(code_points: &[u32]) -> Option<ClassGuess> {
    let symbols = code_points
        .iter()
        .filter(|c| SYMBOL_BLOCKS.iter().any(|b| b.contains(c)))
        .count();
    let share = symbols as f32 / code_points.len().max(1) as f32;
    (share >= 1.0 / 3.0).then_some(ClassGuess {
        class: Classification::Symbol,
        confidence: (0.5 + share / 2.0).min(0.9),
        source: ClassSource::Coverage,
    })
}

/// The class the font declares in `OS/2` and `post`.
fn declared(font: &FontRef<'_>) -> Option<Declared> {
    let fixed = font.post().is_ok_and(|p| p.is_fixed_pitch() != 0);
    let os2 = font.os2().ok();
    let family_class = os2.as_ref().map_or(0, |t| t.s_family_class());
    let panose = os2.as_ref().map(|t| t.panose_10()).unwrap_or_default();
    from_declared(family_class, panose, fixed)
}

/// A declared class.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Declared {
    pub guess: ClassGuess,
    /// It comes from the PANOSE serif style digit only.
    pub serif_style_only: bool,
}

/// The class from the IBM family class, the PANOSE numbers and the
/// fixed-pitch flag; `None` when they say nothing.
///
/// Mono comes first: a fixed-pitch face is mono whatever its serifs. The
/// family class comes before PANOSE: it names the class directly.
pub(crate) fn from_declared(family_class: i16, panose: &[u8], fixed: bool) -> Option<Declared> {
    use Classification::*;
    let declared = |class, confidence, serif_style_only| {
        Some(Declared {
            guess: ClassGuess {
                class,
                confidence,
                source: ClassSource::Declared,
            },
            serif_style_only,
        })
    };
    let guess = |class, confidence| declared(class, confidence, false);
    let serif_style = |class, confidence| declared(class, confidence, true);
    let digit = |i: usize| panose.get(i).copied().unwrap_or(0);
    let family_type = digit(0);
    // Proportion (Latin Text) or spacing (Latin Hand Written, Latin Symbol).
    let fixed_panose = match family_type {
        2 => digit(3) == 9,
        3 | 5 => digit(3) == 3,
        _ => false,
    };
    if fixed || fixed_panose {
        return guess(Mono, 0.95);
    }
    match family_class >> 8 {
        1..=3 | 7 => return guess(Serif, 0.9),
        4 | 5 => return guess(Slab, 0.9),
        8 => return guess(Sans, 0.9),
        9 => return guess(Display, 0.85),
        10 => return guess(Script, 0.9),
        12 => return guess(Symbol, 0.9),
        _ => {}
    }
    match family_type {
        // Latin Text: the serif style digit.
        2 => match digit(1) {
            // Cove, obtuse cove, square cove, obtuse square cove.
            2..=5 => serif_style(Serif, 0.85),
            // Square.
            6 => serif_style(Slab, 0.8),
            // Thin, bone, exaggerated, triangle.
            7..=10 => serif_style(Serif, 0.8),
            // Normal sans, obtuse sans, perpendicular sans.
            11..=13 => serif_style(Sans, 0.85),
            // Flared: small flared serifs, the serif side of the line.
            14 => serif_style(Serif, 0.6),
            // Rounded.
            15 => serif_style(Sans, 0.8),
            _ => None,
        },
        3 => guess(Script, 0.85),
        4 => guess(Display, 0.8),
        5 => guess(Symbol, 0.9),
        _ => None,
    }
}

/// The class from the words of a family name: "Mono", "Slab", "Script",
/// "Sans", "Serif" (in that order: "Spline Sans Mono" is a mono, "Sans
/// Serif" a sans).
pub(crate) fn from_name(family: &str) -> Option<ClassGuess> {
    use Classification::*;
    let words: Vec<String> = family
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .collect();
    let has = |w: &str| words.iter().any(|x| x == w);
    let class = if has("mono") {
        Mono
    } else if has("slab") {
        Slab
    } else if has("script") {
        Script
    } else if has("sans") {
        Sans
    } else if has("serif") {
        Serif
    } else {
        return None;
    };
    Some(ClassGuess {
        class,
        confidence: 0.6,
        source: ClassSource::Name,
    })
}

/// The class from glyph outline measurements; `None` when the face lacks
/// the letters they need.
fn from_outlines(font: &FontRef<'_>) -> Option<ClassGuess> {
    Some(Probe::new(font)?.measure()?.class())
}

/// Measurements of one face's Latin letters, in em units (the em is `1.0`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Measures {
    /// Cap height (the top of `H`).
    pub cap_height: f32,
    /// Width of the `l` stem (`I` without an `l`) at a third of its height.
    pub stem: f32,
    /// Every probed letter (`i m W l M 1`, four at least) has the same advance.
    pub fixed_advance: bool,
    /// Foot width over stem width at the baseline of `H`, `I` and `n`, the
    /// median of the three: about `1.0` for a sans, `1.2` and more with serifs.
    pub foot_ratio: f32,
    /// Thickness of the foot of that stem, over the stem width: the height
    /// where the stem is back to its own width.
    pub foot_height: f32,
    /// How square the foot is: the height where it is still at 90 % of its
    /// base width over its whole height. `1.0` for an unbracketed slab,
    /// lower for a bracketed serif.
    pub foot_squareness: f32,
    /// Thinnest stroke of `o` over its thickest: `1.0` without contrast.
    pub contrast: f32,
    /// Share of the lowercase probes (`a e n m u c`) whose ink reaches out of
    /// the advance box: the joining strokes of a connected script.
    pub overhang: f32,
    /// How irregular the `l` and `I` stems are between a quarter and three
    /// quarters of their height: the spread of their centers around a
    /// straight line and of their widths, over the width. Near `0.0` for a
    /// drawn stem (upright or slanted), more for a hand-drawn one.
    pub wobble: f32,
    /// The `o` is drawn with horizontal and vertical lines only (pixels,
    /// bars).
    pub pixel: bool,
    /// The lowercase is as tall as the capitals, or `a` is the `A` glyph.
    pub unicase: bool,
    /// The `I` has more than one run of ink at mid-height (inline, outline,
    /// shadowed or striped letters).
    pub split_stem: bool,
    /// Contours of the `I`: one for a plain stem, a few with overlapping
    /// serifs, many for a face drawn in dots.
    pub pieces: usize,
    /// The ring of the `o` has a gap: some ray from its center meets no
    /// ink (a stencil face).
    pub open_ring: bool,
}

impl Measures {
    /// The class these measurements point to.
    pub(crate) fn class(&self) -> ClassGuess {
        use Classification::*;
        let guess = |class, confidence: f32| ClassGuess {
            class,
            confidence: confidence.clamp(0.5, 0.9),
            source: ClassSource::Outlines,
        };
        if self.pixel || self.pieces >= DOTS {
            return guess(Display, 0.85);
        }
        if self.split_stem || self.open_ring {
            return guess(Display, 0.7);
        }
        if self.fixed_advance {
            return guess(Mono, 0.9);
        }
        if self.overhang >= 0.5 {
            return guess(Script, 0.5 + 0.4 * self.overhang);
        }
        if self.stem >= self.cap_height * 0.31 {
            return guess(Display, 0.6);
        }
        if self.unicase {
            return guess(Display, 0.6);
        }
        // A wobbly stem with feet like a serif's is a bracketed serif, not a hand.
        if self.wobble >= WOBBLE && self.foot_ratio < 1.35 {
            return guess(Script, 0.5 + 4.0 * self.wobble);
        }
        if self.foot_ratio >= SERIF_FOOT {
            let slab =
                self.foot_height >= 0.4 && self.foot_squareness >= 0.88 && self.contrast >= 0.6;
            if slab {
                return guess(Slab, 0.5 + 0.4 * self.foot_squareness);
            }
            return guess(Serif, 0.5 + (self.foot_ratio - SERIF_FOOT));
        }
        // The closer the foot to a serif, the less sure.
        guess(Sans, 0.9 - 2.0 * (self.foot_ratio - 1.0).max(0.0))
    }
}

/// A foot this much wider than its stem is a serif.
const SERIF_FOOT: f32 = 1.2;
/// An `I` of this many contours is drawn in dots.
const DOTS: usize = 6;
/// Stems this irregular are drawn by hand.
const WOBBLE: f32 = 0.012;

/// A face, ready to measure.
pub(crate) struct Probe<'a> {
    charmap: Charmap<'a>,
    metrics: GlyphMetrics<'a>,
    outlines: OutlineGlyphCollection<'a>,
    upem: f32,
}

impl<'a> Probe<'a> {
    pub(crate) fn new(font: &FontRef<'a>) -> Option<Self> {
        let upem = f32::from(font.head().ok()?.units_per_em());
        (upem > 0.0).then(|| Self {
            charmap: font.charmap(),
            metrics: font.glyph_metrics(Size::unscaled(), LocationRef::default()),
            outlines: font.outline_glyphs(),
            upem,
        })
    }

    fn gid(&self, c: char) -> Option<GlyphId> {
        self.charmap.map(c)
    }

    fn advance(&self, gid: GlyphId) -> Option<f32> {
        self.metrics.advance_width(gid).map(|a| a / self.upem)
    }

    /// The outline of `c`, in em units; `None` when the face does not map
    /// it, it is empty or it cannot be drawn.
    fn glyph(&self, c: char) -> Option<Glyph> {
        let gid = self.gid(c)?;
        let outline = self.outlines.get(gid)?;
        let mut pen = Flatten::new(1.0 / self.upem);
        outline
            .draw(
                DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
                &mut pen,
            )
            .ok()?;
        let contours = pen.finish();
        if contours.iter().all(|c| c.len() < 3) {
            return None;
        }
        Some(Glyph {
            advance: self.advance(gid)?,
            contours,
            curves: pen.curves,
            diagonals: pen.diagonals,
        })
    }

    /// Measures the face; `None` without the capitals `H` and `I`.
    pub(crate) fn measure(&self) -> Option<Measures> {
        let h = self.glyph('H')?;
        let i = self.glyph('I')?;
        let (_, cap) = h.y_range()?;
        if cap <= 0.0 {
            return None;
        }
        let n = self.glyph('n');
        let x_height = self
            .glyph('x')
            .and_then(|x| x.y_range())
            .map_or(cap, |r| r.1);
        let l = self.glyph('l');
        let stem_glyph = l.as_ref().unwrap_or(&i);

        let mut m = Measures {
            cap_height: cap,
            contrast: 1.0,
            foot_ratio: 1.0,
            ..Measures::default()
        };

        let advances: Vec<f32> = ['i', 'm', 'W', 'l', 'M', '1']
            .iter()
            .filter_map(|&c| self.advance(self.gid(c)?))
            .collect();
        m.fixed_advance = advances.len() >= 4
            && advances[0] > 0.0
            && advances.iter().all(|a| (a - advances[0]).abs() < 0.005);

        m.split_stem = i.runs(cap * 0.5).len() > 1;
        m.pieces = i.contours.len();
        if let Some((lo, hi)) = stem_glyph.y_range() {
            m.stem = stem_glyph
                .runs(lo + (hi - lo) / 3.0)
                .iter()
                .map(|r| r.1 - r.0)
                .fold(0.0, f32::max);
        }

        let mut feet: Vec<Foot> = [(Some(&h), cap), (Some(&i), cap), (n.as_ref(), x_height)]
            .into_iter()
            .filter_map(|(g, height)| g?.foot(height))
            .collect();
        feet.sort_by(|a, b| a.ratio.total_cmp(&b.ratio));
        if let Some(foot) = feet.get(feet.len() / 2) {
            m.foot_ratio = foot.ratio;
            m.foot_height = foot.height;
            m.foot_squareness = foot.squareness;
        }

        if let Some(o) = self.glyph('o') {
            m.pixel = o.curves == 0 && o.diagonals == 0;
            m.contrast = o.contrast().unwrap_or(1.0);
            m.open_ring = o.is_open_ring();
        }

        let lower: Vec<Glyph> = ['a', 'e', 'n', 'm', 'u', 'c']
            .iter()
            .filter_map(|&c| self.glyph(c))
            .collect();
        if !lower.is_empty() {
            let out = lower.iter().filter(|g| g.overhangs(0.04)).count();
            m.overhang = out as f32 / lower.len() as f32;
        }

        m.wobble = [l.as_ref(), Some(&i)]
            .into_iter()
            .filter_map(|g| g?.wobble())
            .fold(f32::INFINITY, f32::min);
        if !m.wobble.is_finite() {
            m.wobble = 0.0;
        }

        let same_a = self.gid('a').is_some() && self.gid('a') == self.gid('A');
        let tall_n = n
            .as_ref()
            .and_then(Glyph::y_range)
            .is_some_and(|(_, top)| top >= cap * 0.9);
        m.unicase = same_a || tall_n;
        Some(m)
    }
}

/// The foot of a stem: how much wider it is at the baseline.
#[derive(Debug, Clone, Copy)]
struct Foot {
    ratio: f32,
    height: f32,
    squareness: f32,
}

/// One drawn glyph, flattened to polygons in em units.
struct Glyph {
    advance: f32,
    contours: Vec<Vec<(f32, f32)>>,
    /// Curve segments before flattening.
    curves: usize,
    /// Line segments that are neither horizontal nor vertical.
    diagonals: usize,
}

impl Glyph {
    fn range(&self, axis: fn(&(f32, f32)) -> f32) -> Option<(f32, f32)> {
        let mut it = self.contours.iter().flatten().map(axis);
        let first = it.next()?;
        Some(it.fold((first, first), |(lo, hi), v| (lo.min(v), hi.max(v))))
    }

    fn y_range(&self) -> Option<(f32, f32)> {
        self.range(|p| p.1)
    }

    fn x_range(&self) -> Option<(f32, f32)> {
        self.range(|p| p.0)
    }

    /// Runs of ink on the horizontal line at `y`, left to right.
    fn runs(&self, y: f32) -> Vec<(f32, f32)> {
        runs_at(&self.contours, y, false)
    }

    /// Runs of ink on the vertical line at `x`, bottom to top.
    fn columns(&self, x: f32) -> Vec<(f32, f32)> {
        runs_at(&self.contours, x, true)
    }

    /// The width of the run that holds `x` on the line at `y`.
    fn width_at(&self, y: f32, x: f32) -> Option<f32> {
        self.runs(y)
            .into_iter()
            .find(|r| r.0 <= x && x <= r.1)
            .map(|r| r.1 - r.0)
    }

    /// The widest foot among the stems that stand on the baseline, each
    /// compared with itself at a third of `height`. `None` without one.
    fn foot(&self, height: f32) -> Option<Foot> {
        let base = height * 0.015;
        let step = height * 0.005;
        let mut best: Option<Foot> = None;
        for (left, right) in self.runs(height / 3.0) {
            let w = right - left;
            let x = (left + right) / 2.0;
            let Some(foot) = self.width_at(base, x).filter(|_| w > 0.0) else {
                continue;
            };
            // The first step up from the base where the stem is no wider than
            // `limit`, by bisection: a foot only narrows as it climbs.
            let steps = ((height / 3.0 - base) / step).ceil().max(0.0) as usize;
            let climb = |limit: f32| {
                let wider = |k: usize| {
                    self.width_at(base + k as f32 * step, x)
                        .is_some_and(|f| f > limit)
                };
                let (mut lo, mut hi) = (0, steps);
                while lo < hi {
                    let mid = (lo + hi) / 2;
                    if wider(mid) {
                        lo = mid + 1;
                    } else {
                        hi = mid;
                    }
                }
                base + lo as f32 * step
            };
            let top = climb(w * 1.1);
            let full = climb(foot * 0.9);
            let candidate = Foot {
                ratio: foot / w,
                height: top / w,
                squareness: full / top,
            };
            if best.is_none_or(|b| candidate.ratio > b.ratio) {
                best = Some(candidate);
            }
        }
        best
    }

    /// Thinnest over thickest stroke of an `o`: the side strokes at
    /// mid-height against the top and bottom strokes at mid-width.
    fn contrast(&self) -> Option<f32> {
        let (y0, y1) = self.y_range()?;
        let (x0, x1) = self.x_range()?;
        let widest = |runs: Vec<(f32, f32)>| runs.iter().map(|r| r.1 - r.0).fold(0.0, f32::max);
        let side = widest(self.runs((y0 + y1) / 2.0));
        let top = widest(self.columns((x0 + x1) / 2.0));
        let (thin, thick) = if side < top { (side, top) } else { (top, side) };
        (thick > 0.0).then(|| thin / thick)
    }

    /// Whether the glyph is two or more pieces and none of them is a hole:
    /// the ring of an `o` cut by stencil bridges. A closed ring is an outer
    /// contour and a counter that winds the other way.
    fn is_open_ring(&self) -> bool {
        let area = |c: &Vec<(f32, f32)>| {
            c.iter()
                .enumerate()
                .map(|(k, &(ax, ay))| {
                    let (bx, by) = c[(k + 1) % c.len()];
                    ax * by - bx * ay
                })
                .sum::<f32>()
        };
        let signs: Vec<bool> = self
            .contours
            .iter()
            .map(area)
            .filter(|a| a.abs() > 1e-6)
            .map(|a| a > 0.0)
            .collect();
        signs.len() >= 2 && signs.iter().all(|&s| s == signs[0])
    }

    /// Whether the ink reaches more than `margin` em out of the advance box.
    fn overhangs(&self, margin: f32) -> bool {
        self.x_range()
            .is_some_and(|(lo, hi)| lo < -margin || hi > self.advance + margin)
    }

    /// See [`Measures::wobble`]. `None` when the stem is not one run of ink
    /// all the way.
    fn wobble(&self) -> Option<f32> {
        let (y0, y1) = self.y_range()?;
        let h = y1 - y0;
        let samples: Vec<(f32, f32, f32)> = (0..=8)
            .map(|k| {
                let y = y0 + h * (0.25 + 0.5 * k as f32 / 8.0);
                let runs = self.runs(y);
                let r = runs
                    .iter()
                    .max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0)))?;
                Some((y, (r.0 + r.1) / 2.0, r.1 - r.0))
            })
            .collect::<Option<_>>()?;
        let n = samples.len() as f32;
        let mean = |f: fn(&(f32, f32, f32)) -> f32| samples.iter().map(f).sum::<f32>() / n;
        let (my, mc, mw) = (mean(|s| s.0), mean(|s| s.1), mean(|s| s.2));
        if mw <= 0.0 {
            return None;
        }
        // A straight line through the centers: a slanted stem fits it, a
        // hand-drawn one does not; and a drawn stem keeps its width.
        let syy: f32 = samples.iter().map(|s| (s.0 - my).powi(2)).sum();
        let syc: f32 = samples.iter().map(|s| (s.0 - my) * (s.1 - mc)).sum();
        let sc = if syy > 0.0 { syc / syy } else { 0.0 };
        let spread = |f: &dyn Fn(&(f32, f32, f32)) -> f32| {
            (samples.iter().map(|s| f(s).powi(2)).sum::<f32>() / n).sqrt()
        };
        let centers = spread(&|s| s.1 - (mc + sc * (s.0 - my)));
        let widths = spread(&|s| s.2 - mw);
        Some(centers.max(widths) / mw)
    }
}

/// Runs of ink where the line at `v` (a `y`, or an `x` when `vertical`)
/// crosses the polygons, by nonzero winding: the overlapping contours of a
/// variable font count once.
fn runs_at(contours: &[Vec<(f32, f32)>], v: f32, vertical: bool) -> Vec<(f32, f32)> {
    let mut crossings: Vec<(f32, i32)> = Vec::new();
    for contour in contours {
        for (k, &a) in contour.iter().enumerate() {
            let b = contour[(k + 1) % contour.len()];
            let ((ax, ay), (bx, by)) = if vertical {
                ((a.1, a.0), (b.1, b.0))
            } else {
                (a, b)
            };
            if (ay <= v && v < by) || (by <= v && v < ay) {
                let x = ax + (v - ay) / (by - ay) * (bx - ax);
                crossings.push((x, if by > ay { 1 } else { -1 }));
            }
        }
    }
    crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = Vec::new();
    let (mut winding, mut start) = (0, 0.0);
    for (x, dir) in crossings {
        let inside = winding != 0;
        winding += dir;
        if !inside && winding != 0 {
            start = x;
        } else if inside && winding == 0 && x > start {
            out.push((start, x));
        }
    }
    out
}

/// A pen that flattens curves into lines and scales to em units.
struct Flatten {
    scale: f32,
    contours: Vec<Vec<(f32, f32)>>,
    current: Vec<(f32, f32)>,
    curves: usize,
    diagonals: usize,
}

/// Lines per flattened curve.
const STEPS: usize = 8;

impl Flatten {
    fn new(scale: f32) -> Self {
        Self {
            scale,
            contours: Vec::new(),
            current: Vec::new(),
            curves: 0,
            diagonals: 0,
        }
    }

    fn last(&self) -> (f32, f32) {
        self.current.last().copied().unwrap_or((0.0, 0.0))
    }

    fn finish(&mut self) -> Vec<Vec<(f32, f32)>> {
        self.close();
        std::mem::take(&mut self.contours)
    }

    /// Appends the points of a Bézier curve from the current point, its
    /// control and end points given in em units.
    fn bezier(&mut self, points: &[(f32, f32)]) {
        self.curves += 1;
        let start = self.last();
        let mut p = [(0.0, 0.0); 4];
        for k in 1..=STEPS {
            let t = k as f32 / STEPS as f32;
            // de Casteljau over [start, points...], in place.
            p[0] = start;
            p[1..=points.len()].copy_from_slice(points);
            for len in (1..=points.len()).rev() {
                for j in 0..len {
                    p[j] = (
                        p[j].0 + t * (p[j + 1].0 - p[j].0),
                        p[j].1 + t * (p[j + 1].1 - p[j].1),
                    );
                }
            }
            self.current.push(p[0]);
        }
    }
}

impl OutlinePen for Flatten {
    fn move_to(&mut self, x: f32, y: f32) {
        self.close();
        self.current.push((x * self.scale, y * self.scale));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (px, py) = self.last();
        let (x, y) = (x * self.scale, y * self.scale);
        if (x - px).abs() > 1e-4 && (y - py).abs() > 1e-4 {
            self.diagonals += 1;
        }
        self.current.push((x, y));
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let s = self.scale;
        self.bezier(&[(cx0 * s, cy0 * s), (x * s, y * s)]);
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let s = self.scale;
        self.bezier(&[(cx0 * s, cy0 * s), (cx1 * s, cy1 * s), (x * s, y * s)]);
    }

    fn close(&mut self) {
        if !self.current.is_empty() {
            self.contours.push(std::mem::take(&mut self.current));
        }
    }
}

#[cfg(test)]
mod tests;
