use std::path::PathBuf;

use fm_api::common::{FontFormat, Health};
use fm_api::library::{LicenseType, ListFamiliesRequest, SourceKind};
use fm_api::organization::FamilyNotesUpdate;
use fm_api::search::{Filters, SearchRequest};
use fm_api::{ErrorCode, FamilyId, SourceId};

use super::*;
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, IndexedFile};
use crate::test_support::TempDir;

/// One file per face: (family as the font names it, style). The first two
/// are one family: the library groups names that differ in case and spaces.
const LIBRARY: &[(&str, &str)] = &[
    ("Source Serif", "Regular"),
    ("source  serif", "Bold"),
    ("Inter", "Regular"),
];

fn file(family: &str, style: &str) -> IndexedFile {
    let mut face = ParsedFace::empty(0, Health::Ok);
    face.family = Some(family.to_owned());
    face.subfamily = Some(style.to_owned());
    face.postscript_name = Some(format!("{}-{style}", family.replace(' ', "")));
    face.weight = Some(400);
    face.width = Some(5);
    IndexedFile {
        path: PathBuf::from(format!("/fonts/{}-{style}.ttf", family.replace(' ', ""))),
        size: 1000,
        modified: None,
        hash: FileHash::from([7; 32]),
        parsed: ParsedFile {
            format: Some(FontFormat::Ttf),
            health: Health::Ok,
            faces: vec![face],
        },
        gone: false,
    }
}

fn files() -> Vec<IndexedFile> {
    LIBRARY.iter().map(|(f, s)| file(f, s)).collect()
}

fn library() -> (Index, SourceId) {
    let mut index = Index::open_in_memory().expect("open");
    let source = index
        .add_source("/fonts", SourceKind::Watched)
        .expect("source")
        .id;
    index.persist_files(source, &files()).expect("persist");
    (index, source)
}

fn face(index: &Index, postscript: &str) -> FaceId {
    index
        .conn
        .query_row(
            "SELECT id FROM face WHERE postscript_name = ?1",
            [postscript],
            |r| r.get(0),
        )
        .map(FaceId)
        .expect("face id")
}

fn family(index: &Index, postscript: &str) -> FamilyId {
    index
        .family_of(face(index, postscript))
        .expect("family")
        .expect("visible")
        .0
}

fn web_license() -> DeclaredLicense {
    DeclaredLicense {
        license_type: Some(LicenseType::Web),
        seats: Some(5),
        purchase_date: Some("2026-03-01".into()),
        receipt: Some(PathBuf::from("/app-data/receipts/invoice.pdf")),
    }
}

fn summary_license(index: &Index, family: FamilyId) -> Option<LicenseType> {
    index
        .family_summary(family)
        .expect("summary")
        .expect("family")
        .declared_license
}

#[test]
fn a_declared_license_applies_to_every_face_of_the_family() {
    let (mut index, _) = library();
    let serif = family(&index, "SourceSerif-Regular");
    let faces = index
        .declare_license(serif, &web_license())
        .expect("declare");
    assert_eq!(
        faces,
        [
            face(&index, "SourceSerif-Regular"),
            face(&index, "sourceserif-Bold")
        ]
    );
    for postscript in ["SourceSerif-Regular", "sourceserif-Bold"] {
        let license = index
            .face_detail(face(&index, postscript))
            .expect("detail")
            .license;
        assert_eq!(license.declared, Some(LicenseType::Web), "{postscript}");
        assert_eq!(license.seats, Some(5));
        assert_eq!(license.purchase_date.as_deref(), Some("2026-03-01"));
        assert_eq!(
            license.receipt.map(PathBuf::from),
            Some(PathBuf::from("/app-data/receipts/invoice.pdf"))
        );
    }
    assert_eq!(summary_license(&index, serif), Some(LicenseType::Web));
    // The other family has none.
    let inter = family(&index, "Inter-Regular");
    assert_eq!(summary_license(&index, inter), None);
    let other = index
        .face_detail(face(&index, "Inter-Regular"))
        .expect("detail")
        .license;
    assert_eq!(
        (
            other.declared,
            other.seats,
            other.purchase_date,
            other.receipt
        ),
        (None, None, None, None)
    );
    // The list row and the license filter see it.
    let listed = index
        .list_families(&ListFamiliesRequest::default())
        .expect("list");
    let row = listed.items.iter().find(|f| f.id == serif).expect("row");
    assert_eq!(row.declared_license, Some(LicenseType::Web));
    let found = index
        .search(&SearchRequest {
            filters: Filters {
                license: Some(LicenseType::Web),
                ..Filters::default()
            },
            ..SearchRequest::default()
        })
        .expect("search");
    let ids: Vec<FamilyId> = found.families.iter().map(|h| h.family.id).collect();
    assert_eq!(ids, [serif]);
}

