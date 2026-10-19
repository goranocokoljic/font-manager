use std::path::PathBuf;

use fm_api::ErrorCode;
use fm_api::common::{FontFormat, Health};
use fm_api::library::SourceKind;
use fm_api::organization::{CreateSetRequest, SetEntry, SetExport};
use fm_api::{FaceId, SetId};

use super::*;
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, IndexedFile};

/// A file at `path` with hash bytes `hash` and one face per style.
fn file(path: &str, hash: u8, family: &str, styles: &[&str]) -> IndexedFile {
    let faces = styles
        .iter()
        .enumerate()
        .map(|(i, style)| {
            let mut face = ParsedFace::empty(u32::try_from(i).expect("index"), Health::Ok);
            face.family = Some(family.to_owned());
            face.subfamily = Some((*style).to_owned());
            face.postscript_name = Some(format!("{}-{style}", family.replace(' ', "")));
            face.weight = Some(if *style == "Bold" { 700 } else { 400 });
            face.width = Some(5);
            face
        })
        .collect();
    IndexedFile {
        path: PathBuf::from(path),
        size: 1000,
        modified: None,
        hash: FileHash::from([hash; 32]),
        parsed: ParsedFile {
            format: Some(if styles.len() > 1 {
                FontFormat::Ttc
            } else {
                FontFormat::Ttf
            }),
            health: Health::Ok,
            faces,
        },
        gone: false,
    }
}

/// Roboto Regular and Bold, Inter, and a collection file with two faces;
/// every file has its own hash.
fn files() -> Vec<IndexedFile> {
    vec![
        file("/fonts/Roboto-Regular.ttf", 1, "Roboto", &["Regular"]),
        file("/fonts/Roboto-Bold.ttf", 2, "Roboto", &["Bold"]),
        file("/fonts/Inter-Regular.ttf", 3, "Inter", &["Regular"]),
        file("/fonts/Noto.ttc", 4, "Noto Sans", &["Light", "Black"]),
    ]
}

