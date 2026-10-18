use std::path::PathBuf;

use fm_api::ErrorCode;
use fm_api::common::{FontFormat, Health};
use fm_api::library::SourceKind;
use fm_api::search::{FilterCount, Filters, SearchRequest};
use fm_api::{FaceId, SourceId, TagId};

use super::*;
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, IndexedFile};

/// One file per face: (family, style).
const LIBRARY: &[(&str, &str)] = &[
    ("Roboto", "Regular"),
    ("Roboto", "Bold"),
    ("Inter", "Regular"),
    ("Source Serif", "Regular"),
];

fn file(family: &str, style: &str) -> IndexedFile {
    let mut face = ParsedFace::empty(0, Health::Ok);
    face.family = Some(family.to_owned());
    face.subfamily = Some(style.to_owned());
    face.postscript_name = Some(format!("{}-{style}", family.replace(' ', "")));
    face.weight = Some(400);
    face.width = Some(5);
    IndexedFile {
        path: PathBuf::from(format!("/fonts/{}-{style}.ttf", family.replace(' ', ""))),
        size: 1000,
        modified: None,
        hash: FileHash::from([7; 32]),
        parsed: ParsedFile {
            format: Some(FontFormat::Ttf),
            health: Health::Ok,
            faces: vec![face],
        },
        gone: false,
    }
}

fn library() -> (Index, SourceId) {
    let mut index = Index::open_in_memory().expect("open");
    let source = index
        .add_source("/fonts", SourceKind::Watched)
        .expect("source")
        .id;
    let files: Vec<_> = LIBRARY.iter().map(|(f, s)| file(f, s)).collect();
    index.persist_files(source, &files).expect("persist");
    (index, source)
}

fn face(index: &Index, postscript: &str) -> FaceId {
    index
        .conn
        .query_row(
            "SELECT id FROM face WHERE postscript_name = ?1",
            [postscript],
            |r| r.get(0),
        )
        .map(FaceId)
        .expect("face id")
}

fn tag(index: &mut Index, faces: &[FaceId], names: &[&str]) -> FmResult<Vec<Tag>> {
    index.tag_faces(&TagFacesRequest {
        faces: faces.to_vec(),
        tags: names.iter().map(|n| (*n).to_owned()).collect(),
    })
}

fn untag(index: &mut Index, faces: &[FaceId], tags: &[TagId]) -> FmResult<()> {
    index.untag_faces(&UntagFacesRequest {
        faces: faces.to_vec(),
        tags: tags.to_vec(),
    })
}

/// The tags as `name count`, in list order.
fn names(index: &Index) -> Vec<String> {
    index
        .tags()
        .expect("tags")
        .iter()
        .map(|t| format!("{} {}", t.name, t.face_count))
        .collect()
}

/// Family names a search with `filters` returns, A to Z.
fn families(index: &Index, filters: Filters) -> Vec<String> {
    let mut out: Vec<String> = index
        .search(&SearchRequest {
            filters,
            ..SearchRequest::default()
        })
        .expect("search")
        .families
        .into_iter()
        .map(|h| h.family.name)
        .collect();
    out.sort();
    out
}

fn count<T: PartialEq>(list: &[FilterCount<T>], value: &T) -> u32 {
    list.iter()
        .find(|c| &c.value == value)
        .map_or(0, |c| c.count)
}

