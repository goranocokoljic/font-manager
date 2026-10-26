use std::path::{Path, PathBuf};

use fm_api::FaceId;
use fm_api::common::{FontFormat, Health};
use fm_api::library::{ProblemKind, SourceKind};
use fm_api::organization::{
    CollectionFacesRequest, CreateCollectionRequest, FamilyNotesUpdate, RenameCollectionRequest,
    TagFacesRequest, UntagFacesRequest, UserMetaUpdate,
};

use super::*;
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, FileIndex, IndexedFile, Rescan, discover};
use crate::sidecar::{FaceKey, Meta, PLAIN_VERSION, SIDECAR_DIR, Sidecar, file_name};
use crate::test_support::TempDir;

/// (file relative to the source, family, style).
const LIBRARY: &[(&str, &str, &str)] = &[
    ("Inter-Regular.ttf", "Inter", "Regular"),
    ("sub/Inter-Bold.ttf", "Inter", "Bold"),
    ("Lora-Regular.ttf", "Lora", "Regular"),
];

fn file(root: &Path, rel: &str, family: &str, style: &str) -> IndexedFile {
    let mut face = ParsedFace::empty(0, Health::Ok);
    face.family = Some(family.to_owned());
    face.subfamily = Some(style.to_owned());
    face.postscript_name = Some(format!("{family}-{style}"));
    face.weight = Some(400);
    face.width = Some(5);
    IndexedFile {
        path: root.join(rel),
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

fn files(root: &Path) -> Vec<IndexedFile> {
    LIBRARY
        .iter()
        .map(|(rel, family, style)| file(root, rel, family, style))
        .collect()
}

/// An index over a source folder that exists, as one machine sees it.
struct Machine {
    index: Index,
    source: SourceId,
    root: PathBuf,
}

impl Machine {
    fn new(root: &Path) -> Self {
        Self::with_kind(root, SourceKind::Watched)
    }

    fn with_kind(root: &Path, kind: SourceKind) -> Self {
        let mut index = Index::open_in_memory().expect("open");
        let source = index
            .add_source(&root.to_string_lossy(), kind)
            .expect("source")
            .id;
        index.persist_files(source, &files(root)).expect("persist");
        Self {
            index,
            source,
            root: root.to_path_buf(),
        }
    }

    /// A scan that finds every file again.
    fn scan(&mut self) {
        self.index
            .persist_files(self.source, &files(&self.root))
            .expect("scan");
    }

    fn face(&self, postscript: &str) -> FaceId {
        self.index
            .conn
            .query_row(
                "SELECT id FROM face WHERE postscript_name = ?1",
                [postscript],
                |r| r.get(0),
            )
            .map(FaceId)
            .expect("face id")
    }

    fn tag(&mut self, postscript: &str, tags: &[&str]) {
        let face = self.face(postscript);
        self.index
            .tag_faces(&TagFacesRequest {
                families: Vec::new(),
                faces: vec![face],
                tags: tags.iter().map(|t| (*t).to_owned()).collect(),
            })
            .expect("tag");
    }

    fn meta(&mut self, postscript: &str, update: UserMetaUpdate) {
        let face = self.face(postscript);
        self.index
            .update_user_meta(&UserMetaUpdate {
                faces: vec![face],
                ..update
            })
            .expect("meta");
    }

    /// Sets the notes of the family of the face `postscript`.
    fn family_notes(&mut self, postscript: &str, notes: &str) {
        let face = self.face(postscript);
        let (family, _) = self
            .index
            .family_of(face)
            .expect("family")
            .expect("visible");
        self.index
            .update_family_notes(&FamilyNotesUpdate {
                family,
                notes: notes.into(),
            })
            .expect("family notes");
    }

    /// Declares the license of the family of the face `postscript`.
    fn declare(&mut self, postscript: &str, license: &DeclaredLicense) {
        let face = self.face(postscript);
        let (family, _) = self
            .index
            .family_of(face)
            .expect("family")
            .expect("visible");
        self.index
            .declare_license(family, license)
            .expect("declare");
    }

    /// The license the index holds for `family`.
    fn local_license(&self, family: &str) -> Option<DeclaredLicense> {
        self.index.declared_license(family).expect("license")
    }

    /// The notes the index holds for `family`.
    fn local_family_notes(&self, family: &str) -> Option<String> {
        self.index.family_notes(family).expect("family notes")
    }

    /// What the index holds for a face, as a sidecar would write it.
    fn local(&self, postscript: &str) -> Meta {
        let face = self.face(postscript).0;
        let tx = self.index.conn.unchecked_transaction().expect("tx");
        let collections = Collections::load(&tx).expect("collections");
        local_meta(&tx, face, &collections).expect("meta")
    }

    fn sidecar_path(&self, family: &str) -> PathBuf {
        self.root.join(SIDECAR_DIR).join(file_name(family))
    }

    fn sidecar(&self, family: &str) -> Sidecar {
        let text = std::fs::read_to_string(self.sidecar_path(family)).expect("sidecar");
        match sidecar::parse(&text) {
            Parsed::Sidecar(s) => s,
            other => panic!("not a sidecar: {other:?}"),
        }
    }

    fn entry(&self, family: &str, file: &str) -> Option<Meta> {
        self.sidecar(family)
            .by_face()
            .get(&FaceKey {
                file: file.into(),
                index: 0,
            })
            .map(|m| (*m).clone())
    }

    /// Edits a family's sidecar as another machine that has read it would.
    fn write_as_other_machine(&self, family: &str, edit: impl FnOnce(&mut Sidecar)) {
        let sidecar = other_machine_edit(self.sidecar(family), edit);
        write_changed(&self.sidecar_path(family), &sidecar.to_json());
    }
}

/// Writes `text` and makes sure the file's stamp changes, as a sync client
/// replacing the file would, even on a file system with coarse times.
fn write_changed(path: &Path, text: &str) {
    let before = std::fs::metadata(path).ok().map(|m| stamp(&m));
    std::fs::write(path, text).expect("write");
    let file = std::fs::File::options()
        .write(true)
        .open(path)
        .expect("open");
    let mut when = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    loop {
        file.set_modified(when).expect("touch");
        if Some(stamp(&std::fs::metadata(path).expect("meta"))) != before {
            break;
        }
        when += std::time::Duration::from_secs(5);
    }
}

/// `sidecar` edited by another machine: each entry it changes records the
/// value it replaced, as this build writes it.
fn other_machine_edit(mut sidecar: Sidecar, edit: impl FnOnce(&mut Sidecar)) -> Sidecar {
    let file_before = sidecar.clone();
    let before: std::collections::HashMap<_, _> =
        sidecar.faces.iter().map(|f| (f.key(), f.clone())).collect();
    edit(&mut sidecar);
    for face in &mut sidecar.faces {
        face.history = sidecar::next_history(&face.meta, before.get(&face.key()));
    }
    if let Some(family) = &mut sidecar.family_meta {
        family.history =
            sidecar::next_family_history(family.notes.as_deref(), file_before.family_meta.as_ref());
        if let Some(license) = &mut family.license {
            license.history = sidecar::next_license_history(
                &license.license,
                file_before
                    .family_meta
                    .as_ref()
                    .and_then(|f| f.license.as_ref()),
            );
        }
    }
    sidecar.normalize();
    sidecar.history = sidecar::next_file_history(&sidecar, Some(&file_before));
    sidecar
}

/// The sidecar text as it is on disk now.
fn read_sidecar(path: &Path) -> Sidecar {
    match sidecar::parse(&std::fs::read_to_string(path).expect("read")) {
        Parsed::Sidecar(s) => s,
        other => panic!("not a sidecar: {other:?}"),
    }
}

fn entry_mut<'a>(sidecar: &'a mut Sidecar, file: &str) -> &'a mut Meta {
    &mut sidecar
        .faces
        .iter_mut()
        .find(|f| f.file == file)
        .expect("entry")
        .meta
}

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| (*n).to_owned()).collect()
}

#[test]
fn metadata_is_mirrored_to_one_sidecar_per_family() {
    let tmp = TempDir::new("sidecar-mirror");
    let mut m = Machine::new(tmp.path());
    assert!(
        !m.root.join(SIDECAR_DIR).exists(),
        "nothing is written before the user sets metadata"
    );

    m.tag("Inter-Bold", &["Display", "client"]);
    m.meta(
        "Inter-Bold",
        UserMetaUpdate {
            favorite: Some(true),
            rating: Some(4),
            notes: Some("Licensed for the 2026 rebrand".into()),
            ..UserMetaUpdate::default()
        },
    );
    let work = m
        .index
        .create_collection(&CreateCollectionRequest {
            rule_group: None,
            name: "Work".into(),
            parent: None,
            rules: None,
        })
        .expect("work");
    let client = m
        .index
        .create_collection(&CreateCollectionRequest {
            rule_group: None,
            name: "Client".into(),
            parent: Some(work.id),
            rules: None,
        })
        .expect("client");
    m.index
        .add_to_collection(&CollectionFacesRequest {
            collection: client.id,
            faces: vec![m.face("Inter-Bold")],
            families: Vec::new(),
        })
        .expect("add");
    m.meta(
        "Lora-Regular",
        UserMetaUpdate {
            rating: Some(2),
            ..UserMetaUpdate::default()
        },
    );

    let inter = m.sidecar("Inter");
    assert_eq!(
        inter.version, PLAIN_VERSION,
        "no pins: readable by older builds"
    );
    assert_eq!(inter.family, "Inter");
    // Only the face with metadata, named by its path with `/` separators.
    assert_eq!(inter.faces.len(), 1);
    let bold = &inter.faces[0];
    assert_eq!(bold.file, "sub/Inter-Bold.ttf");
    assert_eq!(bold.postscript_name.as_deref(), Some("Inter-Bold"));
    assert_eq!(
        bold.meta,
        Meta {
            favorite: true,
            rating: Some(4),
            notes: Some("Licensed for the 2026 rebrand".into()),
            tags: names(&["client", "Display"]),
            collections: vec![names(&["Work", "Client"])],
            pins: Vec::new(),
        }
    );
    assert_eq!(
        m.entry("Lora", "Lora-Regular.ttf").and_then(|e| e.rating),
        Some(2)
    );
    assert_ne!(m.sidecar_path("Inter"), m.sidecar_path("Lora"));
}

