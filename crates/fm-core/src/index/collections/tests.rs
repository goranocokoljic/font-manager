use std::path::PathBuf;

use fm_api::common::Slant;
use fm_api::common::{FontFormat, Health};
use fm_api::library::LicenseType;
use fm_api::library::SourceKind;
use fm_api::organization::{Rule, RuleOp, TagFacesRequest, UntagFacesRequest, UserMetaUpdate};
use fm_api::search::{Filters, SearchRequest, WeightRange, WidthRange};
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
            rule_group: None,
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
            rule_group: None,
            name: "   ".into(),
            parent: None,
            rules: None,
        })
        .unwrap_err();
    assert_eq!(empty.code(), ErrorCode::InvalidRequest);
    let twice = index
        .create_collection(&CreateCollectionRequest {
            rule_group: None,
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
            rule_group: None,
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
            rule_group: None,
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

fn smart(index: &mut Index, name: &str, op: RuleOp, rules: Vec<Rule>) -> FmResult<Collection> {
    index.create_collection(&CreateCollectionRequest {
        name: name.into(),
        parent: None,
        rules: None,
        rule_group: Some(RuleGroup { op, rules }),
    })
}

fn leaf(filters: Filters) -> Rule {
    Rule::Filters { filters }
}

/// The faces a search filtered by the collection `c` finds, by PostScript name.
fn searched(index: &Index, c: CollectionId) -> FmResult<Vec<String>> {
    let res = index.search(&SearchRequest {
        filters: Filters {
            collection: Some(c),
            ..Filters::default()
        },
        ..SearchRequest::default()
    })?;
    let mut names: Vec<String> = res
        .families
        .iter()
        .flat_map(|h| &h.faces)
        .map(|f| {
            index
                .conn
                .query_row(
                    "SELECT postscript_name FROM face WHERE id = ?1",
                    [f.0],
                    |r| r.get(0),
                )
                .expect("name")
        })
        .collect();
    names.sort_unstable();
    Ok(names)
}

/// What `list_collections` counts for `c`, and what a search by it finds.
fn live(index: &Index, c: CollectionId) -> (u32, Vec<String>) {
    let count = index
        .collections()
        .expect("list")
        .into_iter()
        .find(|x| x.id == c)
        .expect("listed")
        .face_count;
    (count, searched(index, c).expect("search"))
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

/// E14.2 criterion 1: a rule phase 1 stored as plain filters reads as one
/// AND group of them, and keeps its `rules`.
#[test]
fn a_phase_1_rule_reads_as_one_and_group() {
    let (index, _) = library();
    index
        .conn
        .execute(
            r#"INSERT INTO collection (id, name, is_smart, rules, position)
               VALUES (7, 'Old smart', 1, '{"formats":["ttf"],"variable":false}', 0)"#,
            [],
        )
        .expect("phase-1 row");
    let old = Filters {
        formats: vec![FontFormat::Ttf],
        variable: Some(false),
        ..Filters::default()
    };
    let c = index.collection(CollectionId(7)).expect("listed");
    assert_eq!(c.rules, Some(old.clone()));
    assert_eq!(c.rule_group, Some(RuleGroup::from(old)));
    assert_eq!(c.face_count, LIBRARY.len() as u32);
}

/// Criteria 1, 2 and 4: AND/OR groups, nested groups and a rule that names
/// another smart collection; search and the list agree.
#[test]
fn smart_rules_combine_with_and_or_and_through_other_collections() {
    let (mut index, _) = library();
    let inter = face(&index, "Inter-Regular");
    let bold = face(&index, "Roboto-Bold");
    let tagged = index
        .tag_faces(&TagFacesRequest {
            families: Vec::new(),
            faces: vec![inter],
            tags: vec!["pick".into()],
        })
        .expect("tag");
    let pick = tagged.iter().find(|t| t.name == "pick").expect("tag").id;
    index
        .update_user_meta(&UserMetaUpdate {
            faces: vec![bold],
            rating: Some(5),
            ..UserMetaUpdate::default()
        })
        .expect("rate");
    let manual = create(&mut index, "Manual", None);
    let serif = face(&index, "SourceSerif-Regular");
    add(&mut index, manual.id, vec![serif], vec![]).expect("add");

    let either = smart(
        &mut index,
        "Picks or stars",
        RuleOp::Or,
        vec![
            leaf(Filters {
                tags: vec![pick],
                ..Filters::default()
            }),
            leaf(Filters {
                min_rating: Some(4),
                ..Filters::default()
            }),
        ],
    )
    .expect("or");
    assert_eq!(
        live(&index, either.id),
        (2, names(&["Inter-Regular", "Roboto-Bold"]))
    );
    // Phase-1 clients still see a smart collection by its `rules`.
    assert_eq!(either.rules, Some(Filters::default()));

    // Through another smart collection and a manual one, in a nested group.
    let nested = smart(
        &mut index,
        "Nested",
        RuleOp::Or,
        vec![
            Rule::Group {
                group: RuleGroup {
                    op: RuleOp::And,
                    rules: vec![
                        leaf(Filters {
                            collection: Some(either.id),
                            ..Filters::default()
                        }),
                        leaf(Filters {
                            min_rating: Some(1),
                            ..Filters::default()
                        }),
                    ],
                },
            },
            leaf(Filters {
                collection: Some(manual.id),
                ..Filters::default()
            }),
        ],
    )
    .expect("nested");
    assert_eq!(
        live(&index, nested.id),
        (2, names(&["Roboto-Bold", "SourceSerif-Regular"]))
    );

    // An empty group, stored before E14.1 refused one, holds every face.
    index
        .conn
        .execute(
            r#"INSERT INTO collection (id, name, is_smart, rules, position)
               VALUES (90, 'All', 1, '{"op":"or","rules":[]}', 9)"#,
            [],
        )
        .expect("stored empty group");
    assert_eq!(live(&index, CollectionId(90)).0, LIBRARY.len() as u32);
}

/// Criterion 3: membership follows the index with no restart.
#[test]
fn smart_collections_follow_index_changes() {
    let (mut index, source) = library();
    let inter = face(&index, "Inter-Regular");
    let tag = index
        .tag_faces(&TagFacesRequest {
            families: Vec::new(),
            faces: vec![inter],
            tags: vec!["later".into()],
        })
        .expect("tag")[0]
        .id;
    index
        .untag_faces(&UntagFacesRequest {
            faces: vec![inter],
            tags: vec![tag],
        })
        .expect("untag");
    let only = |index: &mut Index, name: &str, filters: Filters| {
        smart(index, name, RuleOp::And, vec![leaf(filters)])
            .expect("smart")
            .id
    };
    let every = only(&mut index, "Every", Filters::default());
    let tagged = only(
        &mut index,
        "Tagged",
        Filters {
            tags: vec![tag],
            ..Filters::default()
        },
    );
    let rated = only(
        &mut index,
        "Rated",
        Filters {
            min_rating: Some(3),
            ..Filters::default()
        },
    );
    let web = only(
        &mut index,
        "Web",
        Filters {
            license: Some(LicenseType::Web),
            ..Filters::default()
        },
    );
    let active = only(
        &mut index,
        "Active",
        Filters {
            active: Some(true),
            ..Filters::default()
        },
    );
    for c in [tagged, rated, web, active] {
        assert_eq!(live(&index, c), (0, vec![]));
    }
    assert_eq!(live(&index, every).0, 5);

    // A scan adds a face, then its file is gone.
    let mut added = file("Lora", "Regular");
    index
        .persist_files(source, std::slice::from_ref(&added))
        .expect("scan adds");
    assert_eq!(live(&index, every).0, 6);
    added.gone = true;
    index
        .persist_files(source, std::slice::from_ref(&added))
        .expect("scan removes");
    assert_eq!(live(&index, every).0, 5);

    let one = (1, names(&["Inter-Regular"]));
    index
        .tag_faces(&TagFacesRequest {
            families: Vec::new(),
            faces: vec![inter],
            tags: vec!["later".into()],
        })
        .expect("tag");
    assert_eq!(live(&index, tagged), one);
    index
        .update_user_meta(&UserMetaUpdate {
            faces: vec![inter],
            rating: Some(4),
            ..UserMetaUpdate::default()
        })
        .expect("rate");
    assert_eq!(live(&index, rated), one);
    // The declared license type, as the license panel writes it.
    index
        .conn
        .execute(
            "UPDATE user_meta SET license_type = 'web' WHERE face_id = ?1",
            [inter.0],
        )
        .expect("license");
    assert_eq!(live(&index, web), one);
    index
        .conn
        .execute(
            "INSERT INTO activation VALUES (?1, 'session', NULL, '2026-10-20T10:00:00Z')",
            [inter.0],
        )
        .expect("activate");
    assert_eq!(live(&index, active), one);
    index
        .conn
        .execute("DELETE FROM activation", [])
        .expect("deactivate");
    assert_eq!(live(&index, active), (0, vec![]));
}

/// Criterion 1: the date-added dimension, in a rule and as a search filter.
#[test]
fn added_within_days_keeps_recent_faces() {
    let (mut index, _) = library();
    index
        .conn
        .execute(
            "UPDATE face SET added = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-30 days')
             WHERE postscript_name LIKE 'Roboto-%'",
            [],
        )
        .expect("old faces");
    let week = Filters {
        added_within_days: Some(7),
        ..Filters::default()
    };
    let recent =
        smart(&mut index, "Recent", RuleOp::And, vec![leaf(week.clone())]).expect("recent");
    assert_eq!(
        live(&index, recent.id),
        (2, names(&["Inter-Regular", "SourceSerif-Regular"]))
    );
    let total = |filters: Filters| {
        index
            .search(&SearchRequest {
                filters,
                ..SearchRequest::default()
            })
            .expect("search")
            .total
    };
    assert_eq!(total(week), 2);
    assert_eq!(
        total(Filters {
            added_within_days: Some(60),
            ..Filters::default()
        }),
        3
    );
}

/// Criterion 4: a rule that leads back to its own collection is refused.
#[test]
fn a_rule_that_refers_to_itself_is_refused() {
    let (mut index, _) = library();
    let manual = create(&mut index, "Manual", None);
    // Directly: the id the new collection gets.
    let next = CollectionId(manual.id.0 + 1);
    let err = smart(
        &mut index,
        "Itself",
        RuleOp::And,
        vec![leaf(Filters {
            collection: Some(next),
            ..Filters::default()
        })],
    )
    .unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
    assert_eq!(index.collections().expect("list").len(), 1);

    // Through another smart collection: stored rules that loop (no request
    // makes them today; an edited rule could).
    let a = smart(&mut index, "A", RuleOp::And, vec![leaf(Filters::default())]).expect("a");
    let b = smart(
        &mut index,
        "B",
        RuleOp::And,
        vec![leaf(Filters {
            collection: Some(a.id),
            ..Filters::default()
        })],
    )
    .expect("b");
    let loop_to_b = serde_json::to_string(&RuleGroup::from(Filters {
        collection: Some(b.id),
        ..Filters::default()
    }))
    .expect("json");
    index
        .conn
        .execute(
            "UPDATE collection SET rules = ?2 WHERE id = ?1",
            params![a.id.0, loop_to_b],
        )
        .expect("loop");
    for c in [a.id, b.id] {
        assert_eq!(
            searched(&index, c).unwrap_err().code(),
            ErrorCode::InvalidRequest
        );
    }
    // The list still answers: a looping rule holds nothing.
    let list = index.collections().expect("list");
    assert!(
        list.iter()
            .filter(|c| c.id == a.id || c.id == b.id)
            .all(|c| c.face_count == 0)
    );
    let err = smart(
        &mut index,
        "Via the loop",
        RuleOp::And,
        vec![leaf(Filters {
            collection: Some(a.id),
            ..Filters::default()
        })],
    )
    .unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
}

#[test]
fn bad_rules_are_refused_and_nothing_is_kept() {
    let (mut index, _) = library();
    let unknown = smart(
        &mut index,
        "Unknown",
        RuleOp::And,
        vec![leaf(Filters {
            collection: Some(CollectionId(99)),
            ..Filters::default()
        })],
    )
    .unwrap_err();
    assert_eq!(unknown.code(), ErrorCode::NotFound);
    let language = smart(
        &mut index,
        "Klingon",
        RuleOp::And,
        vec![leaf(Filters {
            supports: Some(fm_api::search::Supports::Languages {
                codes: vec!["tlh".into()],
            }),
            ..Filters::default()
        })],
    )
    .unwrap_err();
    assert_eq!(language.code(), ErrorCode::InvalidRequest);
    let both = index
        .create_collection(&CreateCollectionRequest {
            name: "Both".into(),
            parent: None,
            rules: Some(Filters::default()),
            rule_group: Some(RuleGroup::from(Filters::default())),
        })
        .unwrap_err();
    assert_eq!(both.code(), ErrorCode::InvalidRequest);
    assert!(index.collections().expect("list").is_empty());
}

/// E14.1 criterion 4: an empty group (at the top or nested) and a weight or
/// width range whose minimum is above its maximum are `InvalidRequest` with
/// a message for the user, and nothing is created.
#[test]
fn empty_groups_and_upside_down_ranges_are_refused_on_create() {
    let (mut index, _) = library();
    let refused = |index: &mut Index, op: RuleOp, rules: Vec<Rule>| {
        let err = smart(index, "Bad", op, rules).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidRequest, "{err:?}");
        err.to_string()
    };
    let top = refused(&mut index, RuleOp::And, vec![]);
    assert!(top.contains("at least one rule"), "{top}");
    let nested = refused(
        &mut index,
        RuleOp::And,
        vec![
            leaf(Filters::default()),
            Rule::Group {
                group: RuleGroup {
                    op: RuleOp::Or,
                    rules: vec![],
                },
            },
        ],
    );
    assert!(nested.contains("at least one rule"), "{nested}");
    let weight = refused(
        &mut index,
        RuleOp::And,
        vec![leaf(Filters {
            weight: Some(WeightRange { min: 700, max: 300 }),
            ..Filters::default()
        })],
    );
    assert!(weight.contains("weight"), "{weight}");
    let width = refused(
        &mut index,
        RuleOp::Or,
        vec![leaf(Filters {
            width: Some(WidthRange { min: 7, max: 2 }),
            ..Filters::default()
        })],
    );
    assert!(width.contains("width"), "{width}");
    // Phase-1 plain rules go through the same check.
    let plain = index
        .create_collection(&CreateCollectionRequest {
            name: "Plain".into(),
            parent: None,
            rules: Some(Filters {
                weight: Some(WeightRange { min: 900, max: 100 }),
                ..Filters::default()
            }),
            rule_group: None,
        })
        .unwrap_err();
    assert_eq!(plain.code(), ErrorCode::InvalidRequest);
    assert!(index.collections().expect("list").is_empty());
}

/// E14.1 criterion 4: `update_collection_rules` replaces a smart
/// collection's rule and the count follows; a refused rule, a loop, a
/// manual collection or an unknown one leaves everything as it was.
#[test]
fn update_collection_rules_replaces_the_rule_or_keeps_the_old_one() {
    let (mut index, _) = library();
    let italic_rule = RuleGroup::from(Filters {
        slant: Some(Slant::Italic),
        ..Filters::default()
    });
    let c = smart(
        &mut index,
        "Italic",
        italic_rule.op,
        italic_rule.rules.clone(),
    )
    .expect("smart");
    assert_eq!(live(&index, c.id), (1, names(&["Roboto-Italic"])));

    let ttf = RuleGroup {
        op: RuleOp::Or,
        rules: vec![leaf(Filters {
            formats: vec![FontFormat::Ttf],
            ..Filters::default()
        })],
    };
    let updated = index
        .update_collection_rules(&UpdateCollectionRulesRequest {
            collection: c.id,
            rule_group: ttf.clone(),
        })
        .expect("update");
    assert_eq!(updated.rule_group, Some(ttf));
    assert_eq!(updated.name, "Italic");
    assert_eq!(live(&index, c.id).0, LIBRARY.len() as u32);

    index
        .update_collection_rules(&UpdateCollectionRulesRequest {
            collection: c.id,
            rule_group: italic_rule.clone(),
        })
        .expect("back to italic");
    let refused = [
        RuleGroup {
            op: RuleOp::And,
            rules: vec![],
        },
        RuleGroup::from(Filters {
            weight: Some(WeightRange { min: 800, max: 200 }),
            ..Filters::default()
        }),
        RuleGroup::from(Filters {
            collection: Some(c.id),
            ..Filters::default()
        }),
    ];
    for rule_group in refused {
        let err = index
            .update_collection_rules(&UpdateCollectionRulesRequest {
                collection: c.id,
                rule_group: rule_group.clone(),
            })
            .unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidRequest, "{rule_group:?}");
        let kept = index.collection(c.id).expect("listed");
        assert_eq!(
            kept.rule_group.as_ref(),
            Some(&italic_rule),
            "{rule_group:?}"
        );
        assert_eq!(live(&index, c.id), (1, names(&["Roboto-Italic"])));
    }

    let manual = create(&mut index, "Manual", None);
    let err = index
        .update_collection_rules(&UpdateCollectionRulesRequest {
            collection: manual.id,
            rule_group: RuleGroup::from(Filters::default()),
        })
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
    assert_eq!(
        index.collection(manual.id).expect("manual").rule_group,
        None
    );
    let err = index
        .update_collection_rules(&UpdateCollectionRulesRequest {
            collection: CollectionId(99),
            rule_group: RuleGroup::from(Filters::default()),
        })
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::NotFound);
}

