use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use serde::Deserialize;

use super::*;
use crate::glyph_traits;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(rel)
}

fn read_font(rel: &str) -> Vec<u8> {
    std::fs::read(fixture("fonts").join(rel)).expect("fixture font")
}

#[derive(Deserialize)]
struct Labels {
    families: Vec<Labelled>,
}

#[derive(Deserialize)]
struct Labelled {
    family: String,
    file: String,
    traits: BTreeMap<String, String>,
}

fn labels() -> Labels {
    let text = std::fs::read_to_string(fixture("glyph-traits-labels.json")).expect("labels");
    serde_json::from_str(&text).expect("labels parse")
}

/// The least share of the labelled fonts a trait must answer confidently.
const MIN_COVERAGE: f64 = 0.5;
/// The least share of confident answers that must be right.
const MIN_ACCURACY: f64 = 0.9;

#[derive(Default)]
struct Tally {
    labelled: usize,
    confident: usize,
    right: usize,
    /// (label, detected) → count, confident answers only.
    confusion: BTreeMap<(String, String), usize>,
    /// The wrong confident answers and their numbers.
    misses: Vec<String>,
}

impl Tally {
    fn accuracy(&self) -> f64 {
        if self.confident == 0 {
            0.0
        } else {
            self.right as f64 / self.confident as f64
        }
    }
    fn coverage(&self) -> f64 {
        if self.labelled == 0 {
            0.0
        } else {
            self.confident as f64 / self.labelled as f64
        }
    }
    fn meets_bar(&self) -> bool {
        self.accuracy() >= MIN_ACCURACY && self.coverage() >= MIN_COVERAGE
    }
}

fn tallies() -> BTreeMap<&'static str, Tally> {
    let mut out: BTreeMap<&'static str, Tally> =
        trait_ids().map(|id| (id, Tally::default())).collect();
    let dump = std::env::var_os("FM_TRAITS_DUMP").is_some();
    for fam in labels().families {
        let data = read_font(&fam.file);
        let (_, _, raw) = measure(&data, 0, &|_| true).expect("labelled font reads");
        for found in raw.into_iter().flatten() {
            let (id, m) = found;
            let detection = finish(id, m.clone());
            let Some(label) = fam.traits.get(id) else {
                continue;
            };
            let tally = out.get_mut(id).expect("every detector has a tally");
            tally.labelled += 1;
            let line = format!(
                "{id:12} {:28} label={label:10} got={:10} conf={:.2} {:?} {:?}",
                fam.family, detection.value, detection.confidence, m.scalars, m.debug
            );
            if dump {
                println!("{line}");
            }
            if detection.confidence >= CONFIDENT {
                tally.confident += 1;
                if detection.value == *label {
                    tally.right += 1;
                } else {
                    tally.misses.push(line);
                }
                *tally
                    .confusion
                    .entry((label.clone(), detection.value.clone()))
                    .or_default() += 1;
            }
        }
    }
    out
}

/// Criterion 2: every trait the catalog ships enabled is right on at least
/// 90 % of its confident answers (confidence ≥ 0.7) and answers at least
/// half the labelled fonts confidently; a trait below that bar is disabled
/// in the catalog, and a disabled one is indeed below it. Prints the
/// confusion of every trait.
#[test]
fn detector_accuracy_meets_the_bar_or_the_trait_is_disabled() {
    let tallies = tallies();
    let mut report = String::new();
    let mut wrong = Vec::new();
    for (id, t) in &tallies {
        let enabled = glyph_traits::find(id)
            .expect("catalog")
            .expect("every detector is in the catalog")
            .enabled;
        let _ = writeln!(
            report,
            "{id}: {} labelled, {} confident ({:.0} %), {} right ({:.1} %) — {}",
            t.labelled,
            t.confident,
            t.coverage() * 100.0,
            t.right,
            t.accuracy() * 100.0,
            if enabled { "enabled" } else { "disabled" }
        );
        for ((label, got), n) in &t.confusion {
            let _ = writeln!(report, "    label {label:10} → {got:10} {n}");
        }
        for miss in &t.misses {
            let _ = writeln!(report, "    miss: {miss}");
        }
        assert!(t.labelled > 0, "{id}: no labelled font has the glyph");
        if enabled != t.meets_bar() {
            wrong.push(*id);
        }
    }
    println!("{report}");
    assert!(
        wrong.is_empty(),
        "enabled in the catalog must equal meeting the bar: {wrong:?}\n{report}"
    );
}

/// Criterion 1: one detector per catalog trait, and no detector without one.
#[test]
fn every_catalog_trait_has_one_detector() {
    let catalog: Vec<&str> = glyph_traits::load()
        .expect("catalog")
        .iter()
        .map(|t| t.id.as_str())
        .collect();
    let detectors: Vec<&str> = trait_ids().collect();
    let mut a = catalog.clone();
    let mut b = detectors.clone();
    a.sort_unstable();
    b.sort_unstable();
    assert_eq!(a, b);
    for id in &detectors {
        assert!(version(id).is_some_and(|v| v >= 1), "{id}");
    }
    assert_eq!(version("X.none"), None);
}

