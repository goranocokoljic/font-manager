//! The trash of this OS, for real (E8.3): a file goes to the Recycle Bin, the
//! Finder trash or the freedesktop.org trash, and comes back byte for byte.
//! The OS record (`$I…`, `.trashinfo`) goes with it, so the user's trash is
//! left as it was. Runs on every CI runner: one test, three implementations.
#![allow(clippy::expect_used)]

use std::fs;
use std::path::PathBuf;

use fm_platform::{TrashErrorKind, trash};

/// A fresh folder under the system temp folder, removed on drop.
struct Dir(PathBuf);

impl Dir {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!("fm-trash-it-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_file_goes_to_the_os_trash_and_comes_back() {
    let dir = Dir::new("round-trip");
    let file = dir.0.join("Schrift ü copy.ttf");
    let content = format!("not a font {}", std::process::id());
    fs::write(&file, &content).expect("write");
    let trash = trash::system().expect("this OS has a trash");

    let item = trash.trash(&file).expect("moved to the trash");
    assert!(!file.exists(), "the file left its folder");
    assert_eq!(
        fs::read_to_string(&item.location).expect("the file is in the trash, not deleted"),
        content
    );
    if let Some(info) = &item.info {
        assert!(info.exists(), "the OS record of the move is there");
    }

    trash.restore(&item).expect("put back");
    assert_eq!(fs::read_to_string(&file).expect("back in place"), content);
    assert!(!item.location.exists(), "nothing is left in the trash");
    if let Some(info) = &item.info {
        assert!(!info.exists(), "the OS record is removed with it");
    }
}

#[test]
fn putting_back_never_overwrites_a_new_file_at_the_old_path() {
    let dir = Dir::new("occupied");
    let file = dir.0.join("Occupied.otf");
    fs::write(&file, "old").expect("write");
    let trash = trash::system().expect("this OS has a trash");
    let item = trash.trash(&file).expect("moved to the trash");
    fs::write(&file, "new").expect("a new file at the old path");

    let err = trash.restore(&item).expect_err("occupied");
    assert_eq!(err.kind, TrashErrorKind::Occupied);
    assert_eq!(fs::read_to_string(&file).expect("new file"), "new");

    // Leave the user's trash as it was.
    fs::remove_file(&file).expect("remove the new file");
    trash.restore(&item).expect("put back");
    assert_eq!(fs::read_to_string(&file).expect("old file"), "old");
}

#[test]
fn a_missing_file_is_not_found_and_nothing_moves() {
    let dir = Dir::new("missing");
    let trash = trash::system().expect("this OS has a trash");
    let err = trash.trash(&dir.0.join("nope.ttf")).expect_err("not there");
    assert_eq!(err.kind, TrashErrorKind::NotFound);
}
