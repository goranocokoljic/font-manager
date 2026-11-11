//! Glyph-trait search (E49.5). The detections are written by hand, not by
//! the detectors, so each test controls exactly which face has which value
//! and how sure: a face that must not match is built to clearly fail.

use std::fs;
use std::path::{Path, PathBuf};

use fm_api::library::{FamilyOrder, FamilySortKey, SourceKind};
use fm_api::search::{Filters, SearchRequest};
use fm_api::{ErrorCode, FaceId};

use super::*;
use crate::glyphs::traits::{Detection, versions};
use crate::index::INDEX_FILE;
use crate::index::glyph_traits::MeasuredFace;
use crate::scanner::{FileIndex, discover};
use crate::test_support::TempDir;

/// Four families, one face each; Epilogue is the variable one.
const FONTS: [(&str, &str); 4] = [
    ("google-static/robotoslab/RobotoSlab-ExtraBold.ttf", "a.ttf"),
    ("google-repo/epilogue/Epilogue[wght].ttf", "b.ttf"),
    ("google-static/karla/Karla-SemiBold.ttf", "c.ttf"),
    (
        "google-static/asap/Asap_Condensed-ExtraLightItalic.ttf",
        "d.ttf",
    ),
];

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts")
}

/// An index holding the scan of [`FONTS`].
fn scanned(dir: &TempDir) -> Index {
    let fonts = dir.path().join("fonts");
    fs::create_dir_all(&fonts).expect("folder");
    for (from, to) in FONTS {
        fs::copy(fixtures().join(from), fonts.join(to)).expect("copy fixture");
    }
    let mut files = FileIndex::new();
    files.rescan(&fonts, &discover(&fonts).expect("discover"));
    let mut index = Index::open(&dir.path().join("data").join(INDEX_FILE))
        .expect("open")
        .index;
    let source = index
        .add_source(&fonts.to_string_lossy(), SourceKind::Watched)
        .expect("source")
        .id;
    index.persist_files(source, files.files()).expect("persist");
    index
}

/// One hand-made detection.
fn d(trait_id: &str, value: &str, confidence: f64, scalars: &[(&str, f64)]) -> (String, Detection) {
    let id = glyph_traits::find(trait_id)
        .expect("catalog")
        .expect("a catalog trait")
        .id
        .as_str();
    (
        trait_id.to_owned(),
        Detection {
            trait_id: id,
            value: value.to_owned(),
            scalars: scalars.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect(),
            confidence,
        },
    )
}

/// The face of file `name` (`a.ttf` …).
fn face(index: &Index, name: &str) -> FaceId {
    index
        .trait_debug_face(name)
        .unwrap_or_else(|| panic!("no face for {name}"))
}

/// The library of the tests, measured by hand:
///
/// | face | R.leg | O.roundness | G.spur |
/// |------|-------|-------------|--------|
/// | a    | straight, sure | round (1.0, corners 0.01), sure | spur |
/// | b    | straight, unsure (0.5) | oval (0.85), sure | none |
/// | c    | — (no glyph) | round (0.97, corners 0.04), sure | — (no glyph) |
/// | d    | curved, sure | — (no glyph) | none |
///
/// Every trait the detectors have is stamped, so the pass has nothing left.
fn library(dir: &TempDir) -> Index {
    let mut index = scanned(dir);
    let all: Vec<(&str, u32)> = versions().collect();
    let todo = index.trait_pass_todo(&all, 0, 100).expect("todo");
    assert_eq!(todo.len(), 4, "four faces to measure");
    let wanted = |name: &str| -> Vec<(String, Detection)> {
        match name {
            "a.ttf" => vec![
                d("R.leg", "straight", 0.9, &[]),
                d(
                    "O.roundness",
                    "round",
                    0.9,
                    &[("roundness", 1.0), ("corners", 0.01)],
                ),
                d("G.spur", "spur", 0.9, &[]),
            ],
            "b.ttf" => vec![
                d("R.leg", "straight", 0.5, &[]),
                d(
                    "O.roundness",
                    "oval",
                    0.9,
                    &[("roundness", 0.85), ("corners", 0.0)],
                ),
                d("G.spur", "none", 0.9, &[]),
            ],
            "c.ttf" => vec![d(
                "O.roundness",
                "round",
                0.95,
                &[("roundness", 0.97), ("corners", 0.04)],
            )],
            "d.ttf" => vec![
                d("R.leg", "curved", 0.95, &[]),
                d("G.spur", "none", 0.8, &[]),
            ],
            other => panic!("unexpected {other}"),
        }
    };
    let names = ["a.ttf", "b.ttf", "c.ttf", "d.ttf"];
    let measured: Vec<MeasuredFace> = todo
        .iter()
        .map(|item| {
            let name = names
                .iter()
                .find(|n| item.path.ends_with(*n))
                .expect("a known file");
            let mut found = wanted(name);
            MeasuredFace {
                face: item.face,
                file_hash: item.file_hash.clone(),
                variable: *name == "b.ttf",
                traits: all
                    .iter()
                    .map(|(id, version)| {
                        let detection = found
                            .iter()
                            .position(|(t, _)| t == id)
                            .map(|at| found.remove(at).1);
                        ((*id).to_owned(), *version, detection)
                    })
                    .collect(),
                descriptors: None,
            }
        })
        .collect();
    assert_eq!(index.write_glyph_traits(&measured).expect("write"), 4);
    index
}

