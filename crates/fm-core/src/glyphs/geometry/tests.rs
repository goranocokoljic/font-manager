use std::path::PathBuf;

use read_fonts::TableProvider;
use read_fonts::tables::glyf::Glyph;

use super::*;

fn read(rel: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts")
        .join(rel);
    std::fs::read(path).expect("fixture")
}

/// Static TrueType (quadratic).
const ROBOTO_SLAB: &str = "google-static/robotoslab/RobotoSlab-ExtraBold.ttf";
/// Variable TrueType; a plain sans like Inter (not in the corpus).
const EPILOGUE: &str = "google-repo/epilogue/Epilogue[wght].ttf";
/// CFF (cubic).
const NUNITO_OTF: &str = "fontshare/nunito/Nunito-ExtraLightItalic.otf";
/// Double-storey `g` (fixtures/glyph-traits-labels.json), TrueType.
const CRIMSON_PRO: &str = "google-repo/crimsonpro/CrimsonPro[wght].ttf";
/// Double-storey `g`, CFF.
const ASAP_OTF: &str = "fontshare/asap/Asap-SemiBold.otf";
/// Single-storey `g`.
const BARLOW: &str = "google-repo/barlow/Barlow-BoldItalic.ttf";
const WOFF2: &str = "fontshare/jet-brains-mono/JetBrainsMono-Variable.woff2";

fn outline(rel: &str, ch: char) -> GlyphOutline {
    let data = read(rel);
    GlyphOutline::load_all(&data, 0, &[ch])
        .and_then(|mut v| v.pop().flatten())
        .unwrap_or_else(|| panic!("{rel}: no outline for {ch:?}"))
}

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// An axis-aligned rectangle, counter-clockwise or clockwise.
fn rect(x0: f64, y0: f64, x1: f64, y1: f64, ccw: bool) -> Contour {
    let mut pts = [p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)];
    if !ccw {
        pts.reverse();
    }
    let segs = (0..4)
        .map(|i| Segment::Line(pts[i], pts[(i + 1) % 4]))
        .collect();
    Contour::new(segs, true)
}

/// An ellipse of semi-axes `rx`, `ry` about `(cx, cy)` as four cubic arcs,
/// counter-clockwise.
fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Contour {
    const K: f64 = 0.552_284_75;
    let pts = [
        p(cx + rx, cy),
        p(cx, cy + ry),
        p(cx - rx, cy),
        p(cx, cy - ry),
    ];
    let tangents = [p(0.0, ry), p(-rx, 0.0), p(0.0, -ry), p(rx, 0.0)];
    let segs = (0..4)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            let (ta, tb) = (tangents[i], tangents[(i + 1) % 4]);
            Segment::Cubic(a, a.add(ta.scale(K)), b.sub(tb.scale(K)), b)
        })
        .collect();
    Contour::new(segs, true)
}

fn glyph(contours: Vec<Contour>) -> GlyphOutline {
    // 100 units per em: a cell of the counter grid is half a unit.
    GlyphOutline::new(contours, 100)
}

// ---- Fixture facts (criterion 3) ----

#[test]
fn an_o_has_two_contours_and_one_counter_in_truetype_and_cff() {
    for rel in [EPILOGUE, ROBOTO_SLAB, NUNITO_OTF] {
        let o = outline(rel, 'O');
        assert_eq!(o.contours().len(), 2, "{rel}");
        let counters = o.counters();
        assert_eq!(counters.len(), 1, "{rel}");
        // The counter is the inside of the inner contour, and the two
        // contours wind the other way round.
        let [outer, inner] = o.contours() else {
            unreachable!()
        };
        let (bo, bi) = (outer.bbox().expect("bbox"), inner.bbox().expect("bbox"));
        let (inner, bi, outer) = if bi.width() < bo.width() {
            (inner, bi, outer)
        } else {
            (outer, bo, inner)
        };
        let cell = f64::from(o.units_per_em()) / COUNTER_CELLS;
        let got = counters[0].bbox;
        for (g, w) in [
            (got.x_min, bi.x_min),
            (got.y_min, bi.y_min),
            (got.x_max, bi.x_max),
            (got.y_max, bi.y_max),
        ] {
            assert!((g - w).abs() <= 2.0 * cell, "{rel}: {got:?} vs {bi:?}");
        }
        let hole = inner.signed_area().abs();
        assert!((counters[0].area / hole - 1.0).abs() < 0.05, "{rel}");
        assert_ne!(outer.winding(), inner.winding(), "{rel}");
        assert_ne!(outer.winding(), Winding::Flat, "{rel}");
    }
}

