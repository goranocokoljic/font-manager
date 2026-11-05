use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use fm_api::common::{Classification, Health, Metrics};
use fm_api::library::SourceKind;
use fm_api::search::{PairingRequest, PairingRole, PairingSuggestion};
use fm_api::{ErrorCode, FaceId};

use crate::fingerprint::{FaceFingerprint, Fingerprint};
use crate::grouping::family_key;
use crate::index::Index;
use crate::pairing::model::{self, Curated, MODEL_REASON};
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, IndexedFile, discover, hash_and_parse};

/// The search budget of the spec (criterion 5).
const BUDGET: Duration = Duration::from_millis(50);

/// The fixture corpus, parsed once for every test of this file.
fn corpus() -> &'static [IndexedFile] {
    static FILES: OnceLock<Vec<IndexedFile>> = OnceLock::new();
    FILES.get_or_init(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts");
        discover(&root)
            .expect("discover")
            .files
            .iter()
            .map(|f| {
                let (hash, parsed) = hash_and_parse(&f.path).expect("read fixture");
                IndexedFile {
                    path: f.path.clone(),
                    size: f.size,
                    modified: f.modified,
                    hash,
                    parsed,
                    gone: false,
                }
            })
            .collect()
    })
}

fn indexed(files: &[IndexedFile]) -> Index {
    let mut index = Index::open_in_memory().expect("open");
    let source = index
        .add_source("/fonts", SourceKind::Watched)
        .expect("source")
        .id;
    index.persist_files(source, files).expect("persist");
    index
}

/// The first face (by id) whose family name is `family`.
fn face_of(index: &Index, family: &str) -> FaceId {
    index
        .conn
        .query_row(
            "SELECT min(id) FROM face WHERE family = ?1",
            [family],
            |r| r.get::<_, Option<u32>>(0),
        )
        .expect("query")
        .map(FaceId)
        .unwrap_or_else(|| panic!("no face of {family}"))
}

fn class_of(index: &Index, face: FaceId) -> Option<Classification> {
    let class: Option<String> = index
        .conn
        .query_row(
            "SELECT json_extract(parsed, '$.classification.class') FROM face WHERE id = ?1",
            [face.0],
            |r| r.get(0),
        )
        .expect("class");
    class.and_then(|c| serde_json::from_value(serde_json::Value::String(c)).ok())
}

fn pairings(index: &Index, face: FaceId, limit: u32) -> Vec<PairingSuggestion> {
    index
        .suggest_pairings(&PairingRequest {
            face,
            limit,
            role: None,
            partner_of: None,
        })
        .expect("pairings")
}

/// The x-height of `face` as a share of the em, as the search cache reads it.
fn x_height_em(face: &ParsedFace) -> Option<f32> {
    let m = face.metrics;
    m.x_height
        .filter(|_| m.units_per_em > 0)
        .map(|x| f32::from(x) / f32::from(m.units_per_em))
}

#[test]
fn the_fixture_faces_carry_an_x_height() {
    let faces: Vec<&ParsedFace> = corpus().iter().flat_map(|f| &f.parsed.faces).collect();
    let known = faces.iter().filter(|f| x_height_em(f).is_some()).count();
    assert!(known * 10 >= faces.len() * 9, "{known} of {}", faces.len());
    assert!(
        faces
            .iter()
            .filter_map(|f| x_height_em(f))
            .all(|x| x > 0.0 && x < 1.5)
    );
}

#[test]
fn libre_baskerville_gets_a_sans_first() {
    let index = indexed(corpus());
    let face = face_of(&index, "Libre Baskerville");
    assert_eq!(class_of(&index, face), Some(Classification::Serif));
    let got = pairings(&index, face, 10);
    let first = got.first().expect("a suggestion");
    assert_eq!(
        class_of(&index, first.face.id),
        Some(Classification::Sans),
        "{got:#?}"
    );
    assert_eq!(first.reasons[0], "contrast: serif with sans serif");
    // Best first, one face per family, never its own family, a reason each.
    assert!(got.windows(2).all(|w| w[0].score >= w[1].score));
    let mut families: Vec<_> = got.iter().map(|s| s.face.family).collect();
    families.sort_unstable();
    families.dedup();
    assert_eq!(families.len(), got.len());
    assert!(
        got.iter()
            .all(|s| s.face.family_name != "Libre Baskerville")
    );
    assert!(got.iter().all(|s| !s.reasons.is_empty()));
    assert!(got.iter().all(|s| (0.0..=1.0).contains(&s.score)));
}

