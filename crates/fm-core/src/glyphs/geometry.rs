//! The geometry of one glyph's outline (E49.3): the measurements the glyph
//! trait detectors (E49.4) are built from.
//!
//! - **Outline**: skrifa draws the glyph unhinted in font units at the
//!   default instance, from `glyf` (quadratic, composites resolved) or
//!   `CFF`/`CFF2` (cubic). Font y grows up.
//! - **Contours** keep their segments as drawn ([`Segment`]). A contour the
//!   font does not close is kept with [`Contour::is_closed`] `false`; for its
//!   area, winding and the fill it is closed by a straight line, as a
//!   rasterizer would. A lone move with nothing drawn is no contour, and a
//!   zero-length line is dropped.
//! - **Counters** ([`GlyphOutline::counters`]) are the enclosed holes of the
//!   glyph filled by the nonzero rule, found on a fine grid. So they do not
//!   depend on the direction the font winds its contours (TrueType and CFF
//!   wind the other way round), and overlapping or self-crossing contours,
//!   as variable fonts keep them, count as the ink they fill.
//! - **Fits**: a line fit ([`line_fit`], [`Segment::line_fit`]) is the
//!   largest distance from the chord; an ellipse fit
//!   ([`Contour::ellipse_fit`]) is the ellipse with the area moments of the
//!   contour, its axis ratio and the mean radial deviation of the outline
//!   from it. [`GlyphOutline::angle_at`] is the direction of the outline
//!   nearest a point: a stem or a terminal angle.
//! - **Malformed data** is never a panic: a face, a glyph or an outline that
//!   cannot be read is `None`, and so is a glyph with no outline (a space)
//!   or a character the face does not map.

use std::borrow::Cow;

use read_fonts::{FileRef, FontRef, TableProvider};
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};

use crate::glyphs::{draws, num_glyphs, sfnt};

/// The letters and figures the glyph traits are read from (E49), in the
/// order of the epic: R G Q a g e t y M W & 1 7 I l.
pub const TRAIT_GLYPHS: [char; 15] = [
    'R', 'G', 'Q', 'a', 'g', 'e', 't', 'y', 'M', 'W', '&', '1', '7', 'I', 'l',
];

/// The least flattening tolerance, in font units; a smaller one is raised
/// to it.
pub const MIN_TOLERANCE: f64 = 1e-3;

/// The most lines one curve is flattened to.
const MAX_STEPS: usize = 1024;

/// A point in font units.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    /// Rightwards.
    pub x: f64,
    /// Upwards.
    pub y: f64,
}

impl Point {
    /// The point `(x, y)`.
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y)
    }

    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y)
    }

    fn scale(self, k: f64) -> Self {
        Self::new(self.x * k, self.y * k)
    }

    fn cross(self, o: Self) -> f64 {
        self.x * o.y - self.y * o.x
    }

    fn dot(self, o: Self) -> f64 {
        self.x * o.x + self.y * o.y
    }

    fn len(self) -> f64 {
        self.x.hypot(self.y)
    }

    /// The distance to `o`.
    pub fn distance(self, o: Self) -> f64 {
        self.sub(o).len()
    }
}

/// An axis-aligned box in font units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BBox {
    /// Left edge.
    pub x_min: f64,
    /// Bottom edge.
    pub y_min: f64,
    /// Right edge.
    pub x_max: f64,
    /// Top edge.
    pub y_max: f64,
}

impl BBox {
    fn at(p: Point) -> Self {
        Self {
            x_min: p.x,
            y_min: p.y,
            x_max: p.x,
            y_max: p.y,
        }
    }

    fn include(&mut self, p: Point) {
        self.x_min = self.x_min.min(p.x);
        self.y_min = self.y_min.min(p.y);
        self.x_max = self.x_max.max(p.x);
        self.y_max = self.y_max.max(p.y);
    }

    fn union(self, o: Self) -> Self {
        Self {
            x_min: self.x_min.min(o.x_min),
            y_min: self.y_min.min(o.y_min),
            x_max: self.x_max.max(o.x_max),
            y_max: self.y_max.max(o.y_max),
        }
    }

    /// `x_max - x_min`.
    pub fn width(&self) -> f64 {
        self.x_max - self.x_min
    }

