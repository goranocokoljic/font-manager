use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use fm_api::ErrorCode;
use fm_api::common::Classification;
use fm_api::library::SourceKind;

use super::*;
use crate::scanner::{FileIndex, discover};
use crate::similar::METRICS_ONLY;

/// The search budget of the spec (criterion 5).
const BUDGET: Duration = Duration::from_millis(50);

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts")
}

/// The scan of `root`, persisted into a fresh in-memory index.
fn indexed(root: &Path) -> Index {
    let mut scanned = FileIndex::new();
    let report = scanned.rescan(root, &discover(root).expect("discover"));
    let mut index = Index::open_in_memory().expect("open");
    let source = index
        .add_source(&root.to_string_lossy(), SourceKind::Watched)
        .expect("source")
        .id;
    index
        .apply_rescan(source, &scanned, &report)
        .expect("persist");
    index
}

/// `fixtures/fonts/google-repo` (about 130 families) indexed; scanned once
/// per test run, each test opening its own copy is too slow.
fn library() -> Index {
    static SCANNED: OnceLock<(FileIndex, crate::scanner::Rescan)> = OnceLock::new();
    let root = fixtures().join("google-repo");
    let (scanned, report) = SCANNED.get_or_init(|| {
        let mut scanned = FileIndex::new();
        let report = scanned.rescan(&root, &discover(&root).expect("discover"));
        (scanned, report)
    });
    let mut index = Index::open_in_memory().expect("open");
    let source = index
        .add_source(&root.to_string_lossy(), SourceKind::Watched)
        .expect("source")
        .id;
    index
        .apply_rescan(source, scanned, report)
        .expect("persist");
    index
}

fn face(index: &Index, file_name: &str) -> FaceId {
    let id: i64 = index
        .conn
        .query_row(
            "SELECT face.id FROM face JOIN font_file ON font_file.id = face.file_id
             WHERE font_file.path LIKE ?1 ORDER BY face.id LIMIT 1",
            [format!("%{file_name}")],
            |r| r.get(0),
        )
        .unwrap_or_else(|e| panic!("{file_name}: {e}"));
    FaceId(u32::try_from(id).expect("id"))
}

fn similar(index: &Index, face: FaceId, limit: u32) -> Vec<SimilarFace> {
    index
        .find_similar(&SimilarRequest { face, limit })
        .expect("similar")
}

/// Every candidate, closest first.
fn ranking(index: &Index, face: FaceId) -> Vec<SimilarFace> {
    similar(index, face, u32::MAX)
}

fn rank_of(ranked: &[SimilarFace], family: &str) -> usize {
    ranked
        .iter()
        .position(|s| s.face.family_name == family)
        .unwrap_or_else(|| panic!("{family} is not ranked"))
}

