//! E56.5: the kept copy takes over what the user put on the copies that
//! leave the library.

use std::fs;
use std::path::{Path, PathBuf};

use fm_api::common::{FontFormat, Health};
use fm_api::library::SourceKind;
use fm_api::organization::{
    CollectionFacesRequest, CreateCollectionRequest, CreateSetRequest, TagFacesRequest,
    UserMetaUpdate,
};
use fm_api::{CollectionId, SetId};
use fm_platform::MockTrash;

use super::*;
use crate::index::{FaceMeta, INDEX_FILE, MergeConflict, MergeField, MergeValue};
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, IndexedFile};
use crate::test_support::TempDir;

/// One file of a fixture: its name, its hash byte and the PostScript names
/// of its faces (more than one: a collection file).
struct Spec(&'static str, u8, &'static [&'static str]);

fn indexed(path: &Path, hash: u8, names: &[&str]) -> IndexedFile {
    let faces = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let mut face = ParsedFace::empty(u32::try_from(i).expect("index"), Health::Ok);
            face.family = Some("Family".to_owned());
            face.subfamily = Some((*name).to_owned());
            face.postscript_name = Some((*name).to_owned());
            face.weight = Some(400);
            face.width = Some(5);
            face
        })
        .collect();
    IndexedFile {
        path: path.to_path_buf(),
        size: 4,
        modified: None,
        hash: FileHash::from([hash; 32]),
        parsed: ParsedFile {
            format: Some(if names.len() > 1 {
                FontFormat::Ttc
            } else {
                FontFormat::Ttf
            }),
            health: Health::Ok,
            faces,
        },
        gone: false,
    }
}

struct Fx {
    dir: TempDir,
    index: Arc<Mutex<Index>>,
    trash: Arc<MockTrash>,
    resolver: DuplicateResolver,
}

impl Fx {
    fn new(label: &str, specs: &[Spec]) -> Self {
        let dir = TempDir::new(&format!("merge-{label}"));
        // Each folder is a source of its own.
        let mut folders: Vec<(String, Vec<IndexedFile>)> = Vec::new();
        for Spec(name, hash, names) in specs {
            let path = Self::under(dir.path(), name);
            let folder = path
                .parent()
                .expect("folder")
                .to_string_lossy()
                .into_owned();
            fs::create_dir_all(&folder).expect("folder");
            fs::write(&path, [*hash; 4]).expect("write");
            let file = indexed(&path, *hash, names);
            match folders.iter_mut().find(|(f, _)| *f == folder) {
                Some((_, files)) => files.push(file),
                None => folders.push((folder, vec![file])),
            }
        }
        let mut index = Index::open(&dir.path().join(INDEX_FILE))
            .expect("open")
            .index;
        for (folder, files) in &folders {
            let source = index
                .add_source(folder, SourceKind::Watched)
                .expect("source")
                .id;
            index.persist_files(source, files).expect("persist");
        }
        let trash = Arc::new(MockTrash::new(dir.path().join("trash")));
        let index = Arc::new(Mutex::new(index));
        let resolver = Self::resolver(&index, trash.clone());
        Self {
            dir,
            index,
            trash,
            resolver,
        }
    }

    /// Two identical copies of one font.
    fn pair(label: &str) -> Self {
        Self::new(
            label,
            &[
                Spec("keep.ttf", 1, &["Family-Regular"]),
                Spec("copy.ttf", 1, &["Family-Regular"]),
            ],
        )
    }

    fn resolver(index: &Arc<Mutex<Index>>, trash: Arc<dyn Trash>) -> DuplicateResolver {
        DuplicateResolver::new(Arc::clone(index), trash, |_| {})
    }