#[test]
fn truetype_and_cff_wind_the_outer_contour_the_other_way_round() {
    let outer = |rel| {
        let o = outline(rel, 'O');
        o.contours()
            .iter()
            .max_by(|a, b| {
                let (a, b) = (a.bbox().expect("bbox"), b.bbox().expect("bbox"));
                a.width().total_cmp(&b.width())
            })
            .expect("contour")
            .winding()
    };
    assert_eq!(outer(ROBOTO_SLAB), Winding::Clockwise);
    assert_eq!(outer(NUNITO_OTF), Winding::CounterClockwise);
}

/// Crimson Pro keeps the overlaps of its variable outlines: the lower loop of
/// its `g` is a contour that crosses itself, and the upper bowl overlaps it.
#[test]
fn a_double_storey_g_has_two_counters_and_a_single_storey_one_has_one() {
    for rel in [CRIMSON_PRO, ASAP_OTF] {
        let g = outline(rel, 'g');
        let counters = g.counters();
        assert_eq!(counters.len(), 2, "{rel}");
        // One storey above the other.
        let (a, b) = (counters[0].bbox, counters[1].bbox);
        assert!(
            a.y_min >= b.y_max || b.y_min >= a.y_max,
            "{rel}: {a:?} {b:?}"
        );
    }
    assert_eq!(outline(BARLOW, 'g').counter_count(), 1, "{BARLOW}");
}

#[test]
fn an_l_has_one_contour_and_no_counter() {
    for rel in [EPILOGUE, ROBOTO_SLAB, NUNITO_OTF] {
        let l = outline(rel, 'l');
        assert_eq!(l.contours().len(), 1, "{rel}");
        assert_eq!(l.counter_count(), 0, "{rel}");
    }
}

#[test]
fn a_space_and_an_unmapped_character_have_no_outline() {
    let data = read(EPILOGUE);
    let got = GlyphOutline::load_all(&data, 0, &[' ', '\u{E000}', 'O']).expect("face");
    assert_eq!(got[0], None, "space");
    assert_eq!(got[1], None, "private use");
    assert!(got[2].is_some(), "O");
}

#[test]
fn malformed_data_is_none_never_a_panic() {
    assert_eq!(GlyphOutline::load_all(b"not a font", 0, &['O']), None);
    assert_eq!(GlyphOutline::load_all(&[], 0, &['O']), None);
    let data = read(ROBOTO_SLAB);
    // A face the file does not have.
    assert_eq!(GlyphOutline::load_all(&data, 3, &['O']), None);
    // Truncated: the header lists tables past the end.
    assert_eq!(
        GlyphOutline::load_all(&data[..data.len() / 3], 0, &['O'])
            .map(|v| v.into_iter().flatten().count())
            .unwrap_or(0),
        0
    );
    // A glyf table of garbage: every glyph claims 0x7F7F contours.
    let mut bad = data.clone();
    let (offset, len) = table(&bad, b"glyf");
    bad[offset..offset + len].fill(0x7F);
    let got = GlyphOutline::load_all(&bad, 0, &['O', 'l']).expect("the face opens");
    assert_eq!(got, vec![None, None]);
}

#[test]
fn a_woff2_file_is_read() {
    let o = outline(WOFF2, 'O');
    assert_eq!((o.contours().len(), o.counter_count()), (2, 1));
    // Decompressed once, it reads the same.
    let data = read(WOFF2);
    let sfnt = sfnt_data(&data).expect("sfnt");
    assert_ne!(&sfnt[..4], b"wOF2");
    let again = GlyphOutline::load_all(&sfnt, 0, &['O']).expect("face");
    assert_eq!(again, vec![Some(o)]);
    assert_eq!(sfnt_data(b"wOF2 and nothing else"), None);
    assert_eq!(sfnt_data(b"plain").as_deref(), Some(&b"plain"[..]));
}

