use std::fs;
use std::path::{Path, PathBuf};

use fm_api::activation::{ActivateRequest, ActivationMode, ConflictPolicy, Target};
use fm_api::common::{FontFormat, Health};
use fm_api::library::{DuplicateGroup, ProblemKind, SourceKind};
use fm_api::{ActivationService, ErrorCode};
use fm_platform::{FontActivator, MockActivator, MockTrash};

use super::*;
use crate::ActivationManager;
use crate::index::INDEX_FILE;
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, IndexedFile};
use crate::test_support::TempDir;

/// Three identical copies of one font and an unrelated font, as real files
/// in a temp folder, an index file over them, and a resolver whose trash is
/// a folder next to them.
struct Fixture {
    dir: TempDir,
    index: Arc<Mutex<Index>>,
    trash: Arc<MockTrash>,
    events: Arc<Mutex<Vec<Event>>>,
    resolver: DuplicateResolver,
}

const CONTENT: &[u8] = b"the same font bytes";

fn indexed(path: &Path, hash: u8, name: &str) -> IndexedFile {
    let mut face = ParsedFace::empty(0, Health::Ok);
    face.family = Some("Family".to_owned());
    face.subfamily = Some("Regular".to_owned());
    face.postscript_name = Some(name.to_owned());
    face.weight = Some(400);
    face.width = Some(5);
    IndexedFile {
        path: path.to_path_buf(),
        size: CONTENT.len() as u64,
        modified: None,
        hash: FileHash::from([hash; 32]),
        parsed: ParsedFile {
            format: Some(FontFormat::Ttf),
            health: Health::Ok,
            faces: vec![face],
        },
        gone: false,
    }
}

impl Fixture {
    fn new(label: &str) -> Self {
        let dir = TempDir::new(&format!("resolve-{label}"));
        let fonts = dir.path().join("fonts");
        fs::create_dir_all(&fonts).expect("fonts");
        let mut files = Vec::new();
        for name in ["a.ttf", "b.ttf", "c.ttf"] {
            let path = fonts.join(name);
            fs::write(&path, CONTENT).expect("write");
            files.push(indexed(&path, 1, "Family-Regular"));
        }
        let other = fonts.join("other.ttf");
        fs::write(&other, b"another font").expect("write");
        files.push(indexed(&other, 2, "Other-Regular"));
        let mut index = Index::open(&dir.path().join(INDEX_FILE))
            .expect("open")
            .index;
        let source = index
            .add_source(&fonts.to_string_lossy(), SourceKind::Watched)
            .expect("source")
            .id;
        index.persist_files(source, &files).expect("persist");
        let trash = Arc::new(MockTrash::new(dir.path().join("trash")));
        let index = Arc::new(Mutex::new(index));
        let events = Arc::new(Mutex::new(Vec::new()));
        let resolver = Self::resolver(&index, &trash, &events);
        Self {
            dir,
            index,
            trash,
            events,
            resolver,
        }
    }

    fn resolver(
        index: &Arc<Mutex<Index>>,
        trash: &Arc<MockTrash>,
        events: &Arc<Mutex<Vec<Event>>>,
    ) -> DuplicateResolver {
        let sink = Arc::clone(events);
        let trash: Arc<dyn Trash> = trash.clone();
        DuplicateResolver::new(Arc::clone(index), trash, move |e| {
            sink.lock().expect("events").push(e);
        })
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join("fonts").join(name)
    }

    fn shown(&self, name: &str) -> String {
        self.path(name).to_string_lossy().into_owned()
    }

    fn request(&self, keep: &str, trash: &[&str], dry_run: bool) -> KeepCopyRequest {
        KeepCopyRequest {
            keep: self.shown(keep),
            trash: trash.iter().map(|n| self.shown(n)).collect(),
            dry_run,
        }
    }

    fn groups(&self) -> Vec<DuplicateGroup> {
        self.index
            .lock()
            .expect("index")
            .duplicates()
            .expect("duplicates")
    }

    fn statuses(r: &Resolution) -> Vec<TrashMoveStatus> {
        r.moves.iter().map(|m| m.status).collect()
    }

    fn events(&self) -> Vec<Event> {
        std::mem::take(&mut *self.events.lock().expect("events"))
    }

    /// Files in the trash folder, without the mock's records.
    fn in_trash(&self) -> usize {
        fs::read_dir(self.trash.dir()).map_or(0, |d| {
            d.flatten()
                .filter(|e| e.path().extension().is_none_or(|x| x != "info"))
                .count()
        })
    }
}

