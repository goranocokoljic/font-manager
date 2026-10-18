use std::fs;

use fm_api::library::SourceKind;
use fm_api::{ErrorCode, SourceId};
use rusqlite::params;

use super::*;
use crate::test_support::TempDir;

fn db_in(dir: &TempDir) -> PathBuf {
    // A non-ASCII folder name: the index must not care.
    dir.path().join("Bibliothèque").join(INDEX_FILE)
}

fn open(path: &Path) -> Opened {
    Index::open(path).expect("open index")
}

fn count(index: &Index, table: &str) -> u32 {
    index
        .conn
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .expect("count")
}

/// Adds one file with one face under `source`; returns the face id.
fn seed_face(index: &Index, source: SourceId, file: &str) -> i64 {
    index
        .conn
        .execute(
            "INSERT INTO font_file (source_id, path, hash, format, size, mtime, health)
             VALUES (?1, ?2, x'00ff', 'otf', 1234, 1700000000000, '{\"status\":\"ok\"}')",
            params![source.0, file],
        )
        .expect("file");
    let file_id = index.conn.last_insert_rowid();
    index
        .conn
        .execute(
            "INSERT INTO face (file_id, family, subfamily, postscript_name, weight, width, slant)
             VALUES (?1, 'Seed Sans', 'Regular', 'SeedSans-Regular', 400, 5, 'upright')",
            [file_id],
        )
        .expect("face");
    index.conn.last_insert_rowid()
}

#[test]
fn fresh_database_has_every_table_of_the_data_model() {
    let dir = TempDir::new("index-fresh");
    let path = db_in(&dir);
    let opened = open(&path);
    assert!(opened.rebuilt.is_none());
    assert!(path.is_file());
    let index = opened.index;
    assert_eq!(index.schema_version().expect("version"), SCHEMA_VERSION);

    let mut stmt = index
        .conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .expect("prepare");
    let tables: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows");
    assert_eq!(tables, schema::TABLES);

    // The columns the spec names for the three core tables.
    for (table, columns) in [
        ("source", &["id", "path", "kind", "paused"][..]),
        (
            "font_file",
            &[
                "id",
                "source_id",
                "path",
                "hash",
                "format",
                "size",
                "mtime",
                "health",
                "gone",
            ][..],
        ),
        (
            "face",
            &[
                "id",
                "file_id",
                "collection_index",
                "family",
                "subfamily",
                "typographic_family",
                "postscript_name",
                "weight",
                "width",
                "slant",
                "is_variable",
                "axes",
                "features",
                "scripts",
                "metrics",
                "fstype",
                "parsed",
                "added",
                "foundry",
                "designer",
                "is_color",
                "healthy",
                "cmap",
                "languages",
            ][..],
        ),
        (
            "user_meta",
            &[
                "face_id",
                "rating",
                "favorite",
                "license_type",
                "seats",
                "notes",
            ][..],
        ),
        ("activation", &["face_id", "mode", "document", "since"][..]),
        (
            "collection",
            &["id", "parent_id", "name", "is_smart", "rules"][..],
        ),
    ] {
        let mut stmt = index
            .conn
            .prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))
            .expect("prepare");
        let found: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("rows");
        assert_eq!(found, columns, "columns of {table}");
    }

    let fk: bool = index
        .conn
        .pragma_query_value(None, "foreign_keys", |r| r.get(0))
        .expect("pragma");
    assert!(fk, "foreign keys are enforced");
}

#[test]
fn reopening_keeps_the_data_and_runs_no_migration_twice() {
    let dir = TempDir::new("index-reopen");
    let path = db_in(&dir);
    let mut index = open(&path).index;
    let added = index
        .add_source("/fonts/work", SourceKind::Watched)
        .expect("add");
    drop(index);

    let opened = open(&path);
    assert!(opened.rebuilt.is_none());
    assert_eq!(
        opened.index.schema_version().expect("version"),
        SCHEMA_VERSION
    );
    assert_eq!(opened.index.sources().expect("sources"), vec![added]);
}