    /// `name` is a file in `fonts/`, or `folder/file` in another source.
    fn under(dir: &Path, name: &str) -> PathBuf {
        if name.contains('/') {
            dir.join(name)
        } else {
            dir.join("fonts").join(name)
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        Self::under(self.dir.path(), name)
    }

    /// The sidecar entry of the face at `index` of the file `name`, as the
    /// sidecar of its source holds it.
    fn sidecar_entry(&self, name: &str, index: u32) -> Option<crate::sidecar::FaceEntry> {
        let path = self.path(name);
        let folder = path.parent().expect("folder");
        let file = path
            .file_name()
            .expect("file")
            .to_string_lossy()
            .into_owned();
        let dir = folder.join(crate::sidecar::SIDECAR_DIR);
        for entry in fs::read_dir(dir).ok()?.flatten() {
            let text = fs::read_to_string(entry.path()).expect("sidecar");
            let crate::sidecar::Parsed::Sidecar(sidecar) = crate::sidecar::parse(&text) else {
                continue;
            };
            if let Some(face) = sidecar
                .faces
                .into_iter()
                .find(|f| f.file == file && f.index == index)
            {
                return Some(face);
            }
        }
        None
    }

    fn shown(&self, name: &str) -> String {
        self.path(name).to_string_lossy().into_owned()
    }

    fn request(&self, keep: &str, trash: &[&str]) -> KeepCopyRequest {
        KeepCopyRequest {
            keep: self.shown(keep),
            trash: trash.iter().map(|n| self.shown(n)).collect(),
            dry_run: false,
        }
    }

    fn index(&self) -> MutexGuard<'_, Index> {
        self.index.lock().expect("index")
    }

    /// The faces of a file, in collection order.
    fn faces(&self, name: &str) -> Vec<FaceId> {
        let shown = self.shown(name);
        let index = self.index();
        let mut files = index.copy_files(&[shown.as_str()], &[]).expect("files");
        files.remove(&shown).expect("a library file").faces
    }

    fn face(&self, name: &str) -> FaceId {
        self.faces(name)[0]
    }

    fn meta(&self, face: FaceId) -> FaceMeta {
        self.index().face_meta(face).expect("meta")
    }

    fn tag(&self, face: FaceId, tag: &str) {
        self.index()
            .tag_faces(&TagFacesRequest {
                faces: vec![face],
                tags: vec![tag.to_owned()],
                families: Vec::new(),
            })
            .expect("tag");
    }

    fn set_meta(
        &self,
        face: FaceId,
        rating: Option<u8>,
        notes: Option<&str>,
        favorite: Option<bool>,
    ) {
        self.index()
            .update_user_meta(&UserMetaUpdate {
                faces: vec![face],
                favorite,
                rating,
                notes: notes.map(str::to_owned),
            })
            .expect("meta");
    }

    fn set_license(&self, face: FaceId, license: &str, seats: Option<u32>) {
        self.index()
            .set_face_license_for_tests(face, license, seats)
            .expect("license");
    }

    fn collection(&self, name: &str, face: FaceId) -> CollectionId {
        let mut index = self.index();
        let c = index
            .create_collection(&CreateCollectionRequest {
                name: name.to_owned(),
                parent: None,
                rules: None,
                rule_group: None,
                color: None,
            })
            .expect("collection")
            .id;
        index
            .add_to_collection(&CollectionFacesRequest {
                collection: c,
                faces: vec![face],
                families: Vec::new(),
            })
            .expect("add");
        c
    }

    fn set(&self, name: &str, face: FaceId) -> SetId {
        self.index()
            .create_set(&CreateSetRequest {
                name: name.to_owned(),
                faces: vec![face],
            })
            .expect("set")
            .id
    }
}

fn tags(m: &FaceMeta) -> Vec<String> {
    m.tags.values().cloned().collect()
}

/// Criterion 1 first showed today's behaviour with this test: before
/// E56.5, `keep_copy` left every one of these on the trashed copy and the
/// kept copy had none of them (see the PR).
#[test]
fn keep_copy_moves_every_kind_of_metadata_to_the_kept_copy() {
    let fx = Fx::pair("all");
    let (keep, copy) = (fx.face("keep.ttf"), fx.face("copy.ttf"));
    fx.tag(copy, "Display");
    fx.set_meta(copy, Some(4), Some("Bought for the poster"), Some(true));
    fx.set_license(copy, "desktop", Some(3));
    let c = fx.collection("Posters", copy);
    let s = fx.set("Client A", copy);
    let before = fx.meta(copy);

    fx.resolver
        .keep_copy(fx.request("keep.ttf", &["copy.ttf"]))
        .expect("keep");

    let kept = fx.meta(keep);
    assert_eq!(tags(&kept), ["Display"]);
    assert!(kept.favorite);
    assert_eq!(kept.rating, Some(4));
    assert_eq!(kept.notes.as_deref(), Some("Bought for the poster"));
    assert_eq!(kept.license, Some((Some("desktop".to_owned()), Some(3))));
    assert_eq!(kept.collections, [c.0].into());
    assert_eq!(kept.sets, [s.0].into());
    assert_eq!(fx.meta(copy), before, "the trashed copy keeps its own");
}

