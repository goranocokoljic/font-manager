use std::fs;
use std::path::{Path, PathBuf};

use fm_api::ErrorCode;
use fm_api::SourceId;
use fm_api::library::SourceKind;

use super::*;
use crate::glyphs::similarity;
use crate::index::INDEX_FILE;
use crate::scanner::{FileIndex, discover};
use crate::test_support::TempDir;

/// Straight-leg R, sans.
const MULISH: &str = "google-repo/mulish/Mulish[wght].ttf";
/// Straight-leg R, sans, another family.
const OPEN_SANS: &str = "google-repo/opensanshebrew/OpenSansHebrew-Bold.ttf";
/// Curved-leg R, handwriting.
const ZEYADA: &str = "google-repo/zeyada/Zeyada.ttf";
/// Curved-leg R, script.
const MINIVER: &str = "google-repo/miniver/Miniver-Regular.ttf";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts")
}

fn copy(fonts: &Path, rel: &str, name: &str) {
    fs::create_dir_all(fonts).expect("folder");
    fs::copy(fixtures().join(rel), fonts.join(name)).expect("copy fixture");
}

/// An index of `fonts` (one watched source), every face's `R` described.
fn described(dir: &TempDir, fonts: &[(&str, &str)]) -> (Index, SourceId) {
    let root = dir.path().join("fonts");
    for (rel, name) in fonts {
        copy(&root, rel, name);
    }
    let mut files = FileIndex::new();
    files.rescan(&root, &discover(&root).expect("discover"));
    let mut index = Index::open(&dir.path().join("data").join(INDEX_FILE))
        .expect("open")
        .index;
    let source = index
        .add_source(&root.to_string_lossy(), SourceKind::Watched)
        .expect("source")
        .id;
    index.persist_files(source, files.files()).expect("persist");
    describe_all(&mut index, 'R' as u32);
    (index, source)
}

fn describe_all(index: &mut Index, code_point: u32) {
    let todo = index.glyph_descriptor_todo(code_point).expect("todo");
    let done = similarity::describe_todo(&todo, code_point);
    index.write_glyph_descriptors(&done).expect("write");
}

fn face(index: &Index, name: &str) -> FaceId {
    index.trait_debug_face(name).expect("face")
}

fn ask(face: FaceId, ch: char, limit: u32) -> SimilarGlyphRequest {
    SimilarGlyphRequest {
        face,
        code_point: u32::from(ch),
        limit,
    }
}

/// Criterion 2: faces ranked by glyph distance, closest first, with the
/// distance; the reference face is not listed.
#[test]
fn faces_are_ranked_by_glyph_distance_closest_first() {
    let dir = TempDir::new("glyph-similar-rank");
    let (index, _) = described(
        &dir,
        &[
            (MULISH, "mulish.ttf"),
            (OPEN_SANS, "open-sans.ttf"),
            (ZEYADA, "zeyada.ttf"),
            (MINIVER, "miniver.ttf"),
        ],
    );
    let mulish = face(&index, "mulish.ttf");
    let found = index
        .find_similar_glyph(&ask(mulish, 'R', 10))
        .expect("rank");
    assert_eq!(found.len(), 3, "every other face");
    assert!(
        found.iter().all(|f| f.face.id != mulish),
        "not the reference"
    );
    assert_eq!(
        found[0].face.id,
        face(&index, "open-sans.ttf"),
        "the other straight-leg sans first"
    );
    assert!(found.windows(2).all(|w| w[0].distance <= w[1].distance));
    assert!(found.iter().all(|f| f.distance > 0.0 && f.distance <= 1.0));
    // The distance exposed is the descriptors' distance.
    let r = |name: &str| {
        let data = fs::read(fixtures().join(name)).unwrap();
        similarity::describe(&data, 0, 'R' as u32).unwrap()
    };
    assert_eq!(
        found[0].distance,
        similarity::distance(&r(MULISH), &r(OPEN_SANS))
    );
    let two = index
        .find_similar_glyph(&ask(mulish, 'R', 2))
        .expect("rank");
    assert_eq!(two, found[..2].to_vec(), "limit keeps the closest");
}

/// Two faces with the same drawing are at distance 0 and keep face-id order,
/// and the other faces of the reference's family are candidates.
#[test]
fn equal_distances_keep_face_id_order_and_the_family_stays() {
    let dir = TempDir::new("glyph-similar-ties");
    let (index, _) = described(
        &dir,
        &[
            (MULISH, "a.ttf"),
            (MULISH, "b.ttf"),
            (MULISH, "c.ttf"),
            (ZEYADA, "zeyada.ttf"),
        ],
    );
    let a = face(&index, "a.ttf");
    let found = index.find_similar_glyph(&ask(a, 'R', 10)).expect("rank");
    let ids: Vec<FaceId> = found.iter().map(|f| f.face.id).collect();
    assert_eq!(ids[..2], [face(&index, "b.ttf"), face(&index, "c.ttf")]);
    assert_eq!(found[0].distance, 0.0);
    assert_eq!(found[1].distance, 0.0);
    assert!(found[2].distance > 0.0);
}