/// E14.1 criterion 3: a search with a draft rule finds what a smart
/// collection with that rule holds, and counts filter values inside it.
#[test]
fn a_draft_rule_searches_like_the_saved_collection() {
    let (mut index, _) = library();
    let rule = RuleGroup {
        op: RuleOp::And,
        rules: vec![
            leaf(Filters {
                formats: vec![FontFormat::Ttf],
                ..Filters::default()
            }),
            Rule::Group {
                group: RuleGroup {
                    op: RuleOp::Or,
                    rules: vec![
                        leaf(Filters {
                            slant: Some(Slant::Italic),
                            ..Filters::default()
                        }),
                        leaf(Filters {
                            foundry: Some("Nobody".into()),
                            ..Filters::default()
                        }),
                    ],
                },
            },
        ],
    };
    let draft = index
        .search(&SearchRequest {
            rule: Some(rule.clone()),
            ..SearchRequest::default()
        })
        .expect("draft");
    let saved = smart(&mut index, "Saved", rule.op, rule.rules.clone()).expect("saved");
    let (count, faces) = live(&index, saved.id);
    let draft_faces: usize = draft.families.iter().map(|h| h.faces.len()).sum();
    assert_eq!(draft_faces, count as usize);
    assert_eq!(faces, names(&["Roboto-Italic"]));
    assert_eq!(draft.total, 1);
    let slants = draft.counts.expect("counts").slant;
    assert_eq!(slants.len(), 1, "{slants:?}");
    assert_eq!((slants[0].value, slants[0].count), (Slant::Italic, 1));
    // Without a draft rule, every family.
    let all = index.search(&SearchRequest::default()).expect("all");
    assert_eq!(all.total, 3);
}

