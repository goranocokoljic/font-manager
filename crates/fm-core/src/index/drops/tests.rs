use std::path::PathBuf;

use fm_api::common::{FontFormat, Health};
use fm_api::library::SourceKind;
use fm_api::organization::{
    CollectionFacesRequest, CreateCollectionRequest, Rule, RuleGroup, RuleOp, TagFacesRequest,
};
use fm_api::search::Filters;
use fm_api::{CollectionId, ErrorCode, SourceId, TagId};

use super::*;
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, IndexedFile};

/// One file per face: (family, style).
const LIBRARY: &[(&str, &str)] = &[
    ("Roboto", "Regular"),
    ("Roboto", "Bold"),
    ("Roboto", "Italic"),
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

fn collection(index: &mut Index, name: &str, faces: Vec<FaceId>) -> CollectionId {
    let c = index
        .create_collection(&CreateCollectionRequest {
            name: name.into(),
            parent: None,
            rules: None,
            rule_group: None,
            color: None,
        })
        .expect("create")
        .id;
    if !faces.is_empty() {
        index
            .add_to_collection(&CollectionFacesRequest {
                collection: c,
                faces,
                families: Vec::new(),
            })
            .expect("add");
    }
    c
}

fn tag(index: &mut Index, faces: Vec<FaceId>, name: &str) -> TagId {
    index
        .tag_faces(&TagFacesRequest {
            faces,
            families: Vec::new(),
            tags: vec![name.into()],
        })
        .expect("tag")[0]
        .id
}

fn of_collection(counts: &DropCounts, id: CollectionId) -> Option<(u32, u32)> {
    counts
        .collections
        .iter()
        .find(|c| c.collection == id)
        .map(|c| (c.before, c.after))
}

fn of_tag(counts: &DropCounts, id: TagId) -> Option<(u32, u32)> {
    counts
        .tags
        .iter()
        .find(|t| t.tag == id)
        .map(|t| (t.before, t.after))
}

#[test]
fn a_drop_adds_only_the_faces_a_target_does_not_hold() {
    let (mut index, _) = library();
    let regular = face(&index, "Roboto-Regular");
    let bold = face(&index, "Roboto-Bold");
    let inter = face(&index, "Inter-Regular");
    let serif = face(&index, "SourceSerif-Regular");
    let holds_regular = collection(&mut index, "Editorial", vec![regular, serif]);
    let empty = collection(&mut index, "Empty", Vec::new());
    let display = tag(&mut index, vec![regular, bold], "display");
    let body = tag(&mut index, vec![serif], "body");

    // The whole Roboto family (3 faces) and Inter, with Regular named twice.
    let roboto = FamilyId(regular.0);
    let counts = index
        .drop_counts(&[regular, inter], &[roboto])
        .expect("counts");
    assert_eq!(counts.faces.len(), 4, "every face once: {:?}", counts.faces);
    // Editorial holds Regular already: 2 + (4 − 1).
    assert_eq!(of_collection(&counts, holds_regular), Some((2, 5)));
    assert_eq!(of_collection(&counts, empty), Some((0, 4)));
    // display is on Regular and Bold: 2 + (4 − 2); body on none of them.
    assert_eq!(of_tag(&counts, display), Some((2, 4)));
    assert_eq!(of_tag(&counts, body), Some((1, 5)));
    // Every collection in list order, every tag A to Z.
    let order: Vec<_> = counts.collections.iter().map(|c| c.collection).collect();
    assert_eq!(order, [holds_regular, empty]);
    let tags: Vec<_> = counts.tags.iter().map(|t| t.tag).collect();
    assert_eq!(tags, [body, display]);
}

#[test]
fn faces_a_target_holds_already_add_nothing() {
    let (mut index, _) = library();
    let regular = face(&index, "Roboto-Regular");
    let c = collection(&mut index, "Has it", vec![regular]);
    let t = tag(&mut index, vec![regular], "has");
    let counts = index.drop_counts(&[regular], &[]).expect("counts");
    assert_eq!(of_collection(&counts, c), Some((1, 1)));
    assert_eq!(of_tag(&counts, t), Some((1, 1)));
}

#[test]
fn the_counts_match_what_a_drop_then_lists() {
    let (mut index, _) = library();
    let regular = face(&index, "Roboto-Regular");
    let inter = face(&index, "Inter-Regular");
    let c = collection(&mut index, "Mixed", vec![regular]);
    let counts = index.drop_counts(&[regular, inter], &[]).expect("counts");
    let after = index
        .add_to_collection(&CollectionFacesRequest {
            collection: c,
            faces: vec![regular, inter],
            families: Vec::new(),
        })
        .expect("add")
        .face_count;
    assert_eq!(of_collection(&counts, c), Some((1, after)));
}

#[test]
fn smart_collections_are_not_drop_targets() {
    let (mut index, _) = library();
    let regular = face(&index, "Roboto-Regular");
    let manual = collection(&mut index, "Manual", Vec::new());
    let smart = index
        .create_collection(&CreateCollectionRequest {
            name: "Smart".into(),
            parent: None,
            rules: None,
            rule_group: Some(RuleGroup {
                op: RuleOp::And,
                rules: vec![Rule::Filters {
                    filters: Filters::default(),
                }],
            }),
            color: None,
        })
        .expect("smart")
        .id;
    let counts = index.drop_counts(&[regular], &[]).expect("counts");
    assert!(of_collection(&counts, manual).is_some());
    assert_eq!(of_collection(&counts, smart), None);
}

#[test]
fn a_face_the_library_does_not_show_adds_nothing() {
    let (mut index, source) = library();
    let regular = face(&index, "Roboto-Regular");
    let bold = face(&index, "Roboto-Bold");
    let c = collection(&mut index, "Shown", Vec::new());
    index
        .conn
        .execute(
            "UPDATE font_file SET gone = 1 WHERE path LIKE '%Roboto-Bold%'",
            [],
        )
        .expect("gone");
    let counts = index.drop_counts(&[regular, bold], &[]).expect("counts");
    assert_eq!(of_collection(&counts, c), Some((0, 1)));
    index.set_source_paused(source, true).expect("pause");
    let counts = index.drop_counts(&[regular], &[]).expect("counts");
    assert_eq!(of_collection(&counts, c), Some((0, 0)));
}

#[test]
fn unknown_faces_and_families_are_not_found_and_nothing_is_empty() {
    let (index, _) = library();
    let err = index.drop_counts(&[FaceId(999)], &[]).unwrap_err();
    assert_eq!(err.code(), ErrorCode::NotFound);
    let err = index.drop_counts(&[], &[FamilyId(999)]).unwrap_err();
    assert_eq!(err.code(), ErrorCode::NotFound);
    let counts = index.drop_counts(&[], &[]).expect("empty");
    assert!(counts.faces.is_empty());
}
