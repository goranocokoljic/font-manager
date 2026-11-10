//! A glyph outline as the detectors read it: flattened, upright (an italic
//! sheared back by its slant) and probed with horizontal and vertical scan
//! lines under the nonzero rule.

use crate::glyphs::geometry::{BBox, Contour, GlyphOutline, Point, Segment};

/// Flattening tolerance, as a fraction of the em.
const TOLERANCE_EM: f64 = 1.0 / 500.0;

/// One glyph, flattened and upright.
#[derive(Debug, Clone)]
pub(crate) struct Shape {
    /// Closed polygons, in font units with the slant removed.
    polys: Vec<Vec<Point>>,
    /// The box of every polygon.
    pub(crate) bbox: BBox,
    /// The outline sheared upright, for counters.
    pub(crate) outline: GlyphOutline,
}

/// The x offset of a point of height `y` in a face slanted by `slant`
/// degrees (positive leans right): `x - y tan(slant)` is upright.
fn unslant(p: Point, tan: f64) -> Point {
    Point::new(p.x - p.y * tan, p.y)
}

impl Shape {
    /// `outline` flattened and sheared upright by `slant_degrees`. `None`
    /// for an outline without area.
    pub(crate) fn new(outline: GlyphOutline, slant_degrees: f64) -> Option<Self> {
        let tan = slant_degrees.to_radians().tan();
        let outline = if tan == 0.0 {
            outline
        } else {
            upright(&outline, tan)
        };
        let tolerance = f64::from(outline.units_per_em()) * TOLERANCE_EM;
        let polys: Vec<Vec<Point>> = outline
            .contours()
            .iter()
            .filter_map(|c| c.flatten(tolerance))
            .filter(|p| p.len() >= 3)
            .collect();
        let mut points = polys.iter().flatten().copied();
        let first = points.next()?;
        let mut bbox = BBox {
            x_min: first.x,
            y_min: first.y,
            x_max: first.x,
            y_max: first.y,
        };
        for p in points {
            bbox.x_min = bbox.x_min.min(p.x);
            bbox.x_max = bbox.x_max.max(p.x);
            bbox.y_min = bbox.y_min.min(p.y);
            bbox.y_max = bbox.y_max.max(p.y);
        }
        (bbox.width() > 0.0 && bbox.height() > 0.0).then_some(Self {
            polys,
            bbox,
            outline,
        })
    }

    /// The height of the box.
    pub(crate) fn height(&self) -> f64 {
        self.bbox.height()
    }

    /// The width of the box.
    pub(crate) fn width(&self) -> f64 {
        self.bbox.width()
    }

    /// The height at fraction `f` of the box, from its bottom.
    pub(crate) fn y_at(&self, f: f64) -> f64 {
        self.bbox.y_min + f * self.height()
    }

    /// The inked intervals of the horizontal line at height `y`, left to
    /// right.
    pub(crate) fn row(&self, y: f64) -> Vec<(f64, f64)> {
        self.runs(|a, b| crossing(a.y, a.x, b.y, b.x, y))
    }

    /// The inked intervals of the vertical line at `x`, bottom to top.
    pub(crate) fn column(&self, x: f64) -> Vec<(f64, f64)> {
        self.runs(|a, b| crossing(a.x, a.y, b.x, b.y, x))
    }

    /// Intervals where the nonzero winding of the line is not zero; `cross`
    /// gives where an edge crosses the line and its direction.
    fn runs(&self, cross: impl Fn(Point, Point) -> Option<(f64, i32)>) -> Vec<(f64, f64)> {
        let mut hits: Vec<(f64, i32)> = Vec::new();
        for poly in &self.polys {
            let n = poly.len();
            for i in 0..n {
                if let Some(hit) = cross(poly[i], poly[(i + 1) % n]) {
                    hits.push(hit);
                }
            }
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut out = Vec::new();
        let mut winding = 0;
        let mut start = 0.0;
        for (at, dir) in hits {
            let before = winding;
            winding += dir;
            if before == 0 && winding != 0 {
                start = at;
            } else if before != 0 && winding == 0 && at > start {
                out.push((start, at));
            }
        }
        out
    }

    /// Whether the point is inked.
    pub(crate) fn inked(&self, p: Point) -> bool {
        self.row(p.y).iter().any(|&(a, b)| a <= p.x && p.x <= b)
    }
}

/// `outline` with every point sheared by [`unslant`].
fn upright(outline: &GlyphOutline, tan: f64) -> GlyphOutline {
    let u = |p| unslant(p, tan);
    let contours = outline
        .contours()
        .iter()
        .map(|c| {
            let segments = c
                .segments()
                .iter()
                .map(|s| match *s {
                    Segment::Line(a, b) => Segment::Line(u(a), u(b)),
                    Segment::Quad(a, c, b) => Segment::Quad(u(a), u(c), u(b)),
                    Segment::Cubic(a, c0, c1, b) => Segment::Cubic(u(a), u(c0), u(c1), u(b)),
                })
                .collect();
            Contour::new(segments, c.is_closed())
        })
        .collect();
    GlyphOutline::new(contours, outline.units_per_em())
}

/// Where the edge from `(a_s, a_t)` to `(b_s, b_t)` crosses the line
/// `s = at`: the `t` there, and +1 or -1 for its direction. Half-open in `s`
/// so a vertex on the line counts once.
fn crossing(a_s: f64, a_t: f64, b_s: f64, b_t: f64, at: f64) -> Option<(f64, i32)> {
    let up = a_s <= at && at < b_s;
    let down = b_s <= at && at < a_s;
    if !(up || down) {
        return None;
    }
    let t = a_t + (at - a_s) / (b_s - a_s) * (b_t - a_t);
    t.is_finite().then_some((t, if up { 1 } else { -1 }))
}

/// The least-squares slope `dy/dx` of points and the largest distance of a
/// point from the fitted line, as a fraction of `scale`. `None` for fewer
/// than three points or no spread in x.
pub(crate) fn fit(points: &[(f64, f64)], scale: f64) -> Option<(f64, f64)> {
    if points.len() < 3 || scale <= 0.0 {
        return None;
    }
    let n = points.len() as f64;
    let mx = points.iter().map(|p| p.0).sum::<f64>() / n;
    let my = points.iter().map(|p| p.1).sum::<f64>() / n;
    let sxx: f64 = points.iter().map(|p| (p.0 - mx).powi(2)).sum();
    if sxx <= 0.0 {
        return None;
    }
    let sxy: f64 = points.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let slope = sxy / sxx;
    let norm = slope.hypot(1.0);
    let worst = points
        .iter()
        .map(|p| ((p.1 - my) - slope * (p.0 - mx)).abs() / norm)
        .fold(0.0, f64::max);
    Some((slope, worst / scale))
}

/// The median of `values`; `None` when empty.
pub(crate) fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    Some(values[values.len() / 2])
}