    /// `y_max - y_min`.
    pub fn height(&self) -> f64 {
        self.y_max - self.y_min
    }
}

/// The direction a contour runs, with y up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Winding {
    /// Positive area.
    CounterClockwise,
    /// Negative area.
    Clockwise,
    /// No area: every point on one line.
    Flat,
}

/// One piece of a contour, as the font draws it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Segment {
    /// From, to.
    Line(Point, Point),
    /// From, control, to.
    Quad(Point, Point, Point),
    /// From, control, control, to.
    Cubic(Point, Point, Point, Point),
}

impl Segment {
    /// Where the segment starts.
    pub fn start(&self) -> Point {
        match *self {
            Self::Line(p, _) | Self::Quad(p, _, _) | Self::Cubic(p, _, _, _) => p,
        }
    }

    /// Where the segment ends.
    pub fn end(&self) -> Point {
        match *self {
            Self::Line(_, p) | Self::Quad(_, _, p) | Self::Cubic(_, _, _, p) => p,
        }
    }

    /// The point at parameter `t` in `0..=1`.
    pub fn point_at(&self, t: f64) -> Point {
        let u = 1.0 - t;
        match *self {
            Self::Line(a, b) => a.scale(u).add(b.scale(t)),
            Self::Quad(a, c, b) => a.scale(u * u).add(c.scale(2.0 * u * t)).add(b.scale(t * t)),
            Self::Cubic(a, c0, c1, b) => a
                .scale(u * u * u)
                .add(c0.scale(3.0 * u * u * t))
                .add(c1.scale(3.0 * u * t * t))
                .add(b.scale(t * t * t)),
        }
    }

    /// The number of equal parameter steps that keep every line within
    /// `tolerance` of the curve: the chord error of a step `h` is at most
    /// `max |B''| h² / 8`.
    fn steps(&self, tolerance: f64) -> usize {
        let second = match *self {
            Self::Line(..) => return 1,
            Self::Quad(a, c, b) => 2.0 * a.sub(c.scale(2.0)).add(b).len(),
            Self::Cubic(a, c0, c1, b) => {
                let d0 = a.sub(c0.scale(2.0)).add(c1).len();
                let d1 = c0.sub(c1.scale(2.0)).add(b).len();
                6.0 * d0.max(d1)
            }
        };
        let n = (second / (8.0 * tolerance)).sqrt().ceil();
        if n.is_finite() && n >= 1.0 {
            (n as usize).min(MAX_STEPS)
        } else {
            1
        }
    }

    /// Appends the polyline of the segment after its start: the inner
    /// points, then the end.
    fn flatten_into(&self, tolerance: f64, out: &mut Vec<Point>) {
        let n = self.steps(tolerance);
        for i in 1..n {
            out.push(self.point_at(i as f64 / n as f64));
        }
        out.push(self.end());
    }

    /// The segment as a polyline within `tolerance` font units of it, start
    /// and end included. `None` for a tolerance that is not a positive number.
    pub fn flatten(&self, tolerance: f64) -> Option<Vec<Point>> {
        let tolerance = valid_tolerance(tolerance)?;
        let mut out = vec![self.start()];
        self.flatten_into(tolerance, &mut out);
        Some(out)
    }

    /// The tight bounding box: the ends and the extremes of the curve.
    pub fn bbox(&self) -> BBox {
        let mut b = BBox::at(self.start());
        b.include(self.end());
        let mut include = |t: Option<f64>| {
            if let Some(t) = t.filter(|&t| t > 0.0 && t < 1.0) {
                b.include(self.point_at(t));
            }
        };
        match *self {
            Self::Line(..) => {}
            Self::Quad(a, c, e) => {
                for (p0, p1, p2) in [(a.x, c.x, e.x), (a.y, c.y, e.y)] {
                    let den = p0 - 2.0 * p1 + p2;
                    include((den != 0.0).then(|| (p0 - p1) / den));
                }
            }
            Self::Cubic(a, c0, c1, e) => {
                for (p0, p1, p2, p3) in [(a.x, c0.x, c1.x, e.x), (a.y, c0.y, c1.y, e.y)] {
                    // B'(t) / 3 = qa t² + qb t + qc
                    let qa = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
                    let qb = 2.0 * (p0 - 2.0 * p1 + p2);
                    let qc = p1 - p0;
                    for t in quadratic_roots(qa, qb, qc) {
                        include(t);
                    }
                }
            }
        }
        b
    }

