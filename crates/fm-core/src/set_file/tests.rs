use fm_api::ErrorCode;
use fm_api::organization::SetEntry;

use super::*;
use crate::test_support::TempDir;

fn export() -> SetExport {
    SetExport {
        name: "Brochure".into(),
        fonts: vec![SetEntry {
            family: "Barlow".into(),
            style: "Bold".into(),
            hash: "ab".repeat(32),
        }],
    }
}

/// The names in `dir`, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_name_without_an_extension_gets_the_set_file_extension() {
    assert_eq!(
        file_path(Path::new("/x/Brochure")),
        PathBuf::from("/x/Brochure.fmset.json")
    );
    assert_eq!(
        file_path(Path::new("/x/Brochure.fmset.json")),
        PathBuf::from("/x/Brochure.fmset.json")
    );
    assert_eq!(
        file_path(Path::new("/x/brochure.json")),
        PathBuf::from("/x/brochure.json")
    );
}

#[test]
fn save_writes_only_the_picked_file_and_load_reads_it_back() {
    let dir = TempDir::new("set-file-save");
    let written = save(&dir.path().join("Brochure"), &export()).expect("saved");
    assert_eq!(written, dir.path().join("Brochure.fmset.json"));
    assert_eq!(names(dir.path()), ["Brochure.fmset.json"]);
    assert_eq!(load(&written).expect("loaded"), export());

    // Saving again replaces the file; still nothing else is written.
    let mut other = export();
    other.name = "Poster".into();
    save(&written, &other).expect("saved again");
    assert_eq!(names(dir.path()), ["Brochure.fmset.json"]);
    assert_eq!(load(&written).expect("loaded").name, "Poster");
}

#[test]
fn a_set_file_with_a_byte_order_mark_loads() {
    let dir = TempDir::new("set-file-bom");
    let path = dir.path().join("bom.fmset.json");
    let json = serde_json::to_string(&export()).expect("json");
    std::fs::write(&path, format!("\u{feff}{json}")).expect("write");
    assert_eq!(load(&path).expect("loaded"), export());
}

#[test]
fn a_missing_file_is_not_found_and_other_json_is_invalid() {
    let dir = TempDir::new("set-file-bad");
    let err = load(&dir.path().join("gone.fmset.json")).expect_err("missing");
    assert_eq!(err.code(), ErrorCode::NotFound);

    for (name, bytes) in [
        ("other.json", br#"{"hello": 1}"#.as_slice()),
        ("binary.fmset.json", b"\xff\xfe\x00junk".as_slice()),
        ("empty.fmset.json", b"".as_slice()),
    ] {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).expect("write");
        let err = load(&path).expect_err(name);
        assert_eq!(err.code(), ErrorCode::InvalidRequest, "{name}");
        assert_eq!(err.message(), "This file is not a Font Manager set file.");
    }
}

#[test]
fn a_folder_that_is_not_there_cannot_be_saved_into() {
    let dir = TempDir::new("set-file-nofolder");
    let err =
        save(&dir.path().join("nope").join("set.fmset.json"), &export()).expect_err("no folder");
    assert_eq!(err.code(), ErrorCode::NotFound);
    assert!(names(dir.path()).is_empty());
}