fn search(index: &Index, filters: Vec<TraitFilter>) -> FmResult<Vec<FaceId>> {
    search_with(index, filters, Filters::default(), None)
}

fn search_with(
    index: &Index,
    filters: Vec<TraitFilter>,
    other: Filters,
    order: Option<FamilyOrder>,
) -> FmResult<Vec<FaceId>> {
    let found = index.search(&SearchRequest {
        filters: other,
        order,
        glyph_traits: Some(filters),
        ..SearchRequest::default()
    })?;
    Ok(found
        .families
        .iter()
        .flat_map(|f| f.faces.clone())
        .collect())
}

fn value(trait_id: &str, value: &str) -> TraitFilter {
    TraitFilter::Value {
        trait_id: trait_id.into(),
        value: value.into(),
    }
}

fn range(trait_id: &str, measure: Option<&str>, min: Option<f64>, max: Option<f64>) -> TraitFilter {
    TraitFilter::Range {
        trait_id: trait_id.into(),
        measure: measure.map(Into::into),
        min,
        max,
    }
}

fn sorted(mut faces: Vec<FaceId>) -> Vec<FaceId> {
    faces.sort_by_key(|f| f.0);
    faces
}

/// Criterion 2: a value filter finds the faces detected with that value,
/// with confidence: not the unsure one (b), not the one that lacks the
/// glyph (c), not the other value (d).
#[test]
fn a_value_filter_finds_the_faces_detected_with_it() {
    let dir = TempDir::new("trait-search-1");
    let index = library(&dir);
    let a = face(&index, "a.ttf");
    assert_eq!(
        search(&index, vec![value("R.leg", "straight")]).expect("search"),
        vec![a]
    );
    assert_eq!(
        search(&index, vec![value("R.leg", "curved")]).expect("search"),
        vec![face(&index, "d.ttf")]
    );
}

/// Criterion 2: a face without the glyph is never a "none". G.spur has a
/// real value `none`; c lacks the G and is not among its results.
#[test]
fn a_face_without_the_glyph_is_not_counted_as_none() {
    let dir = TempDir::new("trait-search-2");
    let index = library(&dir);
    let found = sorted(search(&index, vec![value("G.spur", "none")]).expect("search"));
    assert_eq!(
        found,
        sorted(vec![face(&index, "b.ttf"), face(&index, "d.ttf")])
    );
    assert!(!found.contains(&face(&index, "c.ttf")));
}

/// Criterion 2/3: a range reads the measure, `>=` and `<=` inclusive; the
/// first measure by default, a named one when given; a face without the
/// glyph (d) is out.
#[test]
fn a_range_filter_reads_the_measured_number() {
    let dir = TempDir::new("trait-search-3");
    let index = library(&dir);
    let (a, b, c) = (
        face(&index, "a.ttf"),
        face(&index, "b.ttf"),
        face(&index, "c.ttf"),
    );
    assert_eq!(
        sorted(search(&index, vec![range("O.roundness", None, Some(0.95), None)]).expect("search")),
        sorted(vec![a, c])
    );
    // Inclusive at the bound: c measures 0.97.
    assert_eq!(
        sorted(search(&index, vec![range("O.roundness", None, Some(0.97), None)]).expect("search")),
        sorted(vec![a, c])
    );
    assert_eq!(
        search(&index, vec![range("O.roundness", None, None, Some(0.9))]).expect("search"),
        vec![b]
    );
    assert_eq!(
        search(
            &index,
            vec![range("O.roundness", Some("corners"), Some(0.028), None)]
        )
        .expect("search"),
        vec![c]
    );
    assert_eq!(
        sorted(
            search(
                &index,
                vec![range("O.roundness", None, Some(0.8), Some(0.99))]
            )
            .expect("search")
        ),
        sorted(vec![b, c])
    );
}