#[test]
fn every_trait_glyph_of_a_sans_loads() {
    let data = read(EPILOGUE);
    let got = GlyphOutline::load_all(&data, 0, &TRAIT_GLYPHS).expect("face");
    assert_eq!(got.len(), 15);
    for (ch, o) in TRAIT_GLYPHS.iter().zip(&got) {
        let o = o.as_ref().unwrap_or_else(|| panic!("{ch:?}"));
        assert!(!o.contours().is_empty(), "{ch:?}");
        assert_eq!(o.units_per_em(), 2000);
    }
}

/// The offset and length of table `tag` in the table directory of `data`.
fn table(data: &[u8], tag: &[u8; 4]) -> (usize, usize) {
    let be = |at: usize| u32::from_be_bytes(data[at..at + 4].try_into().expect("4 bytes")) as usize;
    let count = usize::from(u16::from_be_bytes([data[4], data[5]]));
    (0..count)
        .map(|i| 12 + 16 * i)
        .find(|&r| &data[r..r + 4] == tag)
        .map(|r| (be(r + 8), be(r + 12)))
        .expect("table")
}

// ---- Composites, overlaps, open contours (criterion 2) ----

#[test]
fn a_composite_glyph_has_the_contours_of_its_components() {
    let data = read(ROBOTO_SLAB);
    let font = FontRef::new(&data).expect("font");
    let gid = font.charmap().map('Ö').expect("Ö");
    let glyf = font.glyf().expect("glyf");
    let raw = font.loca(None).expect("loca").get_glyf(gid, &glyf);
    assert!(
        matches!(raw, Ok(Some(Glyph::Composite(_)))),
        "Ö should be a composite in the fixture"
    );
    let o = outline(ROBOTO_SLAB, 'O');
    let umlaut = outline(ROBOTO_SLAB, 'Ö');
    assert_eq!(umlaut.contours().len(), o.contours().len() + 2);
    assert_eq!(umlaut.counter_count(), 1);
    let (bo, bu) = (o.bbox().expect("bbox"), umlaut.bbox().expect("bbox"));
    assert!(bu.y_max > bo.y_max + 50.0, "{bo:?} {bu:?}");
}

#[test]
fn a_hole_is_a_counter_whichever_way_the_font_winds() {
    for outer_ccw in [true, false] {
        let g = glyph(vec![
            rect(0.0, 0.0, 100.0, 100.0, outer_ccw),
            rect(30.0, 30.0, 70.0, 70.0, !outer_ccw),
        ]);
        let counters = g.counters();
        assert_eq!(counters.len(), 1, "outer ccw {outer_ccw}");
        // 40 × 40, to the half-unit grid.
        assert!((counters[0].area - 1600.0).abs() < 80.0, "{counters:?}");
    }
}

#[test]
fn overlapping_contours_are_not_counters() {
    // Two strokes of a t, crossing, wound the same way.
    let t = glyph(vec![
        rect(40.0, 0.0, 60.0, 100.0, false),
        rect(0.0, 60.0, 100.0, 80.0, false),
    ]);
    assert_eq!(t.counter_count(), 0);
    // A contour inside another, wound the same way, is ink on ink.
    let nested = glyph(vec![
        rect(0.0, 0.0, 100.0, 100.0, true),
        rect(30.0, 30.0, 70.0, 70.0, true),
    ]);
    assert_eq!(nested.counter_count(), 0);
}

#[test]
fn a_counter_partly_covered_by_another_contour_still_counts() {
    // An O with a bar across the top of its counter.
    let g = glyph(vec![
        rect(0.0, 0.0, 100.0, 100.0, true),
        rect(20.0, 20.0, 80.0, 80.0, false),
        rect(10.0, 60.0, 90.0, 90.0, true),
    ]);
    let counters = g.counters();
    assert_eq!(counters.len(), 1);
    // What is left of it: 60 × 40.
    assert!((counters[0].area - 2400.0).abs() < 100.0, "{counters:?}");
    assert!((counters[0].bbox.y_max - 60.0).abs() <= 1.0, "{counters:?}");
}

#[test]
fn a_dot_inside_a_counter_is_ink_not_a_counter() {
    let g = glyph(vec![
        rect(0.0, 0.0, 100.0, 100.0, true),
        rect(20.0, 20.0, 80.0, 80.0, false),
        rect(45.0, 45.0, 55.0, 55.0, true),
    ]);
    // One counter, a ring around the dot.
    let counters = g.counters();
    assert_eq!(counters.len(), 1);
    assert!((counters[0].area - 3500.0).abs() < 150.0, "{counters:?}");
}