#[test]
fn clicker_script_gets_no_script() {
    let index = indexed(corpus());
    let face = face_of(&index, "Clicker Script");
    assert_eq!(class_of(&index, face), Some(Classification::Script));
    let got = pairings(&index, face, 1000);
    assert!(!got.is_empty());
    for s in &got {
        let class = class_of(&index, s.face.id);
        assert!(
            matches!(class, Some(Classification::Sans | Classification::Serif)),
            "{} is {class:?}",
            s.face.family_name
        );
    }
}

#[test]
fn a_noto_sans_face_gets_a_noto_serif_face_as_same_superfamily() {
    let index = indexed(corpus());
    let face = face_of(&index, "Noto Sans Armenian");
    let got = pairings(&index, face, 5);
    let serif = got
        .iter()
        .find(|s| s.face.family_name.starts_with("Noto Serif"))
        .unwrap_or_else(|| panic!("no Noto Serif in {got:#?}"));
    assert!(
        serif.reasons.iter().any(|r| r == "same superfamily"),
        "{serif:?}"
    );
}

#[test]
fn an_unknown_or_hidden_face_is_not_found_and_limit_caps() {
    let mut index = indexed(corpus());
    let err = index
        .suggest_pairings(&PairingRequest {
            face: FaceId(99_999),
            limit: 5,
            role: None,
            partner_of: None,
        })
        .expect_err("unknown");
    assert_eq!(err.code(), ErrorCode::NotFound);
    let face = face_of(&index, "Libre Baskerville");
    assert_eq!(pairings(&index, face, 3).len(), 3);
    assert!(pairings(&index, face, 0).is_empty());
    // A paused source hides its faces from pairing, as from search.
    index
        .conn
        .execute("UPDATE source SET paused = 1", [])
        .expect("pause");
    let err = index
        .suggest_pairings(&PairingRequest {
            face,
            limit: 5,
            role: None,
            partner_of: None,
        })
        .expect_err("hidden");
    assert_eq!(err.code(), ErrorCode::NotFound);
    index = indexed(&[]);
    assert!(
        index
            .suggest_pairings(&PairingRequest {
                face,
                limit: 5,
                role: None,
                partner_of: None
            })
            .is_err()
    );
}

/// A sans face of `family` with `designer` and an x-height of `x_height`
/// thousandths of the em, in a file of its own: no contrast or superfamily
/// with another one, so only the designer and x-height rules can hold.
fn plain_sans(i: u8, family: &str, designer: &str, x_height: i16) -> IndexedFile {
    let mut face = ParsedFace::empty(0, Health::Ok);
    face.family = Some(family.into());
    face.subfamily = Some("Regular".into());
    face.weight = Some(400);
    face.designer = Some(designer.into());
    face.metrics = Metrics {
        units_per_em: 2000,
        ascender: 1600,
        descender: -400,
        x_height: Some(x_height * 2),
        cap_height: Some(1400),
    };
    face.classification = Some(crate::classify::ClassGuess {
        class: Classification::Sans,
        confidence: 0.8,
        source: crate::classify::ClassSource::Declared,
    });
    IndexedFile {
        path: PathBuf::from(format!("/fonts/plain-{i}.ttf")),
        size: 1000,
        modified: None,
        hash: FileHash::from([i; 32]),
        parsed: ParsedFile {
            format: Some(fm_api::common::FontFormat::Ttf),
            health: Health::Ok,
            faces: vec![face],
        },
        gone: false,
    }
}

