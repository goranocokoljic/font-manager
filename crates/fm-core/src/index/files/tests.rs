use std::fs;
use std::time::Instant;

use fm_api::ErrorCode;
use fm_api::common::Slant;
use fm_api::library::SourceKind;
use rusqlite::Connection;

use super::*;
use crate::index::{INDEX_FILE, SCHEMA_VERSION};
use crate::scanner::discover;
use crate::test_support::TempDir;

const STATIC: &str = "google-static/asap/Asap_Condensed-ExtraLightItalic.ttf";
const VARIABLE: &str = "google-repo/asapsharp/AsapSharp[wdth,wght].ttf";
const COLLECTION: &str = "derived/agdasima/agdasima.ttc";
const TRUNCATED: &str = "derived/agdasima/agdasima-truncated.ttf";
const TYPE1: &str = "type1/inconsolata/Inconsolata-zi4r.pfb";
const WOFF: &str = "fontshare/public-sans/PublicSans-Variable.woff";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts")
}

fn put(path: &Path, data: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create folder");
    }
    fs::write(path, data).expect("write font");
}

fn copy_fixture(root: &Path, rel: &str, name: &str) -> PathBuf {
    let to = root.join(name);
    put(&to, &fs::read(fixtures().join(rel)).expect("read fixture"));
    to
}

fn scan(root: &Path) -> (FileIndex, Rescan) {
    let mut index = FileIndex::new();
    let rescan = index.rescan(root, &discover(root).expect("discover"));
    (index, rescan)
}

fn rescan(index: &mut FileIndex, root: &Path) -> Rescan {
    index.rescan(root, &discover(root).expect("discover"))
}

/// An index on disk (WAL, as the app runs it) with one watched source.
fn open_db(dir: &TempDir) -> (Index, SourceId, PathBuf) {
    let path = dir.path().join("data").join(INDEX_FILE);
    let mut index = Index::open(&path).expect("open").index;
    let source = index
        .add_source(
            &dir.path().join("fonts").to_string_lossy(),
            SourceKind::Watched,
        )
        .expect("source")
        .id;
    (index, source, path)
}

fn memory_db() -> (Index, SourceId) {
    let mut index = Index::open_in_memory().expect("open");
    let source = index
        .add_source("/fonts", SourceKind::Watched)
        .expect("source")
        .id;
    (index, source)
}

fn count(conn: &Connection, sql: &str) -> usize {
    conn.query_row(sql, [], |r| r.get::<_, i64>(0))
        .expect("count") as usize
}

/// `n` copies of one parsed fixture under distinct paths.
fn copies(n: usize, rel: &str) -> Vec<IndexedFile> {
    let path = fixtures().join(rel);
    let (hash, parsed) = crate::scanner::hash_and_parse(&path).expect("parse");
    (0..n)
        .map(|i| IndexedFile {
            path: PathBuf::from(format!("/fonts/batch/{i:05}.ttf")),
            size: 1000 + i as u64,
            modified: time_from_nanos(1_700_000_000_000_000_000 + i as i64),
            hash,
            parsed: parsed.clone(),
            gone: false,
        })
        .collect()
}

#[test]
fn persisted_files_load_back_unchanged() {
    let dir = TempDir::new("files-roundtrip");
    let fonts = dir.path().join("fonts");
    copy_fixture(&fonts, STATIC, "static.ttf");
    copy_fixture(&fonts, VARIABLE, "variable.ttf");
    copy_fixture(&fonts, COLLECTION, "collection.ttc");
    copy_fixture(&fonts, TRUNCATED, "truncated.ttf");
    copy_fixture(&fonts, TYPE1, "type1.pfb");
    copy_fixture(&fonts, WOFF, "web.woff");
    put(&fonts.join("junk.otf"), b"not a font at all");
    copy_fixture(&fonts, STATIC, "Schrift-ü/static copy.ttf");
    let (scanned, _) = scan(&fonts);
    assert_eq!(scanned.len(), 8);
    assert!(
        scanned.files().any(|f| f.parsed.faces.len() > 1
            && f.parsed.faces.iter().all(|x| !x.code_points.is_empty())),
        "the collection gives several faces with coverage"
    );

    let (mut index, source, _) = open_db(&dir);
    let done = index
        .persist_files(source, scanned.files())
        .expect("persist");
    let faces: usize = scanned.files().map(|f| f.parsed.faces.len()).sum();
    assert_eq!(
        done,
        Persisted {
            files: 8,
            faces,
            batches: 1
        }
    );

    let loaded = index.load_files().expect("load");
    assert_eq!(loaded, scanned, "what was saved loads back exactly");
    assert_eq!(count(&index.conn, "SELECT COUNT(*) FROM font_file"), 8);
    assert_eq!(count(&index.conn, "SELECT COUNT(*) FROM face"), faces);
    assert_eq!(
        index.sources().expect("sources")[0].face_count as usize,
        faces
    );
}