/// The default smart collections in sidebar order.
const DEFAULTS: [&str; 6] = [
    "Recently added",
    "Recently activated",
    "Variable",
    "Monospace",
    "Web-licensed",
    "Problems",
];

#[test]
fn a_new_library_gets_the_six_defaults_as_plain_rules() {
    let (mut index, _) = library();
    assert!(index.create_default_collections().expect("defaults"));
    let all = index.collections().expect("list");
    assert_eq!(tree(&index), DEFAULTS);
    let rule = |name: &str| {
        all.iter()
            .find(|c| c.name == name)
            .and_then(|c| c.rule_group.as_ref())
            .and_then(|g| g.plain_filters().cloned())
            .unwrap_or_else(|| panic!("{name} is not one plain rule"))
    };
    let want = |set: fn(&mut Filters)| {
        let mut f = Filters::default();
        set(&mut f);
        f
    };
    assert_eq!(
        rule("Recently added"),
        want(|f| f.added_within_days = Some(30))
    );
    assert_eq!(
        rule("Recently activated"),
        want(|f| f.activated_within_days = Some(30))
    );
    assert_eq!(rule("Variable"), want(|f| f.variable = Some(true)));
    assert_eq!(
        rule("Monospace"),
        want(|f| f.classification = vec![Classification::Mono])
    );
    assert_eq!(rule("Web-licensed"), want(|f| f.web_ok = Some(true)));
    assert_eq!(rule("Problems"), want(|f| f.problems = Some(true)));
}

