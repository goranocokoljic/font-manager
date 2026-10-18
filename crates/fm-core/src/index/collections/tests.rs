use std::path::PathBuf;

use fm_api::common::{FontFormat, Health};
use fm_api::library::SourceKind;
use fm_api::search::SearchRequest;
use fm_api::{FamilyId, SourceId};

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

fn create(index: &mut Index, name: &str, parent: Option<CollectionId>) -> Collection {
    index
        .create_collection(&CreateCollectionRequest {
            name: name.into(),
            parent,
            rules: None,
        })
        .expect("create")
}

fn move_to(
    index: &mut Index,
    c: CollectionId,
    parent: Option<CollectionId>,
    position: u32,
) -> FmResult<Collection> {
    index.move_collection(&MoveCollectionRequest {
        collection: c,
        parent,
        position,
    })
}

/// The tree as `name` or `parent/name`, in list order.
fn tree(index: &Index) -> Vec<String> {
    let all = index.collections().expect("list");
    all.iter()
        .map(|c| match c.parent {
            None => c.name.clone(),
            Some(p) => {
                let parent = all.iter().find(|x| x.id == p).expect("parent listed first");
                format!("{}/{}", parent.name, c.name)
            }
        })
        .collect()
}

fn add(
    index: &mut Index,
    c: CollectionId,
    faces: Vec<FaceId>,
    families: Vec<FamilyId>,
) -> FmResult<Collection> {
    index.add_to_collection(&CollectionFacesRequest {
        collection: c,
        faces,
        families,
    })
}

#[test]
fn create_rename_nest_reorder_and_delete() {
    let (mut index, _) = library();
    let clients = create(&mut index, "Clients", None);
    let brand = create(&mut index, "Brand", None);
    let acme = create(&mut index, "Acme", Some(clients.id));
    assert_eq!(acme.parent, Some(clients.id));
    assert_eq!(tree(&index), ["Clients", "Clients/Acme", "Brand"]);

    let renamed = index
        .rename_collection(&RenameCollectionRequest {
            collection: brand.id,
            name: "  Marque déposée  ".into(),
        })
        .expect("rename");
    assert_eq!(renamed.name, "Marque déposée");

    // Reorder at the top level.
    move_to(&mut index, renamed.id, None, 0).expect("reorder");
    assert_eq!(tree(&index), ["Marque déposée", "Clients", "Clients/Acme"]);

    // Nest it, last among the children (a position past the end).
    let nested = move_to(&mut index, renamed.id, Some(clients.id), 99).expect("nest");
    assert_eq!(nested.parent, Some(clients.id));
    assert_eq!(
        tree(&index),
        ["Clients", "Clients/Acme", "Clients/Marque déposée"]
    );
    move_to(&mut index, renamed.id, Some(clients.id), 0).expect("first child");
    assert_eq!(
        tree(&index),
        ["Clients", "Clients/Marque déposée", "Clients/Acme"]
    );

    // Back to the top level, after Clients.
    move_to(&mut index, acme.id, None, 1).expect("un-nest");
    assert_eq!(tree(&index), ["Clients", "Clients/Marque déposée", "Acme"]);

    // A new collection goes last among its siblings.
    create(&mut index, "Zed", None);
    assert_eq!(
        tree(&index),
        ["Clients", "Clients/Marque déposée", "Acme", "Zed"]
    );

    // Deleting a parent deletes its children.
    index.delete_collection(clients.id).expect("delete");
    assert_eq!(tree(&index), ["Acme", "Zed"]);
    let gone = index.delete_collection(clients.id).unwrap_err();
    assert_eq!(gone.code(), ErrorCode::NotFound);
}

#[test]
fn names_are_required_and_unique_among_siblings() {
    let (mut index, _) = library();
    let a = create(&mut index, "A", None);
    let b = create(&mut index, "B", None);
    let empty = index
        .create_collection(&CreateCollectionRequest {
            name: "   ".into(),
            parent: None,
            rules: None,
        })
        .unwrap_err();
    assert_eq!(empty.code(), ErrorCode::InvalidRequest);
    let twice = index
        .create_collection(&CreateCollectionRequest {
            name: " A ".into(),
            parent: None,
            rules: None,
        })
        .unwrap_err();
    assert_eq!(twice.code(), ErrorCode::Conflict);
    let clash = index
        .rename_collection(&RenameCollectionRequest {
            collection: b.id,
            name: "A".into(),
        })
        .unwrap_err();
    assert_eq!(clash.code(), ErrorCode::Conflict);
    // Renaming to its own name is no clash; the same name under another parent is fine.
    index
        .rename_collection(&RenameCollectionRequest {
            collection: a.id,
            name: "A".into(),
        })
        .expect("same name");
    let child = create(&mut index, "A", Some(b.id));
    // Moving it next to its namesake is a clash, and nothing moves.
    let moved = move_to(&mut index, child.id, None, 0).unwrap_err();
    assert_eq!(moved.code(), ErrorCode::Conflict);
    assert_eq!(tree(&index), ["A", "B", "B/A"]);

    let orphan = index
        .create_collection(&CreateCollectionRequest {
            name: "C".into(),
            parent: Some(CollectionId(999)),
            rules: None,
        })
        .unwrap_err();
    assert_eq!(orphan.code(), ErrorCode::NotFound);
}

#[test]
fn a_collection_cannot_go_inside_itself() {
    let (mut index, _) = library();
    let a = create(&mut index, "A", None);
    let b = create(&mut index, "B", Some(a.id));
    let c = create(&mut index, "C", Some(b.id));
    for parent in [a.id, c.id] {
        let err = move_to(&mut index, a.id, Some(parent), 0).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidRequest);
    }
    assert_eq!(tree(&index), ["A", "A/B", "B/C"]);
    let unknown = move_to(&mut index, a.id, Some(CollectionId(999)), 0).unwrap_err();
    assert_eq!(unknown.code(), ErrorCode::NotFound);
}

