use std::path::{Path, PathBuf};

use fm_api::FaceId;
use fm_api::common::{FontFormat, Health};
use fm_api::library::{ProblemKind, SourceKind};
use fm_api::organization::{
    CollectionFacesRequest, CreateCollectionRequest, RenameCollectionRequest, TagFacesRequest,
    UntagFacesRequest, UserMetaUpdate,
};

use super::*;
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, FileIndex, IndexedFile, Rescan, discover};
use crate::sidecar::{FaceKey, Meta, SIDECAR_VERSION, Sidecar, file_name};
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
            name: "Work".into(),
            parent: None,
            rules: None,
        })
        .expect("work");
    let client = m
        .index
        .create_collection(&CreateCollectionRequest {
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
    assert_eq!(inter.version, SIDECAR_VERSION);
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
            name: "Old".into(),
            parent: None,
            rules: None,
        })
        .expect("parent");
    let child = m
        .index
        .create_collection(&CreateCollectionRequest {
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