    /// The signed area the segment adds to its contour (Green's theorem,
    /// exact for lines and Bézier curves).
    fn area(&self) -> f64 {
        match *self {
            Self::Line(a, b) => a.cross(b) / 2.0,
            Self::Quad(a, c, b) => (2.0 * a.cross(c) + 2.0 * c.cross(b) + a.cross(b)) / 6.0,
            Self::Cubic(p0, p1, p2, p3) => {
                (6.0 * p0.cross(p1)
                    + 3.0 * p0.cross(p2)
                    + p0.cross(p3)
                    + 3.0 * p1.cross(p2)
                    + 3.0 * p1.cross(p3)
                    + 6.0 * p2.cross(p3))
                    / 20.0
            }
        }
    }

    /// How straight the segment is: [`line_fit`] of its polyline at
    /// `tolerance`. `None` when its ends meet or the tolerance is not a
    /// positive number.
    pub fn line_fit(&self, tolerance: f64) -> Option<LineFit> {
        line_fit(&self.flatten(tolerance)?)
    }
}

/// The real roots of `a t² + b t + c` (a linear one when `a` is 0).
fn quadratic_roots(a: f64, b: f64, c: f64) -> [Option<f64>; 2] {
    if a.abs() < 1e-12 {
        return [(b != 0.0).then(|| -c / b), None];
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return [None, None];
    }
    let s = disc.sqrt();
    [Some((-b + s) / (2.0 * a)), Some((-b - s) / (2.0 * a))]
}

/// How far a run of points is from the straight line between its ends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineFit {
    /// The distance between the first and the last point.
    pub chord: f64,
    /// The largest distance of a point from the chord.
    pub max_deviation: f64,
}

impl LineFit {
    /// `max_deviation / chord`: 0 for a straight line, independent of size.
    pub fn relative_deviation(&self) -> f64 {
        self.max_deviation / self.chord
    }
}

/// The line fit of `points`: the chord from the first to the last point and
/// the largest distance of any point from it. `None` for fewer than two
/// points, or when the first and the last are the same point.
pub fn line_fit(points: &[Point]) -> Option<LineFit> {
    let (&a, &b) = (points.first()?, points.last()?);
    let d = b.sub(a);
    let chord = d.len();
    if !(chord > 0.0 && chord.is_finite()) {
        return None;
    }
    let max_deviation = points
        .iter()
        .map(|&p| d.cross(p.sub(a)).abs() / chord)
        .fold(0.0, f64::max);
    Some(LineFit {
        chord,
        max_deviation,
    })
}

/// The ellipse with the same area moments as a closed contour, and how well
/// the contour follows it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EllipseFit {
    /// The centre: the centroid of the contour's area.
    pub center: Point,
    /// The longer semi-axis, in font units.
    pub major: f64,
    /// The shorter semi-axis, in font units.
    pub minor: f64,
    /// The direction of the major axis, in degrees in `0..180`.
    pub angle_degrees: f64,
    /// `minor / major`, in `0..=1`: 1 is a circle.
    pub axis_ratio: f64,
    /// The mean of `|r - 1|` along the outline, weighted by length, where
    /// `r` is the radius of a point in units of the ellipse's radius in its
    /// direction: 0 when the contour is the ellipse, about 0.1 for a square.
    pub mean_radial_deviation: f64,
}

/// One contour of a glyph.
#[derive(Debug, Clone, PartialEq)]
pub struct Contour {
    segments: Vec<Segment>,
    closed: bool,
}

impl Contour {
    /// A contour of `segments`, each starting where the one before ends.
    /// `closed` says whether the font closed it.
    pub fn new(segments: Vec<Segment>, closed: bool) -> Self {
        Self { segments, closed }
    }

    /// The segments as drawn. A closed contour whose last point is not its
    /// first ends with the closing line.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Whether the font closed the contour. An open one is closed by a
    /// straight line for its area, winding and the fill.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// The signed area, positive counter-clockwise (y up).
    pub fn signed_area(&self) -> f64 {
        let mut area: f64 = self.segments.iter().map(Segment::area).sum();
        if let (Some(first), Some(last)) = (self.segments.first(), self.segments.last()) {
            area += Segment::Line(last.end(), first.start()).area();
        }
        area
    }