#[test]
fn faces_and_families_are_added_and_a_face_can_be_in_many() {
    let (mut index, _) = library();
    let a = create(&mut index, "A", None);
    let b = create(&mut index, "B", None);
    let roboto = FamilyId(face(&index, "Roboto-Regular").0);
    let inter = face(&index, "Inter-Regular");

    let filled = add(&mut index, a.id, vec![inter], vec![roboto]).expect("add");
    assert_eq!(filled.face_count, 4); // three Roboto styles and Inter
    // Adding again changes nothing; the same face goes into B as well.
    assert_eq!(
        add(&mut index, a.id, vec![inter], vec![])
            .expect("again")
            .face_count,
        4
    );
    assert_eq!(
        add(&mut index, b.id, vec![inter], vec![])
            .expect("b")
            .face_count,
        1
    );

    // The counts the sidebar shows.
    let counts: Vec<(String, u32)> = index
        .collections()
        .expect("list")
        .into_iter()
        .map(|c| (c.name, c.face_count))
        .collect();
    assert_eq!(counts, [("A".to_owned(), 4), ("B".to_owned(), 1)]);

    // Search shows what the collection holds.
    let res = index
        .search(&SearchRequest {
            filters: Filters {
                collection: Some(a.id),
                ..Filters::default()
            },
            ..SearchRequest::default()
        })
        .expect("search");
    let mut names: Vec<_> = res
        .families
        .iter()
        .map(|h| h.family.name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["Inter", "Roboto"]);

    let removed = index
        .remove_from_collection(&CollectionFacesRequest {
            collection: a.id,
            faces: vec![],
            families: vec![roboto],
        })
        .expect("remove");
    assert_eq!(removed.face_count, 1);
    // Deleting a collection leaves the faces and their other collections.
    index.delete_collection(b.id).expect("delete");
    assert_eq!(index.collection(a.id).expect("a").face_count, 1);
    let faces: u32 = index
        .conn
        .query_row("SELECT COUNT(*) FROM face", [], |r| r.get(0))
        .expect("faces");
    assert_eq!(faces, LIBRARY.len() as u32);
}

#[test]
fn unknown_faces_families_and_smart_collections_are_refused() {
    let (mut index, _) = library();
    let a = create(&mut index, "A", None);
    let inter = face(&index, "Inter-Regular");

    // One unknown face refuses the whole request.
    let err = add(&mut index, a.id, vec![inter, FaceId(999)], vec![]).unwrap_err();
    assert_eq!(err.code(), ErrorCode::NotFound);
    assert_eq!(index.collection(a.id).expect("a").face_count, 0);

    // A face id that is not the first face of its family is no family id.
    let bold = face(&index, "Roboto-Bold");
    let err = add(&mut index, a.id, vec![], vec![FamilyId(bold.0)]).unwrap_err();
    assert_eq!(err.code(), ErrorCode::NotFound);

    let err = add(&mut index, CollectionId(999), vec![inter], vec![]).unwrap_err();
    assert_eq!(err.code(), ErrorCode::NotFound);

    let smart = index
        .create_collection(&CreateCollectionRequest {
            name: "Variable".into(),
            parent: None,
            rules: Some(Filters {
                variable: Some(true),
                ..Filters::default()
            }),
        })
        .expect("smart");
    assert_eq!(smart.rules.as_ref().and_then(|r| r.variable), Some(true));
    let err = add(&mut index, smart.id, vec![inter], vec![]).unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
    assert_eq!(
        index.collection(smart.id).expect("smart").rules,
        smart.rules
    );
}

#[test]
fn counts_leave_out_gone_files_and_paused_sources() {
    let (mut index, source) = library();
    let a = create(&mut index, "A", None);
    let roboto = FamilyId(face(&index, "Roboto-Regular").0);
    add(&mut index, a.id, vec![], vec![roboto]).expect("add");
    index
        .conn
        .execute(
            "UPDATE font_file SET gone = 1 WHERE path LIKE '%Roboto-Bold%'",
            [],
        )
        .expect("gone");
    assert_eq!(index.collection(a.id).expect("a").face_count, 2);
    index.set_source_paused(source, true).expect("pause");
    assert_eq!(index.collection(a.id).expect("a").face_count, 0);
    index.set_source_paused(source, false).expect("resume");
    assert_eq!(index.collection(a.id).expect("a").face_count, 2);
}

/// Collections made before positions existed keep their order (by id) and
/// their faces after the migration.
#[test]
fn a_version_5_index_keeps_its_collections() {
    let dir = crate::test_support::TempDir::new("collections-migrate-v5");
    let path = dir.path().join(crate::index::INDEX_FILE);
    {
        let conn = rusqlite::Connection::open(&path).expect("open");
        for sql in &crate::index::schema::MIGRATIONS[..5] {
            conn.execute_batch(sql).expect("old schema");
        }
        conn.execute_batch(
            "PRAGMA user_version = 5;
             INSERT INTO collection (id, name) VALUES (3, 'Third'), (1, 'First');
             INSERT INTO collection (id, parent_id, name) VALUES (2, 3, 'Child');",
        )
        .expect("old collections");
    }
    let mut index = Index::open(&path).expect("open").index;
    assert_eq!(tree(&index), ["First", "Third", "Third/Child"]);
    create(&mut index, "New", None);
    assert_eq!(tree(&index), ["First", "Third", "Third/Child", "New"]);
}