fn library(files: &[IndexedFile]) -> Index {
    let mut index = Index::open_in_memory().expect("open");
    let source = index
        .add_source("/fonts", SourceKind::Watched)
        .expect("source")
        .id;
    index.persist_files(source, files).expect("persist");
    index
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

fn create(index: &mut Index, name: &str, faces: &[FaceId]) -> FmResult<ProjectSet> {
    index.create_set(&CreateSetRequest {
        name: name.to_owned(),
        faces: faces.to_vec(),
    })
}

fn hex(byte: u8) -> String {
    FileHash::from([byte; 32]).to_hex()
}

#[test]
fn sets_are_created_named_and_listed_a_to_z_with_their_faces() {
    let mut index = library(&files());
    let regular = face(&index, "Roboto-Regular");
    let bold = face(&index, "Roboto-Bold");
    let inter = face(&index, "Inter-Regular");

    let client = create(&mut index, "  Client B ", &[inter, bold, inter]).expect("create");
    assert_eq!(client.name, "Client B");
    let mut want = vec![bold, inter];
    want.sort();
    assert_eq!(client.faces, want, "a face named twice is in the set once");

    let alpha = create(&mut index, "alpha", &[regular]).expect("create");
    let empty = create(&mut index, "Moodboard", &[]).expect("an empty set is fine");
    assert!(empty.faces.is_empty());

    let listed: Vec<(SetId, String)> = index
        .sets()
        .expect("sets")
        .into_iter()
        .map(|s| (s.id, s.name))
        .collect();
    assert_eq!(
        listed,
        vec![
            (alpha.id, "alpha".to_owned()),
            (client.id, "Client B".to_owned()),
            (empty.id, "Moodboard".to_owned()),
        ]
    );
    assert_eq!(index.set(client.id).expect("set"), client);
}

#[test]
fn bad_names_and_unknown_faces_are_refused_without_a_change() {
    let mut index = library(&files());
    let regular = face(&index, "Roboto-Regular");
    create(&mut index, "Client", &[regular]).expect("create");

    let err = create(&mut index, "   ", &[regular]).expect_err("empty name");
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
    let err = create(&mut index, "CLIENT", &[regular]).expect_err("taken name");
    assert_eq!(err.code(), ErrorCode::Conflict);
    let err = create(&mut index, "Other", &[regular, FaceId(9_999)]).expect_err("unknown face");
    assert_eq!(err.code(), ErrorCode::NotFound);

    assert_eq!(index.sets().expect("sets").len(), 1);
}

#[test]
fn deleting_a_set_keeps_its_faces() {
    let mut index = library(&files());
    let regular = face(&index, "Roboto-Regular");
    let set = create(&mut index, "Client", &[regular]).expect("create");

    index.delete_set(set.id).expect("delete");
    assert!(index.sets().expect("sets").is_empty());
    assert_eq!(face(&index, "Roboto-Regular"), regular);
    let left: u32 = index
        .conn
        .query_row("SELECT COUNT(*) FROM set_face", [], |r| r.get(0))
        .expect("count");
    assert_eq!(left, 0);

    let err = index.delete_set(set.id).expect_err("gone");
    assert_eq!(err.code(), ErrorCode::NotFound);
    let err = index.set(set.id).expect_err("gone");
    assert_eq!(err.code(), ErrorCode::NotFound);
}

#[test]
fn an_export_names_each_face_by_family_style_and_file_hash() {
    let mut index = library(&files());
    let bold = face(&index, "Roboto-Bold");
    let black = face(&index, "NotoSans-Black");
    let set = create(&mut index, "Client", &[black, bold]).expect("create");

    let export = index.export_set(set.id).expect("export");
    assert_eq!(export.name, "Client");
    assert_eq!(
        export.fonts,
        vec![
            SetEntry {
                family: "Roboto".into(),
                style: "Bold".into(),
                hash: hex(2),
            },
            SetEntry {
                family: "Noto Sans".into(),
                style: "Black".into(),
                hash: hex(4),
            },
        ]
    );
    let json = serde_json::to_string(&export).expect("json");
    let back: SetExport = serde_json::from_str(&json).expect("parse");
    assert_eq!(back, export);

    let err = index.export_set(SetId(9_999)).expect_err("unknown");
    assert_eq!(err.code(), ErrorCode::NotFound);
}

#[test]
fn an_export_is_refused_when_a_file_is_not_read_yet() {
    let mut index = library(&files());
    let regular = face(&index, "Roboto-Regular");
    let set = create(&mut index, "Client", &[regular]).expect("create");
    index
        .conn
        .execute(
            "UPDATE font_file SET hash = NULL WHERE path = '/fonts/Roboto-Regular.ttf'",
            [],
        )
        .expect("unread");

    let err = index.export_set(set.id).expect_err("no hash");
    assert_eq!(err.code(), ErrorCode::Conflict);
}

#[test]
fn an_import_matches_by_hash_at_another_path_and_reports_the_rest() {
    let mut source = library(&files());
    let regular = face(&source, "Roboto-Regular");
    let inter = face(&source, "Inter-Regular");
    let light = face(&source, "NotoSans-Light");
    let set = create(&mut source, "Client", &[regular, inter, light]).expect("create");
    let export = source.export_set(set.id).expect("export");

    // A teammate's library: Inter and the collection at other paths, no Roboto.
    let mut target = library(&[
        file("/elsewhere/inter.ttf", 3, "Inter", &["Regular"]),
        file("/elsewhere/noto.ttc", 4, "Noto Sans", &["Light", "Black"]),
    ]);
    let json = serde_json::to_string(&export).expect("json");
    let parsed: SetExport = serde_json::from_str(&json).expect("parse");
    let out = target.import_set(&parsed).expect("import");

    assert_eq!(out.set.name, "Client");
    let mut want = vec![
        face(&target, "Inter-Regular"),
        face(&target, "NotoSans-Light"),
    ];
    want.sort();
    assert_eq!(
        out.set.faces, want,
        "the Light face, not Black, of the collection"
    );
    assert_eq!(
        out.missing,
        vec![SetEntry {
            family: "Roboto".into(),
            style: "Regular".into(),
            hash: hex(1),
        }]
    );
    assert_eq!(target.sets().expect("sets"), vec![out.set]);
}

#[test]
fn unreadable_hashes_gone_files_and_unknown_collection_faces_are_missing() {
    let mut gone = file("/fonts/Old.ttf", 9, "Old", &["Regular"]);
    gone.gone = true;
    let mut all = files();
    all.push(gone);
    let mut index = library(&all);
    let entry = |family: &str, style: &str, hash: String| SetEntry {
        family: family.into(),
        style: style.into(),
        hash,
    };
    let export = SetExport {
        name: "Mixed".into(),
        fonts: vec![
            entry("Inter", "Regular", "not a hash".into()),
            entry("Old", "Regular", hex(9)),
            entry("Noto Sans", "Medium", hex(4)),
            entry("Roboto", "Bold", hex(2)),
            entry("Roboto", "Bold", hex(2)),
            entry("Renamed", "Whatever", hex(3)),
        ],
    };

    let out = index.import_set(&export).expect("import");
    let mut want = vec![face(&index, "Roboto-Bold"), face(&index, "Inter-Regular")];
    want.sort();
    assert_eq!(
        out.set.faces, want,
        "a one-face file matches by hash alone; a duplicate entry is one face"
    );
    assert_eq!(
        out.missing
            .iter()
            .map(|e| e.family.as_str())
            .collect::<Vec<_>>(),
        vec!["Inter", "Old", "Noto Sans"]
    );
}

#[test]
fn an_import_takes_a_free_name_and_refuses_an_empty_one() {
    let mut index = library(&files());
    create(&mut index, "Client", &[]).expect("create");
    create(&mut index, "Client (2)", &[]).expect("create");
    let export = SetExport {
        name: " client ".into(),
        fonts: vec![],
    };

    let out = index.import_set(&export).expect("import");
    assert_eq!(out.set.name, "client (3)");
    assert!(out.set.faces.is_empty());
    assert!(out.missing.is_empty());

    let err = index
        .import_set(&SetExport {
            name: "  ".into(),
            fonts: vec![],
        })
        .expect_err("empty name");
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
    assert_eq!(index.sets().expect("sets").len(), 3);
}
