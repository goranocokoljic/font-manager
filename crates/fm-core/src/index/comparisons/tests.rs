use std::path::PathBuf;

use fm_api::common::{FontFormat, Health};
use fm_api::comparison::CompareMode;
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
    ("Lora", "Italic"),
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

fn faces(index: &Index, postscripts: &[&str]) -> Vec<FaceId> {
    postscripts.iter().map(|p| face(index, p)).collect()
}

fn request(name: &str, faces: Vec<FaceId>) -> SaveComparisonRequest {
    SaveComparisonRequest {
        id: None,
        name: name.into(),
        faces,
        mode: CompareMode::Stacked,
        sample_text: "Hamburgefonstiv".into(),
    }
}

/// The comparison's faces as `Family Style`, in its order.
fn names(c: &Comparison) -> Vec<String> {
    c.faces
        .iter()
        .map(|f| format!("{} {}", f.family_name, f.style))
        .collect()
}

fn rows(index: &Index, table: &str) -> u32 {
    index
        .conn
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .expect("count")
}

#[test]
fn a_comparison_keeps_its_faces_in_order_its_mode_and_its_text() {
    let (mut index, _) = library();
    let ids = faces(
        &index,
        &["SourceSerif-Regular", "Roboto-Bold", "Inter-Regular"],
    );
    let saved = index
        .save_comparison(&request("  Body text  ", ids.clone()))
        .expect("save");
    assert_eq!(saved.name, "Body text");
    assert_eq!(saved.mode, CompareMode::Stacked);
    assert_eq!(saved.sample_text, "Hamburgefonstiv");
    assert_eq!(
        names(&saved),
        ["Source Serif Regular", "Roboto Bold", "Inter Regular"]
    );
    assert!(saved.missing.is_empty());
    assert_eq!(saved.faces.iter().map(|f| f.id).collect::<Vec<_>>(), ids);
    assert_eq!(index.comparison(saved.id).expect("get"), saved);
}

#[test]
fn comparisons_are_listed_by_name_renamed_overwritten_and_deleted() {
    let (mut index, _) = library();
    let pair = faces(&index, &["Roboto-Regular", "Inter-Regular"]);
    let b = index
        .save_comparison(&request("headlines", pair.clone()))
        .expect("save b");
    let a = index
        .save_comparison(&request("Body", pair.clone()))
        .expect("save a");
    let listed = index.list_comparisons().expect("list");
    assert_eq!(
        listed.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        ["Body", "headlines"]
    );
    assert_eq!(listed[0].face_count, 2);
    assert_eq!(listed[0].mode, CompareMode::Stacked);

    let renamed = index.rename_comparison(b.id, " Alpha ").expect("rename");
    assert_eq!(renamed.name, "Alpha");
    assert_eq!(index.list_comparisons().expect("list")[0].id, b.id);

    // Saving over an id replaces its faces, mode and text; the id stays.
    let three = faces(&index, &["Lora-Italic", "Roboto-Bold", "Inter-Regular"]);
    let over = index
        .save_comparison(&SaveComparisonRequest {
            id: Some(a.id),
            name: "Body".into(),
            faces: three,
            mode: CompareMode::Overlay,
            sample_text: String::new(),
        })
        .expect("overwrite");
    assert_eq!(over.id, a.id);
    assert_eq!(over.mode, CompareMode::Overlay);
    assert_eq!(over.sample_text, "");
    assert_eq!(
        names(&over),
        ["Lora Italic", "Roboto Bold", "Inter Regular"]
    );
    assert_eq!(rows(&index, "comparison_face"), 5);

    index.delete_comparison(b.id).expect("delete");
    assert_eq!(
        index.comparison(b.id).map_err(|e| e.code()),
        Err(ErrorCode::NotFound)
    );
    assert_eq!(rows(&index, "comparison_face"), 3);
    // The faces stay in the library.
    assert_eq!(rows(&index, "face"), 5);
    assert_eq!(
        index.delete_comparison(b.id).map_err(|e| e.code()),
        Err(ErrorCode::NotFound)
    );
}

