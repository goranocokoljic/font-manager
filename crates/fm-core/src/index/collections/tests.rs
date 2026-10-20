use std::path::PathBuf;

use fm_api::common::{FontFormat, Health};
use fm_api::library::LicenseType;
use fm_api::library::SourceKind;
use fm_api::organization::{Rule, RuleOp, TagFacesRequest, UntagFacesRequest, UserMetaUpdate};
use fm_api::search::{Filters, SearchRequest};
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

    // An empty group holds every face.
    let all = smart(&mut index, "All", RuleOp::Or, vec![]).expect("all");
    assert_eq!(live(&index, all.id).0, LIBRARY.len() as u32);
}

/// Criterion 3: membership follows the index with no restart.
#[test]
fn smart_collections_follow_index_changes() {
    let (mut index, source) = library();
    let inter = face(&index, "Inter-Regular");
    let tag = index
        .tag_faces(&TagFacesRequest {
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
    let a = smart(&mut index, "A", RuleOp::And, vec![]).expect("a");
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