#[test]
fn the_index_feeds_designer_and_x_height_to_the_rules() {
    let index = indexed(&[
        plain_sans(1, "Alder", "Ada Lovelace", 500),
        plain_sans(2, "Birch", "ada lovelace", 510),
        plain_sans(3, "Cedar", "Bob", 505),
        plain_sans(4, "Damson", "Ada Lovelace", 700),
        plain_sans(5, "Elm", "Bob", 700),
    ]);
    // The rules alone: the model reorders and adds a reason of its own.
    let req = PairingRequest {
        face: face_of(&index, "Alder"),
        limit: 10,
        role: None,
        partner_of: None,
    };
    let got = index.suggest_with(&req, None).expect("pairings");
    let reasons: Vec<(&str, Vec<&str>)> = got
        .iter()
        .map(|s| {
            let r = s.reasons.iter().map(String::as_str).collect();
            (s.face.family_name.as_str(), r)
        })
        .collect();
    assert_eq!(
        reasons,
        [
            ("Birch", vec!["same designer", "similar x-height"]),
            ("Damson", vec!["same designer"]),
            ("Cedar", vec!["similar x-height"]),
        ],
        "{got:#?}"
    );
}

/// [`plain_sans`] by one designer, so the rules let every pair through, with
/// a stroke contrast (a fingerprint), a width class and a copyright. The
/// x-heights are far apart: no x-height reason gets in the way.
fn role_face(i: u8, family: &str, contrast: f32, width: u16, copyright: &str) -> IndexedFile {
    let mut file = plain_sans(i, family, "Ada Lovelace", 300 + i16::from(i) * 100);
    let face = &mut file.parsed.faces[0];
    face.width = Some(width);
    face.copyright = Some(copyright.into());
    face.fingerprint = FaceFingerprint::Taken(Fingerprint {
        vector: vec![0.5; 16],
        x_height_ratio: 0.7,
        contrast,
        stroke_width: 0.1,
        width_class: f32::from(width),
        advance_ratio: 1.0,
    });
    file
}

fn role_library() -> Index {
    indexed(&[
        role_face(1, "Alder", 0.5, 5, "Copyright 2011 Someone"),
        // Low contrast, a far year.
        role_face(2, "Birch", 0.1, 5, "Copyright 1950 Someone"),
        // High contrast, narrow, the same era.
        role_face(3, "Cedar", 0.9, 3, "(c) 2013-2020 Someone"),
    ])
}

/// Family name and reasons, as the rules alone answer (the model adds a
/// reason of its own).
fn ask_role(
    index: &Index,
    face: FaceId,
    role: Option<PairingRole>,
    partner_of: Option<FaceId>,
) -> Vec<(String, Vec<String>)> {
    index
        .suggest_with(
            &PairingRequest {
                face,
                limit: 10,
                role,
                partner_of,
            },
            None,
        )
        .expect("pairings")
        .into_iter()
        .map(|s| (s.face.family_name, s.reasons))
        .collect()
}

fn owned(family: &str, reasons: &[&str]) -> (String, Vec<String>) {
    (
        family.to_owned(),
        reasons.iter().map(|r| (*r).to_owned()).collect(),
    )
}

#[test]
fn the_index_feeds_contrast_width_and_era_to_the_role() {
    let index = role_library();
    let alder = face_of(&index, "Alder");
    assert_eq!(
        ask_role(&index, alder, Some(PairingRole::Body), None),
        [
            owned("Birch", &["lower contrast for body", "same designer"]),
            owned("Cedar", &["same era", "same designer"]),
        ]
    );
    assert_eq!(
        ask_role(&index, alder, Some(PairingRole::Headline), None),
        [
            owned(
                "Cedar",
                &[
                    "higher contrast for headline",
                    "narrower for headline",
                    "same era",
                    "same designer"
                ]
            ),
            owned("Birch", &["same designer"]),
        ]
    );
    let caption = ask_role(&index, alder, Some(PairingRole::Caption), None);
    assert_eq!(caption[0].1[0], "lower contrast for caption");
}