/// One kind of metadata the user can put on a face.
#[derive(Debug, Clone, Copy)]
enum Kind {
    Tag,
    Collection,
    Set,
    Favorite,
    Rating,
    License,
    Notes,
}

const KINDS: [Kind; 7] = [
    Kind::Tag,
    Kind::Collection,
    Kind::Set,
    Kind::Favorite,
    Kind::Rating,
    Kind::License,
    Kind::Notes,
];

#[test]
fn each_field_alone_moves_to_the_kept_copy_and_nothing_else_does() {
    for kind in KINDS {
        let fx = Fx::pair(&format!("alone-{kind:?}"));
        let (keep, copy) = (fx.face("keep.ttf"), fx.face("copy.ttf"));
        let mut want = FaceMeta::default();
        match kind {
            Kind::Tag => {
                fx.tag(copy, "Grotesk");
                want.tags = fx.meta(copy).tags;
            }
            Kind::Collection => {
                want.collections = [fx.collection("Shelf", copy).0].into();
            }
            Kind::Set => want.sets = [fx.set("Job", copy).0].into(),
            Kind::Favorite => {
                fx.set_meta(copy, None, None, Some(true));
                want.favorite = true;
            }
            Kind::Rating => {
                fx.set_meta(copy, Some(2), None, None);
                want.rating = Some(2);
            }
            Kind::License => {
                fx.set_license(copy, "web", Some(7));
                want.license = Some((Some("web".to_owned()), Some(7)));
            }
            Kind::Notes => {
                fx.set_meta(copy, None, Some("Only for headlines"), None);
                want.notes = Some("Only for headlines".to_owned());
            }
        }
        let plan = fx
            .resolver
            .merge_plan(&fx.request("keep.ttf", &["copy.ttf"]))
            .expect("plan");
        assert!(plan.conflicts.is_empty(), "{kind:?}: nothing to choose");

        fx.resolver
            .keep_copy(fx.request("keep.ttf", &["copy.ttf"]))
            .expect("keep");

        assert_eq!(fx.meta(keep), want, "{kind:?}");
    }
}

#[test]
fn tags_collections_and_sets_join_and_favorite_is_on_when_any_copy_has_it() {
    let fx = Fx::pair("join");
    let (keep, copy) = (fx.face("keep.ttf"), fx.face("copy.ttf"));
    fx.tag(keep, "Mine");
    fx.tag(copy, "Theirs");
    let c1 = fx.collection("Kept shelf", keep);
    let c2 = fx.collection("Their shelf", copy);
    let s1 = fx.set("Kept job", keep);
    let s2 = fx.set("Their job", copy);
    fx.set_meta(copy, None, None, Some(true));

    fx.resolver
        .keep_copy(fx.request("keep.ttf", &["copy.ttf"]))
        .expect("keep");

    let kept = fx.meta(keep);
    assert_eq!(tags(&kept), ["Mine", "Theirs"]);
    assert_eq!(kept.collections, [c1.0, c2.0].into());
    assert_eq!(kept.sets, [s1.0, s2.0].into());
    assert!(kept.favorite);
}

#[test]
fn a_favorite_on_the_kept_copy_stays_when_the_other_has_none() {
    let fx = Fx::pair("fav-kept");
    let keep = fx.face("keep.ttf");
    fx.set_meta(keep, None, None, Some(true));
    fx.resolver
        .keep_copy(fx.request("keep.ttf", &["copy.ttf"]))
        .expect("keep");
    assert!(fx.meta(keep).favorite);
}

