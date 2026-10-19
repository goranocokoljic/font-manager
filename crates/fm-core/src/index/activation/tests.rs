use std::path::PathBuf;

use fm_api::FamilyId;
use fm_api::common::Health;
use fm_api::library::SourceKind;

use super::*;
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, IndexedFile};

fn index_with(paths: &[PathBuf]) -> (Index, Vec<FaceId>) {
    let mut index = Index::open_in_memory().expect("open");
    let source = index
        .add_source("/fonts", SourceKind::Watched)
        .expect("source")
        .id;
    let files: Vec<IndexedFile> = paths
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let mut face = ParsedFace::empty(0, Health::Ok);
            face.family = Some("Family".into());
            face.subfamily = Some(format!("Style {i}"));
            face.postscript_name = Some(format!("Family-{i}"));
            IndexedFile {
                path: path.clone(),
                size: 1,
                modified: None,
                hash: FileHash::from([1; 32]),
                parsed: ParsedFile {
                    format: Some(FontFormat::Ttf),
                    health: Health::Ok,
                    faces: vec![face],
                },
                gone: false,
            }
        })
        .collect();
    index.persist_files(source, &files).expect("persist");
    let ids = index
        .face_ids("SELECT id FROM face WHERE id > ?1 ORDER BY id", 0)
        .expect("ids");
    (index, ids)
}

#[test]
fn a_face_list_keeps_its_order_without_repeats() {
    let (index, ids) = index_with(&[PathBuf::from("/fonts/a.ttf"), PathBuf::from("/fonts/b.ttf")]);
    let faces = index
        .target_faces(&Target::Faces {
            faces: vec![ids[1], ids[0], ids[1]],
        })
        .expect("faces");
    assert_eq!(faces, vec![ids[1], ids[0]]);
    let family = index
        .target_faces(&Target::Family {
            family: FamilyId(ids[0].0),
        })
        .expect("family");
    assert_eq!(family, ids);
}

#[test]
fn registrations_round_trip_with_paths_that_are_not_utf8() {
    #[cfg(windows)]
    let odd = fm_platform::fs::path_from_bytes(&[0x41, 0x00, 0x00, 0xD8]).expect("path");
    #[cfg(unix)]
    let odd = {
        use std::os::unix::ffi::OsStrExt;
        PathBuf::from(std::ffi::OsStr::from_bytes(b"/fonts/\xff.ttf"))
    };
    assert!(odd.to_str().is_none());
    let (mut index, ids) = index_with(&[odd.clone(), PathBuf::from("/fonts/b.ttf")]);
    let reg = Registration {
        path: odd.clone(),
        mode: Mode::Permanent,
        handle: odd.clone().into_os_string(),
    };
    index
        .record_activations(&[(ids[0], ActivationMode::Permanent, reg.clone())])
        .expect("record");
    let rows = index.active_rows().expect("rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].face, ids[0]);
    assert_eq!(rows[0].mode, ActivationMode::Permanent);
    assert_eq!(rows[0].registration, Some(reg));
    let face = index.activation_faces(&[ids[0]]).expect("faces");
    assert_eq!(face[0].path, odd);
}

#[test]
fn removing_an_activation_removes_its_registration_and_last_session_follows_faces() {
    let (mut index, ids) = index_with(&[PathBuf::from("/fonts/a.ttf")]);
    let reg = Registration {
        path: PathBuf::from("/fonts/a.ttf"),
        mode: Mode::Session,
        handle: "h1".into(),
    };
    index
        .record_activations(&[(ids[0], ActivationMode::Session, reg)])
        .expect("record");
    index.set_last_session(&ids).expect("last");
    assert_eq!(index.last_session().expect("last"), ids);
    index.remove_activations(&ids).expect("remove");
    assert_eq!(index.active_rows().expect("rows"), vec![]);
    let left: u32 = index
        .conn
        .query_row("SELECT COUNT(*) FROM activation_registration", [], |r| {
            r.get(0)
        })
        .expect("count");
    assert_eq!(left, 0);
    index.set_last_session(&[]).expect("clear");
    assert_eq!(index.last_session().expect("last"), vec![]);
}

#[test]
fn an_active_face_cannot_leave_the_index() {
    let (mut index, ids) = index_with(&[PathBuf::from("/fonts/a.ttf")]);
    let reg = Registration {
        path: PathBuf::from("/fonts/a.ttf"),
        mode: Mode::Session,
        handle: "h1".into(),
    };
    index
        .record_activations(&[(ids[0], ActivationMode::Session, reg)])
        .expect("record");
    let err = index
        .conn
        .execute("DELETE FROM face WHERE id = ?1", [ids[0].0])
        .expect_err("restricted");
    assert!(err.to_string().contains("FOREIGN KEY"), "{err}");
}

#[test]
fn activatable_faces_leave_out_what_cannot_be_registered() {
    let (index, ids) = index_with(&[
        PathBuf::from("/fonts/a.ttf"),
        PathBuf::from("/fonts/b.ttf"),
        PathBuf::from("/fonts/c.ttf"),
    ]);
    index
        .conn
        .execute_batch(
            "UPDATE font_file SET gone = 1 WHERE path = '/fonts/b.ttf';
             UPDATE font_file SET format = 'woff2' WHERE path = '/fonts/c.ttf';",
        )
        .expect("update");
    assert_eq!(index.activatable_faces(10).expect("faces"), vec![ids[0]]);
    assert_eq!(index.activatable_faces(0).expect("faces"), vec![]);
}