#[test]
fn every_kind_of_change_reaches_the_sidecar() {
    let tmp = TempDir::new("sidecar-changes");
    let mut m = Machine::new(tmp.path());
    let face = m.face("Inter-Regular");
    m.tag("Inter-Regular", &["serif", "body"]);
    let parent = m
        .index
        .create_collection(&CreateCollectionRequest {
            rule_group: None,
            name: "Old".into(),
            parent: None,
            rules: None,
        })
        .expect("parent");
    let child = m
        .index
        .create_collection(&CreateCollectionRequest {
            rule_group: None,
            name: "Child".into(),
            parent: Some(parent.id),
            rules: None,
        })
        .expect("child");
    let add = CollectionFacesRequest {
        collection: child.id,
        faces: vec![face],
        families: Vec::new(),
    };
    m.index.add_to_collection(&add).expect("add");

    // Renaming a parent changes the path of the faces of its children.
    m.index
        .rename_collection(&RenameCollectionRequest {
            collection: parent.id,
            name: "New".into(),
        })
        .expect("rename");
    let entry = m.entry("Inter", "Inter-Regular.ttf").expect("entry");
    assert_eq!(entry.collections, [names(&["New", "Child"])]);

    let serif = m
        .index
        .tags()
        .expect("tags")
        .into_iter()
        .find(|t| t.name == "serif")
        .expect("serif")
        .id;
    m.index
        .untag_faces(&UntagFacesRequest {
            faces: vec![face],
            tags: vec![serif],
        })
        .expect("untag");
    assert_eq!(
        m.entry("Inter", "Inter-Regular.ttf").expect("entry").tags,
        ["body"]
    );

    m.index.delete_collection(parent.id).expect("delete");
    assert!(
        m.entry("Inter", "Inter-Regular.ttf")
            .expect("entry")
            .collections
            .is_empty()
    );

    // With nothing left on the face, its entry goes.
    let body = m.index.tags().expect("tags")[0].id;
    m.index
        .untag_faces(&UntagFacesRequest {
            faces: vec![face],
            tags: vec![body],
        })
        .expect("untag");
    assert!(m.entry("Inter", "Inter-Regular.ttf").is_none());
    assert!(m.index.write_sidecars().expect("write") == SidecarSync::default());
}

#[test]
fn a_change_made_on_another_machine_merges_on_scan() {
    let tmp = TempDir::new("sidecar-merge");
    let mut m = Machine::new(tmp.path());
    m.tag("Inter-Bold", &["display", "old"]);
    m.meta(
        "Inter-Bold",
        UserMetaUpdate {
            rating: Some(3),
            ..UserMetaUpdate::default()
        },
    );

    m.write_as_other_machine("Inter", |s| {
        let e = entry_mut(s, "sub/Inter-Bold.ttf");
        e.rating = Some(5);
        e.notes = Some("from the laptop".into());
        e.tags = names(&["display", "headline"]);
        e.collections = vec![names(&["Synced", "Deep"])];
        s.faces.push(FaceEntry {
            file: "Inter-Regular.ttf".into(),
            index: 0,
            postscript_name: None,
            meta: Meta {
                favorite: true,
                ..Meta::default()
            },
            history: Vec::new(),
        });
    });
    m.scan();

    let bold = m.local("Inter-Bold");
    assert_eq!(bold.rating, Some(5));
    assert_eq!(bold.notes.as_deref(), Some("from the laptop"));
    // "old" was removed there and left alone here: it goes.
    assert_eq!(bold.tags, ["display", "headline"]);
    assert_eq!(bold.collections, [names(&["Synced", "Deep"])]);
    assert!(m.local("Inter-Regular").favorite);
    assert!(
        m.index
            .tags()
            .expect("tags")
            .iter()
            .all(|t| t.name != "old"),
        "a tag no face carries is deleted"
    );
    assert!(m.index.sidecar_problems().expect("problems").is_empty());
    // A second scan of the unchanged file merges nothing again.
    assert_eq!(
        m.index.sync_sidecars(m.source).expect("sync"),
        SidecarSync::default()
    );
}

#[test]
fn a_rescan_merges_too() {
    let tmp = TempDir::new("sidecar-rescan");
    let mut m = Machine::new(tmp.path());
    m.tag("Lora-Regular", &["a"]);
    m.write_as_other_machine("Lora", |s| {
        entry_mut(s, "Lora-Regular.ttf").tags = names(&["a", "b"]);
    });
    let mut scanned = FileIndex::new();
    for f in files(&m.root) {
        scanned.insert(f);
    }
    let rescan = Rescan {
        unchanged: LIBRARY.iter().map(|(rel, ..)| m.root.join(rel)).collect(),
        ..Rescan::default()
    };
    m.index
        .apply_rescan(m.source, &scanned, &rescan)
        .expect("rescan");
    assert_eq!(m.local("Lora-Regular").tags, ["a", "b"]);
}

#[test]
fn a_new_machine_takes_the_metadata_from_the_sidecars() {
    let tmp = TempDir::new("sidecar-new-machine");
    let mut first = Machine::new(tmp.path());
    first.tag("Inter-Bold", &["display"]);
    first.meta(
        "Inter-Bold",
        UserMetaUpdate {
            rating: Some(4),
            notes: Some("notes".into()),
            ..UserMetaUpdate::default()
        },
    );
    let work = first
        .index
        .create_collection(&CreateCollectionRequest {
            rule_group: None,
            name: "Work".into(),
            parent: None,
            rules: None,
        })
        .expect("work");
    first
        .index
        .add_to_collection(&CollectionFacesRequest {
            collection: work.id,
            faces: vec![first.face("Inter-Bold")],
            families: Vec::new(),
        })
        .expect("add");
    let written = first.sidecar("Inter");

    // A second index over the same folder: another machine, or this one
    // after the index was rebuilt.
    let second = Machine::new(tmp.path());
    assert_eq!(second.local("Inter-Bold"), first.local("Inter-Bold"));
    assert_eq!(
        second.index.collections().expect("collections")[0].name,
        "Work"
    );
    assert_eq!(second.sidecar("Inter"), written, "nothing to write back");
}

#[test]
fn a_field_changed_on_both_machines_is_a_problem_and_keeps_both_values() {
    let tmp = TempDir::new("sidecar-conflict");
    let mut m = Machine::new(tmp.path());
    m.meta(
        "Inter-Bold",
        UserMetaUpdate {
            rating: Some(3),
            ..UserMetaUpdate::default()
        },
    );
    // Both machines change the rating before they sync.
    let before = std::fs::read_to_string(m.sidecar_path("Inter")).expect("read");
    m.meta(
        "Inter-Bold",
        UserMetaUpdate {
            rating: Some(4),
            ..UserMetaUpdate::default()
        },
    );
    let theirs = match sidecar::parse(&before) {
        Parsed::Sidecar(s) => s,
        other => panic!("{other:?}"),
    };
    // The other machine changed the same 3 stars to 2, and the sync client
    // kept its file.
    let theirs = other_machine_edit(theirs, |s| {
        entry_mut(s, "sub/Inter-Bold.ttf").rating = Some(2);
    });
    write_changed(&m.sidecar_path("Inter"), &theirs.to_json());

    let sync = m.index.sync_sidecars(m.source).expect("sync");
    assert_eq!(sync.conflicts, 1);
    assert_eq!(
        m.local("Inter-Bold").rating,
        Some(4),
        "this machine's value stays"
    );
    assert_eq!(
        m.entry("Inter", "sub/Inter-Bold.ttf")
            .and_then(|e| e.rating),
        Some(4)
    );

    let problems = m.index.sidecar_problems().expect("problems");
    assert_eq!(problems.len(), 1);
    let p = &problems[0];
    assert_eq!(p.kind, ProblemKind::SidecarConflict);
    assert_eq!(p.face, Some(m.face("Inter-Bold")));
    assert_eq!(Path::new(&p.path), m.root.join("sub/Inter-Bold.ttf"));
    assert_eq!(
        p.other.as_deref().map(Path::new),
        Some(m.sidecar_path("Inter").as_path())
    );
    assert!(p.message.contains("2 stars"), "{}", p.message);
    assert!(p.message.contains("4 stars"), "{}", p.message);

    // It stays through later scans, and goes when the user sets the rating.
    m.scan();
    assert_eq!(m.index.sidecar_problems().expect("problems").len(), 1);
    m.meta(
        "Inter-Bold",
        UserMetaUpdate {
            notes: Some("unrelated".into()),
            ..UserMetaUpdate::default()
        },
    );
    assert_eq!(m.index.sidecar_problems().expect("problems").len(), 1);
    m.meta(
        "Inter-Bold",
        UserMetaUpdate {
            rating: Some(2),
            ..UserMetaUpdate::default()
        },
    );
    assert!(m.index.sidecar_problems().expect("problems").is_empty());
}

#[test]
fn an_unreadable_sidecar_is_a_problem_and_is_never_overwritten() {
    let tmp = TempDir::new("sidecar-unreadable");
    let mut m = Machine::new(tmp.path());
    m.tag("Lora-Regular", &["a"]);
    let path = m.sidecar_path("Lora");
    let good = std::fs::read_to_string(&path).expect("read");
    let garbage = b"{ \"version\": 1, \"faces\": [ \xff truncated";
    std::fs::write(&path, garbage).expect("write");

    m.scan();
    m.tag("Lora-Regular", &["b"]);
    assert_eq!(std::fs::read(&path).expect("read"), garbage);
    let problems = m.index.sidecar_problems().expect("problems");
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].kind, ProblemKind::SidecarConflict);
    assert_eq!(problems[0].face, None);
    assert_eq!(Path::new(&problems[0].path), path);
    assert_eq!(
        m.local("Lora-Regular").tags,
        ["a", "b"],
        "the index keeps working"
    );

    // Once the file is fixed it merges, the problem goes and the edits made
    // meanwhile are written.
    write_changed(&path, &good);
    m.scan();
    assert!(m.index.sidecar_problems().expect("problems").is_empty());
    assert_eq!(
        m.entry("Lora", "Lora-Regular.ttf").expect("entry").tags,
        ["a", "b"]
    );
}