#[test]
fn keep_this_one_moves_the_other_copies_to_the_trash_and_never_deletes_them() {
    let fx = Fixture::new("keep");
    let faces_before: Vec<_> = fx.groups()[0].copies.iter().map(|c| c.face).collect();

    let r = fx
        .resolver
        .keep_copy(fx.request("a.ttf", &["b.ttf", "c.ttf"], false))
        .expect("keep");

    assert!(r.id.is_some(), "recorded for undo");
    assert!(!r.dry_run && !r.undone);
    assert_eq!(
        Fixture::statuses(&r),
        [TrashMoveStatus::Trashed, TrashMoveStatus::Trashed]
    );
    assert!(fx.path("a.ttf").exists(), "the kept copy stays");
    assert!(!fx.path("b.ttf").exists() && !fx.path("c.ttf").exists());
    assert_eq!(
        fx.in_trash(),
        2,
        "both copies are in the trash, not deleted"
    );
    for entry in fs::read_dir(fx.trash.dir()).expect("trash").flatten() {
        if entry.path().extension().is_none_or(|x| x != "info") {
            assert_eq!(fs::read(entry.path()).expect("in trash"), CONTENT);
        }
    }
    assert!(fx.groups().is_empty(), "no duplicates are left");
    let problems = fx
        .index
        .lock()
        .expect("index")
        .problems()
        .expect("problems");
    assert!(!problems.iter().any(|p| matches!(
        p.kind,
        ProblemKind::DuplicateFile | ProblemKind::DuplicateFace
    )));
    assert_eq!(fx.resolver.list_resolutions().expect("history"), [r]);
    let removed: Vec<FaceId> = fx
        .events()
        .into_iter()
        .flat_map(|e| match e {
            Event::LibraryChanged { removed, .. } => removed,
            _ => Vec::new(),
        })
        .collect();
    assert_eq!(removed.len(), 2);
    assert!(removed.iter().all(|f| faces_before.contains(&Some(*f))));
}

#[test]
fn a_dry_run_shows_what_will_move_and_moves_nothing() {
    let fx = Fixture::new("dry");
    let r = fx
        .resolver
        .keep_copy(fx.request("a.ttf", &["b.ttf", "c.ttf"], true))
        .expect("plan");

    assert_eq!((r.id, r.dry_run), (None, true));
    assert_eq!(r.kept, fx.shown("a.ttf"));
    let planned: Vec<(&str, TrashMoveStatus)> = r
        .moves
        .iter()
        .map(|m| (m.path.as_str(), m.status))
        .collect();
    let (b, c) = (fx.shown("b.ttf"), fx.shown("c.ttf"));
    assert_eq!(
        planned,
        [
            (b.as_str(), TrashMoveStatus::Planned),
            (c.as_str(), TrashMoveStatus::Planned)
        ]
    );
    assert!(fx.path("b.ttf").exists() && fx.path("c.ttf").exists());
    assert_eq!(fx.in_trash(), 0);
    assert_eq!(fx.groups()[0].copies.len(), 3);
    assert_eq!(fx.resolver.list_resolutions().expect("history"), []);
    assert!(fx.events().is_empty());
}

#[test]
fn undo_puts_every_copy_back_with_its_face_ids() {
    let fx = Fixture::new("undo");
    let before = fx.groups();
    let r = fx
        .resolver
        .keep_copy(fx.request("a.ttf", &["b.ttf", "c.ttf"], false))
        .expect("keep");
    fx.events();

    let undone = fx
        .resolver
        .undo_resolution(r.id.expect("id"))
        .expect("undo");

    assert!(undone.undone);
    assert_eq!(
        Fixture::statuses(&undone),
        [TrashMoveStatus::Restored, TrashMoveStatus::Restored]
    );
    assert_eq!(fs::read(fx.path("b.ttf")).expect("back"), CONTENT);
    assert_eq!(fs::read(fx.path("c.ttf")).expect("back"), CONTENT);
    assert_eq!(fx.in_trash(), 0, "nothing is left in the trash");
    assert_eq!(
        fx.groups(),
        before,
        "the same copies with the same face ids"
    );
    assert_eq!(fx.resolver.list_resolutions().expect("history"), [undone]);
    assert!(matches!(
        fx.events().as_slice(),
        [Event::LibraryChanged { added, .. }] if added.len() == 2
    ));
}

#[test]
fn undo_from_the_history_works_after_a_restart() {
    let fx = Fixture::new("restart");
    let id = fx
        .resolver
        .keep_copy(fx.request("a.ttf", &["b.ttf"], false))
        .expect("keep")
        .id
        .expect("id");
    // A new run of the app: a fresh index connection and resolver.
    let reopened = Index::open(&fx.dir.path().join(INDEX_FILE))
        .expect("reopen")
        .index;
    let index = Arc::new(Mutex::new(reopened));
    let resolver = Fixture::resolver(&index, &fx.trash, &fx.events);

    let history = resolver.list_resolutions().expect("history");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, Some(id));
    resolver.undo_resolution(id).expect("undo");
    assert!(fx.path("b.ttf").exists());
}

