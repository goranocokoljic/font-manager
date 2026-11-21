//! The session tables (schema V28): an index from an earlier version opens
//! with its "Keep this one" actions listed as sessions of one set.

use fm_api::cleanup::SessionKind;
use fm_api::library::TrashMoveStatus;

use super::*;
use crate::index::INDEX_FILE;
use crate::test_support::TempDir;

#[test]
fn a_version_27_index_lists_its_resolutions_as_sessions_of_one_set() {
    let dir = TempDir::new("sessions-migrate-v27");
    let path = dir.path().join(INDEX_FILE);
    {
        let conn = rusqlite::Connection::open(&path).expect("open");
        for sql in &crate::index::schema::MIGRATIONS[..27] {
            conn.execute_batch(sql).expect("old schema");
        }
        conn.execute_batch(
            "PRAGMA user_version = 27;
             INSERT INTO resolution (id, at, kept, undone)
                 VALUES (1, '2026-11-01T10:00:00Z', '/fonts/a.ttf', 0);
             INSERT INTO resolution_move
                 (resolution_id, position, path, display, status, message, location, info)
                 VALUES (1, 0, '/fonts/b.ttf', '/fonts/b.ttf', 'trashed', NULL, '/trash/b.ttf', NULL),
                        (1, 1, '/fonts/c.ttf', '/fonts/c.ttf', 'skipped', 'This copy is active.', NULL, NULL);",
        )
        .expect("old rows");
    }
    let index = Index::open(&path).expect("open").index;
    let heads = index.session_heads().expect("heads");
    assert_eq!(heads.len(), 1);
    let head = &heads[0];
    assert_eq!(head.kind, SessionKind::KeepCopy);
    assert_eq!(head.state, "done");
    let moves = index.session_moves(head.id).expect("moves");
    let s = session(head, &moves, false);
    assert_eq!(s.name, "Keep a.ttf");
    assert_eq!(s.at, "2026-11-01T10:00:00Z");
    assert_eq!(s.sets, 1);
    assert_eq!(s.trashed, 1);
    assert!(!s.cancelled && !s.running && !s.undone);
    assert_eq!(s.files.len(), 2);
    assert!(
        s.files
            .iter()
            .all(|f| f.kept == "/fonts/a.ttf" && f.set.is_none())
    );
    assert_eq!(s.files[0].status, TrashMoveStatus::Trashed);
    assert_eq!(s.files[1].message.as_deref(), Some("This copy is active."));
    // The E8.3 history still lists it, unchanged.
    let old = index.resolutions().expect("resolutions");
    assert_eq!(old.len(), 1);
    assert_eq!(old[0].kept, "/fonts/a.ttf");
    assert_eq!(old[0].moves.len(), 2);
}

#[test]
fn a_session_left_running_by_a_crash_reads_as_cancelled() {
    let mut index = Index::open_in_memory().expect("index");
    let (id, _) = index.start_session("Spring cleanup").expect("start");
    let head = index.session_head(id).expect("head").expect("there");
    let s = session(&head, &[], false);
    assert!(s.cancelled && !s.running, "no apply runs: {s:?}");
    let s = session(&head, &[], true);
    assert!(s.running && !s.cancelled);
    index.finish_session(id, false).expect("finish");
    let head = index.session_head(id).expect("head").expect("there");
    assert!(!session(&head, &[], false).cancelled);
    assert_eq!(
        index.resolutions().expect("history").len(),
        0,
        "not a keep-copy action"
    );
}