#[test]
fn the_face_columns_hold_what_the_user_sees() {
    let dir = TempDir::new("files-columns");
    let fonts = dir.path().join("fonts");
    let path = copy_fixture(&fonts, STATIC, "static.ttf");
    let (scanned, _) = scan(&fonts);
    let face = &scanned.get(&path).expect("file").parsed.faces[0];

    let (mut index, source, _) = open_db(&dir);
    index
        .persist_files(source, scanned.files())
        .expect("persist");
    let row: (
        String,
        String,
        String,
        u16,
        u16,
        String,
        bool,
        String,
        String,
    ) = index
        .conn
        .query_row(
            "SELECT f.format, face.family, face.subfamily, face.weight, face.width, face.slant,
                    face.is_variable, face.postscript_name, f.health
             FROM face JOIN font_file f ON f.id = face.file_id",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                ))
            },
        )
        .expect("row");
    let style = face_style(&path, face);
    assert_eq!(row.0, "ttf");
    assert_eq!(row.1, family_name(&path, face));
    assert_eq!(row.2, style.name);
    assert_eq!((row.3, row.4), (style.weight, style.width));
    // Asap Condensed ExtraLight Italic: light, condensed, italic; none of
    // these is the default a missing value would give.
    assert_eq!(Some(row.3), face.weight);
    assert!(row.3 < 400, "light: {}", row.3);
    assert!(row.4 < 5, "condensed: {}", row.4);
    assert_eq!(style.slant, Slant::Italic);
    assert_eq!(row.5, "italic");
    assert!(!row.6);
    assert_eq!(Some(row.7), face.postscript_name.clone());
    assert_eq!(row.8, r#"{"kind":"ok"}"#);
}

#[test]
fn each_batch_of_500_commits_in_its_own_transaction() {
    let dir = TempDir::new("files-batches");
    let (mut index, source, path) = open_db(&dir);
    let files = copies(1_234, STATIC);
    let reader = Connection::open(&path).expect("second connection");

    let mut seen = Vec::new();
    let done = index
        .persist_with(source, &files, &mut |written| {
            // Another connection sees exactly the committed batches.
            seen.push((written, count(&reader, "SELECT COUNT(*) FROM font_file")));
        })
        .expect("persist");

    assert_eq!(BATCH_SIZE, 500);
    assert_eq!(seen, [(500, 500), (1_000, 1_000), (1_234, 1_234)]);
    assert_eq!(done.batches, 3);
    assert_eq!(done.files, 1_234);
    assert_eq!(count(&reader, "SELECT COUNT(*) FROM face"), 1_234);
}

#[test]
fn a_failed_batch_is_rolled_back_whole_and_earlier_batches_stay() {
    let dir = TempDir::new("files-rollback");
    let (mut index, source, path) = open_db(&dir);
    let files = copies(1_234, STATIC);
    // File 700 sits in the second batch (500..1000).
    index
        .conn
        .execute_batch(
            "CREATE TEMP TRIGGER fail_one BEFORE INSERT ON face
             WHEN NEW.file_id = (SELECT id FROM font_file WHERE path = '/fonts/batch/00700.ttf')
             BEGIN SELECT RAISE(ABORT, 'disk said no'); END;",
        )
        .expect("trigger");

    let err = index
        .persist_files(source, &files)
        .expect_err("second batch fails");
    assert_eq!(err.code(), ErrorCode::Io);

    let reader = Connection::open(&path).expect("second connection");
    assert_eq!(count(&reader, "SELECT COUNT(*) FROM font_file"), 500);
    assert_eq!(count(&reader, "SELECT COUNT(*) FROM face"), 500);
    assert_eq!(
        count(
            &reader,
            "SELECT COUNT(*) FROM font_file WHERE path >= '/fonts/batch/00500.ttf'"
        ),
        0,
        "no file of the failed batch is left behind"
    );

    // Writing again completes the job.
    index
        .conn
        .execute_batch("DROP TRIGGER fail_one")
        .expect("drop");
    index.persist_files(source, &files).expect("persist again");
    assert_eq!(count(&reader, "SELECT COUNT(*) FROM font_file"), 1_234);
}