#[test]
fn undo_never_overwrites_a_file_at_the_old_path_and_can_be_retried() {
    let fx = Fixture::new("occupied");
    let id = fx
        .resolver
        .keep_copy(fx.request("a.ttf", &["b.ttf", "c.ttf"], false))
        .expect("keep")
        .id
        .expect("id");
    fs::write(fx.path("b.ttf"), b"a new file").expect("new file");

    let first = fx.resolver.undo_resolution(id).expect("undo");
    assert!(!first.undone);
    assert_eq!(
        Fixture::statuses(&first),
        [TrashMoveStatus::Failed, TrashMoveStatus::Restored]
    );
    assert!(first.moves[0].message.is_some());
    assert_eq!(fs::read(fx.path("b.ttf")).expect("new"), b"a new file");

    fs::remove_file(fx.path("b.ttf")).expect("the user moves it away");
    let second = fx.resolver.undo_resolution(id).expect("retry");
    assert!(second.undone);
    assert_eq!(
        Fixture::statuses(&second),
        [TrashMoveStatus::Restored, TrashMoveStatus::Restored]
    );
    assert_eq!(fs::read(fx.path("b.ttf")).expect("back"), CONTENT);
}

#[test]
fn a_copy_the_app_activated_stays_where_it_is() {
    let fx = Fixture::new("active");
    let b_face = fx.groups()[0]
        .copies
        .iter()
        .find(|c| c.path == fx.shown("b.ttf"))
        .and_then(|c| c.face)
        .expect("face");
    let activator: Arc<dyn FontActivator> = Arc::new(MockActivator::new());
    ActivationManager::new(Arc::clone(&fx.index), activator, |_| {})
        .activate(ActivateRequest {
            target: Target::Faces {
                faces: vec![b_face],
            },
            mode: ActivationMode::Session,
            on_conflict: ConflictPolicy::KeepExisting,
        })
        .expect("activate");

    let plan = fx
        .resolver
        .keep_copy(fx.request("a.ttf", &["b.ttf", "c.ttf"], true))
        .expect("plan");
    assert_eq!(
        Fixture::statuses(&plan),
        [TrashMoveStatus::Skipped, TrashMoveStatus::Planned]
    );
    let r = fx
        .resolver
        .keep_copy(fx.request("a.ttf", &["b.ttf", "c.ttf"], false))
        .expect("keep");
    assert_eq!(
        Fixture::statuses(&r),
        [TrashMoveStatus::Skipped, TrashMoveStatus::Trashed]
    );
    assert_eq!(r.moves[0].message.as_deref(), Some(ACTIVE_COPY));
    assert!(fx.path("b.ttf").exists());
}

#[test]
fn a_copy_on_a_drive_without_a_trash_is_skipped_not_deleted() {
    let fx = Fixture::new("notrash");
    fx.trash.refuse(fx.path("b.ttf"));
    let r = fx
        .resolver
        .keep_copy(fx.request("a.ttf", &["b.ttf"], false))
        .expect("keep");
    assert_eq!(r.id, None, "nothing moved, nothing to undo");
    assert_eq!(Fixture::statuses(&r), [TrashMoveStatus::Skipped]);
    assert!(
        r.moves[0]
            .message
            .as_deref()
            .is_some_and(|m| m.contains("no trash"))
    );
    assert_eq!(fs::read(fx.path("b.ttf")).expect("still there"), CONTENT);
    assert_eq!(fx.resolver.list_resolutions().expect("history"), []);
}

#[test]
fn only_a_duplicate_of_the_kept_copy_can_move() {
    let fx = Fixture::new("stranger");
    let err = fx
        .resolver
        .keep_copy(fx.request("a.ttf", &["other.ttf"], false))
        .expect_err("not a copy");
    assert_eq!(err.code, ErrorCode::InvalidRequest);
    let notes = fx.dir.path().join("notes.txt");
    fs::write(&notes, b"user notes").expect("write");
    let err = fx
        .resolver
        .keep_copy(KeepCopyRequest {
            keep: fx.shown("a.ttf"),
            trash: vec![notes.to_string_lossy().into_owned()],
            dry_run: false,
        })
        .expect_err("not in the library");
    assert_eq!(err.code, ErrorCode::InvalidRequest);
    assert!(fx.path("other.ttf").exists() && notes.exists());
    assert_eq!(fx.in_trash(), 0);
}