/// Sets one single-value field on a face.
fn put(fx: &Fx, face: FaceId, field: MergeField, v: u8) -> MergeValue {
    match field {
        MergeField::Rating => {
            fx.set_meta(face, Some(v), None, None);
            MergeValue::Rating(v)
        }
        MergeField::License => {
            let t = ["desktop", "web", "app"][usize::from(v % 3)];
            fx.set_license(face, t, Some(u32::from(v)));
            MergeValue::License(Some(t.to_owned()), Some(u32::from(v)))
        }
        MergeField::Notes => {
            let n = format!("note {v}");
            fx.set_meta(face, None, Some(&n), None);
            MergeValue::Notes(n)
        }
    }
}

fn get(m: &FaceMeta, field: MergeField) -> Option<MergeValue> {
    crate::index::metadata_value_for_tests(m, field)
}

const FIELDS: [MergeField; 3] = [MergeField::Rating, MergeField::License, MergeField::Notes];

#[test]
fn a_single_value_keeps_the_kept_copy_s_and_takes_the_other_s_when_it_has_none() {
    for field in FIELDS {
        let fx = Fx::pair(&format!("single-{field:?}"));
        let (keep, copy) = (fx.face("keep.ttf"), fx.face("copy.ttf"));
        let theirs = put(&fx, copy, field, 4);
        fx.resolver
            .keep_copy(fx.request("keep.ttf", &["copy.ttf"]))
            .expect("keep");
        assert_eq!(get(&fx.meta(keep), field), Some(theirs), "{field:?}: taken");

        let fx = Fx::pair(&format!("same-{field:?}"));
        let (keep, copy) = (fx.face("keep.ttf"), fx.face("copy.ttf"));
        let mine = put(&fx, keep, field, 2);
        put(&fx, copy, field, 2);
        let plan = fx
            .resolver
            .merge_plan(&fx.request("keep.ttf", &["copy.ttf"]))
            .expect("plan");
        assert!(
            plan.conflicts.is_empty(),
            "{field:?}: the same value is no conflict"
        );
        fx.resolver
            .keep_copy(fx.request("keep.ttf", &["copy.ttf"]))
            .expect("keep");
        assert_eq!(get(&fx.meta(keep), field), Some(mine), "{field:?}: kept");
    }
}

#[test]
fn a_conflict_on_each_single_value_field_is_listed_and_the_kept_value_is_the_default() {
    for field in FIELDS {
        let fx = Fx::pair(&format!("conflict-{field:?}"));
        let (keep, copy) = (fx.face("keep.ttf"), fx.face("copy.ttf"));
        let mine = put(&fx, keep, field, 2);
        let theirs = put(&fx, copy, field, 4);

        let plan = fx
            .resolver
            .merge_plan(&fx.request("keep.ttf", &["copy.ttf"]))
            .expect("plan");
        assert_eq!(
            plan.conflicts,
            [MergeConflict {
                kept_face: keep,
                field,
                default_from: keep,
                default: mine.clone(),
                other_face: copy,
                other: theirs.clone(),
            }],
            "{field:?}: both values listed"
        );

        fx.resolver
            .keep_copy(fx.request("keep.ttf", &["copy.ttf"]))
            .expect("keep");
        assert_eq!(
            get(&fx.meta(keep), field),
            Some(mine),
            "{field:?}: the default"
        );
        assert_eq!(
            get(&fx.meta(copy), field),
            Some(theirs),
            "{field:?}: the other value stays on its copy, for undo"
        );
    }
}

#[test]
fn the_user_can_take_the_other_value_of_a_conflict_before_applying() {
    for field in FIELDS {
        let fx = Fx::pair(&format!("take-{field:?}"));
        let (keep, copy) = (fx.face("keep.ttf"), fx.face("copy.ttf"));
        put(&fx, keep, field, 2);
        let theirs = put(&fx, copy, field, 4);

        fx.resolver
            .keep_copy_with(
                fx.request("keep.ttf", &["copy.ttf"]),
                &[MergeChoice {
                    kept_face: keep,
                    field,
                    take: copy,
                }],
            )
            .expect("keep");

        assert_eq!(get(&fx.meta(keep), field), Some(theirs), "{field:?}");
    }
}