#[test]
fn no_role_and_no_partner_is_the_plain_answer() {
    let index = role_library();
    let alder = face_of(&index, "Alder");
    assert_eq!(
        ask_role(&index, alder, None, None),
        [
            owned("Birch", &["same designer"]),
            owned("Cedar", &["same designer"]),
        ]
    );
    // The missing fields read as absent: an old client's JSON still works.
    let old: PairingRequest =
        serde_json::from_str(&format!("{{\"face\":{},\"limit\":3}}", alder.0)).expect("json");
    assert_eq!((old.role, old.partner_of), (None, None));
    // The plain score is the rules' own; a role moves it.
    let score = |role| {
        index
            .suggest_with(
                &PairingRequest {
                    face: alder,
                    limit: 1,
                    role,
                    partner_of: None,
                },
                None,
            )
            .expect("pairings")[0]
            .score
    };
    assert!((score(None) - crate::pairing::DESIGNER).abs() < 1e-6);
    assert!((score(Some(PairingRole::Body)) - crate::pairing::DESIGNER).abs() > 0.05);
}

#[test]
fn partner_of_is_the_base_and_the_replaced_family_is_left_out() {
    let index = indexed(&[
        role_face(1, "Alder", 0.5, 5, "Copyright 2011 Someone"),
        role_face(2, "Birch", 0.1, 5, "Copyright 1950 Someone"),
        role_face(3, "Cedar", 0.9, 3, "(c) 2013-2020 Someone"),
        role_face(4, "Damson", 0.3, 5, "Copyright 1950 Someone"),
    ]);
    let (alder, birch, cedar) = (
        face_of(&index, "Alder"),
        face_of(&index, "Birch"),
        face_of(&index, "Cedar"),
    );
    // Replacing Alder next to Birch: neither of them is suggested.
    let got = ask_role(&index, alder, Some(PairingRole::Headline), Some(birch));
    let mut names: Vec<&str> = got.iter().map(|(name, _)| name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["Cedar", "Damson"]);
    // Damson is compared with Birch: more contrast than its 0.1, the same year.
    let damson = got.iter().find(|(n, _)| n == "Damson").expect("damson");
    assert_eq!(
        damson.1,
        ["higher contrast for headline", "same era", "same designer"]
    );
    // Compared with Alder (0.5, 2011) Damson earns neither: the base did change.
    let from_alder = ask_role(&index, alder, Some(PairingRole::Headline), None);
    let damson = from_alder
        .iter()
        .find(|(n, _)| n == "Damson")
        .expect("damson");
    assert_eq!(damson.1, ["same designer"]);

    // "Given this body face, suggest headlines": the face in both fields is
    // the same as the role alone.
    assert_eq!(
        ask_role(&index, birch, Some(PairingRole::Headline), Some(birch)),
        ask_role(&index, birch, Some(PairingRole::Headline), None)
    );

    // A partner that is not visible is NotFound, and so is the replaced face.
    let not_found = |index: &Index, face, partner_of| {
        index
            .suggest_with(
                &PairingRequest {
                    face,
                    limit: 5,
                    role: None,
                    partner_of,
                },
                None,
            )
            .expect_err("not visible")
            .code()
    };
    let gone = FaceId(99_999);
    assert_eq!(not_found(&index, alder, Some(gone)), ErrorCode::NotFound);
    assert_eq!(not_found(&index, gone, Some(cedar)), ErrorCode::NotFound);
    index
        .conn
        .execute("UPDATE source SET paused = 1", [])
        .expect("pause");
    assert_eq!(not_found(&index, alder, Some(birch)), ErrorCode::NotFound);
}

#[test]
fn the_fixture_corpus_answers_for_every_role_with_role_reasons() {
    let index = indexed(corpus());
    let face = face_of(&index, "Libre Baskerville");
    let ask = |role| PairingRequest {
        face,
        limit: 40,
        role,
        partner_of: None,
    };
    let plain = index.suggest_pairings(&ask(None)).expect("plain");
    for (role, reason) in [
        (PairingRole::Headline, "for headline"),
        (PairingRole::Body, "lower contrast for body"),
        (PairingRole::Caption, "lower contrast for caption"),
    ] {
        let got = index.suggest_pairings(&ask(Some(role))).expect("role");
        assert!(!got.is_empty(), "{role:?}");
        assert!(got.windows(2).all(|w| w[0].score >= w[1].score));
        assert!(got.iter().all(|s| (0.0..=1.0).contains(&s.score)));
        assert!(got.iter().all(|s| !s.reasons.is_empty()));
        let says =
            |test: &dyn Fn(&str) -> bool| got.iter().any(|s| s.reasons.iter().any(|r| test(r)));
        assert!(says(&|r| r.contains(reason)), "{role:?}: {got:#?}");
        // The fixtures' copyright years and x-heights reach the rule.
        assert!(says(&|r| r == "same era"), "{role:?}: {got:#?}");
        assert!(
            says(&|r| r.starts_with("x-height within ")),
            "{role:?}: {got:#?}"
        );
        assert_ne!(
            got.iter().map(|s| s.face.id).collect::<Vec<_>>(),
            plain.iter().map(|s| s.face.id).collect::<Vec<_>>(),
            "{role:?} ranks as the plain request does"
        );
    }
}

