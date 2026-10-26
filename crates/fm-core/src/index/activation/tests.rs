use std::path::PathBuf;

use fm_api::FamilyId;
use fm_api::common::Health;
use fm_api::library::SourceKind;

use super::*;
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, IndexedFile};

fn index_with(paths: &[PathBuf]) -> (Index, Vec<FaceId>) {
    let mut index = Index::open_in_memory().expect("open");
    let source = index
        .add_source("/fonts", SourceKind::Watched)
        .expect("source")
        .id;
    let files: Vec<IndexedFile> = paths
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let mut face = ParsedFace::empty(0, Health::Ok);
            face.family = Some("Family".into());
            face.subfamily = Some(format!("Style {i}"));
            face.postscript_name = Some(format!("Family-{i}"));
            IndexedFile {
                path: path.clone(),
                size: 1,
                modified: None,
                hash: FileHash::from([1; 32]),
                parsed: ParsedFile {
                    format: Some(FontFormat::Ttf),
                    health: Health::Ok,
                    faces: vec![face],
                },
                gone: false,
            }
        })
        .collect();
    index.persist_files(source, &files).expect("persist");
    let ids = index
        .face_ids("SELECT id FROM face WHERE id > ?1 ORDER BY id", 0)
        .expect("ids");
    (index, ids)
}

#[test]
fn a_face_list_keeps_its_order_without_repeats() {
    let (index, ids) = index_with(&[PathBuf::from("/fonts/a.ttf"), PathBuf::from("/fonts/b.ttf")]);
    let faces = index
        .target_faces(&Target::Faces {
            faces: vec![ids[1], ids[0], ids[1]],
        })
        .expect("faces");
    assert_eq!(faces, vec![ids[1], ids[0]]);
    let family = index
        .target_faces(&Target::Family {
            family: FamilyId(ids[0].0),
        })
        .expect("family");
    assert_eq!(family, ids);
}