/// A face that does not draw the character is no candidate, and as the
/// reference is `NotFound`; so is an unknown face. A number that is no
/// character is an invalid request.
#[test]
fn a_face_without_the_glyph_is_no_candidate_and_no_reference() {
    let dir = TempDir::new("glyph-similar-missing");
    let (mut index, _) = described(&dir, &[(MULISH, "mulish.ttf"), (OPEN_SANS, "open.ttf")]);
    // Hebrew alef: Open Sans Hebrew draws it, Mulish does not.
    let alef = 0x05D0;
    describe_all(&mut index, alef);
    let open = face(&index, "open.ttf");
    let mulish = face(&index, "mulish.ttf");
    let found = index
        .find_similar_glyph(&SimilarGlyphRequest {
            face: open,
            code_point: alef,
            limit: 10,
        })
        .expect("rank");
    assert!(found.is_empty(), "Mulish has no alef: {found:?}");
    let err = index
        .find_similar_glyph(&SimilarGlyphRequest {
            face: mulish,
            code_point: alef,
            limit: 10,
        })
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::NotFound);
    let err = index
        .find_similar_glyph(&ask(FaceId(999_999), 'R', 3))
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::NotFound);
    let err = index
        .find_similar_glyph(&SimilarGlyphRequest {
            face: open,
            code_point: 0xD800,
            limit: 3,
        })
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
}

/// A face of a paused source is not searchable: no candidate.
#[test]
fn a_face_of_a_paused_source_is_no_candidate() {
    let dir = TempDir::new("glyph-similar-paused");
    let (mut index, source) = described(&dir, &[(MULISH, "mulish.ttf"), (OPEN_SANS, "open.ttf")]);
    let mulish = face(&index, "mulish.ttf");
    assert_eq!(
        index
            .find_similar_glyph(&ask(mulish, 'R', 5))
            .unwrap()
            .len(),
        1
    );
    index.set_source_paused(source, true).expect("pause");
    // The reference itself is in the paused source too; it still has its
    // descriptor, and nothing is left to rank.
    assert!(
        index
            .find_similar_glyph(&ask(mulish, 'R', 5))
            .unwrap()
            .is_empty()
    );
}

/// A descriptor of an old version or of a file read again since is not
/// read: the face is described again first.
#[test]
fn a_stale_descriptor_is_not_ranked() {
    let dir = TempDir::new("glyph-similar-stale");
    let (index, _) = described(&dir, &[(MULISH, "mulish.ttf"), (OPEN_SANS, "open.ttf")]);
    let mulish = face(&index, "mulish.ttf");
    index.trait_debug_sql(&format!(
        "UPDATE glyph_descriptor SET version = {} WHERE face_id != {}",
        similarity::VERSION + 1,
        mulish.0
    ));
    assert!(
        index
            .find_similar_glyph(&ask(mulish, 'R', 5))
            .unwrap()
            .is_empty()
    );
    assert_eq!(index.glyph_descriptor_todo('R' as u32).unwrap().len(), 1);
    index.trait_debug_sql(&format!(
        "UPDATE glyph_descriptor SET version = {}, file_hash = x'00'",
        similarity::VERSION
    ));
    assert_eq!(index.glyph_descriptor_todo('R' as u32).unwrap().len(), 2);
    assert_eq!(
        index
            .find_similar_glyph(&ask(mulish, 'R', 5))
            .unwrap_err()
            .code(),
        ErrorCode::NotFound
    );
}

/// A search sorted by the `R` of `face`, with `order` asked for too.
fn like(face: FaceId, ch: char) -> fm_api::search::SearchRequest {
    fm_api::search::SearchRequest {
        like_glyph: Some(GlyphReference {
            face,
            code_point: u32::from(ch),
        }),
        page: fm_api::common::Page {
            offset: 0,
            limit: 100,
        },
        ..Default::default()
    }
}

fn family_names(found: &fm_api::search::SearchResponse) -> Vec<String> {
    found
        .families
        .iter()
        .map(|f| f.family.name.clone())
        .collect()
}