#[test]
fn a_sidecar_from_a_newer_version_is_reported_and_left_alone() {
    let tmp = TempDir::new("sidecar-newer");
    let mut m = Machine::new(tmp.path());
    m.tag("Lora-Regular", &["a"]);
    let path = m.sidecar_path("Lora");
    let newer = r#"{"version": 99, "faces": {"future": true}}"#;
    write_changed(&path, newer);
    m.scan();
    m.tag("Lora-Regular", &["b"]);
    assert_eq!(std::fs::read_to_string(&path).expect("read"), newer);
    let problems = m.index.sidecar_problems().expect("problems");
    assert_eq!(problems.len(), 1);
    assert!(
        problems[0].message.contains("newer"),
        "{}",
        problems[0].message
    );
}

#[test]
fn entries_of_faces_not_indexed_here_are_kept_and_taken_when_they_arrive() {
    let tmp = TempDir::new("sidecar-unindexed");
    let mut m = Machine::new(tmp.path());
    m.tag("Inter-Regular", &["a"]);
    // The other machine has a font this one has not received yet.
    m.write_as_other_machine("Inter", |s| {
        s.faces.push(FaceEntry {
            file: "Inter-Italic.ttf".into(),
            index: 0,
            postscript_name: Some("Inter-Italic".into()),
            meta: Meta {
                rating: Some(5),
                ..Meta::default()
            },
            history: Vec::new(),
        });
    });
    m.scan();
    m.tag("Inter-Regular", &["b"]);
    assert_eq!(
        m.entry("Inter", "Inter-Italic.ttf").and_then(|e| e.rating),
        Some(5),
        "rewriting the sidecar keeps the entry"
    );

    // The font arrives through the sync client.
    let mut all = files(&m.root);
    all.push(file(&m.root, "Inter-Italic.ttf", "Inter", "Italic"));
    m.index.persist_files(m.source, &all).expect("scan");
    assert_eq!(m.local("Inter-Italic").rating, Some(5));
}

#[test]
fn a_deleted_sidecar_is_written_again_without_losing_metadata() {
    let tmp = TempDir::new("sidecar-deleted");
    let mut m = Machine::new(tmp.path());
    m.tag("Inter-Regular", &["a"]);
    std::fs::remove_file(m.sidecar_path("Inter")).expect("remove");
    m.scan();
    m.tag("Lora-Regular", &["x"]);
    assert_eq!(m.local("Inter-Regular").tags, ["a"]);
    m.tag("Inter-Regular", &["b"]);
    assert_eq!(
        m.entry("Inter", "Inter-Regular.ttf").expect("entry").tags,
        ["a", "b"]
    );
}

#[test]
fn a_sync_clients_conflicted_copy_is_merged_and_left_alone() {
    let tmp = TempDir::new("sidecar-copy");
    let mut m = Machine::new(tmp.path());
    m.tag("Lora-Regular", &["a"]);
    let copy = other_machine_edit(m.sidecar("Lora"), |s| {
        entry_mut(s, "Lora-Regular.ttf").tags = names(&["a", "from-copy"]);
    });
    let copy_path = m.root.join(SIDECAR_DIR).join("Lora (conflicted copy).json");
    std::fs::write(&copy_path, copy.to_json()).expect("write");

    m.scan();
    assert_eq!(m.local("Lora-Regular").tags, ["a", "from-copy"]);
    assert_eq!(
        m.entry("Lora", "Lora-Regular.ttf").expect("entry").tags,
        ["a", "from-copy"]
    );
    assert_eq!(
        std::fs::read_to_string(&copy_path).expect("copy"),
        copy.to_json()
    );
}

#[test]
fn smart_collection_on_the_way_is_a_problem_not_a_lost_membership() {
    let tmp = TempDir::new("sidecar-smart");
    let mut m = Machine::new(tmp.path());
    m.tag("Lora-Regular", &["a"]);
    m.index
        .create_collection(&CreateCollectionRequest {
            rule_group: None,
            name: "Serifs".into(),
            parent: None,
            rules: Some(fm_api::search::Filters::default()),
        })
        .expect("smart");
    m.write_as_other_machine("Lora", |s| {
        entry_mut(s, "Lora-Regular.ttf").collections = vec![names(&["Serifs"])];
    });
    m.scan();
    let problems = m.index.sidecar_problems().expect("problems");
    assert_eq!(problems.len(), 1);
    assert!(
        problems[0].message.contains("Serifs"),
        "{}",
        problems[0].message
    );
    // The membership stays in the sidecar for the machine that made it,
    // through this machine's later edits and scans.
    m.tag("Lora-Regular", &["b"]);
    m.scan();
    let entry = m.entry("Lora", "Lora-Regular.ttf").expect("entry");
    assert_eq!(entry.collections, [names(&["Serifs"])]);
    assert_eq!(entry.tags, ["a", "b"]);
}

#[test]
fn a_membership_blocked_here_survives_the_round_trip_on_the_other_machine() {
    let tmp = TempDir::new("sidecar-smart-round-trip");
    // Two machines over one synced folder: "Serifs" is manual on B and
    // smart on A.
    let mut b = Machine::new(tmp.path());
    let serifs = b
        .index
        .create_collection(&CreateCollectionRequest {
            rule_group: None,
            name: "Serifs".into(),
            parent: None,
            rules: None,
        })
        .expect("manual");
    b.index
        .add_to_collection(&CollectionFacesRequest {
            collection: serifs.id,
            faces: vec![b.face("Lora-Regular")],
            families: Vec::new(),
        })
        .expect("add");

    let mut a = Index::open_in_memory().expect("open");
    a.create_collection(&CreateCollectionRequest {
        rule_group: None,
        name: "Serifs".into(),
        parent: None,
        rules: Some(fm_api::search::Filters::default()),
    })
    .expect("smart");
    let a_source = a
        .add_source(&tmp.path().to_string_lossy(), SourceKind::Watched)
        .expect("source")
        .id;
    a.persist_files(a_source, &files(tmp.path()))
        .expect("scan a");
    assert_eq!(a.sidecar_problems().expect("problems").len(), 1);
    let lora = a
        .conn
        .query_row(
            "SELECT id FROM face WHERE postscript_name = 'Lora-Regular'",
            [],
            |r| r.get(0),
        )
        .map(FaceId)
        .expect("face");
    a.tag_faces(&TagFacesRequest {
        families: Vec::new(),
        faces: vec![lora],
        tags: vec!["from-a".into()],
    })
    .expect("tag on a");
    // Make sure B sees a new stamp, whatever the file system's time step.
    let path = b.sidecar_path("Lora");
    let text = std::fs::read_to_string(&path).expect("read");
    write_changed(&path, &text);

    b.scan();
    let local = b.local("Lora-Regular");
    assert_eq!(
        local.collections,
        [names(&["Serifs"])],
        "B keeps its membership"
    );
    assert_eq!(local.tags, ["from-a"]);
    assert!(b.index.sidecar_problems().expect("problems").is_empty());
}

#[test]
fn a_carried_membership_is_joined_once_the_smart_collection_is_gone() {
    let tmp = TempDir::new("sidecar-smart-gone");
    let mut m = Machine::new(tmp.path());
    m.tag("Lora-Regular", &["a"]);
    let smart = m
        .index
        .create_collection(&CreateCollectionRequest {
            rule_group: None,
            name: "Serifs".into(),
            parent: None,
            rules: Some(fm_api::search::Filters::default()),
        })
        .expect("smart");
    m.write_as_other_machine("Lora", |s| {
        entry_mut(s, "Lora-Regular.ttf").collections = vec![names(&["Serifs"])];
    });
    m.scan();
    assert_eq!(m.index.sidecar_problems().expect("problems").len(), 1);

    // The smart collection goes; an unrelated edit to the family follows.
    m.index.delete_collection(smart.id).expect("delete");
    m.tag("Lora-Regular", &["b"]);
    let entry = m.entry("Lora", "Lora-Regular.ttf").expect("entry");
    assert_eq!(
        entry.collections,
        [names(&["Serifs"])],
        "the other machine's membership is not written away"
    );
    m.scan();
    assert_eq!(
        m.local("Lora-Regular").collections,
        [names(&["Serifs"])],
        "joined here now that it can be"
    );
    assert!(m.index.sidecar_problems().expect("problems").is_empty());
}

#[test]
fn a_carried_membership_the_other_machine_dropped_is_let_go() {
    let tmp = TempDir::new("sidecar-smart-dropped");
    let mut m = Machine::new(tmp.path());
    m.tag("Lora-Regular", &["a"]);
    m.index
        .create_collection(&CreateCollectionRequest {
            rule_group: None,
            name: "Serifs".into(),
            parent: None,
            rules: Some(fm_api::search::Filters::default()),
        })
        .expect("smart");
    m.write_as_other_machine("Lora", |s| {
        entry_mut(s, "Lora-Regular.ttf").collections = vec![names(&["Serifs"])];
    });
    m.scan();
    m.write_as_other_machine("Lora", |s| {
        entry_mut(s, "Lora-Regular.ttf").collections.clear();
    });
    m.scan();
    assert!(m.index.sidecar_problems().expect("problems").is_empty());
    assert!(
        m.entry("Lora", "Lora-Regular.ttf")
            .expect("entry")
            .collections
            .is_empty()
    );
}

#[test]
fn an_entry_the_other_machine_never_had_is_kept() {
    let tmp = TempDir::new("sidecar-missing-entry");
    let mut m = Machine::new(tmp.path());
    m.tag("Inter-Regular", &["x"]);
    let older = m.sidecar("Inter");
    // This machine adds an entry; the other machine, from the older file,
    // rates another face; the sync client keeps the other machine's file.
    m.tag("Inter-Bold", &["mine"]);
    let theirs = other_machine_edit(older, |s| {
        entry_mut(s, "Inter-Regular.ttf").rating = Some(5);
    });
    write_changed(&m.sidecar_path("Inter"), &theirs.to_json());

    m.scan();
    assert_eq!(m.local("Inter-Bold").tags, ["mine"], "not wiped");
    assert_eq!(m.local("Inter-Regular").rating, Some(5));
    let on_disk = read_sidecar(&m.sidecar_path("Inter"));
    assert!(on_disk.faces.iter().any(|f| f.file == "sub/Inter-Bold.ttf"));
}