#[test]
fn too_few_or_too_many_faces_or_an_empty_name_is_an_invalid_request() {
    let (mut index, _) = library();
    let all = faces(
        &index,
        &[
            "Roboto-Regular",
            "Roboto-Bold",
            "Inter-Regular",
            "SourceSerif-Regular",
            "Lora-Italic",
        ],
    );
    let invalid = |index: &mut Index, req: SaveComparisonRequest| {
        index
            .save_comparison(&req)
            .map(|_| ())
            .map_err(|e| e.code())
    };
    assert_eq!(
        invalid(&mut index, request("One", all[..1].to_vec())),
        Err(ErrorCode::InvalidRequest)
    );
    assert_eq!(
        invalid(&mut index, request("None", Vec::new())),
        Err(ErrorCode::InvalidRequest)
    );
    assert_eq!(
        invalid(&mut index, request("Five", all.clone())),
        Err(ErrorCode::InvalidRequest)
    );
    assert_eq!(
        invalid(&mut index, request("   ", all[..2].to_vec())),
        Err(ErrorCode::InvalidRequest)
    );
    assert_eq!(
        invalid(&mut index, request("Twice", vec![all[0], all[1], all[0]])),
        Err(ErrorCode::InvalidRequest)
    );
    assert_eq!(
        invalid(&mut index, request("Unknown", vec![all[0], FaceId(9999)])),
        Err(ErrorCode::NotFound)
    );
    assert_eq!(rows(&index, "comparison"), 0);
    // The edges of the range are fine.
    index
        .save_comparison(&request("Two", all[..2].to_vec()))
        .expect("two");
    let four = index
        .save_comparison(&request("Four", all[..4].to_vec()))
        .expect("four");
    // A bad overwrite or rename leaves the saved one as it was.
    let mut bad = request("Four", all[..1].to_vec());
    bad.id = Some(four.id);
    assert_eq!(invalid(&mut index, bad), Err(ErrorCode::InvalidRequest));
    assert_eq!(
        index.rename_comparison(four.id, "").map_err(|e| e.code()),
        Err(ErrorCode::InvalidRequest)
    );
    assert_eq!(index.comparison(four.id).expect("kept"), four);
    let mut unknown = request("Ghost", all[..2].to_vec());
    unknown.id = Some(ComparisonId(9999));
    assert_eq!(invalid(&mut index, unknown), Err(ErrorCode::NotFound));
    assert_eq!(
        index
            .rename_comparison(ComparisonId(9999), "x")
            .map_err(|e| e.code()),
        Err(ErrorCode::NotFound)
    );
}

#[test]
fn a_removed_or_hidden_face_is_named_as_missing_and_the_rest_open() {
    let (mut index, source) = library();
    let ids = faces(&index, &["Roboto-Bold", "Lora-Italic", "Inter-Regular"]);
    let saved = index
        .save_comparison(&request("Mix", ids.clone()))
        .expect("save");

    // Lora's file is gone from the folder: the rescan marks it gone.
    let mut gone = file("Lora", "Italic");
    gone.gone = true;
    index.persist_files(source, &[gone]).expect("rescan");

    let got = index.comparison(saved.id).expect("reopen");
    assert_eq!(names(&got), ["Roboto Bold", "Inter Regular"]);
    assert_eq!(
        got.missing,
        [MissingFace {
            id: ids[1],
            name: "Lora Italic".into()
        }]
    );
    // The list still counts the face it was saved with.
    assert_eq!(index.list_comparisons().expect("list")[0].face_count, 3);

    // A paused folder hides every face; the rows stay and come back.
    index.set_source_paused(source, true).expect("pause");
    let hidden = index.comparison(saved.id).expect("paused");
    assert!(hidden.faces.is_empty());
    assert_eq!(hidden.missing.len(), 3);
    index.set_source_paused(source, false).expect("resume");
    assert_eq!(
        names(&index.comparison(saved.id).expect("resumed")),
        ["Roboto Bold", "Inter Regular"]
    );
}

#[test]
fn a_face_that_leaves_the_index_is_missing_even_when_its_id_is_reused() {
    let (mut index, _) = library();
    let more = index
        .add_source("/more", SourceKind::Watched)
        .expect("source")
        .id;
    let mut lora = file("Lora", "Bold");
    lora.path = PathBuf::from("/more/Lora-Bold.ttf");
    index.persist_files(more, &[lora]).expect("persist");
    let ids = faces(&index, &["Inter-Regular", "Lora-Bold"]);
    let saved = index
        .save_comparison(&request("Pair", ids.clone()))
        .expect("save");
    // The folder is removed (its faces leave the index), and a new font is
    // indexed: SQLite gives it the freed highest id.
    index.remove_source(more).expect("remove");
    let other = index
        .add_source("/other", SourceKind::Watched)
        .expect("source")
        .id;
    let mut fira = file("Fira Sans", "Book");
    fira.path = PathBuf::from("/other/FiraSans-Book.ttf");
    index.persist_files(other, &[fira]).expect("persist");
    assert_eq!(face(&index, "FiraSans-Book"), ids[1], "the id is reused");

    let got = index.comparison(saved.id).expect("reopen");
    assert_eq!(names(&got), ["Inter Regular"]);
    assert_eq!(
        got.missing,
        [MissingFace {
            id: ids[1],
            name: "Lora Bold".into()
        }]
    );
}

#[test]
fn comparisons_outlive_a_restart() {
    let dir = TempDir::new("comparisons");
    let path = dir.path().join(INDEX_FILE);
    let saved = {
        let mut index = Index::open(&path).expect("open").index;
        fill(&mut index);
        let ids = faces(&index, &["Roboto-Bold", "Inter-Regular"]);
        index.save_comparison(&request("Kept", ids)).expect("save")
    };
    let index = Index::open(&path).expect("reopen").index;
    assert_eq!(index.comparison(saved.id).expect("get"), saved);
    assert_eq!(index.list_comparisons().expect("list").len(), 1);
}