#[test]
fn registrations_round_trip_with_paths_that_are_not_utf8() {
    #[cfg(windows)]
    let odd = fm_platform::fs::path_from_bytes(&[0x41, 0x00, 0x00, 0xD8]).expect("path");
    #[cfg(unix)]
    let odd = {
        use std::os::unix::ffi::OsStrExt;
        PathBuf::from(std::ffi::OsStr::from_bytes(b"/fonts/\xff.ttf"))
    };
    assert!(odd.to_str().is_none());
    let (mut index, ids) = index_with(&[odd.clone(), PathBuf::from("/fonts/b.ttf")]);
    let reg = Registration {
        path: odd.clone(),
        mode: Mode::Permanent,
        handle: odd.clone().into_os_string(),
    };
    index
        .record_activations(&[(ids[0], ActivationMode::Permanent, reg.clone())])
        .expect("record");
    let rows = index.active_rows().expect("rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].face, ids[0]);
    assert_eq!(rows[0].mode, ActivationMode::Permanent);
    assert_eq!(rows[0].registration, Some(reg));
    let face = index.activation_faces(&[ids[0]]).expect("faces");
    assert_eq!(face[0].path, odd);
}

#[test]
fn removing_an_activation_removes_its_registration_and_last_session_follows_faces() {
    let (mut index, ids) = index_with(&[PathBuf::from("/fonts/a.ttf")]);
    let reg = Registration {
        path: PathBuf::from("/fonts/a.ttf"),
        mode: Mode::Session,
        handle: "h1".into(),
    };
    index
        .record_activations(&[(ids[0], ActivationMode::Session, reg)])
        .expect("record");
    index.set_last_session(&ids).expect("last");
    assert_eq!(index.last_session().expect("last"), ids);
    index.remove_activations(&ids).expect("remove");
    assert_eq!(index.active_rows().expect("rows"), vec![]);
    let left: u32 = index
        .conn
        .query_row("SELECT COUNT(*) FROM activation_registration", [], |r| {
            r.get(0)
        })
        .expect("count");
    assert_eq!(left, 0);
    index.set_last_session(&[]).expect("clear");
    assert_eq!(index.last_session().expect("last"), vec![]);
}

#[test]
fn an_active_face_cannot_leave_the_index() {
    let (mut index, ids) = index_with(&[PathBuf::from("/fonts/a.ttf")]);
    let reg = Registration {
        path: PathBuf::from("/fonts/a.ttf"),
        mode: Mode::Session,
        handle: "h1".into(),
    };
    index
        .record_activations(&[(ids[0], ActivationMode::Session, reg)])
        .expect("record");
    let err = index
        .conn
        .execute("DELETE FROM face WHERE id = ?1", [ids[0].0])
        .expect_err("restricted");
    assert!(err.to_string().contains("FOREIGN KEY"), "{err}");
}

#[test]
fn activatable_faces_leave_out_what_cannot_be_registered() {
    let (index, ids) = index_with(&[
        PathBuf::from("/fonts/a.ttf"),
        PathBuf::from("/fonts/b.ttf"),
        PathBuf::from("/fonts/c.ttf"),
    ]);
    index
        .conn
        .execute_batch(
            "UPDATE font_file SET gone = 1 WHERE path = '/fonts/b.ttf';
             UPDATE font_file SET format = 'woff2' WHERE path = '/fonts/c.ttf';",
        )
        .expect("update");
    assert_eq!(index.activatable_faces(10).expect("faces"), vec![ids[0]]);
    assert_eq!(index.activatable_faces(0).expect("faces"), vec![]);
}

/// When a face was last activated, as `face.last_activated` holds it.
fn last_activated(index: &Index, face: FaceId) -> Option<String> {
    index
        .conn
        .query_row(
            "SELECT last_activated FROM face WHERE id = ?1",
            [face.0],
            |r| r.get(0),
        )
        .expect("last activated")
}

/// The faces a search for "activated within `days`" finds.
fn activated_within(index: &Index, days: u32) -> Vec<FaceId> {
    let res = index
        .search(&fm_api::search::SearchRequest {
            filters: fm_api::search::Filters {
                activated_within_days: Some(days),
                ..Default::default()
            },
            ..Default::default()
        })
        .expect("search");
    let mut faces: Vec<FaceId> = res.families.iter().flat_map(|f| f.faces.clone()).collect();
    faces.sort_unstable();
    faces
}

#[test]
fn the_last_activation_time_outlives_deactivation() {
    let (mut index, ids) = index_with(&[
        PathBuf::from("/fonts/a.ttf"),
        PathBuf::from("/fonts/b.ttf"),
        PathBuf::from("/fonts/c.ttf"),
    ]);
    assert_eq!(last_activated(&index, ids[0]), None);
    let reg = |h: &str| Registration {
        path: PathBuf::from("/fonts/a.ttf"),
        mode: Mode::Session,
        handle: h.into(),
    };
    index
        .record_activations(&[
            (ids[0], ActivationMode::Session, reg("h0")),
            (ids[1], ActivationMode::Session, reg("h1")),
        ])
        .expect("record");
    let since = index.active_rows().expect("rows")[0].since.clone();
    let at = last_activated(&index, ids[0]).expect("recorded");
    // Whole seconds, as `face.added`: the second of `since`.
    assert_eq!(at.len(), "2026-10-21T10:00:00Z".len(), "{at}");
    assert_eq!(at[..19], since[..19]);

    index.remove_activations(&[ids[0]]).expect("deactivate");
    assert_eq!(last_activated(&index, ids[0]), Some(at.clone()));
    assert_eq!(activated_within(&index, 30), vec![ids[0], ids[1]]);

    // Activated 40 days ago: out of a 30-day window, inside a 60-day one.
    index
        .conn
        .execute(
            "UPDATE face SET last_activated = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-40 days')
             WHERE id = ?1",
            [ids[0].0],
        )
        .expect("long ago");
    assert_eq!(activated_within(&index, 30), vec![ids[1]]);
    assert_eq!(activated_within(&index, 60), vec![ids[0], ids[1]]);

    // Activating again moves the time on.
    index
        .record_activations(&[(ids[0], ActivationMode::Permanent, reg("h0"))])
        .expect("again");
    assert_eq!(activated_within(&index, 30), vec![ids[0], ids[1]]);
}

#[test]
fn a_version_13_index_takes_the_time_of_faces_active_now() {
    let dir = crate::test_support::TempDir::new("last-activated-migrate-v13");
    let path = dir.path().join(crate::index::INDEX_FILE);
    {
        let conn = rusqlite::Connection::open(&path).expect("open");
        for sql in &crate::index::schema::MIGRATIONS[..13] {
            conn.execute_batch(sql).expect("old schema");
        }
        conn.execute_batch(
            "PRAGMA user_version = 13;
             INSERT INTO source (id, path, kind) VALUES (1, '/fonts', 'watched');
             INSERT INTO font_file (id, source_id, path, format, size, health)
                 VALUES (1, 1, '/fonts/a.ttf', 'ttf', 1, 'ok');
             INSERT INTO face (id, file_id, collection_index, family, subfamily,
                               postscript_name, weight, width, slant)
                 VALUES (1, 1, 0, 'A', 'Regular', 'A-Regular', 400, 5, 'upright'),
                        (2, 1, 1, 'A', 'Bold', 'A-Bold', 700, 5, 'upright');
             INSERT INTO activation (face_id, mode, since)
                 VALUES (1, 'session', '2026-10-01T08:30:15.250Z');",
        )
        .expect("old rows");
    }
    let index = Index::open(&path).expect("open").index;
    assert_eq!(
        last_activated(&index, FaceId(1)).as_deref(),
        Some("2026-10-01T08:30:15Z")
    );
    assert_eq!(last_activated(&index, FaceId(2)), None);
}

/// E29.3: a face E29.2 activated for a document takes it as its tag; a
/// permanent face loses a document it should never have had.
#[test]
fn a_version_20_index_tags_the_faces_documents_activated() {
    let dir = crate::test_support::TempDir::new("activation-document-migrate-v20");
    let path = dir.path().join(crate::index::INDEX_FILE);
    {
        let conn = rusqlite::Connection::open(&path).expect("open");
        for sql in &crate::index::schema::MIGRATIONS[..20] {
            conn.execute_batch(sql).expect("old schema");
        }
        conn.execute_batch(
            "PRAGMA user_version = 20;
             INSERT INTO source (id, path, kind) VALUES (1, '/fonts', 'watched');
             INSERT INTO font_file (id, source_id, path, format, size, health)
                 VALUES (1, 1, '/fonts/a.ttf', 'ttf', 1, 'ok');
             INSERT INTO face (id, file_id, collection_index, family, subfamily,
                               postscript_name, weight, width, slant)
                 VALUES (1, 1, 0, 'A', 'Regular', 'A-Regular', 400, 5, 'upright'),
                        (2, 1, 1, 'A', 'Bold', 'A-Bold', 700, 5, 'upright'),
                        (3, 1, 2, 'A', 'Black', 'A-Black', 900, 5, 'upright');
             INSERT INTO activation (face_id, mode, document, since)
                 VALUES (1, 'session', 'poster.indd', '2026-10-01T08:30:15.250Z'),
                        (2, 'session', NULL, '2026-10-01T08:30:15.250Z'),
                        (3, 'permanent', 'poster.indd', '2026-10-01T08:30:15.250Z');",
        )
        .expect("old rows");
    }
    let mut index = Index::open(&path).expect("open").index;
    let docs: Vec<(FaceId, Option<String>)> = index
        .active_rows()
        .expect("rows")
        .into_iter()
        .map(|r| (r.face, r.document))
        .collect();
    assert_eq!(
        docs,
        vec![
            (FaceId(1), Some("poster.indd".into())),
            (FaceId(2), None),
            (FaceId(3), None),
        ]
    );
    assert_eq!(
        index.close_document("poster.indd").expect("close"),
        vec![FaceId(1)]
    );
}