/// Criterion 2/3: trait filters combine with AND
/// (`--trait R.leg=straight --trait O.roundness>=0.95`).
#[test]
fn trait_filters_combine_with_each_other() {
    let dir = TempDir::new("trait-search-4");
    let index = library(&dir);
    let both = vec![
        value("R.leg", "straight"),
        range("O.roundness", None, Some(0.95), None),
    ];
    assert_eq!(
        search(&index, both).expect("search"),
        vec![face(&index, "a.ttf")]
    );
    // c is round but has no R: out.
    let none = vec![
        value("R.leg", "curved"),
        range("O.roundness", None, Some(0.95), None),
    ];
    assert!(search(&index, none).expect("search").is_empty());
}

/// Criterion 2: trait filters combine with the other filters and with an
/// order, and a face they leave out counts under no filter value.
#[test]
fn trait_filters_combine_with_filters_order_and_counts() {
    let dir = TempDir::new("trait-search-5");
    let index = library(&dir);
    let (a, c) = (face(&index, "a.ttf"), face(&index, "c.ttf"));
    let round = || vec![range("O.roundness", None, Some(0.95), None)];
    let statics = Filters {
        variable: Some(false),
        ..Filters::default()
    };
    assert_eq!(
        sorted(search_with(&index, round(), statics.clone(), None).expect("search")),
        sorted(vec![a, c])
    );
    // The filter removes a face the trait lets through: G.spur=none finds the
    // variable b and the static d; static only, just d.
    let (b, d) = (face(&index, "b.ttf"), face(&index, "d.ttf"));
    let none = || vec![value("G.spur", "none")];
    assert_eq!(
        sorted(search_with(&index, none(), Filters::default(), None).expect("search")),
        sorted(vec![b, d])
    );
    assert_eq!(
        search_with(&index, none(), statics.clone(), None).expect("search"),
        vec![d]
    );
    let variable = Filters {
        variable: Some(true),
        ..Filters::default()
    };
    assert_eq!(
        search_with(&index, none(), variable, None).expect("search"),
        vec![b]
    );
    // And a text query: "karla" keeps c of the round faces.
    let karla = index
        .search(&SearchRequest {
            query: "karla".into(),
            glyph_traits: Some(round()),
            ..SearchRequest::default()
        })
        .expect("search");
    let found: Vec<FaceId> = karla
        .families
        .iter()
        .flat_map(|f| f.faces.clone())
        .collect();
    assert_eq!(found, vec![c]);
    let by_name = |descending| FamilyOrder {
        by: FamilySortKey::Name,
        descending,
    };
    let up = search_with(&index, round(), Filters::default(), Some(by_name(false))).expect("up");
    let down = search_with(&index, round(), Filters::default(), Some(by_name(true))).expect("down");
    assert_eq!(up.len(), 2);
    assert_eq!(up, down.into_iter().rev().collect::<Vec<_>>());
    // Karla (c) before Roboto Slab (a), A to Z.
    assert_eq!(up, vec![c, a]);

    // The counts see only a and c: two static faces, no variable one.
    let found = index
        .search(&SearchRequest {
            glyph_traits: Some(round()),
            ..SearchRequest::default()
        })
        .expect("search");
    let counts = found.counts.expect("counts");
    let variable: Vec<(bool, u32)> = counts.variable.iter().map(|c| (c.value, c.count)).collect();
    assert_eq!(variable, vec![(false, 2)]);
    assert_eq!(found.total, 2);
}

/// No trait filter, or an empty list, filters nothing.
#[test]
fn no_trait_filter_keeps_every_face() {
    let dir = TempDir::new("trait-search-6");
    let index = library(&dir);
    assert_eq!(search(&index, Vec::new()).expect("search").len(), 4);
    let all = index.search(&SearchRequest::default()).expect("search");
    assert_eq!(all.total, 4);
}

