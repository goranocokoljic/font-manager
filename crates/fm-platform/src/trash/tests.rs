use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

/// A fresh folder under the system temp folder, removed on drop.
struct Dir(PathBuf);

impl Dir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fm-platform-trash-{label}-{}-{n}",
            std::process::id()
        ));
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
fn a_mock_trash_moves_the_file_away_and_back() {
    let dir = Dir::new("mock");
    let file = dir.0.join("Font.ttf");
    fs::write(&file, b"font").expect("write");
    let trash = MockTrash::new(dir.0.join("trash"));

    let item = trash.trash(&file).expect("trash");
    assert!(!file.exists());
    assert_eq!(fs::read(&item.location).expect("in trash"), b"font");

    trash.restore(&item).expect("restore");
    assert_eq!(fs::read(&file).expect("back"), b"font");
    assert!(!item.location.exists());
    assert!(!item.info.as_ref().expect("info").exists());
}

#[test]
fn restore_never_overwrites_a_file_at_the_old_path() {
    let dir = Dir::new("occupied");
    let file = dir.0.join("Font.ttf");
    fs::write(&file, b"old").expect("write");
    let trash = MockTrash::new(dir.0.join("trash"));
    let item = trash.trash(&file).expect("trash");
    fs::write(&file, b"new").expect("write new");

    let err = trash.restore(&item).expect_err("occupied");
    assert_eq!(err.kind, TrashErrorKind::Occupied);
    assert_eq!(fs::read(&file).expect("new"), b"new");
    assert_eq!(fs::read(&item.location).expect("still in trash"), b"old");
}

#[test]
fn restore_reports_a_file_the_user_removed_from_the_trash() {
    let dir = Dir::new("emptied");
    let file = dir.0.join("Font.ttf");
    fs::write(&file, b"font").expect("write");
    let trash = MockTrash::new(dir.0.join("trash"));
    let item = trash.trash(&file).expect("trash");
    fs::remove_file(&item.location).expect("empty the trash");

    let err = trash.restore(&item).expect_err("emptied");
    assert_eq!(err.kind, TrashErrorKind::Emptied);
    assert!(!file.exists());
}

#[test]
fn a_refused_move_leaves_the_file_where_it_is() {
    let dir = Dir::new("refused");
    let file = dir.0.join("Font.ttf");
    fs::write(&file, b"font").expect("write");
    let trash = MockTrash::new(dir.0.join("trash"));
    trash.refuse(&file);

    let err = trash.trash(&file).expect_err("refused");
    assert_eq!(err.kind, TrashErrorKind::NoTrash);
    assert_eq!(fs::read(&file).expect("still there"), b"font");
}

#[test]
fn a_missing_file_is_not_found() {
    let dir = Dir::new("missing");
    let trash = MockTrash::new(dir.0.join("trash"));
    let err = trash.trash(&dir.0.join("nope.ttf")).expect_err("missing");
    assert_eq!(err.kind, TrashErrorKind::NotFound);
}