#[test]
fn an_entry_the_other_machine_removed_is_removed() {
    let tmp = TempDir::new("sidecar-removed-entry");
    let mut m = Machine::new(tmp.path());
    m.tag("Inter-Regular", &["x"]);
    m.tag("Inter-Bold", &["gone"]);
    m.write_as_other_machine("Inter", |s| {
        s.faces.retain(|f| f.file != "sub/Inter-Bold.ttf");
    });
    m.scan();
    assert!(m.local("Inter-Bold").is_empty());
    assert_eq!(m.local("Inter-Regular").tags, ["x"]);
}

#[test]
fn no_sidecars_in_system_sources() {
    let tmp = TempDir::new("sidecar-system");
    let mut m = Machine::with_kind(tmp.path(), SourceKind::System);
    m.tag("Lora-Regular", &["a"]);
    m.scan();
    assert!(!m.root.join(SIDECAR_DIR).exists());
    let dirty: u32 = m
        .index
        .conn
        .query_row("SELECT COUNT(*) FROM sidecar_dirty", [], |r| r.get(0))
        .expect("count");
    assert_eq!(dirty, 0);
}

#[test]
fn a_paused_source_is_written_when_resumed() {
    let tmp = TempDir::new("sidecar-paused");
    let mut m = Machine::new(tmp.path());
    m.index.set_source_paused(m.source, true).expect("pause");
    m.tag("Lora-Regular", &["a"]);
    assert!(!m.root.join(SIDECAR_DIR).exists());
    m.index.set_source_paused(m.source, false).expect("resume");
    m.scan();
    assert_eq!(
        m.entry("Lora", "Lora-Regular.ttf").expect("entry").tags,
        ["a"]
    );
}

#[test]
fn a_missing_source_folder_is_never_created() {
    let tmp = TempDir::new("sidecar-missing");
    let root = tmp.path().join("offline-share");
    let mut index = Index::open_in_memory().expect("open");
    let source = index
        .add_source(&root.to_string_lossy(), SourceKind::Watched)
        .expect("source")
        .id;
    index.persist_files(source, &files(&root)).expect("persist");
    let face: u32 = index
        .conn
        .query_row("SELECT min(id) FROM face", [], |r| r.get(0))
        .expect("face");
    index
        .tag_faces(&TagFacesRequest {
            families: Vec::new(),
            faces: vec![FaceId(face)],
            tags: vec!["a".into()],
        })
        .expect("tagging still works");
    assert!(!root.exists());
}

#[test]
fn a_failed_write_is_retried_by_the_next_write() {
    let tmp = TempDir::new("sidecar-retry");
    let mut m = Machine::new(tmp.path());
    // A file where the folder should be: every write into it fails.
    let dir = m.root.join(SIDECAR_DIR);
    std::fs::write(&dir, b"in the way").expect("block");
    m.tag("Lora-Regular", &["a"]);
    let sync = m.index.write_sidecars().expect("write");
    assert_eq!(sync.failed, 1);
    assert_eq!(sync.written, 0);

    std::fs::remove_file(&dir).expect("unblock");
    let sync = m.index.write_sidecars().expect("write");
    assert_eq!(sync.written, 1);
    assert_eq!(
        m.entry("Lora", "Lora-Regular.ttf").expect("entry").tags,
        ["a"]
    );
}

#[test]
fn a_real_scan_of_fixture_fonts_round_trips_through_a_second_machine() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts/fontshare");
    let tmp = TempDir::new("sidecar-fixtures");
    for rel in [
        "public-sans/PublicSans-Bold.ttf",
        "sora/Sora-MediumItalic.ttf",
    ] {
        let to = tmp.path().join("synced").join(rel);
        std::fs::create_dir_all(to.parent().expect("parent")).expect("mkdir");
        std::fs::copy(fixtures.join(rel), &to).expect("copy");
    }
    let root = tmp.path().join("synced");
    let scan = |index: &mut Index| {
        let source = index
            .add_source(&root.to_string_lossy(), SourceKind::Watched)
            .expect("source")
            .id;
        let mut scanned = FileIndex::new();
        let report = scanned.rescan(&root, &discover(&root).expect("discover"));
        index.apply_rescan(source, &scanned, &report).expect("scan");
    };
    let mut first = Index::open_in_memory().expect("open");
    scan(&mut first);
    let faces: Vec<u32> = {
        let mut stmt = first
            .conn
            .prepare("SELECT id FROM face ORDER BY id")
            .expect("faces");
        stmt.query_map([], |r| r.get(0))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("ids")
    };
    assert_eq!(faces.len(), 2);
    first
        .tag_faces(&TagFacesRequest {
            families: Vec::new(),
            faces: faces.iter().copied().map(FaceId).collect(),
            tags: vec!["brand".into()],
        })
        .expect("tag");
    assert_eq!(
        list_sidecars(&root.join(SIDECAR_DIR)).len(),
        2,
        "two families"
    );

    let mut second = Index::open_in_memory().expect("open");
    scan(&mut second);
    let tags = second.tags().expect("tags");
    assert_eq!(tags.len(), 1);
    assert_eq!((tags[0].name.as_str(), tags[0].face_count), ("brand", 2));
}

// --- virtual styles (E10.2) ------------------------------------------------

fn pin_request(
    face: FaceId,
    name: &str,
    wght: Option<f32>,
    collection: Option<fm_api::CollectionId>,
    set: Option<fm_api::SetId>,
) -> fm_api::organization::PinInstanceRequest {
    fm_api::organization::PinInstanceRequest {
        face,
        name: name.into(),
        coordinates: wght
            .map(|value| fm_api::common::AxisValue {
                tag: "wght".into(),
                value,
            })
            .into_iter()
            .collect(),
        collection,
        set,
    }
}