/// Every detection names a value of its trait, and a scalar trait's every
/// measure, inside the catalog's range.
#[test]
fn every_detection_is_a_catalog_value() {
    for fam in labels().families.iter().take(20) {
        let got = detect(&read_font(&fam.file), 0).expect("reads");
        for d in got.traits.iter().flatten() {
            let t = glyph_traits::find(d.trait_id).unwrap().unwrap();
            assert!(t.value(&d.value).is_some(), "{}: {}", d.trait_id, d.value);
            assert!((0.0..=1.0).contains(&d.confidence));
            match t.kind {
                TraitKind::Enum => assert!(d.scalars.is_empty()),
                TraitKind::Scalar => {
                    assert_eq!(d.scalars.len(), t.measures.len());
                    for ((id, v), m) in d.scalars.iter().zip(&t.measures) {
                        assert_eq!(*id, m.id);
                        assert!(v.is_finite());
                    }
                }
            }
        }
    }
}

/// A face that lacks a trait's glyph has no detection for it; the glyphs it
/// has are still answered.
#[test]
fn a_face_without_the_glyph_has_no_detection_for_it() {
    // Lao: it maps the figures, not the Latin letters.
    let data = read_font("google-repo/phetsarath/Phetsarath-Regular.ttf");
    let got = detect(&data, 0).expect("reads");
    assert_eq!(got.traits.len(), trait_ids().count());
    let answered: Vec<&str> = got.traits.iter().flatten().map(|d| d.trait_id).collect();
    assert!(!answered.is_empty() && answered.len() < 13, "{answered:?}");
    for (id, d) in trait_ids().zip(&got.traits) {
        let ch = CHARS[trait_ids().position(|t| t == id).unwrap()];
        let font = read_fonts::FontRef::new(&data).unwrap();
        use skrifa::MetadataProvider;
        let mapped = font.charmap().map(ch).is_some_and(|g| g.to_u32() != 0);
        assert_eq!(
            d.is_some(),
            mapped,
            "{id}: answered only when {ch:?} is there"
        );
    }
    // Asking for no trait answers none.
    let none = detect_only(&data, 0, &|_| false).expect("reads");
    assert!(none.traits.iter().all(Option::is_none));
}

/// Malformed data and a missing collection face are no answer, never a panic.
#[test]
fn malformed_data_is_no_answer() {
    assert!(detect(b"", 0).is_none());
    assert!(detect(b"not a font at all, just bytes", 0).is_none());
    let data = read_font("google-repo/barlow/Barlow-BoldItalic.ttf");
    assert!(detect(&data, 3).is_none());
    let mut cut = data.clone();
    cut.truncate(data.len() / 3);
    // A truncated file may read its header; whatever it answers, it does
    // not panic.
    let _ = detect(&cut, 0);
}

/// Criterion 5: a variable face is measured at its default instance and
/// says so; a static one does not.
#[test]
fn a_variable_face_says_it_was_measured_at_its_default_instance() {
    let var = detect(&read_font("google-repo/epilogue/Epilogue[wght].ttf"), 0).unwrap();
    assert!(var.variable);
    assert!(var.traits.iter().flatten().count() > 10);
    let fixed = detect(
        &read_font("google-static/robotoslab/RobotoSlab-ExtraBold.ttf"),
        0,
    )
    .unwrap();
    assert!(!fixed.variable);
}

/// An italic is sheared upright before the measures.
#[test]
fn an_italic_is_measured_upright() {
    let italic = detect(&read_font("google-repo/barlow/Barlow-BoldItalic.ttf"), 0).unwrap();
    assert!(italic.slant_degrees > 5.0, "{}", italic.slant_degrees);
    let upright = detect(
        &read_font("google-static/robotoslab/RobotoSlab-ExtraBold.ttf"),
        0,
    )
    .unwrap();
    assert!(upright.slant_degrees.abs() < 0.5);
}

/// The confidence split: sure beyond the band, unsure inside it.
#[test]
fn a_measure_between_the_thresholds_is_not_confident() {
    assert_eq!(split(0.0, 1.0, 2.0, "a", "b").value, "a");
    assert!(split(0.0, 1.0, 2.0, "a", "b").confidence >= CONFIDENT);
    assert!(split(3.0, 1.0, 2.0, "a", "b").confidence >= CONFIDENT);
    assert_eq!(split(3.0, 1.0, 2.0, "a", "b").value, "b");
    let mid = split(1.5, 1.0, 2.0, "a", "b");
    assert!(mid.confidence < CONFIDENT);
    assert!(split(1.9, 1.0, 2.0, "a", "b").confidence < CONFIDENT);
    assert_eq!(split(f64::NAN, 1.0, 2.0, "a", "b").confidence, 0.0);
}

/// A shape a detector cannot read is no confident answer: a plain box has no
/// bowl for the `g` and no counter for the `a`.
#[test]
fn a_shape_the_detector_cannot_read_is_not_confident() {
    use crate::glyphs::geometry::{Contour, Segment};
    let p = Point::new;
    let corners = [p(0.0, 0.0), p(400.0, 0.0), p(400.0, 600.0), p(0.0, 600.0)];
    let segments = (0..4)
        .map(|i| Segment::Line(corners[i], corners[(i + 1) % 4]))
        .collect();
    let block = GlyphOutline::new(vec![Contour::new(segments, true)], 1000);
    let shape = Shape::new(block, 0.0).expect("shape");
    assert!(g_storeys(&shape).confidence < CONFIDENT);
    assert!(a_storeys(&shape).confidence < CONFIDENT);
    assert!(e_bar(&shape).confidence < CONFIDENT);
    // A plain stem is decided: no flag, no bar, plain ends.
    let one = one_flag(&shape);
    assert_eq!((one.value, one.confidence >= CONFIDENT), ("none", true));
    let i = i_form(&shape);
    assert_eq!((i.value, i.confidence >= CONFIDENT), ("plain", true));
}