#[test]
fn a_choice_that_names_no_conflict_is_refused_and_nothing_moves() {
    let fx = Fx::pair("bad-choice");
    let (keep, copy) = (fx.face("keep.ttf"), fx.face("copy.ttf"));
    let err = fx
        .resolver
        .keep_copy_with(
            fx.request("keep.ttf", &["copy.ttf"]),
            &[MergeChoice {
                kept_face: keep,
                field: MergeField::Rating,
                take: copy,
            }],
        )
        .expect_err("no conflict");
    assert_eq!(err.code, ErrorCode::InvalidRequest);
    assert!(fx.path("copy.ttf").exists());
}

#[test]
fn with_two_leaving_copies_the_first_value_is_the_default_and_the_second_a_conflict() {
    let fx = Fx::new(
        "three",
        &[
            Spec("keep.ttf", 1, &["Family-Regular"]),
            Spec("one.ttf", 1, &["Family-Regular"]),
            Spec("two.ttf", 1, &["Family-Regular"]),
        ],
    );
    let (keep, one, two) = (fx.face("keep.ttf"), fx.face("one.ttf"), fx.face("two.ttf"));
    fx.set_meta(one, None, Some("first"), None);
    fx.set_meta(two, None, Some("second"), None);
    let req = fx.request("keep.ttf", &["one.ttf", "two.ttf"]);

    let plan = fx.resolver.merge_plan(&req).expect("plan");
    assert_eq!(plan.conflicts.len(), 1);
    assert_eq!(
        (plan.conflicts[0].default_from, plan.conflicts[0].other_face),
        (one, two)
    );
    fx.resolver
        .keep_copy_with(
            req,
            &[MergeChoice {
                kept_face: keep,
                field: MergeField::Notes,
                take: two,
            }],
        )
        .expect("keep");
    assert_eq!(fx.meta(keep).notes.as_deref(), Some("second"));
}

#[test]
fn a_collection_file_and_loose_files_merge_face_by_face() {
    let specs = [
        Spec("pack.ttc", 3, &["Family-Regular", "Family-Bold"]),
        Spec("regular.ttf", 4, &["Family-Regular"]),
        Spec("bold.ttf", 5, &["Family-Bold"]),
    ];
    // The collection file stays: each of its faces takes over its loose file.
    let fx = Fx::new("ttc-keep", &specs);
    let pack = fx.faces("pack.ttc");
    fx.tag(fx.face("regular.ttf"), "From regular");
    fx.tag(fx.face("bold.ttf"), "From bold");
    fx.resolver
        .keep_copy(fx.request("pack.ttc", &["regular.ttf", "bold.ttf"]))
        .expect("keep");
    assert_eq!(tags(&fx.meta(pack[0])), ["From regular"]);
    assert_eq!(tags(&fx.meta(pack[1])), ["From bold"]);

    // A loose file stays: it takes over its own face of the collection only.
    let fx = Fx::new("ttc-trash", &specs);
    let pack = fx.faces("pack.ttc");
    let regular = fx.face("regular.ttf");
    fx.tag(pack[0], "Regular in pack");
    fx.tag(pack[1], "Bold in pack");
    fx.resolver
        .keep_copy(fx.request("regular.ttf", &["pack.ttc"]))
        .expect("keep");
    assert_eq!(tags(&fx.meta(regular)), ["Regular in pack"]);
    assert_eq!(
        tags(&fx.meta(pack[1])),
        ["Bold in pack"],
        "a face without a match keeps its data on its copy"
    );
}

#[test]
fn a_failed_move_merges_nothing() {
    let fx = Fx::pair("refused");
    let (keep, copy) = (fx.face("keep.ttf"), fx.face("copy.ttf"));
    fx.tag(copy, "Theirs");
    fx.set_meta(copy, Some(5), Some("theirs"), Some(true));
    let before = fx.meta(keep);
    fx.trash.refuse(fx.path("copy.ttf"));

    let r = fx
        .resolver
        .keep_copy(fx.request("keep.ttf", &["copy.ttf"]))
        .expect("keep");

    assert_ne!(r.moves[0].status, TrashMoveStatus::Trashed);
    assert_eq!(fx.meta(keep), before, "the kept copy is as it was");
}