#[test]
fn defaults_are_made_once_and_a_renamed_or_deleted_one_stays_so() {
    let dir = crate::test_support::TempDir::new("collections-defaults-once");
    let path = dir.path().join(crate::index::INDEX_FILE);
    {
        let mut index = Index::open(&path).expect("open").index;
        assert!(index.create_default_collections().expect("defaults"));
        let all = index.collections().expect("list");
        let id = |name: &str| all.iter().find(|c| c.name == name).expect(name).id;
        index
            .rename_collection(&RenameCollectionRequest {
                collection: id("Variable"),
                name: "Variable fonts".into(),
            })
            .expect("rename");
        index.delete_collection(id("Problems")).expect("delete");
    }
    let want = [
        "Recently added",
        "Recently activated",
        "Variable fonts",
        "Monospace",
        "Web-licensed",
    ];
    // The next start.
    let mut index = Index::open(&path).expect("reopen").index;
    assert!(!index.create_default_collections().expect("defaults"));
    assert_eq!(tree(&index), want);
    // An index rebuild.
    index.rebuild().expect("rebuild");
    assert!(!index.create_default_collections().expect("defaults"));
    assert_eq!(tree(&index), want);
}

#[test]
fn a_default_whose_name_is_taken_is_skipped() {
    let (mut index, _) = library();
    create(&mut index, "Variable", None);
    assert!(index.create_default_collections().expect("defaults"));
    let all = index.collections().expect("list");
    assert_eq!(all.len(), 6);
    let variable: Vec<_> = all.iter().filter(|c| c.name == "Variable").collect();
    assert_eq!(variable.len(), 1);
    assert!(
        variable[0].rule_group.is_none(),
        "the user's own collection stays"
    );
    assert_eq!(all[0].name, "Variable");
}