    /// The direction the contour runs.
    pub fn winding(&self) -> Winding {
        let area = self.signed_area();
        let scale = self.bbox().map_or(0.0, |b| b.width() * b.height());
        if !area.is_finite() || area.abs() <= scale * 1e-9 || area == 0.0 {
            Winding::Flat
        } else if area > 0.0 {
            Winding::CounterClockwise
        } else {
            Winding::Clockwise
        }
    }

    /// The tight bounding box; `None` for a contour with no segments.
    pub fn bbox(&self) -> Option<BBox> {
        self.segments.iter().map(Segment::bbox).reduce(BBox::union)
    }

    /// The contour as a polyline within `tolerance` font units of it: the
    /// start, then every point after it. The start is not repeated at the
    /// end; the polygon closes from the last point back to the first.
    /// `None` for a tolerance that is not a positive number.
    pub fn flatten(&self, tolerance: f64) -> Option<Vec<Point>> {
        Some(self.flatten_valid(valid_tolerance(tolerance)?))
    }

    fn flatten_valid(&self, tolerance: f64) -> Vec<Point> {
        let mut out = Vec::new();
        self.flatten_append(tolerance, &mut out);
        out
    }

    /// Appends the polygon of [`flatten`](Self::flatten) to an empty `out`.
    fn flatten_append(&self, tolerance: f64, out: &mut Vec<Point>) {
        let Some(first) = self.segments.first() else {
            return;
        };
        out.push(first.start());
        for s in &self.segments {
            s.flatten_into(tolerance, out);
        }
        if out.len() > 1 && out.last() == out.first() {
            out.pop();
        }
    }

    /// The ellipse fit at `tolerance` ([`EllipseFit`]). `None` for an open
    /// contour, one without area, or a tolerance that is not a positive number.
    pub fn ellipse_fit(&self, tolerance: f64) -> Option<EllipseFit> {
        if !self.closed || self.winding() == Winding::Flat {
            return None;
        }
        let poly = self.flatten(tolerance)?;
        ellipse_of(&poly)
    }
}

/// The moment ellipse of the polygon `poly` and its radial deviation.
fn ellipse_of(poly: &[Point]) -> Option<EllipseFit> {
    let n = poly.len();
    if n < 3 {
        return None;
    }
    // Area, first and second moments of the polygon's area about the origin.
    let (mut a, mut mx, mut my, mut ixx, mut iyy, mut ixy) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for i in 0..n {
        let p = poly[i];
        let q = poly[(i + 1) % n];
        let c = p.cross(q);
        a += c;
        mx += (p.x + q.x) * c;
        my += (p.y + q.y) * c;
        ixx += (p.x * p.x + p.x * q.x + q.x * q.x) * c;
        iyy += (p.y * p.y + p.y * q.y + q.y * q.y) * c;
        ixy += (p.x * q.y + 2.0 * p.x * p.y + 2.0 * q.x * q.y + q.x * p.y) * c;
    }
    a /= 2.0;
    if !(a.abs() > 0.0 && a.is_finite()) {
        return None;
    }
    let center = Point::new(mx / (6.0 * a), my / (6.0 * a));
    // Central second moments per unit area (the covariance of the area).
    let sxx = ixx / (12.0 * a) - center.x * center.x;
    let syy = iyy / (12.0 * a) - center.y * center.y;
    let sxy = ixy / (24.0 * a) - center.x * center.y;
    let mean = (sxx + syy) / 2.0;
    let d = ((sxx - syy) / 2.0).hypot(sxy);
    let (l1, l2) = (mean + d, mean - d);
    if !(l2 > 0.0 && l1.is_finite()) {
        return None;
    }
    // A filled ellipse with semi-axis s has variance s² / 4 along it.
    let major = 2.0 * l1.sqrt();
    let minor = 2.0 * l2.sqrt();
    let theta = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let (sin, cos) = theta.sin_cos();
    let radius = |p: Point| {
        let v = p.sub(center);
        let u = v.x * cos + v.y * sin;
        let w = -v.x * sin + v.y * cos;
        (u / major).hypot(w / minor)
    };
    let (mut weight, mut sum) = (0.0, 0.0);
    for i in 0..n {
        let p = poly[i];
        let q = poly[(i + 1) % n];
        let len = p.distance(q);
        let mid = p.add(q).scale(0.5);
        sum += (radius(mid) - 1.0).abs() * len;
        weight += len;
    }
    if weight.is_nan() || weight <= 0.0 {
        return None;
    }
    Some(EllipseFit {
        center,
        major,
        minor,
        angle_degrees: direction_degrees(Point::new(cos, sin)),
        axis_ratio: minor / major,
        mean_radial_deviation: sum / weight,
    })
}