fn styles(
    index: &Index,
    collection: Option<fm_api::CollectionId>,
    set: Option<fm_api::SetId>,
) -> Vec<(String, Vec<(String, f32)>)> {
    index
        .virtual_styles(&fm_api::organization::VirtualStylesRequest { collection, set })
        .expect("styles")
        .into_iter()
        .map(|v| {
            (
                v.name,
                v.coordinates
                    .into_iter()
                    .map(|c| (c.tag, c.value))
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn pinned_styles_of_a_real_variable_font_round_trip_through_a_second_machine() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf");
    let tmp = TempDir::new("sidecar-pins");
    let root = tmp.path().join("synced");
    std::fs::create_dir_all(&root).expect("mkdir");
    let font = root.join("AsapSharp[wdth,wght].ttf");
    std::fs::copy(&fixture, &font).expect("copy");
    let bytes = std::fs::read(&font).expect("font");
    let scan = |index: &mut Index| {
        let source = index
            .add_source(&root.to_string_lossy(), SourceKind::Watched)
            .expect("source")
            .id;
        let mut scanned = FileIndex::new();
        let report = scanned.rescan(&root, &discover(&root).expect("discover"));
        index.apply_rescan(source, &scanned, &report).expect("scan");
    };
    let face_of = |index: &Index| -> FaceId {
        index
            .conn
            .query_row("SELECT id FROM face", [], |r| r.get(0))
            .map(FaceId)
            .expect("face")
    };

    let mut first = Index::open_in_memory().expect("open");
    scan(&mut first);
    let face = face_of(&first);
    let brand = first
        .create_collection(&CreateCollectionRequest {
            rule_group: None,
            name: "Brand".into(),
            parent: None,
            rules: None,
        })
        .expect("collection")
        .id;
    let client = first
        .create_set(&fm_api::organization::CreateSetRequest {
            name: "Client".into(),
            faces: Vec::new(),
        })
        .expect("set")
        .id;
    first
        .pin_instance(&pin_request(
            face,
            "Semibold",
            Some(600.0),
            Some(brand),
            None,
        ))
        .expect("pin");
    first
        .pin_instance(&pin_request(face, "Heavy", Some(800.0), None, Some(client)))
        .expect("pin");
    let in_brand = styles(&first, Some(brand), None);
    let in_client = styles(&first, None, Some(client));
    assert_eq!(in_brand.len(), 1);

    // The family's sidecar carries both, as format 2.
    let dir = root.join(SIDECAR_DIR);
    let names_on_disk = list_sidecars(&dir);
    assert_eq!(names_on_disk.len(), 1);
    let on_disk = read_sidecar(&dir.join(&names_on_disk[0]));
    assert_eq!(on_disk.version, crate::sidecar::PINS_VERSION);
    assert_eq!(on_disk.faces[0].meta.pins.len(), 2);

    // A second index over the folder (another machine, or this one after
    // the index was rebuilt from scratch) gets the same styles back.
    let mut second = Index::open_in_memory().expect("open");
    scan(&mut second);
    let brand2 = second.collections().expect("collections")[0].id;
    let client2 = second.sets().expect("sets")[0].id;
    assert_eq!(second.sets().expect("sets")[0].name, "Client");
    assert_eq!(styles(&second, Some(brand2), None), in_brand);
    assert_eq!(styles(&second, None, Some(client2)), in_client);

    // Deleting the set on the second machine removes its style everywhere.
    second.delete_set(client2).expect("delete");
    assert_eq!(
        read_sidecar(&dir.join(&names_on_disk[0])).faces[0]
            .meta
            .pins
            .len(),
        1
    );
    scan_again(&mut first, &root);
    assert_eq!(styles(&first, None, Some(client)), vec![]);
    assert_eq!(styles(&first, Some(brand), None), in_brand);

    // The font file was never written.
    assert_eq!(std::fs::read(&font).expect("font"), bytes);
}

/// A rescan of an index that already has the source at `root`.
fn scan_again(index: &mut Index, root: &Path) {
    let source = index.sources().expect("sources")[0].id;
    let mut scanned = FileIndex::new();
    let report = scanned.rescan(root, &discover(root).expect("discover"));
    index.apply_rescan(source, &scanned, &report).expect("scan");
}

#[test]
fn a_pin_in_a_collection_that_is_smart_here_is_a_problem_and_is_kept() {
    let tmp = TempDir::new("sidecar-pin-smart");
    let mut m = Machine::new(tmp.path());
    m.tag("Lora-Regular", &["a"]);
    m.index
        .create_collection(&CreateCollectionRequest {
            rule_group: None,
            name: "Serifs".into(),
            parent: None,
            rules: Some(fm_api::search::Filters::default()),
        })
        .expect("smart");
    let pin = crate::sidecar::Pin {
        name: "Wide".into(),
        collection: Some(names(&["Serifs"])),
        set: None,
        coordinates: [("wdth".to_owned(), 110.0)].into_iter().collect(),
    };
    m.write_as_other_machine("Lora", |s| {
        entry_mut(s, "Lora-Regular.ttf").pins = vec![pin.clone()];
    });
    m.scan();
    let problems = m.index.sidecar_problems().expect("problems");
    assert_eq!(problems.len(), 1);
    assert!(
        problems[0].message.contains("\"Wide\"") && problems[0].message.contains("Serifs"),
        "{}",
        problems[0].message
    );
    // Kept in the sidecar through this machine's later edits.
    m.tag("Lora-Regular", &["b"]);
    m.scan();
    let entry = m.entry("Lora", "Lora-Regular.ttf").expect("entry");
    assert_eq!(entry.pins, [pin]);
    assert_eq!(entry.tags, ["a", "b"]);
}

#[test]
fn renaming_a_collection_renames_it_in_its_pins() {
    let tmp = TempDir::new("sidecar-pin-rename");
    let mut m = Machine::new(tmp.path());
    let work = m
        .index
        .create_collection(&CreateCollectionRequest {
            rule_group: None,
            name: "Work".into(),
            parent: None,
            rules: None,
        })
        .expect("work")
        .id;
    // A face without axes cannot be pinned through the API; write the row
    // as a merge would, to test the mirror alone.
    let face = m.face("Lora-Regular").0;
    {
        let tx = m.index.conn.transaction().expect("tx");
        virtual_styles::insert_style(&tx, face, "Wide", &[], Holder::Collection(work.0))
            .expect("insert");
        tx.commit().expect("commit");
    }
    m.index.write_sidecars().expect("write");
    let names_of = |m: &Machine| -> Vec<Option<Vec<String>>> {
        m.entry("Lora", "Lora-Regular.ttf")
            .expect("entry")
            .pins
            .into_iter()
            .map(|p| p.collection)
            .collect()
    };
    assert_eq!(names_of(&m), [Some(names(&["Work"]))]);
    m.index
        .rename_collection(&RenameCollectionRequest {
            collection: work,
            name: "Clients".into(),
        })
        .expect("rename");
    assert_eq!(names_of(&m), [Some(names(&["Clients"]))]);
}

fn family_entry(notes: &str) -> crate::sidecar::FamilyEntry {
    crate::sidecar::FamilyEntry {
        notes: Some(notes.into()),
        history: Vec::new(),
        license: None,
    }
}

#[test]
fn family_notes_are_mirrored_to_the_sidecar_and_cleared_from_it() {
    let tmp = TempDir::new("sidecar-family-notes");
    let mut m = Machine::new(tmp.path());
    m.family_notes("Inter-Bold", "Licensed for the 2026 rebrand");
    let on_disk = m.sidecar("Inter");
    assert_eq!(
        on_disk.family_notes(),
        Some("Licensed for the 2026 rebrand")
    );
    assert_eq!(on_disk.version, crate::sidecar::FAMILY_NOTES_VERSION);
    // A family with notes and no face metadata still has a sidecar; the
    // other family's sidecar is not written.
    assert!(on_disk.faces.is_empty());
    assert!(!m.sidecar_path("Lora").exists());

    // A second index over the folder (another machine, or this one after the
    // index was made from scratch) reads them back.
    let second = Machine::new(tmp.path());
    assert_eq!(
        second.local_family_notes("Inter").as_deref(),
        Some("Licensed for the 2026 rebrand")
    );

    // An empty string clears them, in the index and in the file.
    m.family_notes("Inter-Regular", "");
    assert_eq!(m.local_family_notes("Inter"), None);
    let cleared = m.sidecar("Inter");
    assert_eq!(cleared.family_meta, None);
    assert_eq!(cleared.version, PLAIN_VERSION);
}

#[test]
fn family_notes_changed_on_another_machine_merge_on_scan() {
    let tmp = TempDir::new("sidecar-family-merge");
    let mut m = Machine::new(tmp.path());
    m.family_notes("Inter-Bold", "first");
    m.write_as_other_machine("Inter", |s| {
        s.family_meta = Some(family_entry("from the laptop"));
    });
    m.scan();
    assert_eq!(
        m.local_family_notes("Inter").as_deref(),
        Some("from the laptop")
    );
    assert!(m.index.sidecar_problems().expect("problems").is_empty());
    // The face notes are not touched by the family's.
    assert_eq!(m.local("Inter-Bold").notes, None);

    // The other machine clears them: cleared here too.
    m.write_as_other_machine("Inter", |s| s.family_meta = None);
    m.scan();
    assert_eq!(m.local_family_notes("Inter"), None);
}

#[test]
fn family_notes_changed_on_both_machines_keep_this_one_and_report_the_other() {
    let tmp = TempDir::new("sidecar-family-conflict");
    let mut m = Machine::new(tmp.path());
    m.family_notes("Inter-Bold", "base");
    // The other machine changes the version it read...
    let theirs = other_machine_edit(m.sidecar("Inter"), |s| {
        s.family_meta = Some(family_entry("theirs"));
    });
    // ...while this one changes it too, before they sync.
    m.family_notes("Inter-Bold", "mine");
    write_changed(&m.sidecar_path("Inter"), &theirs.to_json());
    m.scan();

    assert_eq!(m.local_family_notes("Inter").as_deref(), Some("mine"));
    assert_eq!(m.sidecar("Inter").family_notes(), Some("mine"));
    let problems = m.index.sidecar_problems().expect("problems");
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].kind, ProblemKind::SidecarConflict);
    assert!(problems[0].face.is_some(), "shown on a font of the family");
    assert!(
        problems[0].message.contains("family") && problems[0].message.contains("\"theirs\""),
        "{}",
        problems[0].message
    );

    // Setting the notes again dismisses the conflict.
    m.family_notes("Inter-Regular", "decided");
    assert!(m.index.sidecar_problems().expect("problems").is_empty());
    assert_eq!(m.sidecar("Inter").family_notes(), Some("decided"));
}

#[test]
fn family_notes_that_arrive_before_their_fonts_are_taken_when_the_fonts_do() {
    // With and without face entries of the family in the file.
    for tag_too in [false, true] {
        let tmp = TempDir::new("sidecar-family-first");
        // Another machine wrote Inter's sidecar into the synced folder.
        let mut other = Machine::new(tmp.path());
        other.family_notes("Inter-Bold", "from the laptop");
        if tag_too {
            other.tag("Inter-Bold", &["display"]);
        }
        drop(other);

        // This machine sees the sidecar before it has the Inter fonts.
        let mut index = Index::open_in_memory().expect("open");
        let source = index
            .add_source(&tmp.path().to_string_lossy(), SourceKind::Watched)
            .expect("source")
            .id;
        let lora: Vec<IndexedFile> = files(tmp.path())
            .into_iter()
            .filter(|f| f.path.ends_with("Lora-Regular.ttf"))
            .collect();
        index.persist_files(source, &lora).expect("lora");
        let mut m = Machine {
            index,
            source,
            root: tmp.path().to_path_buf(),
        };
        assert_eq!(m.local_family_notes("Inter"), None, "tag_too={tag_too}");

        // The fonts arrive: the note is taken, not taken for a removal.
        m.scan();
        assert_eq!(
            m.local_family_notes("Inter").as_deref(),
            Some("from the laptop"),
            "tag_too={tag_too}"
        );
        assert_eq!(
            m.sidecar("Inter").family_notes(),
            Some("from the laptop"),
            "tag_too={tag_too}"
        );
        // An edit here later keeps it in the file.
        m.tag("Inter-Regular", &["body"]);
        assert_eq!(
            m.sidecar("Inter").family_notes(),
            Some("from the laptop"),
            "tag_too={tag_too}"
        );
    }
}

#[test]
fn family_notes_cleared_elsewhere_while_the_fonts_are_away_stay_cleared() {
    // Cleared there, or changed there: the other machine's value wins.
    for theirs in [None, Some("changed on the laptop")] {
        let tmp = TempDir::new("sidecar-family-away");
        let mut m = Machine::new(tmp.path());
        m.family_notes("Inter-Bold", "first");
        let synced = m.sidecar("Inter");

        // Inter's fonts leave this index for a while (a sync client is
        // replacing them); the family's notes stay in the index.
        let lora: Vec<IndexedFile> = files(tmp.path())
            .into_iter()
            .filter(|f| f.path.ends_with("Lora-Regular.ttf"))
            .collect();
        m.index.persist_files(m.source, &lora).expect("away");
        m.index
            .conn
            .execute("DELETE FROM face WHERE family = 'Inter'", [])
            .expect("faces gone");
        assert_eq!(m.local_family_notes("Inter").as_deref(), Some("first"));

        // Meanwhile the other machine changes the notes, and this one reads
        // the file while the family is not here.
        let edited = other_machine_edit(synced, |s| {
            s.family_meta = theirs.map(family_entry);
        });
        write_changed(&m.sidecar_path("Inter"), &edited.to_json());
        m.index
            .persist_files(m.source, &lora)
            .expect("scan while away");

        // The fonts come back: the other machine's value is taken.
        m.scan();
        assert_eq!(
            m.local_family_notes("Inter").as_deref(),
            theirs,
            "theirs={theirs:?}"
        );
        assert_eq!(
            m.sidecar("Inter").family_notes(),
            theirs,
            "theirs={theirs:?}"
        );
        assert!(
            m.index.sidecar_problems().expect("problems").is_empty(),
            "no conflict: theirs={theirs:?}"
        );
    }
}

fn license_entry(license: crate::sidecar::License) -> crate::sidecar::LicenseEntry {
    crate::sidecar::LicenseEntry {
        license,
        history: Vec::new(),
    }
}

fn desktop_license(root: &Path) -> DeclaredLicense {
    DeclaredLicense {
        license_type: Some(fm_api::library::LicenseType::Desktop),
        seats: Some(4),
        purchase_date: Some("2026-03-01".into()),
        receipt: Some(root.join(SIDECAR_DIR).join("receipts").join("invoice.pdf")),
    }
}

#[test]
fn a_declared_license_is_mirrored_to_the_sidecar_and_read_back_by_a_teammate() {
    let tmp = TempDir::new("sidecar-license");
    let mut m = Machine::new(tmp.path());
    m.declare("Inter-Bold", &desktop_license(tmp.path()));
    let on_disk = m.sidecar("Inter");
    assert_eq!(on_disk.version, crate::sidecar::SIDECAR_VERSION);
    assert_eq!(
        on_disk.family_license(),
        Some(&crate::sidecar::License {
            license_type: Some(fm_api::library::LicenseType::Desktop),
            seats: Some(4),
            purchase_date: Some("2026-03-01".into()),
            receipt: Some("receipts/invoice.pdf".into()),
        })
    );
    assert!(!m.sidecar_path("Lora").exists());

    // A teammate syncs the folder to another place: the declaration comes
    // with it, the receipt in their copy of the folder.
    let synced = TempDir::new("sidecar-license-teammate");
    std::fs::create_dir_all(synced.path().join(SIDECAR_DIR)).expect("folder");
    std::fs::copy(
        m.sidecar_path("Inter"),
        synced.path().join(SIDECAR_DIR).join(file_name("Inter")),
    )
    .expect("sync");
    let teammate = Machine::new(synced.path());
    assert_eq!(
        teammate.local_license("Inter"),
        Some(desktop_license(synced.path()))
    );
    assert_eq!(teammate.local_license("Lora"), None);

    // Clearing it clears the file too.
    m.declare("Inter-Regular", &DeclaredLicense::default());
    assert_eq!(m.local_license("Inter"), None);
    let cleared = m.sidecar("Inter");
    assert_eq!(cleared.family_meta, None);
    assert_eq!(cleared.version, PLAIN_VERSION);
}

#[test]
fn a_license_and_notes_share_the_family_entry_and_keep_their_own_history() {
    let tmp = TempDir::new("sidecar-license-notes");
    let mut m = Machine::new(tmp.path());
    m.family_notes("Inter-Bold", "for the rebrand");
    let notes_history = m.sidecar("Inter").family_meta.expect("family").history;
    m.declare("Inter-Bold", &desktop_license(tmp.path()));
    let on_disk = m.sidecar("Inter");
    assert_eq!(on_disk.family_notes(), Some("for the rebrand"));
    assert!(on_disk.family_license().is_some());
    let family = on_disk.family_meta.expect("family");
    // Declaring a license is not a change of the notes.
    assert_eq!(family.history, notes_history);
    assert_eq!(family.license.expect("license").history.len(), 1);
    // Clearing the notes keeps the license in the file (version 4).
    m.family_notes("Inter-Bold", "");
    let on_disk = m.sidecar("Inter");
    assert_eq!(on_disk.family_notes(), None);
    assert!(on_disk.family_license().is_some());
    assert_eq!(on_disk.version, crate::sidecar::SIDECAR_VERSION);
}

#[test]
fn notes_cleared_on_another_machine_stay_cleared_while_a_license_is_declared() {
    let tmp = TempDir::new("sidecar-license-notes-clear");
    let mut m = Machine::new(tmp.path());
    m.family_notes("Inter-Bold", "for the rebrand");
    m.declare("Inter-Bold", &desktop_license(tmp.path()));
    // A second machine over the same folder takes both in, then clears the
    // notes: its file keeps the family entry for the license only.
    let mut other = Machine::new(tmp.path());
    assert_eq!(
        other.local_family_notes("Inter").as_deref(),
        Some("for the rebrand")
    );
    other.family_notes("Inter-Regular", "");
    let written = m.sidecar("Inter");
    assert_eq!(written.family_notes(), None);
    assert!(written.family_license().is_some());

    m.scan();
    assert_eq!(m.local_family_notes("Inter"), None);
    assert_eq!(m.local_license("Inter"), Some(desktop_license(tmp.path())));
    assert_eq!(m.sidecar("Inter").family_notes(), None);
    assert!(m.index.sidecar_problems().expect("problems").is_empty());
}

#[test]
fn a_license_changed_on_another_machine_merges_on_scan() {
    let tmp = TempDir::new("sidecar-license-merge");
    let mut m = Machine::new(tmp.path());
    m.declare("Inter-Bold", &desktop_license(tmp.path()));
    let web = crate::sidecar::License {
        license_type: Some(fm_api::library::LicenseType::Web),
        seats: Some(10),
        purchase_date: None,
        receipt: Some("receipts/web.pdf".into()),
    };
    m.write_as_other_machine("Inter", |s| {
        if let Some(family) = &mut s.family_meta {
            family.license = Some(license_entry(web.clone()));
        }
    });
    m.scan();
    assert_eq!(
        m.local_license("Inter"),
        Some(DeclaredLicense {
            license_type: Some(fm_api::library::LicenseType::Web),
            seats: Some(10),
            purchase_date: None,
            receipt: Some(
                tmp.path()
                    .join(SIDECAR_DIR)
                    .join("receipts")
                    .join("web.pdf")
            ),
        })
    );
    assert!(m.index.sidecar_problems().expect("problems").is_empty());

    // The other machine clears it, keeping its notes: cleared here too.
    m.write_as_other_machine("Inter", |s| {
        if let Some(family) = &mut s.family_meta {
            family.license = None;
            family.notes = Some("kept".into());
        }
    });
    m.scan();
    assert_eq!(m.local_license("Inter"), None);
    assert_eq!(m.local_family_notes("Inter").as_deref(), Some("kept"));
}

#[test]
fn a_license_changed_on_both_machines_keeps_this_one_and_reports_the_other() {
    let tmp = TempDir::new("sidecar-license-conflict");
    let mut m = Machine::new(tmp.path());
    m.declare("Inter-Bold", &desktop_license(tmp.path()));
    let theirs = other_machine_edit(m.sidecar("Inter"), |s| {
        if let Some(family) = &mut s.family_meta {
            family.license = Some(license_entry(crate::sidecar::License {
                license_type: Some(fm_api::library::LicenseType::Trial),
                ..crate::sidecar::License::default()
            }));
        }
    });
    let mine = DeclaredLicense {
        license_type: Some(fm_api::library::LicenseType::App),
        seats: Some(2),
        ..DeclaredLicense::default()
    };
    m.declare("Inter-Bold", &mine);
    write_changed(&m.sidecar_path("Inter"), &theirs.to_json());
    m.scan();

    assert_eq!(m.local_license("Inter"), Some(mine.clone()));
    assert_eq!(
        m.sidecar("Inter")
            .family_license()
            .and_then(|l| l.license_type),
        Some(fm_api::library::LicenseType::App)
    );
    let problems = m.index.sidecar_problems().expect("problems");
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].kind, ProblemKind::SidecarConflict);
    assert!(
        problems[0].message.contains("license type \"trial\"")
            && problems[0]
                .message
                .contains("license type \"app\", 2 seats"),
        "{}",
        problems[0].message
    );

    // Declaring again dismisses the conflict.
    m.declare("Inter-Regular", &mine);
    assert!(m.index.sidecar_problems().expect("problems").is_empty());
}