#[test]
fn only_the_copies_that_moved_give_their_metadata() {
    let fx = Fx::new(
        "partly",
        &[
            Spec("keep.ttf", 1, &["Family-Regular"]),
            Spec("moves.ttf", 1, &["Family-Regular"]),
            Spec("stays.ttf", 1, &["Family-Regular"]),
        ],
    );
    let keep = fx.face("keep.ttf");
    fx.tag(fx.face("moves.ttf"), "Moved");
    fx.tag(fx.face("stays.ttf"), "Stayed");
    fx.trash.refuse(fx.path("stays.ttf"));

    fx.resolver
        .keep_copy(fx.request("keep.ttf", &["moves.ttf", "stays.ttf"]))
        .expect("keep");

    assert_eq!(tags(&fx.meta(keep)), ["Moved"]);
}

/// A trash that moves like [`MockTrash`] and then breaks the merge log, as
/// a full disk would: the transaction of the log cannot be written.
#[derive(Debug)]
struct BreaksTheMergeLog {
    inner: Arc<MockTrash>,
    db: PathBuf,
}

impl Trash for BreaksTheMergeLog {
    fn trash(&self, path: &Path) -> Result<fm_platform::Trashed, fm_platform::TrashError> {
        let item = self.inner.trash(path)?;
        let conn = rusqlite::Connection::open(&self.db).expect("open the index");
        conn.execute_batch("DROP TABLE IF EXISTS resolution_merge")
            .expect("break the log");
        Ok(item)
    }

    fn restore(&self, item: &fm_platform::Trashed) -> Result<(), fm_platform::TrashError> {
        self.inner.restore(item)
    }
}

#[test]
fn when_the_log_cannot_be_written_the_kept_copy_s_metadata_is_as_it_was() {
    let fx = Fx::pair("nolog");
    let (keep, copy) = (fx.face("keep.ttf"), fx.face("copy.ttf"));
    fx.tag(copy, "Theirs");
    fx.set_meta(copy, Some(5), Some("theirs"), Some(true));
    let before = fx.meta(keep);
    let trash: Arc<dyn Trash> = Arc::new(BreaksTheMergeLog {
        inner: Arc::clone(&fx.trash),
        db: fx.dir.path().join(INDEX_FILE),
    });
    let resolver = Fx::resolver(&fx.index, trash);

    resolver
        .keep_copy(fx.request("keep.ttf", &["copy.ttf"]))
        .expect_err("the log cannot be written");

    assert!(fx.path("copy.ttf").exists(), "the copy is back");
    assert_eq!(
        fx.meta(keep),
        before,
        "the merge was rolled back with the log"
    );
    assert_eq!(fx.resolver.list_resolutions().expect("history"), []);
}

#[test]
fn undo_puts_both_sides_back_exactly() {
    let fx = Fx::pair("undo");
    let (keep, copy) = (fx.face("keep.ttf"), fx.face("copy.ttf"));
    fx.tag(keep, "Mine");
    fx.set_meta(keep, Some(3), None, None);
    fx.tag(copy, "Theirs");
    fx.collection("Their shelf", copy);
    fx.set("Their job", copy);
    fx.set_meta(copy, Some(5), Some("theirs"), Some(true));
    fx.set_license(copy, "app", Some(2));
    let (kept_before, copy_before) = (fx.meta(keep), fx.meta(copy));

    let r = fx
        .resolver
        .keep_copy(fx.request("keep.ttf", &["copy.ttf"]))
        .expect("keep");
    assert_ne!(fx.meta(keep), kept_before, "the merge wrote something");
    fx.resolver
        .undo_resolution(r.id.expect("id"))
        .expect("undo");

    assert_eq!(fx.meta(keep), kept_before);
    assert_eq!(fx.meta(copy), copy_before);
}