/// The direction of the outline near a point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutlineAngle {
    /// The direction of the nearest edge, in degrees in `0..180`: 0 is
    /// horizontal, 90 vertical.
    pub degrees: f64,
    /// The distance from the point to the outline, in font units.
    pub distance: f64,
    /// The index of the nearest contour in [`GlyphOutline::contours`].
    pub contour: usize,
}

/// One glyph's outline at the default instance.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphOutline {
    contours: Vec<Contour>,
    units_per_em: u16,
}

impl GlyphOutline {
    /// An outline of `contours` in a font with `units_per_em`.
    pub fn new(contours: Vec<Contour>, units_per_em: u16) -> Self {
        Self {
            contours,
            units_per_em,
        }
    }

    /// The outline of character `ch` in `font`. `None` when the face does
    /// not map it, the glyph has no outline or its data is malformed.
    pub fn load(font: &FontRef<'_>, ch: char) -> Option<Self> {
        Self::load_from(&Face::new(font)?, ch)
    }

    fn load_from(face: &Face<'_>, ch: char) -> Option<Self> {
        let gid = face
            .charmap
            .map(ch)
            .filter(|&gid| draws(gid, face.num_glyphs))?;
        let glyph = face.outlines.get(gid)?;
        let mut pen = Collector::default();
        glyph
            .draw(
                DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
                &mut pen,
            )
            .ok()?;
        let contours = pen.finish()?;
        if contours.is_empty() {
            return None;
        }
        Some(Self::new(contours, face.units_per_em))
    }

    /// The outlines of `chars` in face `collection_index` of the font file
    /// `data` (TrueType, OpenType, a collection, WOFF or WOFF2), the file
    /// and its tables parsed once. `None` when the face cannot be read; a
    /// character [`load`](Self::load) cannot answer is `None` in the list.
    pub fn load_all(
        data: &[u8],
        collection_index: u32,
        chars: &[char],
    ) -> Option<Vec<Option<Self>>> {
        let sfnt = sfnt(data).ok()?;
        let font = font_ref(&sfnt, collection_index)?;
        Self::load_many(&font, chars)
    }

    /// The outlines of `chars` in `font`, its tables parsed once: what
    /// [`load_all`](Self::load_all) answers for a face already opened.
    pub(crate) fn load_many(font: &FontRef<'_>, chars: &[char]) -> Option<Vec<Option<Self>>> {
        let face = Face::new(font)?;
        Some(chars.iter().map(|&ch| Self::load_from(&face, ch)).collect())
    }

    /// The contours, in the order the font draws them.
    pub fn contours(&self) -> &[Contour] {
        &self.contours
    }

    /// The font's units per em.
    pub fn units_per_em(&self) -> u16 {
        self.units_per_em
    }

    /// The tight bounding box of all contours.
    pub fn bbox(&self) -> Option<BBox> {
        self.contours
            .iter()
            .filter_map(Contour::bbox)
            .reduce(BBox::union)
    }

    /// The flattening tolerance the outline's own measures use: a
    /// thousandth of the em.
    pub fn fine_tolerance(&self) -> f64 {
        (f64::from(self.units_per_em) / 1000.0).max(MIN_TOLERANCE)
    }