// E28.1: conflicts the user resolves, conflicted copies, concurrent edits.

/// A machine over its own copy of the source folder (same fonts, same
/// relative paths), as a second computer that syncs the folder sees it.
fn machine(label: &str) -> (TempDir, Machine) {
    let tmp = TempDir::new(label);
    let m = Machine::new(tmp.path());
    (tmp, m)
}

/// The `.json` file names in a machine's sidecar folder.
fn sidecar_files(m: &Machine) -> Vec<String> {
    list_sidecars(&m.root.join(SIDECAR_DIR))
}

/// Writes `text` to the sidecar folder of `m` as `name`, as a sync client
/// would; a file with that text already is left alone.
fn put(m: &Machine, name: &str, text: &str) {
    let dir = m.root.join(SIDECAR_DIR);
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join(name);
    if std::fs::read_to_string(&path).ok().as_deref() == Some(text) {
        return;
    }
    write_changed(&path, text);
}

fn text(m: &Machine, name: &str) -> String {
    std::fs::read_to_string(m.root.join(SIDECAR_DIR).join(name)).expect("sidecar text")
}

/// A sync client that could merge: every sidecar of `from`, conflicted
/// copies included, replaces the one of `to`.
fn sync(from: &Machine, to: &Machine) {
    for name in sidecar_files(from) {
        put(to, &name, &text(from, &name));
    }
}

/// The name of a sync client's conflicted copy of the sidecar of `family`.
fn copy_name(family: &str, pattern: &str) -> String {
    let stem = file_name(family).trim_end_matches(".json").to_owned();
    match pattern {
        "dropbox" => format!("{stem} (Goran's conflicted copy 2026-10-26).json"),
        "onedrive" => format!("{stem}-DESKTOP-4F2K9.json"),
        "gdrive" => format!("{stem} (1).json"),
        "icloud" => format!("{stem} 2.json"),
        other => format!("{stem} ({other} conflicted copy).json"),
    }
}

fn conflicts(m: &Machine) -> Vec<fm_api::library::SidecarConflict> {
    m.index.sidecar_conflicts().expect("conflicts")
}

fn rate(m: &mut Machine, postscript: &str, rating: u8) {
    m.meta(
        postscript,
        UserMetaUpdate {
            rating: Some(rating),
            ..UserMetaUpdate::default()
        },
    );
}

/// The newest version id of the entry of `file` in the sidecar of `family`.
fn entry_head(m: &Machine, family: &str, file: &str) -> String {
    m.sidecar(family)
        .faces
        .iter()
        .find(|f| f.file == file)
        .and_then(|f| f.history.first().cloned())
        .expect("entry history")
}

