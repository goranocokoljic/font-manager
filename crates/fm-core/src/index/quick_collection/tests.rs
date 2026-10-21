use std::path::PathBuf;

use fm_api::common::{FontFormat, Health};
use fm_api::library::SourceKind;
use fm_api::{ErrorCode, FaceId, SourceId};

use super::*;
use crate::index::INDEX_FILE;
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, IndexedFile};
use crate::test_support::TempDir;

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

fn fill(index: &mut Index) -> SourceId {
    let source = index
        .add_source("/fonts", SourceKind::Watched)
        .expect("source")
        .id;
    let files: Vec<_> = LIBRARY.iter().map(|(f, s)| file(f, s)).collect();
    index.persist_files(source, &files).expect("persist");
    source
}

fn library() -> (Index, SourceId) {
    let mut index = Index::open_in_memory().expect("open");
    let source = fill(&mut index);
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

/// The Quick Collection as `Family Style`, in its order.
fn names(quick: &QuickCollection) -> Vec<String> {
    quick
        .faces
        .iter()
        .map(|f| format!("{} {}", f.family_name, f.style))
        .collect()
}

#[test]
fn faces_are_listed_first_added_first_and_once() {
    let (mut index, _) = library();
    let inter = face(&index, "Inter-Regular");
    let bold = face(&index, "Roboto-Bold");
    let serif = face(&index, "SourceSerif-Regular");
    assert!(index.quick_collection().expect("empty").faces.is_empty());

    let got = index
        .add_to_quick_collection(&[inter, bold, inter])
        .expect("add");
    assert_eq!(names(&got), ["Inter Regular", "Roboto Bold"]);
    // Adding a face again keeps its place; a new one goes to the end.
    let got = index
        .add_to_quick_collection(&[serif, inter])
        .expect("add again");
    assert_eq!(
        names(&got),
        ["Inter Regular", "Roboto Bold", "Source Serif Regular"]
    );
    assert_eq!(index.quick_collection().expect("list"), got);
    // A face's row is grouped into its family, as the library lists it.
    let roboto = face(&index, "Roboto-Regular");
    assert_eq!(got.faces[1].family.0, roboto.0);
}

#[test]
fn removing_and_clearing_leave_the_library_alone() {
    let (mut index, _) = library();
    let all: Vec<_> = ["Roboto-Regular", "Roboto-Bold", "Inter-Regular"]
        .iter()
        .map(|p| face(&index, p))
        .collect();
    index.add_to_quick_collection(&all).expect("add");
    let serif = face(&index, "SourceSerif-Regular");
    let got = index
        .remove_from_quick_collection(&[all[1], serif])
        .expect("remove");
    assert_eq!(names(&got), ["Roboto Regular", "Inter Regular"]);
    index.clear_quick_collection().expect("clear");
    assert!(index.quick_collection().expect("list").faces.is_empty());
    let faces: u32 = index
        .conn
        .query_row("SELECT COUNT(*) FROM face", [], |r| r.get(0))
        .expect("count");
    assert_eq!(faces, 4);
}

#[test]
fn an_unknown_face_adds_nothing() {
    let (mut index, _) = library();
    let inter = face(&index, "Inter-Regular");
    let err = index
        .add_to_quick_collection(&[inter, FaceId(9999)])
        .expect_err("unknown face");
    assert_eq!(err.code(), ErrorCode::NotFound);
    assert!(index.quick_collection().expect("list").faces.is_empty());
}

#[test]
fn a_hidden_face_is_not_listed_and_a_removed_one_is_gone() {
    let (mut index, source) = library();
    let inter = face(&index, "Inter-Regular");
    index.add_to_quick_collection(&[inter]).expect("add");
    index.set_source_paused(source, true).expect("pause");
    assert!(index.quick_collection().expect("paused").faces.is_empty());
    index.set_source_paused(source, false).expect("resume");
    assert_eq!(
        names(&index.quick_collection().expect("resumed")),
        ["Inter Regular"]
    );
    index.remove_source(source).expect("remove source");
    let rows: u32 = index
        .conn
        .query_row("SELECT COUNT(*) FROM quick_face", [], |r| r.get(0))
        .expect("count");
    assert_eq!(rows, 0);
}

#[test]
fn the_quick_collection_outlives_a_restart() {
    let dir = TempDir::new("quick-collection");
    let path = dir.path().join(INDEX_FILE);
    let (bold, inter) = {
        let mut index = Index::open(&path).expect("open").index;
        fill(&mut index);
        let bold = face(&index, "Roboto-Bold");
        let inter = face(&index, "Inter-Regular");
        index.add_to_quick_collection(&[bold, inter]).expect("add");
        (bold, inter)
    };
    let index = Index::open(&path).expect("reopen").index;
    let got = index.quick_collection().expect("list");
    assert_eq!(
        got.faces.iter().map(|f| f.id).collect::<Vec<_>>(),
        [bold, inter]
    );
}