#[test]
fn each_field_can_be_cleared_and_an_empty_license_deletes_the_row() {
    let (mut index, _) = library();
    let serif = family(&index, "SourceSerif-Regular");
    index
        .declare_license(serif, &web_license())
        .expect("declare");
    let only_type = DeclaredLicense {
        license_type: Some(LicenseType::Desktop),
        ..DeclaredLicense::default()
    };
    index.declare_license(serif, &only_type).expect("narrow");
    let license = index
        .face_detail(face(&index, "SourceSerif-Regular"))
        .expect("detail")
        .license;
    assert_eq!(license.declared, Some(LicenseType::Desktop));
    assert_eq!(
        (license.seats, license.purchase_date, license.receipt),
        (None, None, None)
    );

    index
        .declare_license(serif, &DeclaredLicense::default())
        .expect("clear");
    assert_eq!(summary_license(&index, serif), None);
    let rows: u32 = index
        .conn
        .query_row("SELECT COUNT(*) FROM family_meta", [], |r| r.get(0))
        .expect("count");
    assert_eq!(rows, 0, "nothing left: the row is deleted");
}

#[test]
fn the_license_and_the_notes_of_a_family_clear_apart() {
    let (mut index, _) = library();
    let serif = family(&index, "SourceSerif-Regular");
    index
        .declare_license(serif, &web_license())
        .expect("declare");
    index
        .update_family_notes(&FamilyNotesUpdate {
            family: serif,
            notes: "for the rebrand".into(),
        })
        .expect("notes");
    // Clearing the notes keeps the license.
    index
        .update_family_notes(&FamilyNotesUpdate {
            family: serif,
            notes: String::new(),
        })
        .expect("clear notes");
    assert_eq!(summary_license(&index, serif), Some(LicenseType::Web));
    // Clearing the license keeps the notes.
    index
        .update_family_notes(&FamilyNotesUpdate {
            family: serif,
            notes: "for the rebrand".into(),
        })
        .expect("notes again");
    index
        .declare_license(serif, &DeclaredLicense::default())
        .expect("clear license");
    let summary = index
        .family_summary(serif)
        .expect("summary")
        .expect("family");
    assert_eq!(summary.declared_license, None);
    assert_eq!(summary.notes.as_deref(), Some("for the rebrand"));
}

#[test]
fn a_declared_license_outlives_a_rescan_and_a_rebuild() {
    let (mut index, source) = library();
    let serif = family(&index, "SourceSerif-Regular");
    index
        .declare_license(serif, &web_license())
        .expect("declare");
    // The fonts go away and come back: new face ids, the same family.
    index.persist_files(source, &[]).expect("emptied");
    index
        .conn
        .execute("DELETE FROM face", [])
        .expect("forget faces");
    index.persist_files(source, &files()).expect("again");
    let again = family(&index, "SourceSerif-Regular");
    assert_eq!(summary_license(&index, again), Some(LicenseType::Web));
    index.rebuild().expect("rebuild");
    index.persist_files(source, &files()).expect("rescan");
    let rebuilt = family(&index, "sourceserif-Bold");
    assert_eq!(summary_license(&index, rebuilt), Some(LicenseType::Web));
    let license = index
        .face_detail(face(&index, "sourceserif-Bold"))
        .expect("detail")
        .license;
    assert_eq!(license.seats, Some(5));
}