#[test]
fn a_resolved_conflict_is_taken_by_the_other_machine_without_a_new_one() {
    use fm_api::library::{ConflictSide, SidecarField};
    for keep in [ConflictSide::Mine, ConflictSide::Theirs] {
        let (_ta, mut a) = machine("e28-resolve-a");
        let (_tb, mut b) = machine("e28-resolve-b");
        rate(&mut a, "Lora-Regular", 4);
        sync(&a, &b);
        b.scan();
        assert_eq!(b.local("Lora-Regular").rating, Some(4));

        // Both change the rating before they sync; the sync client keeps A's.
        rate(&mut a, "Lora-Regular", 5);
        rate(&mut b, "Lora-Regular", 2);
        sync(&a, &b);
        b.scan();
        let listed = conflicts(&b);
        assert_eq!(listed.len(), 1, "{keep:?}");
        let c = &listed[0];
        assert_eq!(c.field, SidecarField::Rating);
        assert_eq!(c.face, b.face("Lora-Regular"));
        assert_eq!(c.family, "Lora");
        assert_eq!((c.mine.as_str(), c.theirs.as_str()), ("2 stars", "5 stars"));
        assert_eq!(Path::new(&c.sidecar_path), b.sidecar_path("Lora"));
        let before = entry_head(&b, "Lora", "Lora-Regular.ttf");

        b.index
            .resolve_sidecar_conflict(c.id, keep)
            .expect("resolve");
        let kept = if keep == ConflictSide::Mine { 2 } else { 5 };
        assert_eq!(b.local("Lora-Regular").rating, Some(kept));
        assert!(conflicts(&b).is_empty());
        assert!(b.index.sidecar_problems().expect("problems").is_empty());
        assert_eq!(
            b.entry("Lora", "Lora-Regular.ttf").expect("entry").rating,
            Some(kept)
        );
        assert_ne!(
            entry_head(&b, "Lora", "Lora-Regular.ttf"),
            before,
            "resolving writes a new version"
        );

        // The other machine takes the kept value, and nothing comes back.
        sync(&b, &a);
        a.scan();
        assert_eq!(a.local("Lora-Regular").rating, Some(kept), "{keep:?}");
        assert!(conflicts(&a).is_empty(), "{keep:?}");
        sync(&a, &b);
        b.scan();
        assert!(conflicts(&b).is_empty());
        assert_eq!(b.local("Lora-Regular").rating, Some(kept));

        // A resolved conflict is gone.
        let again = b.index.resolve_sidecar_conflict(c.id, keep).unwrap_err();
        assert_eq!(again.code(), fm_api::ErrorCode::NotFound);
    }
}

#[test]
fn a_conflict_from_a_conflicted_copy_resolves_on_both_machines() {
    use fm_api::library::ConflictSide;
    for keep in [ConflictSide::Mine, ConflictSide::Theirs] {
        let (_ta, mut a) = machine("e28-copy-resolve-a");
        let (_tb, mut b) = machine("e28-copy-resolve-b");
        m_notes(&mut a, "Inter-Bold", "first");
        sync(&a, &b);
        b.scan();

        // Both change the notes at the same time. The sync client keeps A's
        // file under its name and B's as a conflicted copy, in both folders.
        m_notes(&mut a, "Inter-Bold", "from A");
        m_notes(&mut b, "Inter-Bold", "from B");
        let copy = copy_name("Inter", "dropbox");
        let theirs = text(&b, &file_name("Inter"));
        put(&a, &copy, &theirs);
        put(&b, &copy, &theirs);
        sync(&a, &b);

        a.scan();
        let listed = conflicts(&a);
        assert_eq!(listed.len(), 1, "{keep:?}");
        assert_eq!(
            (listed[0].mine.as_str(), listed[0].theirs.as_str()),
            ("\"from A\"", "\"from B\"")
        );
        assert!(listed[0].sidecar_path.ends_with(&copy));
        a.index
            .resolve_sidecar_conflict(listed[0].id, keep)
            .expect("resolve");
        let kept = if keep == ConflictSide::Mine {
            "from A"
        } else {
            "from B"
        };
        assert_eq!(a.local("Inter-Bold").notes.as_deref(), Some(kept));

        // B wrote "from B" and never saw A's version; A's file now holds B's
        // version in its history, so B takes the result without a conflict.
        sync(&a, &b);
        b.scan();
        assert_eq!(
            b.local("Inter-Bold").notes.as_deref(),
            Some(kept),
            "{keep:?}"
        );
        assert!(conflicts(&b).is_empty(), "{keep:?}");
        assert!(conflicts(&a).is_empty(), "{keep:?}");
        // The copy is never rewritten.
        assert_eq!(text(&a, &copy), theirs);
        assert_eq!(text(&b, &copy), theirs);
    }
}

fn m_notes(m: &mut Machine, postscript: &str, notes: &str) {
    m.meta(
        postscript,
        UserMetaUpdate {
            notes: Some(notes.into()),
            ..UserMetaUpdate::default()
        },
    );
}

#[test]
fn family_notes_and_licenses_resolve_like_face_fields() {
    use fm_api::library::{ConflictSide, SidecarField};
    let (_ta, mut a) = machine("e28-family-a");
    let (_tb, mut b) = machine("e28-family-b");
    a.family_notes("Lora-Regular", "shared");
    sync(&a, &b);
    b.scan();
    a.family_notes("Lora-Regular", "A's notes");
    b.family_notes("Lora-Regular", "B's notes");
    sync(&a, &b);
    b.scan();
    let listed = conflicts(&b);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].field, SidecarField::FamilyNotes);
    assert_eq!(listed[0].theirs, "\"A's notes\"");
    b.index
        .resolve_sidecar_conflict(listed[0].id, ConflictSide::Theirs)
        .expect("resolve");
    assert_eq!(b.local_family_notes("Lora").as_deref(), Some("A's notes"));
    sync(&b, &a);
    a.scan();
    assert_eq!(a.local_family_notes("Lora").as_deref(), Some("A's notes"));
    assert!(conflicts(&a).is_empty());
    assert!(conflicts(&b).is_empty());
}

#[test]
fn a_conflict_this_machine_comes_to_agree_with_is_settled() {
    let (_ta, mut a) = machine("e28-settle-a");
    let (_tb, mut b) = machine("e28-settle-b");
    rate(&mut a, "Lora-Regular", 3);
    sync(&a, &b);
    b.scan();
    rate(&mut a, "Lora-Regular", 5);
    rate(&mut b, "Lora-Regular", 1);
    sync(&a, &b);
    b.scan();
    assert_eq!(conflicts(&b).len(), 1);
    // B's user picks the other machine's value by hand: nothing is left to
    // choose, on either machine.
    rate(&mut b, "Lora-Regular", 5);
    assert!(conflicts(&b).is_empty());
    sync(&b, &a);
    a.scan();
    assert!(conflicts(&a).is_empty());
    assert_eq!(a.local("Lora-Regular").rating, Some(5));
}

#[test]
fn only_value_conflicts_are_listed_to_resolve() {
    let tmp = TempDir::new("e28-value-only");
    let mut m = Machine::new(tmp.path());
    m.tag("Lora-Regular", &["a"]);
    // A sidecar that cannot be read is a Problems entry, not a conflict to
    // resolve.
    write_changed(&m.root.join(SIDECAR_DIR).join(file_name("Inter")), "{");
    m.scan();
    assert_eq!(m.index.sidecar_problems().expect("problems").len(), 1);
    assert!(conflicts(&m).is_empty());
}

#[test]
fn every_sync_clients_conflicted_copy_is_merged_and_left_alone() {
    for pattern in ["dropbox", "onedrive", "gdrive", "icloud"] {
        let tmp = TempDir::new(&format!("e28-copy-{pattern}"));
        let mut m = Machine::new(tmp.path());
        m.tag("Lora-Regular", &["a"]);
        let copy = other_machine_edit(m.sidecar("Lora"), |s| {
            entry_mut(s, "Lora-Regular.ttf").tags = names(&["a", "from-copy"]);
            s.family_meta = Some(family_entry("notes from the copy"));
        });
        let name = copy_name("Lora", pattern);
        assert_eq!(
            sidecar::original_name(&name).as_deref(),
            Some(file_name("Lora").as_str()),
            "{pattern}"
        );
        put(&m, &name, &copy.to_json());

        m.scan();
        assert_eq!(
            m.local("Lora-Regular").tags,
            ["a", "from-copy"],
            "{pattern}"
        );
        assert_eq!(
            m.local_family_notes("Lora").as_deref(),
            Some("notes from the copy"),
            "{pattern}: the family's notes in a copy are merged too"
        );
        // The result goes to the family's sidecar; the copy is left alone.
        let main = m.sidecar("Lora");
        assert_eq!(
            main.family_notes(),
            Some("notes from the copy"),
            "{pattern}"
        );
        assert_eq!(
            m.entry("Lora", "Lora-Regular.ttf").expect("entry").tags,
            ["a", "from-copy"]
        );
        assert_eq!(text(&m, &name), copy.to_json(), "{pattern}");
        assert!(
            conflicts(&m).is_empty(),
            "{pattern}: a copy that descends is no conflict"
        );
    }
}

#[test]
fn an_old_conflicted_copy_brings_nothing_back() {
    let tmp = TempDir::new("e28-old-copy");
    let mut m = Machine::new(tmp.path());
    m.tag("Lora-Regular", &["a"]);
    rate(&mut m, "Lora-Regular", 2);
    let old = text(&m, &file_name("Lora"));
    rate(&mut m, "Lora-Regular", 4);
    // A copy of a version this machine wrote before and changed since.
    put(&m, &copy_name("Lora", "gdrive"), &old);
    m.scan();
    assert_eq!(m.local("Lora-Regular").rating, Some(4));
    assert!(conflicts(&m).is_empty());
}

#[test]
fn a_half_written_sidecar_or_copy_is_reported_and_never_rewritten() {
    let tmp = TempDir::new("e28-half");
    let mut m = Machine::new(tmp.path());
    m.tag("Lora-Regular", &["a"]);
    m.tag("Inter-Regular", &["b"]);
    let main = m.sidecar_path("Lora");
    let full = std::fs::read_to_string(&main).expect("read");
    let half = &full[..full.len() / 2];
    write_changed(&main, half);
    let copy = copy_name("Inter", "icloud");
    let inter = text(&m, &file_name("Inter"));
    put(&m, &copy, &inter[..inter.len() / 2]);

    m.scan();
    m.tag("Lora-Regular", &["c"]);
    let problems = m.index.sidecar_problems().expect("problems");
    let mut paths: Vec<PathBuf> = problems.iter().map(|p| PathBuf::from(&p.path)).collect();
    paths.sort();
    let mut want = vec![main.clone(), m.root.join(SIDECAR_DIR).join(&copy)];
    want.sort();
    assert_eq!(paths, want);
    assert!(problems.iter().all(|p| p.face.is_none()));
    assert_eq!(std::fs::read_to_string(&main).expect("read"), half);
    assert_eq!(text(&m, &copy), &inter[..inter.len() / 2]);
    assert_eq!(m.local("Lora-Regular").tags, ["a", "c"]);
}