    /// The enclosed counters (holes) of the filled glyph: one for an `O`,
    /// two for a double-storey `g`, none for an `l`.
    ///
    /// The outline is filled by the nonzero rule on a grid of
    /// [`COUNTER_CELLS`] cells to the em, and every unfilled region the
    /// outside cannot reach is a counter. So overlapping contours, a contour
    /// that crosses itself and the winding direction of the font change
    /// nothing, and a region smaller than [`MIN_COUNTER`] of the em squared
    /// (a sliver where two strokes meet) is no counter.
    pub fn counters(&self) -> Vec<Counter> {
        let Some(grid) = Grid::fill(self) else {
            return Vec::new();
        };
        let min_area = (MIN_COUNTER * f64::from(self.units_per_em)).powi(2);
        grid.holes()
            .into_iter()
            .filter(|c| c.area >= min_area)
            .collect()
    }

    /// The number of [`counters`](Self::counters).
    pub fn counter_count(&self) -> usize {
        self.counters().len()
    }

    /// The direction of the outline nearest `p`, the outline flattened at
    /// `tolerance`: the angle of a stem or a terminal at that point. `None`
    /// for an outline without edges or a tolerance that is not a positive
    /// number.
    pub fn angle_at(&self, p: Point, tolerance: f64) -> Option<OutlineAngle> {
        let tolerance = valid_tolerance(tolerance)?;
        let mut best: Option<(f64, Point, usize)> = None;
        for (i, c) in self.contours.iter().enumerate() {
            let poly = c.flatten_valid(tolerance);
            let n = poly.len();
            // An open contour has no closing edge to measure.
            let edges = if c.closed { n } else { n.saturating_sub(1) };
            for k in 0..edges {
                let (a, b) = (poly[k], poly[(k + 1) % n]);
                let d = b.sub(a);
                let len2 = d.dot(d);
                if len2.is_nan() || len2 <= 0.0 {
                    continue;
                }
                let t = (p.sub(a).dot(d) / len2).clamp(0.0, 1.0);
                let dist = p.distance(a.add(d.scale(t)));
                if best.is_none_or(|(bd, _, _)| dist < bd) {
                    best = Some((dist, d, i));
                }
            }
        }
        let (distance, d, contour) = best?;
        Some(OutlineAngle {
            degrees: direction_degrees(d),
            distance,
            contour,
        })
    }
}

/// The direction of `d` in degrees, folded into `0..180`.
fn direction_degrees(d: Point) -> f64 {
    let deg = d.y.atan2(d.x).to_degrees().rem_euclid(180.0);
    if deg >= 180.0 { 0.0 } else { deg }
}

/// One enclosed counter of a glyph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Counter {
    /// Its area in font units squared, to the grid's precision.
    pub area: f64,
    /// Its bounding box, to the grid's precision.
    pub bbox: BBox,
}

/// The cells per em of the counter grid.
pub const COUNTER_CELLS: f64 = 200.0;

/// The most cells a side of the counter grid has; a larger glyph gets
/// larger cells.
const MAX_GRID_SIDE: f64 = 512.0;

/// The side of the smallest counter, as a fraction of the em.
pub const MIN_COUNTER: f64 = 0.01;

/// The glyph filled on a grid of square cells, sampled at the cell centres,
/// as runs of filled cells per row.
struct Grid {
    /// The left edge of the grid.
    x0: f64,
    /// The bottom edge of the grid.
    y0: f64,
    cell: f64,
    cols: usize,
    /// Per row, bottom row first: the filled columns `[from, to)`, sorted and
    /// apart.
    ink: Vec<Vec<(usize, usize)>>,
}