/// E49.9 criterion 1: a likeness sort orders the families by their closest
/// face's glyph distance to the reference, the reference's own family first
/// (distance 0), each with its distance; the family's closest face is its
/// first face; a family order asked for as well is not used.
#[test]
fn a_likeness_sort_orders_families_by_glyph_distance() {
    let dir = TempDir::new("glyph-like-sort");
    let (index, _) = described(
        &dir,
        &[
            (MULISH, "mulish.ttf"),
            (OPEN_SANS, "open-sans.ttf"),
            (ZEYADA, "zeyada.ttf"),
            (MINIVER, "miniver.ttf"),
        ],
    );
    let mulish = face(&index, "mulish.ttf");
    let found = index.search(&like(mulish, 'R')).expect("search");
    assert_eq!(found.total, 4);
    let first = &found.families[0];
    assert_eq!(first.faces[0], mulish, "the reference's family first");
    assert_eq!(first.glyph_distance, Some(0.0));
    // The order is the ranking of find_similar_glyph, face by face.
    let ranked = index
        .find_similar_glyph(&ask(mulish, 'R', 10))
        .expect("rank");
    let rest: Vec<(FaceId, Option<f32>)> = found.families[1..]
        .iter()
        .map(|f| (f.faces[0], f.glyph_distance))
        .collect();
    let expected: Vec<(FaceId, Option<f32>)> = ranked
        .iter()
        .map(|f| (f.face.id, Some(f.distance)))
        .collect();
    assert_eq!(rest, expected);
    // A name order asked for as well does not change it.
    let mut by_name = like(mulish, 'R');
    by_name.order = Some(fm_api::library::FamilyOrder {
        by: fm_api::library::FamilySortKey::Name,
        descending: true,
    });
    assert_eq!(
        family_names(&index.search(&by_name).expect("search")),
        family_names(&found)
    );
    // Without the likeness sort, the families carry no distance.
    let plain = index
        .search(&fm_api::search::SearchRequest {
            page: fm_api::common::Page {
                offset: 0,
                limit: 100,
            },
            ..Default::default()
        })
        .expect("search");
    assert!(plain.families.iter().all(|f| f.glyph_distance.is_none()));
}

/// The likeness sort keeps the query and the filters: it orders what they
/// find, and the reference face need not be one of the results.
#[test]
fn a_likeness_sort_combines_with_the_query_and_filters() {
    let dir = TempDir::new("glyph-like-filters");
    let (index, _) = described(
        &dir,
        &[
            (MULISH, "mulish.ttf"),
            (OPEN_SANS, "open-sans.ttf"),
            (ZEYADA, "zeyada.ttf"),
            (MINIVER, "miniver.ttf"),
        ],
    );
    let mulish = face(&index, "mulish.ttf");
    let mut req = like(mulish, 'R');
    // Mulish is the variable one: a static-only filter leaves it out.
    req.filters.variable = Some(false);
    let found = index.search(&req).expect("search");
    assert_eq!(found.total, 3);
    assert!(found.families.iter().all(|f| !f.faces.contains(&mulish)));
    let distances: Vec<f32> = found
        .families
        .iter()
        .map(|f| f.glyph_distance.expect("drawn"))
        .collect();
    assert!(distances.windows(2).all(|w| w[0] <= w[1]), "{distances:?}");
    assert_eq!(found.families[0].faces[0], face(&index, "open-sans.ttf"));
    // A query narrows the same way.
    let mut req = like(mulish, 'R');
    req.query = "zeyada".into();
    let found = index.search(&req).expect("search");
    assert_eq!(family_names(&found), ["Zeyada"]);
}

/// A family whose faces do not draw the character follows the others, with
/// no distance; an unknown reference, or one that does not draw the
/// character, is `NotFound`; a number that is no character is invalid.
#[test]
fn a_likeness_sort_puts_families_without_the_glyph_last() {
    let dir = TempDir::new("glyph-like-missing");
    let (mut index, _) = described(
        &dir,
        &[
            (MULISH, "mulish.ttf"),
            (OPEN_SANS, "open.ttf"),
            (ZEYADA, "zeyada.ttf"),
        ],
    );
    let alef = 0x05D0;
    describe_all(&mut index, alef);
    let open = face(&index, "open.ttf");
    let mut req = like(open, 'R');
    req.like_glyph = Some(GlyphReference {
        face: open,
        code_point: alef,
    });
    let found = index.search(&req).expect("search");
    assert_eq!(found.total, 3, "no family is dropped");
    assert_eq!(found.families[0].faces[0], open);
    assert_eq!(found.families[0].glyph_distance, Some(0.0));
    assert!(
        found.families[1..]
            .iter()
            .all(|f| f.glyph_distance.is_none())
    );
    // Those keep the default order (A to Z here).
    let rest = family_names(&found)[1..].to_vec();
    let mut sorted = rest.clone();
    sorted.sort();
    assert_eq!(rest, sorted);

    let mulish = face(&index, "mulish.ttf");
    req.like_glyph = Some(GlyphReference {
        face: mulish,
        code_point: alef,
    });
    assert_eq!(index.search(&req).unwrap_err().code(), ErrorCode::NotFound);
    req.like_glyph = Some(GlyphReference {
        face: FaceId(999_999),
        code_point: 'R' as u32,
    });
    assert_eq!(index.search(&req).unwrap_err().code(), ErrorCode::NotFound);
    req.like_glyph = Some(GlyphReference {
        face: open,
        code_point: 0xD800,
    });
    assert_eq!(
        index.search(&req).unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
}