#[test]
fn an_unknown_source_writes_nothing() {
    let (mut index, _) = memory_db();
    let err = index
        .persist_files(SourceId(99), &copies(3, STATIC))
        .expect_err("unknown source");
    assert_eq!(err.code(), ErrorCode::NotFound);
    assert_eq!(count(&index.conn, "SELECT COUNT(*) FROM font_file"), 0);
}

#[test]
fn writing_a_file_again_keeps_its_face_ids_and_user_data() {
    let (mut index, source) = memory_db();
    let mut files = copies(1, STATIC);
    index.persist_files(source, &files).expect("persist");
    let face: i64 = index
        .conn
        .query_row("SELECT id FROM face", [], |r| r.get(0))
        .expect("face");
    index
        .conn
        .execute_batch(&format!(
            "INSERT INTO tag (id, name) VALUES (1, 'display');
             INSERT INTO face_tag VALUES ({face}, 1);
             INSERT INTO user_meta (face_id, rating) VALUES ({face}, 5);"
        ))
        .expect("user data");

    // The file changed on disk: new bytes, new size, new time.
    let (hash, parsed) = crate::scanner::hash_and_parse(&fixtures().join(VARIABLE)).expect("parse");
    files[0].hash = hash;
    files[0].parsed = parsed;
    files[0].size = 4321;
    index.persist_files(source, &files).expect("persist again");

    let (id, variable): (i64, bool) = index
        .conn
        .query_row("SELECT id, is_variable FROM face", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .expect("face");
    assert_eq!(id, face, "same face id");
    assert!(variable, "new metadata");
    assert_eq!(count(&index.conn, "SELECT COUNT(*) FROM font_file"), 1);
    assert_eq!(count(&index.conn, "SELECT COUNT(*) FROM face_tag"), 1);
    assert_eq!(count(&index.conn, "SELECT COUNT(*) FROM user_meta"), 1);
    assert_eq!(
        index.load_files().expect("load").get(&files[0].path),
        Some(&files[0])
    );
}

#[test]
fn a_file_with_fewer_faces_drops_the_others_except_active_ones() {
    let (mut index, source) = memory_db();
    let mut files = copies(1, STATIC);
    let one = files[0].parsed.faces[0].clone();
    files[0].parsed.faces = (0..3)
        .map(|i| ParsedFace {
            index: i,
            ..one.clone()
        })
        .collect();
    index.persist_files(source, &files).expect("persist");
    assert_eq!(count(&index.conn, "SELECT COUNT(*) FROM face"), 3);
    index
        .conn
        .execute(
            "INSERT INTO activation SELECT id, 'session', NULL, '2026-10-18T10:00:00Z'
             FROM face WHERE collection_index = 2",
            [],
        )
        .expect("activate");

    files[0].parsed.faces.truncate(1);
    index.persist_files(source, &files).expect("persist again");

    let mut stmt = index
        .conn
        .prepare("SELECT collection_index FROM face ORDER BY collection_index")
        .expect("prepare");
    let left: Vec<u32> = stmt
        .query_map([], |r| r.get(0))
        .expect("query")
        .map(|r| r.expect("row"))
        .collect();
    assert_eq!(
        left,
        [0, 2],
        "face 1 is gone; the active face 2 stays (rule 3)"
    );
}

#[test]
fn a_broken_file_without_faces_is_kept_with_its_health() {
    let (mut index, source) = memory_db();
    let mut files = copies(2, STATIC);
    index.persist_files(source, &files).expect("persist");
    files[0].parsed = crate::parser::parse(b"not a font at all");
    index.persist_files(source, &files).expect("persist again");

    assert_eq!(count(&index.conn, "SELECT COUNT(*) FROM face"), 1);
    let loaded = index.load_files().expect("load");
    let broken = loaded.get(&files[0].path).expect("file");
    assert!(matches!(broken.parsed.health, Health::Broken(_)));
    assert!(broken.parsed.faces.is_empty());
    assert_eq!(broken, &files[0]);
}

#[test]
fn a_non_utf8_path_is_stored_exactly() {
    let (mut index, source) = memory_db();
    let odd = fm_platform::fs::path_from_bytes(&[0x41, 0x00, 0x00, 0xD8]).expect("path");
    assert!(odd.to_str().is_none(), "{odd:?} should not be UTF-8");
    let mut files = copies(2, STATIC);
    files[0].path = odd.clone();
    files[1].path = PathBuf::from("/fonts/Schrift-ü/a.ttf");
    index.persist_files(source, &files).expect("persist");

    let loaded = index.load_files().expect("load");
    assert_eq!(loaded.get(&odd), Some(&files[0]));
    assert_eq!(loaded.get(&files[1].path), Some(&files[1]));
    assert_eq!(
        count(
            &index.conn,
            "SELECT COUNT(*) FROM font_file WHERE typeof(path) = 'blob'"
        ),
        1
    );
}

#[test]
fn a_rescan_after_loading_skips_unchanged_files() {
    let dir = TempDir::new("files-restart");
    let fonts = dir.path().join("fonts");
    copy_fixture(&fonts, STATIC, "a.ttf");
    copy_fixture(&fonts, COLLECTION, "b.ttc");
    let (scanned, first) = scan(&fonts);
    let (mut index, source, _) = open_db(&dir);
    index.apply_rescan(source, &scanned, &first).expect("apply");

    // Next start: load, rescan.
    let mut loaded = index.load_files().expect("load");
    let report = rescan(&mut loaded, &fonts);
    assert_eq!(report.unchanged.len(), 2, "{report:?}");
    assert!(report.added.is_empty() && report.changed.is_empty());

    // After a rebuild every file is read again.
    index.rebuild().expect("rebuild");
    let mut loaded = index.load_files().expect("load");
    let report = rescan(&mut loaded, &fonts);
    assert_eq!(report.changed.len(), 2, "{report:?}");
    assert!(report.unchanged.is_empty());
}

#[test]
fn apply_rescan_writes_gone_files_and_clears_the_flag_when_they_return() {
    let dir = TempDir::new("files-gone");
    let fonts = dir.path().join("fonts");
    let a = copy_fixture(&fonts, STATIC, "a.ttf");
    copy_fixture(&fonts, VARIABLE, "b.ttf");
    let (mut files, first) = scan(&fonts);
    let (mut index, source, _) = open_db(&dir);
    index.apply_rescan(source, &files, &first).expect("apply");
    let gone = |index: &Index| count(&index.conn, "SELECT COUNT(*) FROM font_file WHERE gone = 1");

    // Moved away: the rescan marks it gone; the row and its face stay.
    let parked = dir.path().join("parked.ttf");
    fs::rename(&a, &parked).expect("move away");
    let report = rescan(&mut files, &fonts);
    assert_eq!(report.gone, std::slice::from_ref(&a));
    let done = index.apply_rescan(source, &files, &report).expect("apply");
    assert_eq!(done.files, 2, "one gone file, one unchanged");
    assert_eq!(gone(&index), 1);
    assert_eq!(count(&index.conn, "SELECT COUNT(*) FROM face"), 2);

    // Moved back with the same size and time: unchanged, found again.
    fs::rename(&parked, &a).expect("move back");
    let report = rescan(&mut files, &fonts);
    assert_eq!(report.unchanged.len(), 2, "{report:?}");
    index.apply_rescan(source, &files, &report).expect("apply");
    assert_eq!(gone(&index), 0);
    assert_eq!(index.load_files().expect("load"), files);
}

#[test]
fn rows_that_cannot_be_trusted_are_read_again() {
    let (mut index, source) = memory_db();
    let files = copies(3, STATIC);
    index.persist_files(source, &files).expect("persist");
    let id =
        |i: usize| format!("(SELECT id FROM font_file WHERE path = '/fonts/batch/0000{i}.ttf')");
    index
        .conn
        .execute_batch(&format!(
            "UPDATE face SET parsed = '{{\"not\": \"a face\"}}' WHERE file_id = {};
             UPDATE face SET coverage = x'010203' WHERE file_id = {};",
            id(0),
            id(1)
        ))
        .expect("damage");

    let loaded = index.load_files().expect("load");
    for (i, file) in files.iter().enumerate() {
        let entry = loaded.get(&file.path).expect("file");
        assert_eq!(entry.modified.is_none(), i < 2, "file {i}");
    }
    assert_eq!(loaded.get(&files[2].path), Some(&files[2]));
}

#[test]
fn a_version_1_index_migrates_and_its_files_are_read_again() {
    let dir = TempDir::new("files-migrate");
    let path = dir.path().join(INDEX_FILE);
    {
        let conn = Connection::open(&path).expect("open");
        conn.execute_batch(crate::index::schema::MIGRATIONS[0])
            .expect("v1");
        conn.execute_batch(
            "PRAGMA user_version = 1;
             INSERT INTO source (id, path, kind) VALUES (1, '/fonts', 'watched');
             INSERT INTO font_file (source_id, path, hash, format, size, mtime, health)
             VALUES (1, '/fonts/old.otf', NULL, 'otf', 10, 1700000000, 'ok');",
        )
        .expect("old rows");
    }
    let index = Index::open(&path).expect("open").index;
    assert_eq!(index.schema_version().expect("version"), SCHEMA_VERSION);
    let loaded = index.load_files().expect("load");
    let old = loaded.get(Path::new("/fonts/old.otf")).expect("kept");
    assert!(!old.gone);
    assert_eq!(old.modified, None, "no trusted stamp: read again");
}

#[test]
fn coverage_packs_into_runs_and_refuses_damage() {
    let points = [0x20, 0x21, 0x22, 0x41, 0x10_FFFF];
    let packed = pack_coverage(&points);
    assert_eq!(packed.len(), 3 * 8);
    assert_eq!(unpack_coverage(&packed).as_deref(), Some(&points[..]));
    assert_eq!(pack_coverage(&[]), Vec::<u8>::new());
    assert_eq!(unpack_coverage(&[]), Some(Vec::new()));
    assert_eq!(unpack_coverage(&[1, 2, 3]), None, "not whole pairs");
    let backwards = [5u32.to_le_bytes(), 4u32.to_le_bytes()].concat();
    assert_eq!(unpack_coverage(&backwards), None);
    let huge = [0u32.to_le_bytes(), u32::MAX.to_le_bytes()].concat();
    assert_eq!(unpack_coverage(&huge), None, "past U+10FFFF");
}

#[test]
fn times_round_trip_through_nanoseconds() {
    let after = UNIX_EPOCH + Duration::new(1_760_000_000, 123_456_700);
    let before = UNIX_EPOCH - Duration::new(86_400, 500);
    for time in [after, before, UNIX_EPOCH] {
        let nanos = time_to_nanos(time).expect("in range");
        assert_eq!(time_from_nanos(nanos), Some(time));
    }
    assert!(time_to_nanos(before).expect("before") < 0);
}

/// Criterion 2 without the reference library: 10,000 files cycled from the
/// fixtures (every format, collections, variable faces, full coverage).
#[test]
fn ten_thousand_files_persist_in_under_ten_seconds() {
    let root = fixtures();
    let (parsed, _) = scan(&root);
    let mut sample: Vec<&IndexedFile> = parsed.files().collect();
    sample.sort_by(|a, b| a.path.cmp(&b.path));
    let files: Vec<IndexedFile> = (0..10_000)
        .map(|i| {
            let from = sample[i % sample.len()];
            let mut file = from.clone();
            file.path = PathBuf::from(format!("/library/{i:05}"))
                .join(from.path.file_name().unwrap_or_default());
            file
        })
        .collect();

    let dir = TempDir::new("files-10k");
    let (mut index, source, _) = open_db(&dir);
    let start = Instant::now();
    let done = index.persist_files(source, &files).expect("persist");
    let elapsed = start.elapsed();
    let load_start = Instant::now();
    let loaded = index.load_files().expect("load");
    eprintln!(
        "persisted {} files / {} faces in {} batches in {elapsed:?}; loaded in {:?}",
        done.files,
        done.faces,
        done.batches,
        load_start.elapsed()
    );
    assert_eq!(done.files, 10_000);
    assert_eq!(done.batches, 20);
    assert_eq!(loaded.len(), 10_000);
    // The budget is for release builds. Unoptimized test builds on shared CI
    // runners take 8 s normally and up to 15 s on a busy Windows runner, so
    // they get three times the room. Release numbers come from scripts/bench.sh.
    let budget = Duration::from_secs(if cfg!(debug_assertions) { 30 } else { 10 });
    assert!(elapsed < budget, "{elapsed:?}");
}

/// Criterion 2 on the reference library (10k fonts): skipped without it.
#[test]
fn the_reference_library_persists_in_under_ten_seconds() {
    let Some(library) = std::env::var_os("FM_REFERENCE_LIBRARY") else {
        eprintln!("skipped: FM_REFERENCE_LIBRARY is not set");
        return;
    };
    let library = PathBuf::from(library);
    let (scanned, report) = scan(&library);
    let dir = TempDir::new("files-reference");
    let path = dir.path().join(INDEX_FILE);
    let mut index = Index::open(&path).expect("open").index;
    let source = index
        .add_source(&library.to_string_lossy(), SourceKind::Watched)
        .expect("source")
        .id;
    let start = Instant::now();
    let done = index
        .apply_rescan(source, &scanned, &report)
        .expect("persist");
    let elapsed = start.elapsed();
    eprintln!(
        "reference library: {} files / {} faces in {} batches persisted in {elapsed:?}",
        done.files, done.faces, done.batches
    );
    assert!(elapsed < Duration::from_secs(10), "{elapsed:?}");
}

/// COR-1 (review cycle 1): a gone file is not read again, so a face row that
/// did not load back must not be deleted when the gone flag is written.
#[test]
fn a_gone_file_keeps_faces_that_did_not_load_back_and_their_user_data() {
    let dir = TempDir::new("files-gone-untrusted");
    let fonts = dir.path().join("fonts");
    let ttc = copy_fixture(&fonts, COLLECTION, "collection.ttc");
    let (scanned, first) = scan(&fonts);
    let faces = scanned.get(&ttc).expect("file").parsed.faces.len();
    assert!(faces > 1, "the collection has several faces");
    let (mut index, source, _) = open_db(&dir);
    index.apply_rescan(source, &scanned, &first).expect("apply");

    // The last face carries user data, and its row does not load back.
    let last = faces - 1;
    index
        .conn
        .execute_batch(&format!(
            "INSERT INTO tag (id, name) VALUES (1, 'display');
             INSERT INTO face_tag SELECT id, 1 FROM face WHERE collection_index = {last};
             INSERT INTO user_meta (face_id, rating)
                 SELECT id, 4 FROM face WHERE collection_index = {last};
             UPDATE face SET parsed = 'from another version' WHERE collection_index = {last};"
        ))
        .expect("user data");
    let mut loaded = index.load_files().expect("load");
    assert_eq!(loaded.get(&ttc).expect("file").parsed.faces.len(), last);

    // The folder goes offline: the file is gone.
    fs::remove_file(&ttc).expect("remove");
    let report = rescan(&mut loaded, &fonts);
    assert_eq!(report.gone, std::slice::from_ref(&ttc));
    index.apply_rescan(source, &loaded, &report).expect("apply");

    assert_eq!(count(&index.conn, "SELECT COUNT(*) FROM face"), faces);
    assert_eq!(count(&index.conn, "SELECT COUNT(*) FROM face_tag"), 1);
    assert_eq!(count(&index.conn, "SELECT COUNT(*) FROM user_meta"), 1);
    assert_eq!(
        count(&index.conn, "SELECT COUNT(*) FROM font_file WHERE gone = 1"),
        1
    );
}

#[test]
fn a_gone_file_not_yet_in_the_index_is_written_in_full() {
    let (mut index, source) = memory_db();
    let mut files = FileIndex::new();
    let mut file = copies(1, STATIC).remove(0);
    file.gone = true;
    let path = file.path.clone();
    files.insert(file.clone());
    let report = Rescan {
        gone: vec![path.clone()],
        ..Rescan::default()
    };
    let done = index.apply_rescan(source, &files, &report).expect("apply");
    assert_eq!(done.faces, 1);
    assert_eq!(index.load_files().expect("load").get(&path), Some(&file));
}