/// A face with every pairing fact set, in a file of its own.
fn synthetic_face(i: usize) -> IndexedFile {
    let classes = [
        Classification::Sans,
        Classification::Serif,
        Classification::Slab,
        Classification::Script,
        Classification::Mono,
    ];
    let family = format!("Kamiten {} {}", ["Sans", "Serif"][i % 2], i / 5);
    let mut face = ParsedFace::empty(0, Health::Ok);
    face.family = Some(family.clone());
    face.subfamily = Some("Regular".into());
    face.postscript_name = Some(format!("Kamiten{i}"));
    face.weight = Some(400);
    face.width = Some(5);
    face.designer = Some(format!("Designer {}", i % 40));
    face.copyright = Some(format!("Copyright {} Kamiten", 1980 + i % 40));
    face.metrics = Metrics {
        units_per_em: 1000,
        ascender: 800,
        descender: -200,
        x_height: Some(450 + (i % 20) as i16 * 10),
        cap_height: Some(700),
    };
    face.classification = Some(crate::classify::ClassGuess {
        class: classes[i % classes.len()],
        confidence: 0.8,
        source: crate::classify::ClassSource::Declared,
    });
    IndexedFile {
        path: PathBuf::from(format!("/fonts/kamiten-{i}.ttf")),
        size: 1000,
        modified: None,
        hash: FileHash::from([7; 32]),
        parsed: ParsedFile {
            format: Some(fm_api::common::FontFormat::Ttf),
            health: Health::Ok,
            faces: vec![face],
        },
        gone: false,
    }
}

fn time_pairings(index: &Index, label: &str) {
    let ids: Vec<u32> = index
        .conn
        .prepare("SELECT id FROM face ORDER BY id LIMIT 5")
        .expect("prepare")
        .query_map([], |r| r.get(0))
        .expect("ids")
        .collect::<Result<_, _>>()
        .expect("ids");
    for id in ids {
        // The plain request and a role's (E42.13): both are in the budget.
        for role in [None, Some(PairingRole::Body)] {
            let req = PairingRequest {
                face: FaceId(id),
                limit: 12,
                role,
                partner_of: None,
            };
            // Warm the search cache and the statement cache, as a running app has.
            index.suggest_pairings(&req).expect("pairings");
            let start = Instant::now();
            let got = index.suggest_pairings(&req).expect("pairings");
            let elapsed = start.elapsed();
            eprintln!(
                "{label}: face {id} ({role:?}) → {} pairings in {elapsed:?}",
                got.len()
            );
            assert!(elapsed < BUDGET, "{label}: face {id} took {elapsed:?}");
        }
    }
}

/// Criterion 5 (E23.4; E33.2 with the model on, as `suggest_pairings`
/// runs it) without the reference library: 10,000 faces. Timed in
/// release builds only; a debug build checks the answers.
#[test]
fn ten_thousand_faces_pair_in_under_50_ms() {
    let files: Vec<IndexedFile> = (0..10_000).map(synthetic_face).collect();
    let index = indexed(&files);
    let got = pairings(&index, face_of(&index, "Kamiten Sans 0"), 12);
    assert_eq!(got.len(), 12);
    if cfg!(debug_assertions) {
        eprintln!("debug build: timing skipped");
        return;
    }
    time_pairings(&index, "synthetic 10k");
}