#[test]
fn one_contour_that_doubles_back_through_a_slit_has_a_counter() {
    // A square ring drawn as one contour: round the outside, along a slit of
    // no width to the inside, round the inside the other way and back.
    let pts = [
        (0.0, 0.0),
        (100.0, 0.0),
        (100.0, 100.0),
        (0.0, 100.0),
        (0.0, 50.0),
        (20.0, 50.0),
        (20.0, 80.0),
        (80.0, 80.0),
        (80.0, 20.0),
        (20.0, 20.0),
        (20.0, 50.0),
        (0.0, 50.0),
    ];
    let segs = (0..pts.len())
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            Segment::Line(p(a.0, a.1), p(b.0, b.1))
        })
        .collect();
    let g = glyph(vec![Contour::new(segs, true)]);
    assert_eq!(g.contours().len(), 1);
    assert_eq!(g.counter_count(), 1);
}

#[test]
fn a_sliver_smaller_than_the_least_counter_is_no_counter() {
    // A hole of 0.5 × 0.5 units in a 100-unit em: below 1 % of the em.
    let g = glyph(vec![
        rect(0.0, 0.0, 50.0, 50.0, true),
        rect(20.0, 20.0, 20.5, 20.5, false),
    ]);
    assert_eq!(g.counter_count(), 0);
    // Twice the least side is a counter.
    let g = glyph(vec![
        rect(0.0, 0.0, 50.0, 50.0, true),
        rect(20.0, 20.0, 22.0, 22.0, false),
    ]);
    assert_eq!(g.counter_count(), 1);
}

#[test]
fn the_pen_keeps_open_contours_and_drops_empty_ones() {
    let mut pen = Collector::default();
    // A lone move, then an open triangle, then a closed one with a zero-length line.
    pen.move_to(5.0, 5.0);
    pen.move_to(0.0, 0.0);
    pen.line_to(100.0, 0.0);
    pen.line_to(0.0, 100.0);
    pen.move_to(200.0, 0.0);
    pen.line_to(300.0, 0.0);
    pen.line_to(300.0, 0.0);
    pen.line_to(200.0, 100.0);
    pen.close();
    let contours = pen.finish().expect("contours");
    assert_eq!(contours.len(), 2);
    let (open, closed) = (&contours[0], &contours[1]);
    assert!(!open.is_closed());
    assert_eq!(open.segments().len(), 2);
    // Closed for the fill: a right triangle of 100 × 100.
    assert!((open.signed_area() - 5000.0).abs() < 1e-9);
    assert_eq!(open.winding(), Winding::CounterClockwise);
    assert_eq!(
        open.ellipse_fit(1.0),
        None,
        "an open contour has no ellipse fit"
    );
    assert!(closed.is_closed());
    assert_eq!(
        closed.segments().len(),
        3,
        "zero-length line dropped, closing line added"
    );
    assert_eq!(closed.segments()[2].end(), p(200.0, 0.0));
}

#[test]
fn a_coordinate_that_is_not_a_number_makes_the_outline_none() {
    let mut pen = Collector::default();
    pen.move_to(0.0, 0.0);
    pen.line_to(f32::NAN, 10.0);
    pen.close();
    assert_eq!(pen.finish(), None);
}

#[test]
fn an_open_contour_has_no_closing_edge_for_the_angle() {
    // An open L: the closing diagonal is not outline.
    let l = glyph(vec![Contour::new(
        vec![
            Segment::Line(p(0.0, 100.0), p(0.0, 0.0)),
            Segment::Line(p(0.0, 0.0), p(100.0, 0.0)),
        ],
        false,
    )]);
    let a = l.angle_at(p(60.0, 60.0), 1.0).expect("angle");
    assert!(a.degrees == 0.0 || (a.degrees - 90.0).abs() < 1e-9, "{a:?}");
}

// ---- Measures (criterion 1) ----