/// Criterion 1: what a filter names is checked against the catalog.
#[test]
fn a_filter_the_catalog_does_not_have_is_invalid() {
    let dir = TempDir::new("trait-search-7");
    let index = library(&dir);
    let invalid = |filter: TraitFilter| {
        let err = search(&index, vec![filter]).expect_err("invalid");
        assert_eq!(err.code(), ErrorCode::InvalidRequest, "{err:?}");
        err.to_string()
    };
    assert!(invalid(value("Z.tail", "long")).contains("There is no glyph trait Z.tail"));
    assert!(invalid(value("R.leg", "wavy")).contains("straight, curved, both"));
    assert!(invalid(range("R.leg", None, Some(0.5), None)).contains("measures no number"));
    assert!(
        invalid(range("O.roundness", Some("depth"), Some(0.5), None))
            .contains("roundness, corners")
    );
    assert!(invalid(range("O.roundness", None, None, None)).contains("needs a lower or an upper"));
    assert!(invalid(range("O.roundness", None, Some(1.0), Some(0.5))).contains("empty"));
    assert!(invalid(range("O.roundness", None, Some(f64::NAN), None)).contains("not a number"));
}

/// Criterion 1: the catalog in catalog order with per-value counts: sure
/// detections only, the unsure counted apart, the faces without the glyph
/// nowhere.
#[test]
fn the_catalog_lists_every_trait_with_its_counts() {
    let dir = TempDir::new("trait-search-8");
    let index = library(&dir);
    let listed = index.list_glyph_traits().expect("list");
    let catalog = glyph_traits::load().expect("catalog");
    assert_eq!(
        listed.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
        catalog.iter().map(|t| t.id.as_str()).collect::<Vec<_>>()
    );
    let get = |id: &str| listed.iter().find(|t| t.id == id).expect("listed");
    let count = |t: &GlyphTraitInfo, v: &str| {
        let v = t.values.iter().find(|x| x.id == v).expect("value");
        (v.faces, v.families)
    };

    let leg = get("R.leg");
    assert_eq!(leg.kind, GlyphTraitKind::Enum);
    assert!(!leg.enabled, "R.leg ships disabled (E49.4)");
    assert_eq!((leg.measured, leg.unsure), (3, 1));
    assert_eq!(count(leg, "straight"), (1, 1));
    assert_eq!(count(leg, "curved"), (1, 1));
    assert_eq!(count(leg, "both"), (0, 0));
    assert_eq!(leg.name, "R leg");
    assert_eq!(leg.glyph, "R");

    let o = get("O.roundness");
    assert_eq!(o.kind, GlyphTraitKind::Scalar);
    assert_eq!((o.measured, o.unsure), (3, 0));
    assert_eq!(count(o, "round"), (2, 2));
    assert_eq!(count(o, "oval"), (1, 1));
    assert_eq!(
        o.measures.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        vec!["roundness", "corners"]
    );
    let round = o.values.iter().find(|v| v.id == "round").expect("round");
    assert_eq!(round.when.len(), 1);
    assert_eq!(round.when[0].measure, "roundness");
    assert_eq!(round.when[0].min, Some(0.95));

    // c lacks the G: two faces under `none`, not three.
    let spur = get("G.spur");
    assert_eq!(spur.measured, 3);
    assert_eq!(count(spur, "none"), (2, 2));
    // A trait no face has: measured nowhere, every count 0.
    let e = get("e.bar");
    assert_eq!(e.measured, 0);
    assert!(e.values.iter().all(|v| v.faces == 0 && v.families == 0));
}

/// The value counts are the faces a value filter finds.
#[test]
fn a_value_count_is_what_its_filter_finds() {
    let dir = TempDir::new("trait-search-9");
    let index = library(&dir);
    for t in index.list_glyph_traits().expect("list") {
        for v in &t.values {
            let found = search(&index, vec![value(&t.id, &v.id)]).expect("search");
            assert_eq!(found.len() as u32, v.faces, "{}={}", t.id, v.id);
        }
    }
}