#[test]
fn copy_names_are_recognized_only_for_sidecar_names() {
    let stem = file_name("Lora").trim_end_matches(".json").to_owned();
    for name in [
        "Lora (conflicted copy).json".to_owned(),
        file_name("Lora"),
        format!("{stem}-.json"),
        format!("{stem} (copy).json"),
        format!("{stem} 2.txt"),
        format!("{stem}-DESK TOP.json"),
    ] {
        assert_eq!(sidecar::original_name(&name), None, "{name}");
    }
}

/// A small deterministic generator, so a failing seed can be run again.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % n
    }
}

const FACES: [&str; 3] = ["Inter-Regular", "Inter-Bold", "Lora-Regular"];

/// What a machine holds for a scalar field of a face, as a conflict stores it.
fn value_json(m: &Machine, postscript: &str, field: &str) -> String {
    let meta = m.local(postscript);
    match field {
        "favorite" => serde_json::to_string(&meta.favorite),
        "rating" => serde_json::to_string(&meta.rating),
        _ => serde_json::to_string(&meta.notes),
    }
    .expect("json")
}

/// Every value a machine set aside or kept in a conflict: (face, field, value).
fn conflict_values(m: &Machine) -> Vec<(u32, String, String)> {
    let mut stmt = m
        .index
        .conn
        .prepare(
            "SELECT face_id, field, local, disk FROM sidecar_conflict WHERE face_id IS NOT NULL",
        )
        .expect("prepare");
    let rows: Vec<(u32, String, Option<String>, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows");
    rows.into_iter()
        .flat_map(|(face, field, local, disk)| {
            let mut out = vec![(face, field.clone(), disk)];
            out.extend(local.map(|l| (face, field, l)));
            out
        })
        .collect()
}

#[test]
fn interleaved_edits_and_syncs_of_two_machines_lose_no_value() {
    for seed in 0..24_u64 {
        let (_ta, a) = machine(&format!("e28-seed-{seed}-a"));
        let (_tb, b) = machine(&format!("e28-seed-{seed}-b"));
        let mut ms = [a, b];
        let mut rng = Lcg(seed);
        // The last value each machine set, by face and field, unless the
        // other machine's user replaced it knowingly afterwards.
        let mut last: Vec<std::collections::HashMap<(usize, &str), String>> =
            vec![Default::default(), Default::default()];
        let mut tags: Vec<String> = Vec::new();
        let mut collections: Vec<(usize, String)> = Vec::new();
        let mut n = 0;
        // What happened, for the message of a failing seed.
        let mut log: Vec<String> = Vec::new();
        for _ in 0..40 {
            n += 1;
            let x = usize::try_from(rng.below(2)).expect("machine");
            let y = 1 - x;
            match rng.below(6) {
                0..=2 => {
                    let face = usize::try_from(rng.below(3)).expect("face");
                    let ps = FACES[face];
                    let field = ["favorite", "rating", "notes"]
                        [usize::try_from(rng.below(3)).expect("field")];
                    // The user sees this machine's value and its conflicts:
                    // the other machine's values among them are replaced
                    // knowingly.
                    let face_id = ms[x].face(ps).0;
                    let seen: Vec<String> = std::iter::once(value_json(&ms[x], ps, field))
                        .chain(
                            conflict_values(&ms[x])
                                .into_iter()
                                .filter(|(f, fld, _)| *f == face_id && fld == field)
                                .map(|(_, _, v)| v),
                        )
                        .collect();
                    if last[y]
                        .get(&(face, field))
                        .is_some_and(|v| seen.contains(v))
                    {
                        last[y].remove(&(face, field));
                    }
                    let mut update = UserMetaUpdate::default();
                    let value = match field {
                        "favorite" => {
                            let v = !ms[x].local(ps).favorite;
                            update.favorite = Some(v);
                            serde_json::to_string(&v)
                        }
                        "rating" => {
                            // An edit changes the value: setting the rating it
                            // has is no edit and makes no version.
                            let now = ms[x].local(ps).rating.unwrap_or(0);
                            let mut v = u8::try_from(rng.below(5) + 1).expect("rating");
                            if v == now {
                                v = v % 5 + 1;
                            }
                            update.rating = Some(v);
                            serde_json::to_string(&Some(v))
                        }
                        _ => {
                            let v = format!("m{x} n{n}");
                            update.notes = Some(v.clone());
                            serde_json::to_string(&Some(v))
                        }
                    }
                    .expect("json");
                    ms[x].meta(ps, update);
                    log.push(format!("{x}: {ps} {field} = {value}"));
                    last[x].insert((face, field), value);
                }
                3 => {
                    let face = usize::try_from(rng.below(3)).expect("face");
                    let tag = format!("t{x}-{n}");
                    ms[x].tag(FACES[face], &[tag.as_str()]);
                    log.push(format!("{x}: {} tag {tag}", FACES[face]));
                    tags.push(tag);
                    if rng.below(2) == 0 {
                        let name = format!("c{x}-{n}");
                        let c = ms[x]
                            .index
                            .create_collection(&CreateCollectionRequest {
                                rule_group: None,
                                name: name.clone(),
                                parent: None,
                                rules: None,
                            })
                            .expect("collection");
                        let face_id = ms[x].face(FACES[face]);
                        ms[x]
                            .index
                            .add_to_collection(&CollectionFacesRequest {
                                collection: c.id,
                                faces: vec![face_id],
                                families: Vec::new(),
                            })
                            .expect("add");
                        collections.push((face, name));
                    }
                }
                4 => {
                    // The sync client merges, or keeps both as a conflicted
                    // copy when the target changed too.
                    let (from, to) = (&ms[x], &ms[y]);
                    let as_copy = rng.below(3) == 0;
                    log.push(format!(
                        "sync {x} -> {y}{}",
                        if as_copy { " as copies" } else { "" }
                    ));
                    if as_copy {
                        for name in sidecar_files(from)
                            .into_iter()
                            .filter(|n| sidecar::original_name(n).is_none())
                        {
                            let family = if name == file_name("Inter") {
                                "Inter"
                            } else {
                                "Lora"
                            };
                            let copy = copy_name(family, &format!("seed{seed}-{n}"));
                            put(to, &copy, &text(from, &name));
                        }
                    } else {
                        sync(from, to);
                    }
                }
                _ => {
                    log.push(format!("{x}: scan"));
                    ms[x].scan();
                }
            }
        }
        // Let the sync client and both machines catch up.
        for _ in 0..3 {
            sync(&ms[0], &ms[1]);
            ms[1].scan();
            sync(&ms[1], &ms[0]);
            ms[0].scan();
        }

        for (x, values) in last.iter().enumerate() {
            for ((face, field), value) in values {
                let ps = FACES[*face];
                let held = ms.iter().any(|m| value_json(m, ps, field) == *value);
                let set_aside = ms.iter().any(|m| {
                    let id = m.face(ps).0;
                    conflict_values(m)
                        .iter()
                        .any(|(f, fld, v)| *f == id && fld == field && v == value)
                });
                assert!(
                    held || set_aside,
                    "seed {seed}: machine {x}'s {field} {value} of {ps} was lost:
{}",
                    log.join(
                        "
"
                    )
                );
            }
        }
        for m in &ms {
            for tag in &tags {
                assert!(
                    FACES.iter().any(|ps| m.local(ps).tags.contains(tag)),
                    "seed {seed}: tag {tag} was lost"
                );
            }
            for (face, name) in &collections {
                assert!(
                    m.local(FACES[*face])
                        .collections
                        .contains(&vec![name.clone()]),
                    "seed {seed}: membership of {name} was lost"
                );
            }
            let fields: std::collections::BTreeSet<String> = conflict_values(m)
                .into_iter()
                .map(|(_, field, _)| field)
                .collect();
            assert!(
                fields
                    .iter()
                    .all(|f| ["favorite", "rating", "notes"].contains(&f.as_str())),
                "seed {seed}: tags and collections never conflict: {fields:?}"
            );
        }
    }
}

#[test]
fn a_sidecar_a_sync_client_changes_is_merged_after_the_watcher_debounce() {
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::Duration;

    use crate::scanner::{Trigger, WatchSettings, Watcher};

    let (_ta, mut a) = machine("e28-watch-a");
    let tmp = TempDir::new("e28-watch-b");
    let mut b = Machine::new(tmp.path());
    a.tag("Lora-Regular", &["a"]);
    sync(&a, &b);
    b.scan();

    let files = Arc::new(Mutex::new(b.index.load_files().expect("files")));
    let (tx, updates) = mpsc::channel();
    let settings = WatchSettings {
        quiet: Duration::from_millis(100),
        max_wait: Duration::from_millis(400),
        rescan_interval: Duration::from_secs(3600),
    };
    let watcher = Watcher::start(files.clone(), settings, move |u| {
        let _ = tx.send(u);
    })
    .expect("watcher");
    watcher.add_source(&b.root).expect("add");
    let first = updates
        .recv_timeout(Duration::from_secs(10))
        .expect("first scan");
    assert_eq!(first.trigger, Trigger::Added);
    while updates.recv_timeout(Duration::from_millis(600)).is_ok() {}

    // Another machine's edit arrives through the sync client.
    a.tag("Lora-Regular", &["from-a"]);
    sync(&a, &b);
    let update = loop {
        let u = updates
            .recv_timeout(Duration::from_secs(10))
            .expect("the watcher reports the sidecar");
        if !u.sidecars.is_empty() {
            break u;
        }
    };
    assert_eq!(update.trigger, Trigger::Events, "no full rescan");
    assert_eq!(update.sidecars, [b.sidecar_path("Lora")]);
    let rescan = update.result.expect("rescan");
    assert!(rescan.is_quiet(), "no font was read again");
    assert!(rescan.unchanged.is_empty(), "no font was looked at");
    let found = b
        .index
        .apply_rescan(b.source, &files.lock().expect("files"), &rescan);
    found.expect("apply");
    assert_eq!(b.local("Lora-Regular").tags, ["a", "from-a"]);
    drop(watcher);
}
