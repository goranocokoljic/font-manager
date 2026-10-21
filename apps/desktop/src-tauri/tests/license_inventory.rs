//! "Export license inventory…" writes the CSV to the file the user picked.

use fm_api::export::LicenseInventoryRequest;
use fm_api::mock::MockApi;
use fm_api::{CollectionId, ErrorCode};
use fm_app_lib::write_license_inventory;

/// A scratch folder; `name` keeps each test's apart (tests run in parallel).
fn scratch(name: &str) -> std::io::Result<std::path::PathBuf> {
    let dir = std::env::temp_dir().join(format!("fm-app-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[test]
fn the_inventory_is_written_to_the_picked_file() {
    let dir = scratch("inventory").expect("scratch folder");
    let path = dir.join("License inventory.csv");
    std::fs::write(&path, "an older export").expect("old file");
    let rows = write_license_inventory(&MockApi::new(), LicenseInventoryRequest::default(), &path)
        .expect("written");
    let csv = std::fs::read_to_string(&path).expect("read");
    assert!(rows > 0);
    // The header plus one line per family.
    assert_eq!(csv.lines().count(), rows as usize + 1);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_unknown_collection_writes_nothing() {
    let dir = scratch("inventory-unknown").expect("scratch folder");
    let path = dir.join("inventory.csv");
    let err = write_license_inventory(
        &MockApi::new(),
        LicenseInventoryRequest {
            collection: Some(CollectionId(9999)),
        },
        &path,
    )
    .expect_err("unknown");
    assert_eq!(err.code, ErrorCode::NotFound);
    assert!(!path.exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_folder_that_is_gone_is_an_error_for_the_user() {
    let dir = scratch("inventory-gone").expect("scratch folder");
    let path = dir.join("gone").join("inventory.csv");
    let err = write_license_inventory(&MockApi::new(), LicenseInventoryRequest::default(), &path)
        .expect_err("no folder");
    assert_eq!(err.code, ErrorCode::NotFound);
    assert_eq!(
        err.message,
        "The license inventory could not be saved there."
    );
    std::fs::remove_dir_all(&dir).ok();
}