#[test]
fn an_index_from_a_newer_app_is_refused_and_left_untouched() {
    let dir = TempDir::new("index-newer");
    let path = db_in(&dir);
    let index = open(&path).index;
    index
        .conn
        .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .expect("bump");
    drop(index);
    let before = fs::read(&path).expect("read");

    let err = Index::open(&path).expect_err("newer schema");
    assert_eq!(err.code(), ErrorCode::Unsupported);
    assert_eq!(fs::read(&path).expect("read"), before);
    assert!(!path.with_extension("db.corrupt").exists());
}

#[test]
fn a_file_that_is_not_a_database_is_rebuilt_with_the_sources_restored() {
    let dir = TempDir::new("index-garbage");
    let path = db_in(&dir);
    let mut index = open(&path).index;
    let work = index
        .add_source("/fonts/work", SourceKind::Watched)
        .expect("add");
    let lib = index
        .add_source("/fonts/library", SourceKind::LibraryFolder)
        .expect("add");
    let lib = index.set_source_paused(lib.id, true).expect("pause");
    let gone = index
        .add_source("/fonts/old", SourceKind::Watched)
        .expect("add");
    index.remove_source(gone.id).expect("remove");
    seed_face(&index, work.id, "/fonts/work/A.otf");
    drop(index);

    let garbage = b"this is not an SQLite database, it is a damaged file".repeat(200);
    fs::write(&path, &garbage).expect("damage");

    let opened = open(&path);
    let rebuilt = opened.rebuilt.expect("rebuilt");
    let moved = rebuilt.moved_to.expect("moved aside");
    assert_eq!(moved, path.with_extension("db.corrupt"));
    assert_eq!(
        fs::read(&moved).expect("read"),
        garbage,
        "kept for diagnosis"
    );

    // The faces come back with the next scan; the sources come back now, with
    // their ids, and the removed one stays removed.
    let restored = vec![
        Source {
            face_count: 0,
            ..work
        },
        lib,
    ];
    assert_eq!(rebuilt.sources, restored);
    assert_eq!(opened.index.sources().expect("sources"), restored);
    assert_eq!(count(&opened.index, "face"), 0);
    assert_eq!(
        opened.index.schema_version().expect("version"),
        SCHEMA_VERSION
    );
}

#[test]
fn damaged_pages_are_rebuilt_not_a_crash() {
    let dir = TempDir::new("index-pages");
    let path = db_in(&dir);
    let mut index = open(&path).index;
    let source = index
        .add_source("/fonts/work", SourceKind::Watched)
        .expect("add");
    for i in 0..2000 {
        seed_face(&index, source.id, &format!("/fonts/work/Font-{i:04}.otf"));
    }
    drop(index); // closing checkpoints the WAL into the file

    // Keep the 100-byte header valid; overwrite the pages after the first two.
    let mut bytes = fs::read(&path).expect("read");
    assert!(bytes.len() > 64 * 1024, "the seed spans many pages");
    for b in &mut bytes[8192..48 * 1024] {
        *b = 0xA5;
    }
    fs::write(&path, &bytes).expect("damage");

    let opened = open(&path);
    let rebuilt = opened.rebuilt.expect("rebuilt");
    assert!(!rebuilt.reason.is_empty());
    assert_eq!(rebuilt.sources.len(), 1);
    assert_eq!(rebuilt.sources[0].id, source.id);
    assert_eq!(count(&opened.index, "font_file"), 0);
}

#[test]
fn an_unreadable_backup_still_opens_an_empty_index() {
    let dir = TempDir::new("index-bad-backup");
    let path = db_in(&dir);
    let mut index = open(&path).index;
    index
        .add_source("/fonts/work", SourceKind::Watched)
        .expect("add");
    drop(index);
    fs::write(&path, b"garbage").expect("damage db");
    fs::write(path.with_file_name(sources::BACKUP_FILE), b"{ not json").expect("damage backup");

    let opened = open(&path);
    assert_eq!(opened.rebuilt.expect("rebuilt").sources, vec![]);
    assert_eq!(opened.index.sources().expect("sources"), vec![]);
}