#[test]
fn the_bbox_is_tight_on_curves() {
    let c = ellipse(0.0, 0.0, 100.0, 50.0);
    let b = c.bbox().expect("bbox");
    for (got, want) in [
        (b.x_min, -100.0),
        (b.x_max, 100.0),
        (b.y_min, -50.0),
        (b.y_max, 50.0),
    ] {
        assert!((got - want).abs() < 1e-9, "{b:?}");
    }
    // A quadratic bulge reaches half its control point's height.
    let q = Segment::Quad(p(0.0, 0.0), p(50.0, 100.0), p(100.0, 0.0)).bbox();
    assert!((q.y_max - 50.0).abs() < 1e-9, "{q:?}");
    // A cubic overshoot.
    let k = Segment::Cubic(p(0.0, 0.0), p(-30.0, 60.0), p(130.0, 60.0), p(100.0, 0.0)).bbox();
    assert!(
        k.x_min < 0.0 && k.x_max > 100.0 && (k.y_max - 45.0).abs() < 1e-9,
        "{k:?}"
    );
}

#[test]
fn the_bbox_of_a_font_glyph_lies_within_the_glyf_header_bounds() {
    let data = read(ROBOTO_SLAB);
    let font = FontRef::new(&data).expect("font");
    let gid = font.charmap().map('O').expect("O");
    let glyf = font.glyf().expect("glyf");
    let Ok(Some(raw)) = font.loca(None).expect("loca").get_glyf(gid, &glyf) else {
        panic!("O")
    };
    let b = outline(ROBOTO_SLAB, 'O').bbox().expect("bbox");
    // The header box covers the control points; the outline reaches its edges
    // on extreme points, which a well-made O has.
    for (got, want) in [
        (b.x_min, f64::from(raw.x_min())),
        (b.y_min, f64::from(raw.y_min())),
        (b.x_max, f64::from(raw.x_max())),
        (b.y_max, f64::from(raw.y_max())),
    ] {
        assert!((got - want).abs() <= 1.0, "{b:?} vs header");
    }
}

#[test]
fn the_area_is_exact_for_curves() {
    let c = ellipse(10.0, 20.0, 100.0, 50.0);
    // The four-arc approximation of an ellipse is within 0.03 % of π r1 r2.
    let want = std::f64::consts::PI * 100.0 * 50.0;
    assert!(
        (c.signed_area() / want - 1.0).abs() < 3e-4,
        "{}",
        c.signed_area()
    );
    assert!((rect(0.0, 0.0, 10.0, 20.0, false).signed_area() + 200.0).abs() < 1e-9);
    assert_eq!(
        Contour::new(vec![Segment::Line(p(0.0, 0.0), p(10.0, 10.0))], false).winding(),
        Winding::Flat
    );
}

#[test]
fn flattening_stays_within_the_tolerance() {
    let c = ellipse(0.0, 0.0, 300.0, 200.0);
    for tol in [5.0, 0.5, 0.05] {
        for s in c.segments() {
            let pts = s.flatten(tol).expect("points");
            assert_eq!(pts.first(), Some(&s.start()));
            assert_eq!(pts.last(), Some(&s.end()));
            let n = pts.len() - 1;
            for i in 0..n {
                // The curve between two points never strays more than tol from the chord.
                for k in 1..4 {
                    let t = (i as f64 + f64::from(k) / 4.0) / n as f64;
                    let on = s.point_at(t);
                    let fit = line_fit(&[pts[i], on, pts[i + 1]]).expect("fit");
                    assert!(fit.max_deviation <= tol, "tol {tol}: {}", fit.max_deviation);
                }
            }
        }
    }
    let coarse = c.flatten(5.0).expect("points").len();
    let fine = c.flatten(0.05).expect("points").len();
    assert!(fine > coarse * 5, "{coarse} {fine}");
    // The start is not repeated at the end.
    let poly = c.flatten(1.0).expect("points");
    assert_ne!(poly.first(), poly.last());
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(c.flatten(bad), None, "{bad}");
    }
}

#[test]
fn a_line_fit_measures_the_largest_distance_from_the_chord() {
    let straight = Segment::Line(p(0.0, 0.0), p(30.0, 40.0))
        .line_fit(1.0)
        .expect("fit");
    assert_eq!((straight.chord, straight.max_deviation), (50.0, 0.0));
    let bulge = Segment::Quad(p(0.0, 0.0), p(50.0, 100.0), p(100.0, 0.0))
        .line_fit(0.01)
        .expect("fit");
    assert!((bulge.max_deviation - 50.0).abs() < 0.01, "{bulge:?}");
    assert!((bulge.relative_deviation() - 0.5).abs() < 1e-3);
    assert_eq!(line_fit(&[p(1.0, 1.0)]), None);
    assert_eq!(line_fit(&[]), None);
    assert_eq!(line_fit(&[p(0.0, 0.0), p(5.0, 5.0), p(0.0, 0.0)]), None);
}