#[test]
fn undo_after_a_later_change_keeps_the_change_and_takes_back_only_the_merge() {
    let fx = Fx::pair("later");
    let (keep, copy) = (fx.face("keep.ttf"), fx.face("copy.ttf"));
    fx.tag(copy, "Theirs");
    fx.set_meta(copy, Some(5), Some("theirs"), Some(true));
    fx.set_license(copy, "app", Some(2));
    let r = fx
        .resolver
        .keep_copy(fx.request("keep.ttf", &["copy.ttf"]))
        .expect("keep");
    // After the cleanup the user changes the rating and adds a tag.
    fx.set_meta(keep, Some(1), None, None);
    fx.tag(keep, "Added later");

    fx.resolver
        .undo_resolution(r.id.expect("id"))
        .expect("undo");

    let kept = fx.meta(keep);
    assert_eq!(kept.rating, Some(1), "the user's change stays");
    assert_eq!(tags(&kept), ["Added later"], "only the merged tag goes");
    assert!(!kept.favorite);
    assert_eq!(kept.notes, None);
    assert_eq!(kept.license, None);
    assert_eq!(tags(&fx.meta(copy)), ["Theirs"]);
}

#[test]
fn a_partial_undo_keeps_the_merge_until_every_copy_is_back() {
    let fx = Fx::pair("partial");
    let (keep, copy) = (fx.face("keep.ttf"), fx.face("copy.ttf"));
    fx.tag(copy, "Theirs");
    let r = fx
        .resolver
        .keep_copy(fx.request("keep.ttf", &["copy.ttf"]))
        .expect("keep");
    // A file is at the old path again: undo cannot put the copy back.
    fs::write(fx.path("copy.ttf"), b"something else").expect("block");
    let id = r.id.expect("id");
    let first = fx.resolver.undo_resolution(id).expect("undo");
    assert!(!first.undone);
    assert_eq!(
        tags(&fx.meta(keep)),
        ["Theirs"],
        "nothing is lost meanwhile"
    );

    fs::remove_file(fx.path("copy.ttf")).expect("unblock");
    assert!(fx.resolver.undo_resolution(id).expect("retry").undone);
    assert!(tags(&fx.meta(keep)).is_empty());
    assert_eq!(tags(&fx.meta(copy)), ["Theirs"]);
}

#[test]
fn the_sidecars_of_both_sources_carry_the_merge() {
    let fx = Fx::new(
        "sidecars",
        &[
            Spec("keep.ttf", 1, &["Family-Regular"]),
            Spec("other/copy.ttf", 1, &["Family-Regular"]),
        ],
    );
    let copy = fx.face("other/copy.ttf");
    fx.tag(copy, "Theirs");
    fx.set_meta(copy, Some(4), Some("theirs"), Some(true));
    assert!(
        fx.sidecar_entry("keep.ttf", 0).is_none(),
        "nothing on it yet"
    );

    let r = fx
        .resolver
        .keep_copy(fx.request("keep.ttf", &["other/copy.ttf"]))
        .expect("keep");

    let kept = fx.sidecar_entry("keep.ttf", 0).expect("written");
    assert_eq!(kept.meta.tags, ["Theirs"]);
    assert_eq!(kept.meta.rating, Some(4));
    assert_eq!(kept.meta.notes.as_deref(), Some("theirs"));
    assert!(kept.meta.favorite);
    let theirs = fx.sidecar_entry("other/copy.ttf", 0).expect("still there");
    assert_eq!(
        theirs.meta.tags,
        ["Theirs"],
        "the other source keeps its own"
    );

    fx.resolver
        .undo_resolution(r.id.expect("id"))
        .expect("undo");
    let kept = fx.sidecar_entry("keep.ttf", 0);
    assert!(
        kept.is_none_or(|k| k.meta.tags.is_empty() && k.meta.rating.is_none()),
        "undo is written to the sidecar too"
    );
}

#[test]
fn a_dry_run_merges_nothing() {
    let fx = Fx::pair("dry");
    let (keep, copy) = (fx.face("keep.ttf"), fx.face("copy.ttf"));
    fx.tag(copy, "Theirs");
    let before = fx.meta(keep);
    let mut req = fx.request("keep.ttf", &["copy.ttf"]);
    req.dry_run = true;
    fx.resolver.keep_copy(req).expect("plan");
    assert_eq!(fx.meta(keep), before);
}