impl Grid {
    /// The filled grid of `glyph`, with one empty cell of margin around its
    /// box. `None` for an outline without a box.
    fn fill(glyph: &GlyphOutline) -> Option<Self> {
        let b = glyph.bbox()?;
        let side = b.width().max(b.height());
        let cell = (f64::from(glyph.units_per_em) / COUNTER_CELLS)
            .max(side / MAX_GRID_SIDE)
            .max(MIN_TOLERANCE);
        if !(cell.is_finite() && side.is_finite()) {
            return None;
        }
        let (x0, y0) = (b.x_min - cell, b.y_min - cell);
        let cols = (b.width() / cell).ceil() as usize + 2;
        let rows = (b.height() / cell).ceil() as usize + 2;
        // The index of the first row or column whose centre is at or past `v`.
        let first = |v: f64, origin: f64, limit: usize| {
            (((v - origin) / cell - 0.5).ceil().max(0.0) as usize).min(limit)
        };
        // Per row: where the edges cross the row's centre line, and which way.
        let mut crossings: Vec<Vec<(f64, i32)>> = vec![Vec::new(); rows];
        let mut poly = Vec::new();
        for c in &glyph.contours {
            poly.clear();
            c.flatten_append(cell / 2.0, &mut poly);
            let n = poly.len();
            for k in 0..n {
                let (a, e) = (poly[k], poly[(k + 1) % n]);
                if a.y == e.y {
                    continue;
                }
                let (lo, hi, dir) = if a.y < e.y { (a, e, 1) } else { (e, a, -1) };
                let slope = (hi.x - lo.x) / (hi.y - lo.y);
                // The rows whose centre is in [lo.y, hi.y).
                for (r, row) in crossings
                    .iter_mut()
                    .enumerate()
                    .take(first(hi.y, y0, rows))
                    .skip(first(lo.y, y0, rows))
                {
                    let y = y0 + (r as f64 + 0.5) * cell;
                    row.push((lo.x + (y - lo.y) * slope, dir));
                }
            }
        }
        let mut ink = Vec::with_capacity(rows);
        for mut row in crossings {
            row.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut runs: Vec<(usize, usize)> = Vec::new();
            let mut winding = 0;
            for pair in row.windows(2) {
                winding += pair[0].1;
                if winding == 0 {
                    continue;
                }
                let (from, to) = (first(pair[0].0, x0, cols), first(pair[1].0, x0, cols));
                if from >= to {
                    continue;
                }
                match runs.last_mut() {
                    Some(last) if last.1 >= from => last.1 = last.1.max(to),
                    _ => runs.push((from, to)),
                }
            }
            ink.push(runs);
        }
        Some(Self {
            x0,
            y0,
            cell,
            cols,
            ink,
        })
    }

    /// The unfilled regions the outside cannot reach, four-connected: the
    /// empty runs of each row joined to the empty runs they touch in the row
    /// below.
    fn holes(&self) -> Vec<Counter> {
        let rows = self.ink.len();
        // Empty runs: (row, from, to); run 0 stands for the outside.
        let mut runs: Vec<(usize, usize, usize)> = vec![(0, 0, 0)];
        let mut row_start = Vec::with_capacity(rows + 1);
        for (r, ink) in self.ink.iter().enumerate() {
            row_start.push(runs.len());
            let mut at = 0;
            for &(from, to) in ink {
                if from > at {
                    runs.push((r, at, from));
                }
                at = to;
            }
            if at < self.cols {
                runs.push((r, at, self.cols));
            }
        }
        row_start.push(runs.len());
        let mut parent: Vec<usize> = (0..runs.len()).collect();
        for r in 0..rows {
            let row = row_start[r]..row_start[r + 1];
            for (i, &(_, from, to)) in runs.iter().enumerate().take(row.end).skip(row.start) {
                // The margin rows and columns are outside.
                if r == 0 || r + 1 == rows || from == 0 || to == self.cols {
                    union(&mut parent, i, 0);
                }
            }
            if r == 0 {
                continue;
            }
            let (mut i, mut j) = (row_start[r - 1], row_start[r]);
            while i < row_start[r] && j < row_start[r + 1] {
                let (_, a0, a1) = runs[i];
                let (_, b0, b1) = runs[j];
                if a0 < b1 && b0 < a1 {
                    union(&mut parent, i, j);
                }
                if a1 <= b1 { i += 1 } else { j += 1 }
            }
        }
        // Per hole: cells and the columns and rows it spans.
        let outside = find(&mut parent, 0);
        let mut holes: Vec<(usize, usize, usize, usize, usize, usize)> = Vec::new();
        let mut slot = vec![usize::MAX; runs.len()];
        for (i, &(r, from, to)) in runs.iter().enumerate().skip(1) {
            let root = find(&mut parent, i);
            if root == outside {
                continue;
            }
            if slot[root] == usize::MAX {
                slot[root] = holes.len();
                holes.push((0, from, r, to, r, root));
            }
            let h = &mut holes[slot[root]];
            h.0 += to - from;
            h.1 = h.1.min(from);
            h.2 = h.2.min(r);
            h.3 = h.3.max(to);
            h.4 = h.4.max(r + 1);
        }
        let corner = |c: usize, r: usize| {
            Point::new(
                self.x0 + c as f64 * self.cell,
                self.y0 + r as f64 * self.cell,
            )
        };
        holes
            .into_iter()
            .map(|(cells, c0, r0, c1, r1, _)| {
                let mut bbox = BBox::at(corner(c0, r0));
                bbox.include(corner(c1, r1));
                Counter {
                    area: cells as f64 * self.cell * self.cell,
                    bbox,
                }
            })
            .collect()
    }
}