/// Criterion 4 (E14.3): on `fixtures/fonts`, each default holds exactly the
/// faces worked out here from the parsed data and the times set below.
#[test]
fn on_the_fixtures_each_default_holds_exactly_its_faces() {
    use std::collections::{BTreeSet, HashMap};

    use fm_api::activation::ActivationMode;
    use fm_platform::{Mode, Registration};

    use crate::license;
    use crate::scanner::{discover, hash_and_parse};

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts");
    let found = discover(&root).expect("discover");
    let files: Vec<IndexedFile> = found
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
        .collect();
    assert!(files.len() >= 150, "the fixture corpus: {}", files.len());

    let mut index = Index::open_in_memory().expect("open");
    let source = index
        .add_source(&root.to_string_lossy(), SourceKind::Watched)
        .expect("source")
        .id;
    index.persist_files(source, &files).expect("persist");
    let mut parsed: HashMap<(String, u32), &ParsedFace> = HashMap::new();
    for file in &files {
        for face in &file.parsed.faces {
            parsed.insert((file.path.to_string_lossy().into_owned(), face.index), face);
        }
    }
    let faces: Vec<(FaceId, &ParsedFace)> = {
        let mut stmt = index
            .conn
            .prepare(
                "SELECT face.id, font_file.path, face.collection_index
                 FROM face JOIN font_file ON font_file.id = face.file_id ORDER BY face.id",
            )
            .expect("prepare");
        stmt.query_map([], |r| {
            Ok((
                FaceId(r.get(0)?),
                r.get::<_, String>(1)?,
                r.get::<_, u32>(2)?,
            ))
        })
        .expect("query")
        .map(|row| {
            let (id, path, at) = row.expect("row");
            (id, *parsed.get(&(path, at)).expect("parsed face"))
        })
        .collect()
    };
    assert_eq!(faces.len(), parsed.len());

    // Times: every fourth face was added 40 days ago; faces 0 and 1 are
    // activated now and face 1 is deactivated again; face 2 was last
    // activated 40 days ago.
    let ids: Vec<FaceId> = faces.iter().map(|(id, _)| *id).collect();
    for id in ids.iter().step_by(4) {
        index
            .conn
            .execute(
                "UPDATE face SET added = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-40 days')
                 WHERE id = ?1",
                [id.0],
            )
            .expect("old face");
    }
    let reg = |i: usize| Registration {
        path: files[0].path.clone(),
        mode: Mode::Session,
        handle: format!("h{i}").into(),
    };
    index
        .record_activations(&[
            (ids[0], ActivationMode::Session, reg(0)),
            (ids[1], ActivationMode::Session, reg(1)),
        ])
        .expect("activate");
    index.remove_activations(&[ids[1]]).expect("deactivate");
    index
        .conn
        .execute(
            "UPDATE face SET last_activated = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-40 days')
             WHERE id = ?1",
            [ids[2].0],
        )
        .expect("long ago");

    let expect = |keep: &dyn Fn(usize, &ParsedFace) -> bool| -> BTreeSet<FaceId> {
        faces
            .iter()
            .enumerate()
            .filter(|(i, (_, face))| keep(*i, face))
            .map(|(_, (id, _))| *id)
            .collect()
    };
    let want: Vec<(&str, BTreeSet<FaceId>)> = vec![
        ("Recently added", expect(&|i, _| i % 4 != 0)),
        ("Recently activated", expect(&|i, _| i < 2)),
        ("Variable", expect(&|_, f| !f.axes.is_empty())),
        (
            "Monospace",
            expect(&|_, f| {
                f.classification
                    .is_some_and(|c| c.class == Classification::Mono)
            }),
        ),
        (
            "Web-licensed",
            expect(&|_, f| {
                license::web_use(
                    license::embedding(f.fs_type.unwrap_or(0)),
                    license::open_license(f.license.as_deref(), f.license_url.as_deref()),
                    None,
                )
                .ok()
            }),
        ),
        ("Problems", expect(&|_, f| f.health != Health::Ok)),
    ];

    assert!(index.create_default_collections().expect("defaults"));
    let all = index.collections().expect("list");
    for (name, want) in want {
        assert!(!want.is_empty(), "{name}: the fixtures hold no such face");
        let c = all.iter().find(|c| c.name == name).expect(name);
        let res = index
            .search(&SearchRequest {
                filters: Filters {
                    collection: Some(c.id),
                    ..Filters::default()
                },
                page: fm_api::common::Page {
                    offset: 0,
                    limit: 10_000,
                },
                ..SearchRequest::default()
            })
            .expect("search");
        let got: BTreeSet<FaceId> = res
            .families
            .iter()
            .flat_map(|f| f.faces.iter().copied())
            .collect();
        assert_eq!(got, want, "{name}");
        assert_eq!(c.face_count as usize, want.len(), "{name} count");
    }
}
