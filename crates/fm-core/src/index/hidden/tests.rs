//! Hidden copies and answered sets in the index and in the sidecars
//! (E56.8): hidden across scans and a rebuild, another file at the path
//! shown, and both carried to a fresh index over the same folder.

use std::path::{Path, PathBuf};

use fm_api::SourceId;
use fm_api::cleanup::{DuplicateAnswer, decision_key};
use fm_api::common::{FontFormat, Health};
use fm_api::library::SourceKind;

use super::*;
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, IndexedFile};
use crate::sidecar::{SIDECAR_DIR, SIDECAR_VERSION, Sidecar, file_name};
use crate::test_support::TempDir;

/// A file of the family `Inter` with the hash `[n; 32]`.
fn file(root: &Path, rel: &str, n: u8) -> IndexedFile {
    let mut face = ParsedFace::empty(0, Health::Ok);
    face.family = Some("Inter".to_owned());
    face.subfamily = Some("Regular".to_owned());
    face.postscript_name = Some("Inter-Regular".to_owned());
    IndexedFile {
        path: rel
            .split('/')
            .fold(root.to_path_buf(), |p, part| p.join(part)),
        size: 1000,
        modified: None,
        hash: FileHash::from([n; 32]),
        parsed: ParsedFile {
            format: Some(FontFormat::Ttf),
            health: Health::Ok,
            faces: vec![face],
        },
        gone: false,
    }
}

fn hex(n: u8) -> String {
    blake3::Hash::from_bytes([n; 32]).to_hex().to_string()
}

/// An index over a folder that exists, with `a/Inter.ttf` (hash 1) and
/// `b/Inter.ttf` (hash 2) on disk.
struct Machine {
    index: Index,
    source: SourceId,
    root: PathBuf,
}

impl Machine {
    fn new(root: &Path) -> Self {
        for dir in ["a", "b"] {
            std::fs::create_dir_all(root.join(dir)).expect("folder");
            std::fs::write(root.join(dir).join("Inter.ttf"), dir).expect("file");
        }
        let mut index = Index::open_in_memory().expect("open");
        let source = index
            .add_source(&root.to_string_lossy(), SourceKind::Watched)
            .expect("source")
            .id;
        let mut m = Self {
            index,
            source,
            root: root.to_path_buf(),
        };
        m.scan(1);
        m
    }

    /// A scan that finds both files, `a` with the hash `[a; 32]`.
    fn scan(&mut self, a: u8) {
        let files = [
            file(&self.root, "a/Inter.ttf", a),
            file(&self.root, "b/Inter.ttf", 2),
        ];
        self.index.persist_files(self.source, &files).expect("scan");
    }

    fn a(&self) -> PathBuf {
        self.root.join("a").join("Inter.ttf")
    }

    fn gone(&self, path: &Path) -> bool {
        self.index
            .conn
            .query_row(
                "SELECT gone FROM font_file WHERE path = ?1",
                [super::super::files::path_to_sql(path)],
                |r| r.get(0),
            )
            .expect("gone")
    }

    fn hide_a(&mut self) {
        let at = self.index.now().expect("now");
        assert!(hide(&self.index.conn, &self.a(), &hex(1), &at, None).expect("hide"));
        self.index.mirror_sidecars();
    }

    fn sidecar(&self) -> Sidecar {
        let path = self.root.join(SIDECAR_DIR).join(file_name("Inter"));
        let text = std::fs::read_to_string(path).expect("sidecar");
        match crate::sidecar::parse(&text) {
            crate::sidecar::Parsed::Sidecar(s) => s,
            other => panic!("not a sidecar: {other:?}"),
        }
    }
}

#[test]
fn a_hidden_copy_stays_hidden_across_scans_and_a_rebuild() {
    let tmp = TempDir::new("hidden-scans");
    let mut m = Machine::new(tmp.path());
    assert!(!m.gone(&m.a()));
    m.hide_a();
    assert!(m.gone(&m.a()), "hiding leaves the file gone");
    assert!(m.a().is_file(), "the file stays where it is");
    m.scan(1);
    assert!(m.gone(&m.a()), "a scan does not show it again");
    m.index.rebuild().expect("rebuild");
    m.scan(1);
    assert!(m.gone(&m.a()), "nor does a rebuild");
    let hidden = m.index.hidden_copies().expect("hidden");
    assert_eq!(hidden.len(), 1);
    assert!(hidden[0].in_effect);
    assert_eq!(hidden[0].hash, hex(1));
    assert_eq!(hidden[0].family.as_deref(), Some("Inter"));
    assert!(!m.gone(&m.root.join("b").join("Inter.ttf")));
}