/// The detections are kept between searches and read again after the
/// database changes: a detection made sure later is found, and measures
/// that are not a JSON object of numbers read as not measured.
#[test]
fn a_changed_detection_is_found_by_the_next_search() {
    let dir = TempDir::new("trait-search-cache");
    let index = library(&dir);
    let (a, b, c) = (
        face(&index, "a.ttf"),
        face(&index, "b.ttf"),
        face(&index, "c.ttf"),
    );
    let straight = || vec![value("R.leg", "straight")];
    let round = || vec![range("O.roundness", None, Some(0.95), None)];
    assert_eq!(search(&index, straight()).expect("search"), vec![a]);
    assert_eq!(
        sorted(search(&index, round()).expect("search")),
        sorted(vec![a, c])
    );

    index.trait_debug_sql(&format!(
        "UPDATE glyph_traits SET confidence = 0.9 WHERE face_id = {} AND trait = 'R.leg';
         UPDATE glyph_traits SET scalars = 'not json' WHERE face_id = {} AND trait = 'O.roundness';",
        b.0, c.0
    ));
    assert_eq!(
        sorted(search(&index, straight()).expect("search")),
        sorted(vec![a, b])
    );
    assert_eq!(search(&index, round()).expect("search"), vec![a]);
    let leg = index
        .list_glyph_traits()
        .expect("list")
        .into_iter()
        .find(|t| t.id == "R.leg")
        .expect("R.leg");
    assert_eq!((leg.measured, leg.unsure), (3, 0));
}

/// The family page's trait tags (E49.10): a face's sure detections of the
/// traits the catalog offers, in catalog order, on its detail too.
#[test]
fn a_face_detail_lists_its_sure_offered_traits_in_catalog_order() {
    let dir = TempDir::new("trait-face-detail");
    let index = library(&dir);
    let tag = |t: &str, v: &str| (t.to_owned(), v.to_owned());
    let tags = |name: &str| -> Vec<(String, String)> {
        index
            .face_detail(face(&index, name))
            .expect("detail")
            .glyph_traits
            .into_iter()
            .map(|d| {
                assert!(f64::from(d.confidence) >= CONFIDENT, "{d:?}");
                (d.trait_id, d.value)
            })
            .collect()
    };
    // a: R.leg is sure but disabled in the catalog, so only O and G show,
    // O first as in the catalog.
    assert_eq!(
        tags("a.ttf"),
        vec![tag("O.roundness", "round"), tag("G.spur", "spur")]
    );
    // b: the unsure R (0.5) is left out.
    assert_eq!(
        tags("b.ttf"),
        vec![tag("O.roundness", "oval"), tag("G.spur", "none")]
    );
    assert_eq!(tags("c.ttf"), vec![tag("O.roundness", "round")]);
}

/// A face not measured yet has no trait tags.
#[test]
fn a_face_not_measured_has_no_trait_tags() {
    let dir = TempDir::new("trait-face-none");
    let index = scanned(&dir);
    let detail = index.face_detail(face(&index, "a.ttf")).expect("detail");
    assert!(detail.glyph_traits.is_empty());
}

/// The bar, the catalog's values and its order, on hand-made rows: 0.7 is
/// shown, just under it is not; a value or a trait the catalog does not list
/// is left out; the order is the catalog's, not the rows'.
#[test]
fn confident_traits_keeps_the_bar_the_values_and_the_catalog_order() {
    use crate::index::glyph_traits::{StoredTrait, confident_traits};
    let catalog = glyph_traits::load().expect("catalog");
    let row = |t: &str, v: &str, confidence: f64| StoredTrait {
        trait_id: t.into(),
        value: v.into(),
        scalar: None,
        scalars: None,
        confidence,
        detector_version: 1,
        instance: "static".into(),
    };
    let stored = [
        row("G.spur", "spur", CONFIDENT),
        row("g.storeys", "double", CONFIDENT - 0.01),
        row("a.storeys", "triple", 0.99),
        row("x.unknown", "any", 0.99),
        row("O.roundness", "round", 1.0),
    ];
    let shown: Vec<(String, String)> = confident_traits(catalog, &stored)
        .into_iter()
        .map(|d| (d.trait_id, d.value))
        .collect();
    assert_eq!(
        shown,
        vec![
            ("O.roundness".to_owned(), "round".to_owned()),
            ("G.spur".to_owned(), "spur".to_owned()),
        ]
    );
}