#[test]
fn an_ellipse_fit_finds_the_axes_and_the_deviation() {
    let circle = ellipse(50.0, -20.0, 100.0, 100.0)
        .ellipse_fit(0.1)
        .expect("fit");
    assert!((circle.axis_ratio - 1.0).abs() < 1e-3, "{circle:?}");
    assert!(circle.mean_radial_deviation < 0.005, "{circle:?}");
    assert!((circle.center.x - 50.0).abs() < 0.1 && (circle.center.y + 20.0).abs() < 0.1);
    assert!((circle.major - 100.0).abs() < 0.5, "{circle:?}");

    let wide = ellipse(0.0, 0.0, 200.0, 100.0)
        .ellipse_fit(0.1)
        .expect("fit");
    assert!((wide.axis_ratio - 0.5).abs() < 2e-3, "{wide:?}");
    assert!(
        wide.angle_degrees < 0.5 || wide.angle_degrees > 179.5,
        "{wide:?}"
    );
    assert!(wide.mean_radial_deviation < 0.005, "{wide:?}");
    let tall = ellipse(0.0, 0.0, 100.0, 200.0)
        .ellipse_fit(0.1)
        .expect("fit");
    assert!((tall.angle_degrees - 90.0).abs() < 0.5, "{tall:?}");

    // A square is round by its moments but far from the ellipse along its edges.
    let square = rect(0.0, 0.0, 100.0, 100.0, false)
        .ellipse_fit(0.1)
        .expect("fit");
    assert!((square.axis_ratio - 1.0).abs() < 1e-9);
    assert!(square.mean_radial_deviation > 0.05, "{square:?}");

    // A real O is round and smooth.
    let o = outline(EPILOGUE, 'O');
    for c in o.contours() {
        let fit = c.ellipse_fit(o.fine_tolerance()).expect("fit");
        assert!(fit.axis_ratio > 0.6 && fit.axis_ratio <= 1.0, "{fit:?}");
        assert!(
            (fit.angle_degrees - 90.0).abs() < 2.0,
            "an upright O: {fit:?}"
        );
        assert!(fit.mean_radial_deviation < 0.05, "{fit:?}");
    }
    assert_eq!(ellipse(0.0, 0.0, 10.0, 10.0).ellipse_fit(-1.0), None);
}

#[test]
fn the_angle_at_a_point_is_the_direction_of_the_nearest_edge() {
    let stem = glyph(vec![rect(0.0, 0.0, 20.0, 100.0, false)]);
    let side = stem.angle_at(p(-3.0, 50.0), 1.0).expect("angle");
    assert!(
        (side.degrees - 90.0).abs() < 1e-9 && (side.distance - 3.0).abs() < 1e-9,
        "{side:?}"
    );
    let top = stem.angle_at(p(10.0, 99.0), 1.0).expect("angle");
    assert!(top.degrees.abs() < 1e-9, "{top:?}");
    // A stroke slanted at 60°.
    let (dx, dy) = (50.0, 50.0 * 3f64.sqrt());
    let slant = glyph(vec![Contour::new(
        vec![
            Segment::Line(p(0.0, 0.0), p(20.0, 0.0)),
            Segment::Line(p(20.0, 0.0), p(20.0 + dx, dy)),
            Segment::Line(p(20.0 + dx, dy), p(dx, dy)),
            Segment::Line(p(dx, dy), p(0.0, 0.0)),
        ],
        true,
    )]);
    let a = slant
        .angle_at(p(dx / 2.0 - 2.0, dy / 2.0), 1.0)
        .expect("angle");
    assert!((a.degrees - 60.0).abs() < 1e-6, "{a:?}");
    // The left side of a real l stem is upright.
    let l = outline(EPILOGUE, 'l');
    let b = l.bbox().expect("bbox");
    let at = l
        .angle_at(
            p(b.x_min - 5.0, (b.y_min + b.y_max) / 2.0),
            l.fine_tolerance(),
        )
        .expect("angle");
    assert!((at.degrees - 90.0).abs() < 1.0, "{at:?}");
    assert_eq!(glyph(Vec::new()).angle_at(p(0.0, 0.0), 1.0), None);
    assert_eq!(stem.angle_at(p(0.0, 0.0), f64::NAN), None);
}
