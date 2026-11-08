//! E44.4 (ADR 0022): a second process receives the files of a native file
//! drag, in place and as a copy, on every CI runner (Linux under `xvfb-run`).
//!
//! A real mouse drag cannot be driven on a runner, so the test hands the OS
//! the exact data the drag hands it ([`fm_platform::drag::probe`]): the
//! shell's data object on the clipboard (Windows), the drag items' pasteboard
//! writers on a named pasteboard (macOS), the drag's `text/uri-list` answer on
//! a named X selection (Linux). A child process of this test binary reads it
//! the way a drop target does.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // a test

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use fm_platform::drag::probe;

/// The channel the child reads from.
const CHANNEL: &str = "FM_FILE_DRAG_CHANNEL";
const CHILD_TEST: &str = "child_reads_the_drag_data";

/// A scratch folder, removed at the end.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture(rel: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts")
        .join(rel);
    assert!(path.is_file(), "missing fixture {}", path.display());
    path
}

/// Every file of `dir` with its bytes, sorted by name.
fn contents(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut all: Vec<(String, Vec<u8>)> = fs::read_dir(dir)
        .expect("scratch")
        .map(|e| {
            let e = e.expect("entry");
            (
                e.file_name().to_string_lossy().into_owned(),
                fs::read(e.path()).expect("bytes"),
            )
        })
        .collect();
    all.sort();
    all
}

#[test]
#[ignore = "run as a child process by the test below"]
fn child_reads_the_drag_data() {
    let channel = std::env::var(CHANNEL).expect("channel");
    let got = probe::receive(&channel).expect("the drag data is readable");
    for path in &got.paths {
        println!("PATH {}", path.display());
    }
    println!("COPY {:?}", got.copy);
}

#[test]
fn a_second_process_receives_the_dragged_files_in_place_as_a_copy() {
    let dir =
        Scratch(std::env::temp_dir().join(format!("fm-file-drag Fönts {}", std::process::id())));
    let _ = fs::remove_dir_all(&dir.0);
    fs::create_dir_all(&dir.0).expect("scratch");
    let files = vec![dir.0.join("Asul Bold.ttf"), dir.0.join("Téko SemiBold.ttf")];
    fs::copy(fixture("google-repo/asul/Asul-Bold.ttf"), &files[0]).expect("copy fixture");
    fs::copy(fixture("fontshare/teko/Teko-SemiBold.ttf"), &files[1]).expect("copy fixture");
    let before = contents(&dir.0);

    let child: Output = probe::share(&files, |channel| {
        Command::new(std::env::current_exe().expect("test binary"))
            .args([CHILD_TEST, "--exact", "--ignored", "--nocapture"])
            .env(CHANNEL, channel)
            .output()
            .expect("the child runs")
    })
    .expect("the drag data is offered");
    let stdout = String::from_utf8_lossy(&child.stdout);
    assert!(
        child.status.success() && stdout.contains("1 passed"),
        "{stdout}{}",
        String::from_utf8_lossy(&child.stderr)
    );

    let received: Vec<PathBuf> = stdout
        .lines()
        .filter_map(|l| l.strip_prefix("PATH "))
        .map(|p| fs::canonicalize(p).expect("a received path exists"))
        .collect();
    let dragged: Vec<PathBuf> = files
        .iter()
        .map(|f| fs::canonicalize(f).expect("dragged"))
        .collect();
    assert_eq!(received, dragged, "the original files, in order: {stdout}");

    // Windows states the copy in the data ("Preferred DropEffect"); macOS and
    // Linux state it in the drag's allowed operation (unit-tested per OS).
    let copy = stdout
        .lines()
        .find_map(|l| l.strip_prefix("COPY "))
        .unwrap_or_default();
    if cfg!(windows) {
        assert_eq!(copy, "Some(true)", "{stdout}");
    } else {
        assert_eq!(copy, "None", "{stdout}");
    }

    // Rule 2: nothing was copied, moved or changed by offering the drag.
    assert_eq!(contents(&dir.0), before);
}

#[test]
fn an_empty_drag_is_refused_before_anything_is_offered() {
    let e = probe::share(&[], |_| ()).expect_err("empty");
    assert_eq!(e.0, "there are no files to drag");
}
