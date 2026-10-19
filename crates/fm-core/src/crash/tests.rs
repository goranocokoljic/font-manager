use std::fs;
use std::path::Path;
use std::process::Command;

use fm_api::library::SourceKind;
use fm_platform::AppPaths;

use super::*;
use crate::index::Index;
use crate::scanner::{FileIndex, discover};
use crate::startup;
use crate::test_support::TempDir;

/// Set for the child process of [`a_crash_writes_a_log_and_the_next_start_is_clean`]:
/// the folder its app paths live in.
const CHILD_ENV: &str = "FM_CRASH_TEST_ROOT";
const CHILD_TEST: &str = "crash::tests::crashing_child";
const CHILD_MESSAGE: &str = "E9.4 deliberate crash in the child";
const FIXTURE: &str = "google-static/asap/Asap_Condensed-ExtraLightItalic.ttf";

fn paths_in(root: &Path) -> AppPaths {
    AppPaths {
        data: root.join("data"),
        config: root.join("config"),
        cache: root.join("cache"),
        logs: root.join("logs"),
        library: root.join("Fonts"),
    }
}

fn report(message: &str) -> CrashReport {
    CrashReport {
        message: message.to_owned(),
        location: Some("src/lib.rs:1:2".to_owned()),
    }
}

fn crash_logs(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with(FILE_PREFIX))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn a_crash_log_holds_the_message_location_thread_and_version() {
    let text = format_log(&report("index broke"), "scan-1", "0: frame");
    assert!(text.contains(env!("CARGO_PKG_VERSION")));
    assert!(text.contains("message: index broke"));
    assert!(text.contains("location: src/lib.rs:1:2"));
    assert!(text.contains("thread: scan-1"));
    assert!(text.contains("0: frame"));
}

#[test]
fn two_crash_logs_in_the_same_millisecond_do_not_overwrite_each_other() {
    let dir = TempDir::new("crash-unique");
    let first = write_text(dir.path(), "first").expect("first");
    // Same process, same millisecond: `create_new` refuses to overwrite.
    match write_text(dir.path(), "second") {
        Ok(second) => assert_ne!(first, second),
        Err(e) => assert_eq!(e.kind(), io::ErrorKind::AlreadyExists),
    }
    assert_eq!(fs::read_to_string(&first).expect("read"), "first");
}

#[test]
fn the_next_start_reports_the_newest_new_crash_once() {
    let dir = TempDir::new("crash-take");
    assert_eq!(take_unreported(dir.path()), None, "no crash yet");
    fs::write(dir.path().join("crash-0000000000000001-7.log"), "old").expect("old");
    fs::write(dir.path().join("crash-0000000000000002-7.log"), "new").expect("new");
    fs::write(dir.path().join("fm.2026-10-19.log"), "a normal log").expect("log");

    let newest = take_unreported(dir.path()).expect("a crash to report");
    assert_eq!(fs::read_to_string(&newest).expect("read"), "new");
    assert_eq!(
        crash_logs(dir.path()),
        vec![
            "crash-0000000000000001-7.reported.log",
            "crash-0000000000000002-7.reported.log"
        ]
    );
    assert!(
        dir.path().join("fm.2026-10-19.log").is_file(),
        "other logs are left alone"
    );
    assert_eq!(take_unreported(dir.path()), None, "reported once only");
}

#[test]
fn old_crash_logs_are_removed_beyond_the_limit() {
    let dir = TempDir::new("crash-keep");
    for i in 0..KEEP_FILES + 3 {
        fs::write(
            dir.path().join(format!("crash-{i:016}-1.reported.log")),
            "x",
        )
        .expect("seed");
    }
    assert_eq!(take_unreported(dir.path()), None);
    let left = crash_logs(dir.path());
    assert_eq!(left.len(), KEEP_FILES);
    assert_eq!(left[0], format!("crash-{:016}-1.reported.log", 3));
}

#[test]
fn a_missing_logs_folder_is_no_crash() {
    let dir = TempDir::new("crash-missing");
    assert_eq!(take_unreported(&dir.path().join("nope")), None);
}

/// The child half of the next test: does nothing unless the parent started it.
/// It opens the index, scans a source, installs the crash hook as
/// [`startup::init`] does, and panics with the index still open.
#[test]
fn crashing_child() {
    let Some(root) = std::env::var_os(CHILD_ENV) else {
        return;
    };
    let root = Path::new(&root);
    let paths = paths_in(root);
    startup::prepare(&paths).expect("prepare");
    let mut index = startup::open_index(&paths).expect("open").index;
    let fonts = root.join("fonts");
    let source = index
        .add_source(&fonts.to_string_lossy(), SourceKind::Watched)
        .expect("source")
        .id;
    let mut files = FileIndex::new();
    let rescan = files.rescan(&fonts, &discover(&fonts).expect("discover"));
    index
        .apply_rescan(source, &files, &rescan)
        .expect("persist");
    install(&paths.logs);
    // The connection is never closed: the WAL is not checkpointed, as when
    // the process dies.
    std::mem::forget::<Index>(index);
    panic!("{CHILD_MESSAGE}");
}

#[test]
fn a_crash_writes_a_log_and_the_next_start_is_clean() {
    let dir = TempDir::new("crash-restart");
    let fonts = dir.path().join("fonts");
    fs::create_dir_all(&fonts).expect("fonts");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/fonts")
            .join(FIXTURE),
        fonts.join("a.ttf"),
    )
    .expect("fixture");

    let out = Command::new(std::env::current_exe().expect("test binary"))
        .args([CHILD_TEST, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, dir.path())
        .output()
        .expect("run the child");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "the child crashed: {stderr}");
    assert!(
        stderr.contains(CHILD_MESSAGE),
        "the previous hook still ran: {stderr}"
    );

    let paths = paths_in(dir.path());
    let logs = crash_logs(&paths.logs);
    assert_eq!(logs.len(), 1, "one crash log: {logs:?}");
    let text = fs::read_to_string(paths.logs.join(&logs[0])).expect("read log");
    assert!(text.contains(CHILD_MESSAGE), "{text}");
    assert!(text.contains("crash"), "{text}");
    assert!(text.contains("backtrace:"), "{text}");

    // The restart: the same steps as `startup::init` after the logger.
    let config = startup::prepare(&paths).expect("settings still usable");
    assert_eq!(config, crate::Config::default());
    let previous = take_unreported(&paths.logs).expect("the crash is reported");
    assert!(previous.is_file());
    let opened = startup::open_index(&paths).expect("the index opens");
    assert!(
        opened.rebuilt.is_none(),
        "a crash does not damage the index"
    );
    let sources = opened.index.sources().expect("sources");
    assert_eq!(sources.len(), 1);
    assert_eq!(
        sources[0].face_count, 1,
        "what the crashed run wrote is kept"
    );
    assert!(opened.index.problems().expect("problems").is_empty());
    assert_eq!(
        take_unreported(&paths.logs),
        None,
        "only the first start reports it"
    );
}