#[test]
fn another_file_at_a_hidden_path_is_shown_and_forgotten() {
    let tmp = TempDir::new("hidden-other-file");
    let mut m = Machine::new(tmp.path());
    m.hide_a();
    m.scan(3);
    assert!(!m.gone(&m.a()), "another file at the path is shown");
    assert!(m.index.hidden_copies().expect("hidden").is_empty());
    m.index.mirror_sidecars();
    assert_eq!(m.sidecar().duplicates, None, "and its sidecar forgets it");
}

#[test]
fn showing_again_clears_the_row_and_the_gone_flag() {
    let tmp = TempDir::new("hidden-show");
    let mut m = Machine::new(tmp.path());
    m.hide_a();
    m.index.show_hidden(&m.a()).expect("show");
    assert!(!m.gone(&m.a()));
    assert!(m.index.hidden_copies().expect("hidden").is_empty());
}

#[test]
fn decisions_are_remembered_by_their_files_hashes_and_can_be_taken_back() {
    let tmp = TempDir::new("hidden-decide");
    let mut m = Machine::new(tmp.path());
    let hashes = vec![hex(2), hex(1)];
    let id = m
        .index
        .decide_set("name:Inter-Regular", &hashes, DuplicateAnswer::KeepBoth)
        .expect("decide");
    let keys = m.index.decided_keys().expect("keys");
    assert!(keys.contains(&decision_key("name:Inter-Regular", &[hex(1), hex(2)])));
    let listed = m.index.duplicate_decisions().expect("decisions");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].files.len(), 2);
    assert_eq!(listed[0].hashes, vec![hex(1), hex(2)], "sorted");
    // Answering again changes the answer, not the count.
    m.index
        .decide_set(
            "name:Inter-Regular",
            &hashes,
            DuplicateAnswer::NotDuplicates,
        )
        .expect("again");
    let listed = m.index.duplicate_decisions().expect("decisions");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].answer, DuplicateAnswer::NotDuplicates);
    m.index.take_back_decision(id).expect("take back");
    assert!(m.index.decided_keys().expect("keys").is_empty());
    assert!(m.index.take_back_decision(id).is_err());
}

/// The sidecar carries both to a fresh index over the same folder (a
/// rebuilt index, another machine); what that index shows again and takes
/// back is carried back.
#[test]
fn a_fresh_index_reads_hidden_copies_and_decisions_back_from_the_sidecar() {
    let tmp = TempDir::new("hidden-sidecar");
    let mut first = Machine::new(tmp.path());
    first.hide_a();
    let hashes = vec![hex(1), hex(2)];
    first
        .index
        .decide_set("name:Inter-Regular", &hashes, DuplicateAnswer::KeepBoth)
        .expect("decide");
    let written = first.sidecar();
    assert_eq!(written.version, SIDECAR_VERSION);
    let dups = written
        .duplicates
        .clone()
        .expect("duplicates in the sidecar");
    assert_eq!(dups.hidden.len(), 1);
    assert_eq!(dups.hidden[0].file, "a/Inter.ttf");
    assert_eq!(dups.decisions.len(), 1);
    assert_eq!(dups.decisions[0].answer, "keepBoth");

    // A fresh index: the scan merges the sidecar.
    let mut second = Machine::new(tmp.path());
    assert!(second.gone(&second.a()), "hidden on the fresh index");
    let hidden = second.index.hidden_copies().expect("hidden");
    assert_eq!(hidden.len(), 1);
    assert_eq!(hidden[0].session, None, "read from the sidecar");
    assert!(
        second
            .index
            .decided_keys()
            .expect("keys")
            .contains(&decision_key("name:Inter-Regular", &hashes))
    );

    // The fresh index shows it again and takes the decision back; the
    // first one merges that from the sidecar.
    second.index.show_hidden(&second.a()).expect("show");
    let id = second.index.duplicate_decisions().expect("decisions")[0].id;
    second.index.take_back_decision(id).expect("take back");
    assert_eq!(second.sidecar().duplicates, None);
    // Make sure the stamp changes on file systems with coarse times.
    let path = tmp.path().join(SIDECAR_DIR).join(file_name("Inter"));
    let f = std::fs::File::options()
        .write(true)
        .open(&path)
        .expect("open");
    f.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(10))
        .expect("touch");
    first.index.sync_sidecars(first.source).expect("sync");
    assert!(!first.gone(&first.a()), "shown on the first index too");
    assert!(first.index.decided_keys().expect("keys").is_empty());
}