/// Criterion 5 (E23.4; E33.2 with the model on) on the reference library
/// (10k fonts): skipped without it.
#[test]
fn the_reference_library_pairs_in_under_50_ms() {
    let Some(library) = std::env::var_os("FM_REFERENCE_LIBRARY") else {
        eprintln!("skipped: FM_REFERENCE_LIBRARY is not set");
        return;
    };
    let library = PathBuf::from(library);
    let mut scanned = crate::scanner::FileIndex::new();
    let report = scanned.rescan(&library, &discover(&library).expect("discover"));
    let mut index = Index::open_in_memory().expect("open");
    let source = index
        .add_source(&library.to_string_lossy(), SourceKind::Watched)
        .expect("source")
        .id;
    index
        .apply_rescan(source, &scanned, &report)
        .expect("persist");
    time_pairings(&index, "reference library");
}

/// The fixture corpus's folder.
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts")
}

/// Weights match when they differ by less than this: `model.json` rounds
/// them to six decimals.
const WEIGHT_TOLERANCE: f64 = 1e-5;

fn same_weights(a: &model::Model, b: &model::Model) -> bool {
    a.weights
        .iter()
        .zip(&b.weights)
        .all(|(x, y)| (x - y).abs() < WEIGHT_TOLERANCE)
}

#[test]
fn the_committed_model_is_not_stale() {
    let committed = model::embedded().expect("model.json loads");
    let trained = model::train_on_folder(&fixtures()).expect("train");
    assert_eq!(trained.seed, model::SEED);
    assert!(
        same_weights(&trained, committed),
        "data/pairings/model.json is stale: run `cargo run -p fm-core --example train_pairings`\n\
         committed {:?}\ntrained   {:?}",
        committed.weights,
        trained.weights
    );
    // Retraining is deterministic, and one pairing less is a different
    // model: the check above sees a stale file.
    let index = indexed(corpus());
    let curated = model::curated().expect("curated");
    let again = index
        .train_pairing_model(&curated, model::SEED)
        .expect("train");
    assert_eq!(
        again,
        index
            .train_pairing_model(&curated, model::SEED)
            .expect("train")
    );
    assert!(same_weights(&again, committed));
    let fewer = index
        .train_pairing_model(&curated[1..], model::SEED)
        .expect("train");
    assert!(!same_weights(&fewer, committed));
}

#[test]
fn every_curated_family_is_in_the_fixtures() {
    let index = indexed(corpus());
    for c in model::curated().expect("curated") {
        for family in &c.families {
            assert!(
                index.regular_face(family).expect("face").is_some(),
                "{family} is not in fixtures/fonts"
            );
        }
    }
}

/// The reciprocal rank of family `to` in `got`; `0.0` when it is not there.
fn reciprocal_rank(got: &[PairingSuggestion], to: &str) -> f64 {
    got.iter()
        .position(|s| family_key(&s.face.family_name) == family_key(to))
        .map_or(0.0, |i| 1.0 / (i + 1) as f64)
}

/// Criterion 4: fixed folds over the curated pairs; a model trained without
/// a fold ranks that fold's true partners higher on average than the rules
/// alone.
#[test]
fn the_model_ranks_held_out_partners_higher_than_the_rules() {
    const FOLDS: usize = 5;
    let index = indexed(corpus());
    let curated = model::curated().expect("curated");
    let (mut rules, mut learned) = (Vec::new(), Vec::new());
    for fold in 0..FOLDS {
        let in_fold = |i: &usize| i % FOLDS == fold;
        let held: Vec<&Curated> = curated
            .iter()
            .enumerate()
            .filter(|(i, _)| in_fold(i))
            .map(|(_, c)| c)
            .collect();
        let train: Vec<Curated> = curated
            .iter()
            .enumerate()
            .filter(|(i, _)| !in_fold(i))
            .map(|(_, c)| c.clone())
            .collect();
        let m = index
            .train_pairing_model(&train, model::SEED)
            .expect("train");
        for c in held {
            let [a, b] = &c.families;
            for (from, to) in [(a, b), (b, a)] {
                let face = index
                    .regular_face(from)
                    .expect("face")
                    .unwrap_or_else(|| panic!("no face of {from}"));
                let req = PairingRequest {
                    face,
                    limit: u32::MAX,
                    role: None,
                    partner_of: None,
                };
                let got = index.suggest_with(&req, None).expect("rules");
                rules.push(reciprocal_rank(&got, to));
                let got = index.suggest_with(&req, Some(&m)).expect("model");
                learned.push(reciprocal_rank(&got, to));
            }
        }
    }
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    let (rules, learned) = (mean(&rules), mean(&learned));
    eprintln!("held-out mean reciprocal rank: rules {rules:.3}, model {learned:.3}");
    assert!(learned > rules, "model {learned:.3} <= rules {rules:.3}");
}