#[test]
fn an_unknown_family_is_not_found() {
    let (mut index, _) = library();
    let err = index
        .declare_license(FamilyId(999), &web_license())
        .expect_err("unknown");
    assert_eq!(err.code(), ErrorCode::NotFound);
    let err = index.license_target(FamilyId(999)).expect_err("unknown");
    assert_eq!(err.code(), ErrorCode::NotFound);
}

#[test]
fn a_declaration_marks_every_face_of_the_family_for_the_sidecar() {
    let (mut index, _) = library();
    index
        .conn
        .execute("DELETE FROM sidecar_dirty", [])
        .expect("clean");
    let serif = family(&index, "SourceSerif-Regular");
    index
        .declare_license(serif, &web_license())
        .expect("declare");
    let dirty: Vec<u32> = index
        .conn
        .prepare("SELECT face_id FROM sidecar_dirty ORDER BY face_id")
        .and_then(|mut s| s.query_map([], |r| r.get(0))?.collect())
        .expect("dirty");
    assert_eq!(
        dirty,
        [
            face(&index, "SourceSerif-Regular").0,
            face(&index, "sourceserif-Bold").0
        ]
    );
}

#[test]
fn a_receipt_in_the_fontlibrary_folder_is_relative_in_the_sidecar() {
    let root = PathBuf::from("/fonts");
    let mut license = web_license();
    license.receipt = Some(root.join(SIDECAR_DIR).join("receipts").join("invoice.pdf"));
    let written = license.to_sidecar(&root);
    assert_eq!(written.receipt.as_deref(), Some("receipts/invoice.pdf"));
    assert_eq!(DeclaredLicense::from_sidecar(&written, &root), license);
    // A teammate's folder is somewhere else: the receipt follows it.
    let theirs = PathBuf::from("/home/them/fonts");
    assert_eq!(
        DeclaredLicense::from_sidecar(&written, &theirs).receipt,
        Some(
            theirs
                .join(SIDECAR_DIR)
                .join("receipts")
                .join("invoice.pdf")
        )
    );
}

#[test]
fn a_receipt_elsewhere_round_trips_as_written_and_never_climbs_out() {
    let root = PathBuf::from("/fonts");
    // In the app's data folder: the absolute path, unchanged.
    let license = web_license();
    let written = license.to_sidecar(&root);
    assert_eq!(DeclaredLicense::from_sidecar(&written, &root), license);
    // A relative path with `..` is not resolved against the folder, and is
    // written back as it came.
    for odd in [
        "../outside.pdf",
        "receipts/../../x.pdf",
        "C:\\Receipts\\a.pdf",
    ] {
        let theirs = sidecar::License {
            receipt: Some(odd.into()),
            ..sidecar::License::default()
        };
        let read = DeclaredLicense::from_sidecar(&theirs, &root);
        assert_eq!(read.receipt, Some(PathBuf::from(odd)), "{odd}");
        assert_eq!(
            read.to_sidecar(&root).receipt.as_deref(),
            Some(odd),
            "{odd}"
        );
    }
}

#[test]
fn the_license_target_names_the_fontlibrary_folder_only_for_a_folder_that_is_there() {
    let dir = TempDir::new("license-target");
    let mut index = Index::open_in_memory().expect("open");
    let root = dir.path().to_path_buf();
    let source = index
        .add_source(root.to_str().expect("utf-8"), SourceKind::Watched)
        .expect("source")
        .id;
    let mut f = file("Inter", "Regular");
    f.path = root.join("Inter-Regular.ttf");
    index.persist_files(source, &[f]).expect("persist");
    let inter = family(&index, "Inter-Regular");
    let target = index.license_target(inter).expect("target");
    assert_eq!(target.sidecar_dir, Some(root.join(SIDECAR_DIR)));
    assert_eq!(target.current, None);

    // "/fonts" is not there.
    let (index, _) = library();
    let serif = family(&index, "SourceSerif-Regular");
    assert_eq!(
        index.license_target(serif).expect("target").sidecar_dir,
        None
    );
}