/// The root of `i` in the union-find `parent`, the path halved.
fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let (ra, rb) = (find(parent, a), find(parent, b));
    if ra != rb {
        parent[ra.max(rb)] = ra.min(rb);
    }
}

/// The tables [`GlyphOutline::load`] reads, parsed once per face.
struct Face<'a> {
    charmap: skrifa::charmap::Charmap<'a>,
    outlines: skrifa::outline::OutlineGlyphCollection<'a>,
    num_glyphs: u32,
    units_per_em: u16,
}

impl<'a> Face<'a> {
    fn new(font: &FontRef<'a>) -> Option<Self> {
        Some(Self {
            charmap: font.charmap(),
            outlines: font.outline_glyphs(),
            num_glyphs: num_glyphs(font).ok()?,
            units_per_em: font.head().ok()?.units_per_em(),
        })
    }
}

/// The TrueType or OpenType data of a font file: WOFF and WOFF2
/// decompressed, anything else as it is. `None` when web font data cannot
/// be decompressed. Decompress once to read many faces or glyphs of a web
/// font with [`GlyphOutline::load_all`].
pub fn sfnt_data(data: &[u8]) -> Option<Cow<'_, [u8]>> {
    sfnt(data).ok()
}

fn valid_tolerance(tolerance: f64) -> Option<f64> {
    (tolerance.is_finite() && tolerance > 0.0).then(|| tolerance.max(MIN_TOLERANCE))
}

/// Face `index` of the TrueType or OpenType data `sfnt`, without the checks
/// the glyph map needs: an outline needs only `head`, `maxp`, `cmap` and the
/// outline tables, each read when it is used.
pub(crate) fn font_ref(sfnt: &[u8], index: u32) -> Option<FontRef<'_>> {
    match FileRef::new(sfnt).ok()? {
        FileRef::Font(font) => (index == 0).then_some(font),
        FileRef::Collection(c) => c.get(index).ok(),
    }
}

/// The pen that collects contours from skrifa.
#[derive(Default)]
struct Collector {
    contours: Vec<Contour>,
    current: Vec<Segment>,
    start: Point,
    last: Point,
    /// A coordinate was not a finite number.
    bad: bool,
}

impl Collector {
    fn point(&mut self, x: f32, y: f32) -> Point {
        if !(x.is_finite() && y.is_finite()) {
            self.bad = true;
        }
        Point::new(f64::from(x), f64::from(y))
    }

    fn push(&mut self, s: Segment) {
        self.current.push(s);
        self.last = s.end();
    }

    fn end_contour(&mut self, closed: bool) {
        if !self.current.is_empty() {
            self.contours
                .push(Contour::new(std::mem::take(&mut self.current), closed));
        }
    }

    /// The contours; `None` when a coordinate was not a number.
    fn finish(mut self) -> Option<Vec<Contour>> {
        self.end_contour(false);
        (!self.bad).then_some(self.contours)
    }
}

impl OutlinePen for Collector {
    fn move_to(&mut self, x: f32, y: f32) {
        self.end_contour(false);
        let p = self.point(x, y);
        self.start = p;
        self.last = p;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.point(x, y);
        if p != self.last {
            self.push(Segment::Line(self.last, p));
        }
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let c = self.point(cx0, cy0);
        let p = self.point(x, y);
        self.push(Segment::Quad(self.last, c, p));
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let c0 = self.point(cx0, cy0);
        let c1 = self.point(cx1, cy1);
        let p = self.point(x, y);
        self.push(Segment::Cubic(self.last, c0, c1, p));
    }

    fn close(&mut self) {
        if !self.current.is_empty() && self.last != self.start {
            self.push(Segment::Line(self.last, self.start));
        }
        self.end_contour(true);
        self.last = self.start;
    }
}

#[cfg(test)]
mod tests;