#[test]
fn nothing_moves_when_the_kept_copy_is_not_on_disk() {
    let fx = Fixture::new("keptgone");
    fs::remove_file(fx.path("a.ttf")).expect("the user removed it");
    let err = fx
        .resolver
        .keep_copy(fx.request("a.ttf", &["b.ttf", "c.ttf"], false))
        .expect_err("kept copy missing");
    assert_eq!(err.code, ErrorCode::NotFound);
    assert!(fx.path("b.ttf").exists() && fx.path("c.ttf").exists());
}

#[test]
fn a_copy_that_left_its_folder_is_skipped() {
    let fx = Fixture::new("left");
    fs::remove_file(fx.path("c.ttf")).expect("gone before a rescan");
    let r = fx
        .resolver
        .keep_copy(fx.request("a.ttf", &["b.ttf", "c.ttf"], false))
        .expect("keep");
    assert_eq!(
        Fixture::statuses(&r),
        [TrashMoveStatus::Trashed, TrashMoveStatus::Skipped]
    );
    assert_eq!(r.moves[1].message.as_deref(), Some(NOT_THERE));
}

#[test]
fn undoing_an_unknown_action_is_not_found() {
    let fx = Fixture::new("unknown");
    let err = fx
        .resolver
        .undo_resolution(ResolutionId(42))
        .expect_err("unknown");
    assert_eq!(err.code, ErrorCode::NotFound);
}

/// A copy whose name is not UTF-8 (raw bytes on Linux, an unpaired
/// surrogate on Windows) moves and comes back by its exact path. macOS
/// file systems refuse such names, so there the test has nothing to do.
#[test]
fn a_copy_with_a_non_utf8_name_moves_and_comes_back() {
    let fx = Fixture::new("nonutf8");
    let name = if cfg!(windows) {
        vec![0x41, 0x00, 0x00, 0xD8, 0x2E, 0x00, 0x74, 0x00] // "A\u{D800}.t"
    } else {
        vec![0x41, 0xFF, 0x2E, 0x74] // "A\xFF.t"
    };
    let Some(file_name) = fm_platform::fs::path_from_bytes(&name) else {
        return;
    };
    let odd = fx.path("x").with_file_name(file_name);
    if fs::write(&odd, CONTENT).is_err() {
        return;
    }
    {
        let mut index = fx.index.lock().expect("index");
        let source = index.sources().expect("sources")[0].id;
        index
            .persist_files(source, &[indexed(&odd, 1, "Family-Regular")])
            .expect("persist");
    }
    let shown = odd.to_string_lossy().into_owned();
    let r = fx
        .resolver
        .keep_copy(KeepCopyRequest {
            keep: fx.shown("a.ttf"),
            trash: vec![shown],
            dry_run: false,
        })
        .expect("keep");
    assert_eq!(Fixture::statuses(&r), [TrashMoveStatus::Trashed]);
    assert!(!odd.exists());
    fx.resolver
        .undo_resolution(r.id.expect("id"))
        .expect("undo");
    assert_eq!(fs::read(&odd).expect("back by its exact path"), CONTENT);
}

/// A trash that moves like [`MockTrash`] and then breaks the index's log
/// table, as a full disk or a damaged file would: the log of the moves
/// cannot be written after the files have moved.
#[derive(Debug)]
struct BreaksTheLog {
    inner: Arc<MockTrash>,
    db: PathBuf,
}

impl Trash for BreaksTheLog {
    fn trash(&self, path: &Path) -> Result<fm_platform::Trashed, fm_platform::TrashError> {
        let item = self.inner.trash(path)?;
        let conn = rusqlite::Connection::open(&self.db).expect("open the index");
        conn.execute_batch("DROP TABLE IF EXISTS resolution_move")
            .expect("break the log");
        Ok(item)
    }
}

#[test]
fn moved_copies_come_back_when_their_undo_record_cannot_be_written() {
    let fx = Fixture::new("nolog");
    let before = fx.groups();
    let trash: Arc<dyn Trash> = Arc::new(BreaksTheLog {
        inner: Arc::clone(&fx.trash),
        db: fx.dir.path().join(INDEX_FILE),
    });
    let sink = Arc::clone(&fx.events);
    let resolver = DuplicateResolver::new(Arc::clone(&fx.index), trash, move |e| {
        sink.lock().expect("events").push(e);
    });

    let err = resolver
        .keep_copy(fx.request("a.ttf", &["b.ttf", "c.ttf"], false))
        .expect_err("the log cannot be written");

    assert_eq!(err.code, ErrorCode::Io);
    // Never a move without an undo: both copies are back in place.
    assert_eq!(fs::read(fx.path("b.ttf")).expect("b back"), CONTENT);
    assert_eq!(fs::read(fx.path("c.ttf")).expect("c back"), CONTENT);
    assert_eq!(fx.in_trash(), 0);
    // The index still shows them; nothing was announced as removed.
    assert_eq!(fx.groups(), before);
    assert!(fx.events().is_empty());
}
