use std::fs;

use fm_api::ErrorCode;

use super::*;
use crate::test_support::TempDir;

#[test]
fn a_purchase_date_is_a_day_that_exists_written_yyyy_mm_dd() {
    for good in ["2026-03-01", "2024-02-29", "2000-02-29", "1999-12-31"] {
        assert!(check_date(good).is_ok(), "{good}");
    }
    for bad in [
        "",
        "2026-3-1",
        "01-03-2026",
        "2026/03/01",
        "2026-13-01",
        "2026-00-10",
        "2026-04-31",
        "2023-02-29",
        "1900-02-29",
        "2026-03-01T00:00",
        "２０２６-03-01",
        "ü26-03-01",
        "yesterday",
    ] {
        let err = check_date(bad).expect_err(bad);
        assert_eq!(err.code(), ErrorCode::InvalidRequest, "{bad}");
        assert!(err.message().contains("YYYY-MM-DD"), "{bad}");
    }
}

#[test]
fn seats_are_zero_or_more() {
    assert_eq!(check_seats(0).expect("zero"), 0);
    assert_eq!(check_seats(25).expect("some"), 25);
    let err = check_seats(-1).expect_err("negative");
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
    assert_eq!(err.message(), "The number of seats cannot be below 0.");
}

#[test]
fn a_receipt_that_is_not_a_file_there_is_an_invalid_request() {
    let tmp = TempDir::new("receipt-check");
    let missing = tmp.path().join("gone.pdf");
    for path in [missing.as_path(), tmp.path(), Path::new("")] {
        let err = check_receipt(path).expect_err("not a file");
        assert_eq!(err.code(), ErrorCode::InvalidRequest);
        assert!(err.message().contains("is not there"));
    }
    let there = tmp.path().join("invoice.pdf");
    fs::write(&there, b"paid").expect("write");
    assert!(check_receipt(&there).is_ok());
}

#[test]
fn a_receipt_is_copied_never_moved_and_never_overwrites() {
    let tmp = TempDir::new("receipt-copy");
    let picked = tmp.path().join("invoice.pdf");
    fs::write(&picked, b"paid in full").expect("write");
    let dir = tmp.path().join(SIDECAR_DIR).join(RECEIPTS_DIR);

    let first = copy_into(&picked, &dir).expect("copy");
    assert_eq!(first, dir.join("invoice.pdf"));
    assert_eq!(fs::read(&first).expect("copy"), b"paid in full");
    // The file the user picked is still there, unchanged.
    assert_eq!(fs::read(&picked).expect("source"), b"paid in full");

    // A second copy of the same name gets a number; the first stays.
    fs::write(&picked, b"second invoice").expect("write");
    let second = copy_into(&picked, &dir).expect("copy");
    assert_eq!(second, dir.join("invoice-2.pdf"));
    assert_eq!(fs::read(&first).expect("first"), b"paid in full");
    assert_eq!(fs::read(&second).expect("second"), b"second invoice");

    // A name without an extension works too.
    let bare = tmp.path().join("RECEIPT");
    fs::write(&bare, b"x").expect("write");
    assert_eq!(copy_into(&bare, &dir).expect("copy"), dir.join("RECEIPT"));
    assert_eq!(copy_into(&bare, &dir).expect("copy"), dir.join("RECEIPT-2"));
}

#[test]
fn a_receipt_with_a_non_ascii_name_is_copied() {
    let tmp = TempDir::new("receipt-unicode");
    let picked = tmp.path().join("Rechnung für Schrift.pdf");
    fs::write(&picked, b"bezahlt").expect("write");
    let dir = tmp.path().join("data").join(RECEIPTS_DIR);
    let copy = copy_into(&picked, &dir).expect("copy");
    assert_eq!(copy, dir.join("Rechnung für Schrift.pdf"));
}

#[test]
fn only_a_copy_in_a_receipts_folder_the_app_uses_is_its_own() {
    let tmp = TempDir::new("receipt-own");
    let app = tmp.path().join("data").join(RECEIPTS_DIR);
    let synced = tmp
        .path()
        .join("fonts")
        .join(SIDECAR_DIR)
        .join(RECEIPTS_DIR);
    assert!(is_app_copy(&app.join("a.pdf"), Some(&app)));
    assert!(is_app_copy(&synced.join("a.pdf"), Some(&app)));
    assert!(is_app_copy(&synced.join("a.pdf"), None));
    // The user's own files are never the app's.
    assert!(!is_app_copy(&tmp.path().join("a.pdf"), Some(&app)));
    assert!(!is_app_copy(
        &tmp.path().join(RECEIPTS_DIR).join("a.pdf"),
        Some(&app)
    ));
    assert!(!is_app_copy(&app.join("nested").join("a.pdf"), Some(&app)));
    assert!(!is_app_copy(Path::new("receipts/a.pdf"), Some(&app)));
}