/// (favorite, rating, notes) of a face; `None` when it has no row.
fn meta(index: &Index, face: FaceId) -> Option<(bool, Option<u8>, Option<String>)> {
    index
        .conn
        .query_row(
            "SELECT favorite, rating, notes FROM user_meta WHERE face_id = ?1",
            [face.0],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .expect("meta")
}

#[test]
fn tags_are_created_reused_without_regard_to_case_and_listed_a_to_z() {
    let (mut index, _) = library();
    let regular = face(&index, "Roboto-Regular");
    let bold = face(&index, "Roboto-Bold");
    let inter = face(&index, "Inter-Regular");

    let made = tag(
        &mut index,
        &[regular, bold],
        &["  Display ", "headline", ""],
    )
    .expect("tag");
    assert_eq!(
        made.iter()
            .map(|t| (t.name.as_str(), t.face_count))
            .collect::<Vec<_>>(),
        [("Display", 2), ("headline", 2)]
    );
    // "display" is the same tag; "Brand" is new. Tagging twice keeps one row.
    let again = tag(
        &mut index,
        &[inter, regular],
        &["display", "Brand", "BRAND"],
    )
    .expect("tag");
    assert_eq!(again[0].id, made[0].id);
    assert_eq!(again[0].name, "Display");
    assert_eq!(again.len(), 2);
    assert_eq!(names(&index), ["Brand 2", "Display 3", "headline 2"]);
}

/// `tag.name` is `COLLATE NOCASE`, which folds A–Z only: names that differ
/// in the case of other letters are different tags (the UI relies on it).
#[test]
fn names_that_differ_beyond_ascii_case_are_different_tags() {
    let (mut index, _) = library();
    let regular = face(&index, "Roboto-Regular");
    let inter = face(&index, "Inter-Regular");
    let upper = tag(&mut index, &[regular], &["Äb"]).expect("tag")[0].id;
    let lower = tag(&mut index, &[inter], &["äb"]).expect("tag")[0].id;
    assert_ne!(upper, lower);
    assert_eq!(names(&index).len(), 2);
    untag(&mut index, &[inter], &[lower]).expect("untag");
    assert_eq!(names(&index), ["Äb 1"]);
}

#[test]
fn untagging_removes_the_tag_from_the_faces_and_drops_an_unused_tag() {
    let (mut index, _) = library();
    let regular = face(&index, "Roboto-Regular");
    let bold = face(&index, "Roboto-Bold");
    let made = tag(&mut index, &[regular, bold], &["display", "brand"]).expect("tag");
    let (display, brand) = (made[0].id, made[1].id);

    untag(&mut index, &[regular], &[display]).expect("untag");
    assert_eq!(names(&index), ["brand 2", "display 1"]);
    // Untagging a face that lacks the tag changes nothing.
    untag(&mut index, &[regular], &[display]).expect("again");
    untag(&mut index, &[bold], &[display, brand]).expect("untag");
    // `display` is on no face: autocomplete no longer offers it.
    assert_eq!(names(&index), ["brand 1"]);
    assert_eq!(
        untag(&mut index, &[regular], &[display]).map_err(|e| e.code()),
        Err(ErrorCode::NotFound)
    );
}

#[test]
fn bad_tag_requests_are_refused_and_change_nothing() {
    let (mut index, _) = library();
    let regular = face(&index, "Roboto-Regular");
    let code = |r: FmResult<Vec<Tag>>| r.map(|_| ()).map_err(|e| e.code());
    assert_eq!(
        code(tag(&mut index, &[regular], &["  ", ""])),
        Err(ErrorCode::InvalidRequest)
    );
    assert_eq!(
        code(tag(&mut index, &[], &["display"])),
        Err(ErrorCode::InvalidRequest)
    );
    // An unknown face fails the whole request: the known one is not tagged.
    assert_eq!(
        code(tag(&mut index, &[regular, FaceId(9999)], &["display"])),
        Err(ErrorCode::NotFound)
    );
    assert!(names(&index).is_empty());
    assert_eq!(
        untag(&mut index, &[FaceId(9999)], &[]).map_err(|e| e.code()),
        Err(ErrorCode::NotFound)
    );
}

#[test]
fn tag_counts_leave_out_gone_files_and_paused_sources() {
    let (mut index, source) = library();
    let regular = face(&index, "Roboto-Regular");
    let bold = face(&index, "Roboto-Bold");
    tag(&mut index, &[regular, bold], &["display"]).expect("tag");
    index
        .conn
        .execute(
            "UPDATE font_file SET gone = 1 WHERE path LIKE '%Roboto-Bold%'",
            [],
        )
        .expect("gone");
    assert_eq!(names(&index), ["display 1"]);
    index.set_source_paused(source, true).expect("pause");
    assert_eq!(names(&index), ["display 0"]);
}

#[test]
fn favorite_rating_and_notes_are_set_kept_and_cleared() {
    let (mut index, _) = library();
    let regular = face(&index, "Roboto-Regular");
    let inter = face(&index, "Inter-Regular");
    assert_eq!(meta(&index, regular), None);

    let update = |favorite, rating, notes: Option<&str>| UserMetaUpdate {
        faces: vec![regular, inter],
        favorite,
        rating,
        notes: notes.map(str::to_owned),
    };
    index
        .update_user_meta(&update(Some(true), Some(4), Some("Client logo")))
        .expect("set");
    assert_eq!(
        meta(&index, inter),
        Some((true, Some(4), Some("Client logo".into())))
    );
    // Fields left out stay.
    index
        .update_user_meta(&update(None, Some(5), None))
        .expect("rate");
    assert_eq!(
        meta(&index, regular),
        Some((true, Some(5), Some("Client logo".into())))
    );
    // 0 stars and empty notes clear; false unfavorites.
    index
        .update_user_meta(&update(Some(false), Some(0), Some("")))
        .expect("clear");
    assert_eq!(meta(&index, regular), Some((false, None, None)));
}

#[test]
fn bad_user_meta_updates_are_refused_and_change_nothing() {
    let (mut index, _) = library();
    let regular = face(&index, "Roboto-Regular");
    let code = |index: &mut Index, faces: Vec<FaceId>, rating| {
        index
            .update_user_meta(&UserMetaUpdate {
                faces,
                favorite: Some(true),
                rating,
                notes: None,
            })
            .map_err(|e| e.code())
    };
    assert_eq!(
        code(&mut index, vec![regular], Some(6)),
        Err(ErrorCode::InvalidRequest)
    );
    assert_eq!(
        code(&mut index, vec![regular, FaceId(9999)], Some(3)),
        Err(ErrorCode::NotFound)
    );
    assert_eq!(meta(&index, regular), None);
}

#[test]
fn tags_favorites_and_ratings_filter_and_count_as_soon_as_they_change() {
    let (mut index, _) = library();
    let regular = face(&index, "Roboto-Regular");
    let inter = face(&index, "Inter-Regular");
    // Search once first, so the cached rows must follow the writes below.
    assert_eq!(families(&index, Filters::default()).len(), 3);

    let display = tag(&mut index, &[regular, inter], &["display"]).expect("tag")[0].id;
    let set = |index: &mut Index, face, favorite, rating| {
        index
            .update_user_meta(&UserMetaUpdate {
                faces: vec![face],
                favorite: Some(favorite),
                rating: Some(rating),
                notes: None,
            })
            .expect("meta");
    };
    set(&mut index, regular, true, 5);
    set(&mut index, inter, false, 2);

    let by_tag = Filters {
        tags: vec![display],
        ..Filters::default()
    };
    assert_eq!(families(&index, by_tag), ["Inter", "Roboto"]);
    let favorites = Filters {
        favorite: Some(true),
        ..Filters::default()
    };
    assert_eq!(families(&index, favorites), ["Roboto"]);
    let three_stars = Filters {
        min_rating: Some(3),
        ..Filters::default()
    };
    assert_eq!(families(&index, three_stars), ["Roboto"]);

    let counts = index
        .search(&SearchRequest::default())
        .expect("search")
        .counts
        .expect("counts");
    assert_eq!(count(&counts.tags, &display), 2);
    assert_eq!(count(&counts.favorite, &true), 1);
    assert_eq!(count(&counts.favorite, &false), 3);
    assert_eq!(count(&counts.min_rating, &2), 2);
    assert_eq!(count(&counts.min_rating, &5), 1);

    // Text search finds the tag name too.
    let found = index
        .search(&SearchRequest {
            query: "display".into(),
            ..SearchRequest::default()
        })
        .expect("search");
    assert_eq!(found.total, 2);

    untag(&mut index, &[inter], &[display]).expect("untag");
    set(&mut index, regular, false, 0);
    let counts = index
        .search(&SearchRequest::default())
        .expect("search")
        .counts
        .expect("counts");
    assert_eq!(count(&counts.tags, &display), 1);
    assert_eq!(count(&counts.favorite, &true), 0);
    assert_eq!(count(&counts.min_rating, &3), 0);
}