fn show(ranked: &[SimilarFace]) -> String {
    ranked
        .iter()
        .take(25)
        .map(|s| format!("{} {:.3} {:?}", s.face.family_name, s.score, s.reasons))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The classification of every face, as search reads it.
fn classes(index: &Index) -> HashMap<u32, Option<Classification>> {
    index.refresh_search_cache().expect("rows");
    let cache = index.search_cache.borrow();
    cache
        .rows
        .iter()
        .map(|r| (r.id, r.facts.classification))
        .collect()
}

/// Criterion 3: for Barlow, a sans ranks above Libre Baskerville and
/// Clicker Script. The fixtures hold Barlow Bold Italic, not Regular.
#[test]
fn a_sans_ranks_above_a_serif_and_a_script_for_barlow() {
    let index = library();
    let ranked = ranking(&index, face(&index, "Barlow-BoldItalic.ttf"));
    let sans = rank_of(&ranked, "Barlow Semi Condensed");
    let condensed = rank_of(&ranked, "Barlow Condensed");
    let serif = rank_of(&ranked, "Libre Baskerville");
    let script = rank_of(&ranked, "Clicker Script");
    let all = show(&ranked);
    eprintln!("{all}");
    assert!(sans < serif && sans < script, "{all}");
    assert!(condensed < serif && condensed < script, "{all}");
}

/// Criterion 3: for Libre Baskerville, a serif (Crimson Pro) ranks above
/// every sans.
#[test]
fn a_serif_ranks_above_every_sans_for_libre_baskerville() {
    let index = library();
    let classes = classes(&index);
    let ranked = ranking(&index, face(&index, "LibreBaskerville[wght].ttf"));
    let serif = rank_of(&ranked, "Crimson Pro");
    let first_sans = ranked
        .iter()
        .position(|s| classes.get(&s.face.id.0) == Some(&Some(Classification::Sans)))
        .expect("a sans is ranked");
    eprintln!("{}", show(&ranked));
    assert!(serif < first_sans, "{}", show(&ranked));
}

/// Criterion 1: the face itself and its family are left out, `limit`
/// holds, scores are in `0.0..=1.0`, closest first.
#[test]
fn the_face_and_its_family_are_left_out_and_the_limit_holds() {
    let index = library();
    let barlow = face(&index, "Barlow-BoldItalic.ttf");
    let family = index.face_summary(barlow).expect("summary").family;
    let ranked = ranking(&index, barlow);
    assert!(ranked.len() > 50, "{}", ranked.len());
    assert!(ranked.iter().all(|s| s.face.id != barlow));
    assert!(ranked.iter().all(|s| s.face.family != family));
    assert!(ranked.iter().all(|s| (0.0..=1.0).contains(&s.score)));
    assert!(ranked.windows(2).all(|w| w[0].score >= w[1].score));

    let five = similar(&index, barlow, 5);
    assert_eq!(five, ranked[..5]);
    assert!(similar(&index, barlow, 0).is_empty());
}

/// Every face of the query's family is left out, not only the face: the
/// two faces of the Agdasima collection never find each other.
#[test]
fn a_family_with_two_faces_leaves_out_both() {
    let index = indexed(&fixtures().join("derived/agdasima"));
    let pair: Vec<FaceId> = ["Agdasima-Regular", "Agdasima-Bold"]
        .iter()
        .map(|ps| {
            index
                .conn
                .query_row(
                    "SELECT id FROM face WHERE postscript_name = ?1 AND file_id =
                     (SELECT id FROM font_file WHERE path LIKE '%.ttc')",
                    [ps],
                    |r| r.get(0),
                )
                .map(FaceId)
                .unwrap_or_else(|e| panic!("{ps}: {e}"))
        })
        .collect();
    for &id in &pair {
        let family = index.face_summary(id).expect("summary").family_name;
        let found = ranking(&index, id);
        assert!(
            found.iter().all(|s| !pair.contains(&s.face.id)),
            "{found:?}"
        );
        assert!(
            found.iter().all(|s| s.face.family_name != family),
            "{found:?}"
        );
    }
}

/// Faces of gone files and paused sources are no candidates.
#[test]
fn hidden_faces_are_not_candidates() {
    let index = library();
    let barlow = face(&index, "Barlow-BoldItalic.ttf");
    let semi = face(&index, "BarlowSemiCondensed-ExtraLightItalic.ttf");
    assert!(ranking(&index, barlow).iter().any(|s| s.face.id == semi));
    index
        .conn
        .execute(
            "UPDATE font_file SET gone = 1
             WHERE id = (SELECT file_id FROM face WHERE id = ?1)",
            [semi.0],
        )
        .expect("gone");
    assert!(ranking(&index, barlow).iter().all(|s| s.face.id != semi));
}

/// Criterion 4: a face without a fingerprint is compared by metrics and
/// classification only, and says so; the others keep their drawings.
#[test]
fn a_face_without_a_fingerprint_says_so() {
    let index = library();
    let barlow = face(&index, "Barlow-BoldItalic.ttf");
    let semi = face(&index, "BarlowSemiCondensed-ExtraLightItalic.ttf");
    assert!(
        ranking(&index, barlow)
            .iter()
            .filter(|s| s.face.id == semi)
            .all(|s| !s.reasons.iter().any(|r| r == METRICS_ONLY))
    );
    index
        .conn
        .execute(
            "UPDATE face_fingerprint SET vector = NULL WHERE face_id = ?1",
            [semi.0],
        )
        .expect("no fingerprint");

    let ranked = ranking(&index, barlow);
    for s in &ranked {
        let metrics_only = s.reasons.iter().any(|r| r == METRICS_ONLY);
        let has_print: bool = index
            .conn
            .query_row(
                "SELECT vector IS NOT NULL FROM face_fingerprint WHERE face_id = ?1",
                [s.face.id.0],
                |r| r.get(0),
            )
            .unwrap_or(false);
        assert_eq!(metrics_only, !has_print, "{}", s.face.family_name);
    }
    let found = ranked.iter().find(|s| s.face.id == semi).expect("ranked");
    assert!(
        !found
            .reasons
            .iter()
            .any(|r| r == "similar contrast" || r == "similar letter shapes"),
        "{found:?}"
    );

    // Asked about the face without one, every result says so.
    let from_semi = ranking(&index, semi);
    assert!(!from_semi.is_empty());
    assert!(
        from_semi
            .iter()
            .all(|s| s.reasons.iter().any(|r| r == METRICS_ONLY))
    );
}

#[test]
fn an_unknown_face_is_not_found() {
    let index = indexed(&fixtures().join("google-repo/crimsonpro"));
    let err = index
        .find_similar(&SimilarRequest {
            face: FaceId(99_999),
            limit: 5,
        })
        .expect_err("unknown");
    assert_eq!(err.code(), ErrorCode::NotFound);
}

/// A write between two searches is seen: the profiles are read again.
#[test]
fn a_write_reads_the_profiles_again() {
    let index = library();
    let barlow = face(&index, "Barlow-BoldItalic.ttf");
    let before = similar(&index, barlow, 3);
    let top = before[0].face.id;
    index
        .conn
        .execute("DELETE FROM face_fingerprint WHERE face_id = ?1", [top.0])
        .expect("delete");
    let after = ranking(&index, barlow);
    let found = after.iter().find(|s| s.face.id == top).expect("ranked");
    assert!(found.reasons.iter().any(|r| r == METRICS_ONLY), "{found:?}");
}

/// Criterion 5 without the reference library: 10,000 faces with
/// fingerprints. Timed in release builds only; a debug build checks the
/// results.
#[test]
fn ten_thousand_faces_find_20_similar_in_under_50_ms() {
    let mut files = super::super::tests::synthetic(2_000);
    for (i, file) in files.iter_mut().enumerate() {
        let n = i as f32;
        file.parsed.faces[0].fingerprint =
            crate::fingerprint::FaceFingerprint::Taken(Fingerprint {
                vector: (0..64)
                    .map(|c| ((n * 0.37 + c as f32) % 7.0) / 7.0)
                    .collect(),
                x_height_ratio: 0.6 + (n % 20.0) / 100.0,
                contrast: (n % 10.0) / 10.0,
                stroke_width: 0.08 + (n % 15.0) / 100.0,
                width_class: 5.0,
                advance_ratio: 0.5 + (n % 30.0) / 100.0,
            });
    }
    let mut index = Index::open_in_memory().expect("open");
    let source = index
        .add_source("/fonts", SourceKind::Watched)
        .expect("source")
        .id;
    index.persist_files(source, &files).expect("persist");
    let faces: Vec<FaceId> = (1..=10).map(|i| FaceId(i * 997)).collect();
    let start = Instant::now();
    similar(&index, faces[0], 20);
    eprintln!(
        "synthetic 10k: first similar search in {:?}",
        start.elapsed()
    );
    for &face in &faces {
        similar(&index, face, 20);
        let start = Instant::now();
        let found = similar(&index, face, 20);
        let elapsed = start.elapsed();
        assert_eq!(found.len(), 20);
        if cfg!(debug_assertions) {
            continue;
        }
        eprintln!("synthetic 10k: face {face} in {elapsed:?}");
        assert!(elapsed < BUDGET, "face {face} took {elapsed:?}");
    }
}

/// Criterion 5 on the reference library (10k fonts): skipped without it.
#[test]
fn the_reference_library_finds_20_similar_in_under_50_ms() {
    let Some(library) = std::env::var_os("FM_REFERENCE_LIBRARY") else {
        eprintln!("skipped: FM_REFERENCE_LIBRARY is not set");
        return;
    };
    let index = indexed(&PathBuf::from(library));
    let faces: Vec<u32> = index
        .conn
        .prepare("SELECT face_id FROM face_fingerprint WHERE vector IS NOT NULL LIMIT 10")
        .expect("prepare")
        .query_map([], |r| r.get(0))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("faces");
    // The first search after a write reads the profiles: reported, not
    // budgeted (it happens once per library change, as for text search).
    let start = Instant::now();
    similar(&index, FaceId(faces[0]), 20);
    eprintln!(
        "reference library: first similar search in {:?}",
        start.elapsed()
    );
    for id in faces {
        let start = Instant::now();
        let found = similar(&index, FaceId(id), 20);
        let elapsed = start.elapsed();
        eprintln!(
            "reference library: face {id} → {} in {elapsed:?}",
            found.len()
        );
        assert_eq!(found.len(), 20);
        assert!(elapsed < BUDGET, "face {id} took {elapsed:?}");
    }
}

/// `google-repo` and Roboto Slab (`google-static/robotoslab`) in one index.
fn image_library() -> Index {
    let mut index = library();
    let root = fixtures().join("google-static/robotoslab");
    let mut scanned = FileIndex::new();
    let report = scanned.rescan(&root, &discover(&root).expect("discover"));
    let source = index
        .add_source(&root.to_string_lossy(), SourceKind::Watched)
        .expect("source")
        .id;
    index
        .apply_rescan(source, &scanned, &report)
        .expect("persist");
    index
}

/// E33.1 criterion 4: the sample line rendered from each face at 48 px, at
/// 2x and as white on black, reduced to a fingerprint, puts its own family
/// in the top 5 of the library.
#[test]
fn an_image_of_each_family_ranks_it_in_the_top_5() {
    use crate::image_match::{fingerprint_png, tests::render};
    let index = image_library();
    let faces = [
        ("google-repo/barlow/Barlow-BoldItalic.ttf", "Barlow"),
        (
            "google-repo/librebaskerville/LibreBaskerville[wght].ttf",
            "Libre Baskerville",
        ),
        ("google-repo/crimsonpro/CrimsonPro[wght].ttf", "Crimson Pro"),
        (
            "google-repo/clickerscript/ClickerScript-Regular.ttf",
            "Clicker Script",
        ),
        (
            "google-static/robotoslab/RobotoSlab-ExtraBold.ttf",
            "Roboto Slab",
        ),
    ];
    let looks = [
        (48.0, "#000000", "#ffffff"),
        (96.0, "#000000", "#ffffff"),
        (48.0, "#ffffff", "#000000"),
    ];
    let mut misses = Vec::new();
    for (rel, family) in faces {
        for (size, fg, bg) in looks {
            let png = render(rel, crate::fingerprint::SAMPLE, size, fg, bg).to_png();
            let print = fingerprint_png(&png).expect("fingerprint");
            let top = index.find_by_image(&print, 5).expect("rank");
            let all = show(&top);
            eprintln!("{family} {size} {fg}:\n{all}\n");
            assert_eq!(top.len(), 5);
            assert!(top.windows(2).all(|p| p[0].score >= p[1].score));
            assert!(top.iter().all(|s| (0.0..=1.0).contains(&s.score)));
            if !top.iter().any(|s| s.face.family_name == family) {
                misses.push(format!("{family} at {size} px, {fg} on {bg}:\n{all}"));
            }
        }
    }
    assert!(misses.is_empty(), "{}", misses.join("\n\n"));
}

/// E33.1: faces without a fingerprint are not candidates for an image, and
/// the limit holds.
#[test]
fn an_image_ranks_only_fingerprinted_faces() {
    use crate::image_match::{fingerprint_png, tests::render};
    let index = library();
    let png = render(
        "google-repo/crimsonpro/CrimsonPro[wght].ttf",
        crate::fingerprint::SAMPLE,
        48.0,
        "#000000",
        "#ffffff",
    )
    .to_png();
    let print = fingerprint_png(&png).expect("fingerprint");
    let all = index.find_by_image(&print, u32::MAX).expect("rank");
    let fingerprinted: u32 = index
        .conn
        .query_row(
            "SELECT COUNT(*) FROM face_fingerprint WHERE vector IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .expect("count");
    assert_eq!(all.len() as u32, fingerprinted);
    assert!(
        all.iter()
            .all(|s| !s.reasons.iter().any(|r| r == METRICS_ONLY))
    );
    assert_eq!(index.find_by_image(&print, 3).expect("rank").len(), 3);
}
