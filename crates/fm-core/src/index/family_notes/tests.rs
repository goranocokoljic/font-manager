use std::path::PathBuf;

use fm_api::common::{FontFormat, Health};
use fm_api::library::SourceKind;
use fm_api::{ErrorCode, FamilyId, SourceId};

use super::*;
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, IndexedFile};

/// One file per face: (family as the font names it, style). The first two
/// are one family: the library groups names that differ in case and spaces.
const LIBRARY: &[(&str, &str)] = &[
    ("Source Serif", "Regular"),
    ("source  serif", "Bold"),
    ("Inter", "Regular"),
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

fn files() -> Vec<IndexedFile> {
    LIBRARY.iter().map(|(f, s)| file(f, s)).collect()
}

fn library() -> (Index, SourceId) {
    let mut index = Index::open_in_memory().expect("open");
    let source = index
        .add_source("/fonts", SourceKind::Watched)
        .expect("source")
        .id;
    index.persist_files(source, &files()).expect("persist");
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

fn family(index: &Index, postscript: &str) -> FamilyId {
    index
        .family_of(face(index, postscript))
        .expect("family")
        .expect("visible")
        .0
}

fn set(index: &mut Index, family: FamilyId, notes: &str) -> FmResult<Vec<FaceId>> {
    index.update_family_notes(&FamilyNotesUpdate {
        family,
        notes: notes.into(),
    })
}

fn summary_notes(index: &Index, family: FamilyId) -> Option<String> {
    index
        .family_summary(family)
        .expect("summary")
        .expect("family")
        .notes
}

#[test]
fn family_notes_are_set_on_the_family_and_cleared_by_an_empty_string() {
    let (mut index, _) = library();
    let serif = family(&index, "SourceSerif-Regular");
    let faces = set(&mut index, serif, "Body text for the annual report").expect("set");
    // Both faces of the grouped family, the ones the list shows again.
    assert_eq!(
        faces,
        [
            face(&index, "SourceSerif-Regular"),
            face(&index, "sourceserif-Bold")
        ]
    );
    assert_eq!(
        summary_notes(&index, serif).as_deref(),
        Some("Body text for the annual report")
    );
    for postscript in ["SourceSerif-Regular", "sourceserif-Bold"] {
        let detail = index.face_detail(face(&index, postscript)).expect("detail");
        assert_eq!(
            detail.family_notes.as_deref(),
            Some("Body text for the annual report")
        );
        // The family's notes are not the face's.
        assert_eq!(detail.notes, None);
    }
    let inter = family(&index, "Inter-Regular");
    assert_eq!(summary_notes(&index, inter), None);
    // The search list carries them too.
    let listed = index
        .list_families(&fm_api::library::ListFamiliesRequest::default())
        .expect("list");
    let row = listed.items.iter().find(|f| f.id == serif).expect("row");
    assert_eq!(
        row.notes.as_deref(),
        Some("Body text for the annual report")
    );

    set(&mut index, serif, "").expect("clear");
    assert_eq!(summary_notes(&index, serif), None);
    let rows: u32 = index
        .conn
        .query_row("SELECT COUNT(*) FROM family_meta", [], |r| r.get(0))
        .expect("count");
    assert_eq!(rows, 0, "an empty string deletes the row");
}

#[test]
fn family_notes_outlive_the_faces_being_indexed_again() {
    let (mut index, source) = library();
    let serif = family(&index, "SourceSerif-Regular");
    set(&mut index, serif, "keep me").expect("set");
    // The fonts go away and come back: new face ids, the same family.
    index.persist_files(source, &[]).expect("emptied");
    index
        .conn
        .execute("DELETE FROM face", [])
        .expect("forget faces");
    index.persist_files(source, &files()).expect("again");
    let again = family(&index, "SourceSerif-Regular");
    assert_eq!(summary_notes(&index, again).as_deref(), Some("keep me"));
    // A rebuild reads every file again and keeps them.
    index.rebuild().expect("rebuild");
    index.persist_files(source, &files()).expect("rescan");
    assert_eq!(summary_notes(&index, again).as_deref(), Some("keep me"));
}

#[test]
fn a_family_notes_change_marks_every_face_of_the_family_for_the_sidecar() {
    let (mut index, _) = library();
    index
        .conn
        .execute("DELETE FROM sidecar_dirty", [])
        .expect("clean");
    let serif = family(&index, "SourceSerif-Regular");
    set(&mut index, serif, "mirrored").expect("set");
    let dirty: Vec<u32> = index
        .conn
        .prepare("SELECT face_id FROM sidecar_dirty ORDER BY face_id")
        .and_then(|mut s| s.query_map([], |r| r.get(0))?.collect())
        .expect("dirty");
    // The source folder does not exist, so the faces stay marked.
    assert_eq!(
        dirty,
        [
            face(&index, "SourceSerif-Regular").0,
            face(&index, "sourceserif-Bold").0
        ]
    );
}

#[test]
fn an_id_that_is_not_a_family_is_not_found() {
    let (mut index, _) = library();
    assert_eq!(
        set(&mut index, FamilyId(999), "x").map_err(|e| e.code()),
        Err(ErrorCode::NotFound)
    );
    // The second face's id is a face id, not the family's.
    let bold = face(&index, "sourceserif-Bold");
    assert_eq!(
        set(&mut index, FamilyId(bold.0), "x").map_err(|e| e.code()),
        Err(ErrorCode::NotFound)
    );
}