#[test]
fn rebuild_forces_a_reread_and_keeps_ids_and_user_data() {
    let mut index = Index::open_in_memory().expect("open");
    let source = index
        .add_source("/fonts/work", SourceKind::Watched)
        .expect("add");
    let face = seed_face(&index, source.id, "/fonts/work/A.otf");
    index
        .conn
        .execute_batch(&format!(
            "INSERT INTO tag (id, name) VALUES (1, 'display');
             INSERT INTO face_tag VALUES ({face}, 1);
             INSERT INTO collection (id, name) VALUES (1, 'Brand');
             INSERT INTO collection_face VALUES (1, {face});
             INSERT INTO project_set (id, name) VALUES (1, 'Client');
             INSERT INTO set_face VALUES (1, {face});
             INSERT INTO user_meta (face_id, rating, favorite) VALUES ({face}, 4, 1);
             INSERT INTO activation VALUES ({face}, 'session', NULL, '2026-10-17T10:00:00Z');"
        ))
        .expect("user data");

    let sources = index.rebuild().expect("rebuild");
    assert_eq!(sources, index.sources().expect("sources"));
    assert_eq!(sources[0].face_count, 1);

    let (hash, mtime): (Option<Vec<u8>>, Option<i64>) = index
        .conn
        .query_row("SELECT hash, mtime FROM font_file", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .expect("stamp");
    assert_eq!((hash, mtime), (None, None), "every file is read again");

    let kept: i64 = index
        .conn
        .query_row("SELECT id FROM face", [], |r| r.get(0))
        .expect("face");
    assert_eq!(kept, face, "face ids stay");
    for table in [
        "face_tag",
        "collection_face",
        "set_face",
        "user_meta",
        "activation",
    ] {
        assert_eq!(count(&index, table), 1, "{table} survives a rebuild");
    }
}

#[test]
fn a_source_with_an_active_face_cannot_be_removed() {
    let mut index = Index::open_in_memory().expect("open");
    let source = index
        .add_source("/fonts/work", SourceKind::Watched)
        .expect("add");
    let face = seed_face(&index, source.id, "/fonts/work/A.otf");
    index
        .conn
        .execute(
            "INSERT INTO activation VALUES (?1, 'permanent', NULL, '2026-10-17T10:00:00Z')",
            [face],
        )
        .expect("activate");

    let err = index.remove_source(source.id).expect_err("active face");
    assert_eq!(err.code(), ErrorCode::Conflict);
    assert_eq!(count(&index, "face"), 1);

    index
        .conn
        .execute("DELETE FROM activation", [])
        .expect("deactivate");
    index.remove_source(source.id).expect("remove");
    assert_eq!(count(&index, "font_file"), 0, "files go with the source");
    assert_eq!(count(&index, "face"), 0, "faces go with their file");
}

#[test]
fn source_changes_validate_and_mirror_to_the_backup() {
    let dir = TempDir::new("index-sources");
    let path = db_in(&dir);
    let mut index = open(&path).index;

    assert_eq!(
        index
            .add_source("  ", SourceKind::Watched)
            .expect_err("empty")
            .code(),
        ErrorCode::InvalidRequest
    );
    let work = index
        .add_source("/fonts/work", SourceKind::Watched)
        .expect("add");
    assert_eq!(
        index
            .add_source("/fonts/work", SourceKind::Watched)
            .expect_err("duplicate")
            .code(),
        ErrorCode::Conflict
    );
    assert_eq!(
        index
            .remove_source(SourceId(99))
            .expect_err("unknown")
            .code(),
        ErrorCode::NotFound
    );
    assert_eq!(
        index
            .set_source_paused(SourceId(99), true)
            .expect_err("unknown")
            .code(),
        ErrorCode::NotFound
    );

    let backup: serde_json::Value = serde_json::from_slice(
        &fs::read(path.with_file_name(sources::BACKUP_FILE)).expect("backup"),
    )
    .expect("json");
    assert_eq!(
        backup,
        serde_json::json!([
            { "id": work.id.0, "path": "/fonts/work", "kind": "watched", "paused": false }
        ])
    );
}