#[test]
fn a_curated_partner_gains_the_model_reason() {
    let index = indexed(corpus());
    let face = index
        .regular_face("Libre Baskerville")
        .expect("face")
        .expect("Libre Baskerville");
    let got = pairings(&index, face, 1000);
    let franklin = got
        .iter()
        .find(|s| s.face.family_name == "Libre Franklin")
        .unwrap_or_else(|| panic!("no Libre Franklin in {got:#?}"));
    assert_eq!(
        franklin.reasons.last().map(String::as_str),
        Some(MODEL_REASON)
    );
    // The rules' reasons come first, and the score stays in range.
    assert_eq!(franklin.reasons[0], "contrast: serif with sans serif");
    assert!(got.iter().all(|s| (0.0..=1.0).contains(&s.score)));
    // Without the model nobody gets the reason.
    let rules = index
        .suggest_with(
            &PairingRequest {
                face,
                limit: 1000,
                role: None,
                partner_of: None,
            },
            None,
        )
        .expect("rules");
    assert!(
        rules
            .iter()
            .all(|s| !s.reasons.iter().any(|r| r == MODEL_REASON))
    );
    // The model only reorders what the rules let through.
    let mut a: Vec<u32> = got.iter().map(|s| s.face.id.0).collect();
    let mut b: Vec<u32> = rules.iter().map(|s| s.face.id.0).collect();
    a.sort_unstable();
    b.sort_unstable();
    assert_eq!(a, b);
}

/// A model that says "pairing" for a shared designer and nothing else.
fn designer_model() -> model::Model {
    let mut weights = [0.0; model::FEATURES.len()];
    let at = |name: &str| {
        model::FEATURES
            .iter()
            .position(|f| *f == name)
            .expect("feature")
    };
    weights[at("bias")] = -6.0;
    weights[at("same_designer")] = 12.0;
    model::Model { seed: 0, weights }
}

#[test]
fn the_model_reason_comes_only_when_the_model_says_pair() {
    let index = indexed(&[
        plain_sans(1, "Alder", "Ada Lovelace", 500),
        plain_sans(2, "Birch", "ada lovelace", 510),
        plain_sans(3, "Cedar", "Bob", 505),
        plain_sans(4, "Damson", "Ada Lovelace", 700),
    ]);
    let req = PairingRequest {
        face: face_of(&index, "Alder"),
        limit: 10,
        role: None,
        partner_of: None,
    };
    let m = designer_model();
    let got = index.suggest_with(&req, Some(&m)).expect("pairings");
    let flagged: Vec<(&str, bool)> = got
        .iter()
        .map(|s| {
            let says = s.reasons.iter().any(|r| r == MODEL_REASON);
            (s.face.family_name.as_str(), says)
        })
        .collect();
    // Cedar passes the rules (similar x-height) but the model scores it
    // low: it keeps the rules' reasons only, and ranks last.
    assert_eq!(
        flagged,
        [("Birch", true), ("Damson", true), ("Cedar", false)],
        "{got:#?}"
    );
    let cedar = &got[2];
    assert_eq!(cedar.reasons, ["similar x-height"]);
    // The blend: half the rules' score, half the model's probability.
    let chance = 1.0 / (1.0 + (6.0_f32).exp());
    let want = 0.5 * crate::pairing::X_HEIGHT + 0.5 * chance;
    assert!(
        (cedar.score - want).abs() < 1e-4,
        "{} vs {want}",
        cedar.score
    );
}
