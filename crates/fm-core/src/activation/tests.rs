use std::path::{Path, PathBuf};
use std::time::Instant;

use fm_api::activation::OutcomeStatus as Status;
use fm_api::common::Health;
use fm_api::library::{ProblemKind, SourceKind};
use fm_api::organization::{CollectionFacesRequest, CreateCollectionRequest, CreateSetRequest};
use fm_api::{CollectionId, FamilyId, SetId};
use fm_platform::{ConflictKind, MockActivator, Mode};

use super::*;
use crate::index::INDEX_FILE;
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, IndexedFile};
use crate::test_support::TempDir;

/// A face of a file: (PostScript name, family, style).
type Spec<'a> = (&'a str, &'a str, &'a str);

fn file(path: &str, format: FontFormat, faces: &[Spec], health: Health) -> IndexedFile {
    let faces = faces
        .iter()
        .enumerate()
        .map(|(i, (ps, family, style))| {
            let mut face = ParsedFace::empty(u32::try_from(i).expect("index"), health.clone());
            face.family = Some((*family).to_owned());
            face.subfamily = Some((*style).to_owned());
            face.postscript_name = Some((*ps).to_owned());
            face.weight = Some(400);
            face.width = Some(5);
            face
        })
        .collect();
    IndexedFile {
        path: PathBuf::from(path),
        size: 1000,
        modified: None,
        hash: FileHash::from([7; 32]),
        parsed: ParsedFile {
            format: Some(format),
            health: Health::Ok,
            faces,
        },
        gone: false,
    }
}

fn library_files() -> Vec<IndexedFile> {
    vec![
        file(
            "/fonts/Roboto-Regular.ttf",
            FontFormat::Ttf,
            &[("Roboto-Regular", "Roboto", "Regular")],
            Health::Ok,
        ),
        file(
            "/fonts/Roboto-Bold.ttf",
            FontFormat::Ttf,
            &[("Roboto-Bold", "Roboto", "Bold")],
            Health::Ok,
        ),
        file(
            "/fonts/Inter.otf",
            FontFormat::Otf,
            &[("Inter-Regular", "Inter", "Regular")],
            Health::Ok,
        ),
        file(
            "/fonts/Pair.ttc",
            FontFormat::Ttc,
            &[
                ("Pair-Light", "Pair", "Light"),
                ("Pair-Heavy", "Pair", "Heavy"),
            ],
            Health::Ok,
        ),
        file(
            "/fonts/Web.woff2",
            FontFormat::Woff2,
            &[("Web-Regular", "Web", "Regular")],
            Health::Ok,
        ),
        file(
            "/fonts/Broken.ttf",
            FontFormat::Ttf,
            &[("Broken-Regular", "Broken", "Regular")],
            Health::Broken("The glyph table is truncated.".into()),
        ),
        file(
            "/fonts/copy/Roboto-Regular.ttf",
            FontFormat::Ttf,
            &[("Roboto-Regular", "Roboto Copy", "Regular")],
            Health::Ok,
        ),
    ]
}

fn seed(index: &mut Index, files: &[IndexedFile]) {
    let source = index
        .add_source("/fonts", SourceKind::Watched)
        .expect("source")
        .id;
    index.persist_files(source, files).expect("persist");
}

struct Fixture {
    index: Arc<Mutex<Index>>,
    os: Arc<MockActivator>,
    events: Arc<Mutex<Vec<Event>>>,
    service: ActivationManager,
}

impl Fixture {
    fn new() -> Self {
        let mut index = Index::open_in_memory().expect("open");
        seed(&mut index, &library_files());
        Self::over(Arc::new(Mutex::new(index)), Arc::new(MockActivator::new()))
    }

    /// A service over this index and activator, as a new run of the app.
    fn over(index: Arc<Mutex<Index>>, os: Arc<MockActivator>) -> Self {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&events);
        let activator: Arc<dyn FontActivator> = os.clone();
        let service = ActivationManager::new(Arc::clone(&index), activator, move |e| {
            sink.lock().expect("events").push(e);
        });
        Self {
            index,
            os,
            events,
            service,
        }
    }

    fn restart(&self) -> Self {
        Self::over(Arc::clone(&self.index), Arc::clone(&self.os))
    }

    fn face(&self, postscript: &str, path: &str) -> FaceId {
        self.index
            .lock()
            .expect("index")
            .conn()
            .query_row(
                "SELECT face.id FROM face JOIN font_file ON font_file.id = face.file_id
                 WHERE postscript_name = ?1 AND font_file.path = ?2",
                [postscript, path],
                |r| r.get(0),
            )
            .map(FaceId)
            .expect("face id")
    }

    fn regular(&self) -> FaceId {
        self.face("Roboto-Regular", "/fonts/Roboto-Regular.ttf")
    }

    fn bold(&self) -> FaceId {
        self.face("Roboto-Bold", "/fonts/Roboto-Bold.ttf")
    }

    fn inter(&self) -> FaceId {
        self.face("Inter-Regular", "/fonts/Inter.otf")
    }

    fn activate(&self, faces: Vec<FaceId>, mode: ActivationMode) -> Vec<ActivationOutcome> {
        self.service
            .activate(ActivateRequest {
                target: Target::Faces { faces },
                mode,
                on_conflict: ConflictPolicy::KeepExisting,
            })
            .expect("activate")
    }

    fn deactivate(&self, faces: Vec<FaceId>) -> Vec<ActivationOutcome> {
        self.service
            .deactivate(DeactivateRequest {
                target: Some(Target::Faces { faces }),
                except: None,
            })
            .expect("deactivate")
    }

    fn active(&self) -> Vec<(FaceId, ActivationMode)> {
        self.service
            .list_active()
            .expect("list")
            .into_iter()
            .map(|a| (a.face, a.mode))
            .collect()
    }

    /// The files the "OS" has registered, with their mode.
    fn registered(&self) -> Vec<(PathBuf, Mode)> {
        let mut all: Vec<_> = self
            .os
            .list_active()
            .expect("os")
            .into_iter()
            .map(|r| (r.path, r.mode))
            .collect();
        all.sort_by(|a, b| a.0.cmp(&b.0));
        all
    }

    fn events(&self) -> Vec<Event> {
        std::mem::take(&mut *self.events.lock().expect("events"))
    }
}

fn statuses(outcomes: &[ActivationOutcome]) -> Vec<Status> {
    outcomes.iter().map(|o| o.outcome.clone()).collect()
}

fn changed(activated: Vec<FaceId>, deactivated: Vec<FaceId>) -> Event {
    Event::ActivationChanged {
        activated,
        deactivated,
    }
}

fn skipped_reason(status: &Status) -> &str {
    match status {
        Status::Skipped { reason } => reason,
        other => panic!("expected Skipped, got {other:?}"),
    }
}

#[test]
fn faces_activate_and_deactivate_through_the_os_with_events() {
    let fx = Fixture::new();
    let (regular, bold) = (fx.regular(), fx.bold());

    let out = fx.activate(vec![regular, bold], ActivationMode::Session);
    assert_eq!(statuses(&out), vec![Status::Done, Status::Done]);
    assert_eq!(
        fx.active(),
        vec![
            (regular, ActivationMode::Session),
            (bold, ActivationMode::Session)
        ]
    );
    assert_eq!(
        fx.registered(),
        vec![
            (PathBuf::from("/fonts/Roboto-Bold.ttf"), Mode::Session),
            (PathBuf::from("/fonts/Roboto-Regular.ttf"), Mode::Session),
        ]
    );
    assert_eq!(fx.events(), vec![changed(vec![regular, bold], vec![])]);

    let again = fx.activate(vec![regular], ActivationMode::Session);
    assert_eq!(statuses(&again), vec![Status::Unchanged]);
    assert_eq!(fx.events(), vec![], "no change, no event");

    let out = fx.deactivate(vec![bold]);
    assert_eq!(statuses(&out), vec![Status::Done]);
    assert_eq!(fx.active(), vec![(regular, ActivationMode::Session)]);
    assert_eq!(
        fx.registered(),
        vec![(PathBuf::from("/fonts/Roboto-Regular.ttf"), Mode::Session)]
    );
    assert_eq!(fx.events(), vec![changed(vec![], vec![bold])]);
}

#[test]
fn a_family_target_activates_every_face_of_the_family() {
    let fx = Fixture::new();
    let (regular, bold) = (fx.regular(), fx.bold());
    let family = FamilyId(regular.0.min(bold.0));
    let out = fx
        .service
        .activate(ActivateRequest {
            target: Target::Family { family },
            mode: ActivationMode::Permanent,
            on_conflict: ConflictPolicy::KeepExisting,
        })
        .expect("activate");
    let faces: Vec<FaceId> = out.iter().map(|o| o.face).collect();
    assert_eq!(faces, vec![regular, bold]);
    assert_eq!(statuses(&out), vec![Status::Done, Status::Done]);
    assert_eq!(
        fx.active(),
        vec![
            (regular, ActivationMode::Permanent),
            (bold, ActivationMode::Permanent)
        ]
    );

    let out = fx
        .service
        .deactivate(DeactivateRequest {
            target: Some(Target::Family { family }),
            except: None,
        })
        .expect("deactivate");
    assert_eq!(statuses(&out), vec![Status::Done, Status::Done]);
    assert_eq!(fx.active(), vec![]);
    assert_eq!(fx.registered(), vec![]);
}

#[test]
fn a_collection_target_activates_its_members() {
    let fx = Fixture::new();
    let (regular, inter) = (fx.regular(), fx.inter());
    let collection = {
        let mut index = fx.index.lock().expect("index");
        let c = index
            .create_collection(&CreateCollectionRequest {
                rule_group: None,
                name: "Brand".into(),
                parent: None,
                rules: None,
            })
            .expect("create");
        index
            .add_to_collection(&CollectionFacesRequest {
                collection: c.id,
                faces: vec![inter, regular],
                families: vec![],
            })
            .expect("add");
        c.id
    };
    let target = Target::Collection { collection };
    let out = fx
        .service
        .activate(ActivateRequest {
            target: target.clone(),
            mode: ActivationMode::Session,
            on_conflict: ConflictPolicy::KeepExisting,
        })
        .expect("activate");
    let mut faces: Vec<FaceId> = out.iter().map(|o| o.face).collect();
    faces.sort();
    let mut want = vec![regular, inter];
    want.sort();
    assert_eq!(faces, want);
    assert!(out.iter().all(|o| o.outcome == Status::Done));

    let out = fx
        .service
        .deactivate(DeactivateRequest {
            target: Some(target),
            except: None,
        })
        .expect("deactivate");
    assert!(out.iter().all(|o| o.outcome == Status::Done));
    assert_eq!(fx.active(), vec![]);
}

#[test]
fn a_set_target_activates_and_deactivates_its_faces_in_one_call() {
    let fx = Fixture::new();
    let (bold, inter) = (fx.bold(), fx.inter());
    let set = fx
        .index
        .lock()
        .expect("index")
        .create_set(&CreateSetRequest {
            name: "Client".into(),
            faces: vec![bold, inter],
        })
        .expect("set")
        .id;
    let target = Target::Set { set };
    let out = fx
        .service
        .activate(ActivateRequest {
            target: target.clone(),
            mode: ActivationMode::Session,
            on_conflict: ConflictPolicy::KeepExisting,
        })
        .expect("activate");
    let mut faces: Vec<FaceId> = out.iter().map(|o| o.face).collect();
    faces.sort();
    let mut want = vec![bold, inter];
    want.sort();
    assert_eq!(faces, want);
    assert!(out.iter().all(|o| o.outcome == Status::Done));
    assert_eq!(fx.active().len(), 2);
    let out = fx
        .service
        .deactivate(DeactivateRequest {
            target: Some(target),
            except: None,
        })
        .expect("deactivate");
    assert!(out.iter().all(|o| o.outcome == Status::Done));
    assert_eq!(fx.active(), vec![]);
}

#[test]
fn deactivate_except_a_set_keeps_the_set_and_drops_the_rest() {
    let fx = Fixture::new();
    let (regular, bold, inter) = (fx.regular(), fx.bold(), fx.inter());
    let set = fx
        .index
        .lock()
        .expect("index")
        .create_set(&CreateSetRequest {
            name: "Client".into(),
            faces: vec![bold],
        })
        .expect("set")
        .id;
    fx.activate(vec![regular, bold, inter], ActivationMode::Session);
    let out = fx
        .service
        .deactivate(DeactivateRequest {
            target: None,
            except: Some(Target::Set { set }),
        })
        .expect("deactivate");
    let mut gone: Vec<FaceId> = out.iter().map(|o| o.face).collect();
    gone.sort();
    let mut want = vec![regular, inter];
    want.sort();
    assert_eq!(gone, want);
    assert!(out.iter().all(|o| o.outcome == Status::Done));
    let active: Vec<FaceId> = fx.active().into_iter().map(|(face, _)| face).collect();
    assert_eq!(active, vec![bold]);
    let err = fx
        .service
        .deactivate(DeactivateRequest {
            target: None,
            except: Some(Target::Set { set: SetId(9_999) }),
        })
        .expect_err("unknown set");
    assert_eq!(err.code, ErrorCode::NotFound);
    assert_eq!(fx.active().len(), 1, "an unknown set changes nothing");
}

#[test]
fn unknown_targets_and_smart_collections_are_refused_without_changes() {
    let fx = Fixture::new();
    let smart = {
        let mut index = fx.index.lock().expect("index");
        index
            .create_collection(&CreateCollectionRequest {
                rule_group: None,
                name: "Smart".into(),
                parent: None,
                rules: Some(fm_api::search::Filters::default()),
            })
            .expect("create")
            .id
    };
    for (target, code) in [
        (
            Target::Faces {
                faces: vec![fx.regular(), FaceId(9_999)],
            },
            ErrorCode::NotFound,
        ),
        (
            Target::Family {
                family: FamilyId(9_999),
            },
            ErrorCode::NotFound,
        ),
        (
            Target::Collection {
                collection: CollectionId(9_999),
            },
            ErrorCode::NotFound,
        ),
        (Target::Set { set: SetId(9_999) }, ErrorCode::NotFound),
        (
            Target::Collection { collection: smart },
            ErrorCode::Unsupported,
        ),
    ] {
        let err = fx
            .service
            .activate(ActivateRequest {
                target: target.clone(),
                mode: ActivationMode::Session,
                on_conflict: ConflictPolicy::KeepExisting,
            })
            .expect_err("refused");
        assert_eq!(err.code, code, "{target:?}");
    }
    assert_eq!(fx.active(), vec![]);
    assert_eq!(fx.registered(), vec![]);
    assert_eq!(fx.events(), vec![]);
}

#[test]
fn state_is_persisted_in_the_index_file() {
    let dir = TempDir::new("activation-persist");
    let path = dir.path().join("Données").join(INDEX_FILE);
    let os = Arc::new(MockActivator::new());
    let (regular, inter) = {
        let mut opened = Index::open(&path).expect("open").index;
        seed(&mut opened, &library_files());
        let fx = Fixture::over(Arc::new(Mutex::new(opened)), Arc::clone(&os));
        let (regular, inter) = (fx.regular(), fx.inter());
        fx.activate(vec![regular], ActivationMode::Permanent);
        fx.activate(vec![inter], ActivationMode::Session);
        (regular, inter)
    };

    let reopened = Index::open(&path).expect("reopen").index;
    let fx = Fixture::over(Arc::new(Mutex::new(reopened)), os);
    assert_eq!(
        fx.active(),
        vec![
            (regular, ActivationMode::Permanent),
            (inter, ActivationMode::Session)
        ]
    );
    // The stored registration is the one the OS gave: deactivating works.
    let out = fx.deactivate(vec![regular, inter]);
    assert_eq!(statuses(&out), vec![Status::Done, Status::Done]);
    assert_eq!(fx.registered(), vec![]);
}

#[test]
fn faces_the_app_did_not_register_are_skipped_and_foreign_fonts_untouched() {
    let fx = Fixture::new();
    let (regular, bold) = (fx.regular(), fx.bold());
    // A font the user installed, with the same file as a library face.
    fx.os
        .install("Roboto-Bold", "/fonts/Roboto-Bold.ttf", ConflictKind::User);
    fx.activate(vec![regular], ActivationMode::Session);
    fx.events();

    let out = fx.deactivate(vec![bold, regular]);
    assert_eq!(skipped_reason(&out[0].outcome), NOT_OURS);
    assert_eq!(out[1].outcome, Status::Done);
    assert_eq!(fx.events(), vec![changed(vec![], vec![regular])]);

    // "Deactivate all" acts on the activation table only.
    fx.activate(vec![regular], ActivationMode::Permanent);
    let out = fx
        .service
        .deactivate(DeactivateRequest::default())
        .expect("all");
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].face, regular);
    assert_eq!(fx.registered(), vec![]);
}

#[test]
fn a_registration_the_activator_does_not_know_is_not_touched_and_no_longer_claimed() {
    let fx = Fixture::new();
    let regular = fx.regular();
    fx.activate(vec![regular], ActivationMode::Permanent);
    // Another run of the app with an activator that never issued this handle.
    let other = Fixture::over(Arc::clone(&fx.index), Arc::new(MockActivator::new()));
    let out = other.deactivate(vec![regular]);
    assert!(matches!(out[0].outcome, Status::Skipped { .. }));
    assert_eq!(other.active(), vec![]);
    assert_eq!(fx.registered().len(), 1, "the first activator still has it");
}

#[test]
fn faces_of_one_collection_file_share_the_file_registration() {
    let fx = Fixture::new();
    let light = fx.face("Pair-Light", "/fonts/Pair.ttc");
    let heavy = fx.face("Pair-Heavy", "/fonts/Pair.ttc");
    let ttc = PathBuf::from("/fonts/Pair.ttc");

    let out = fx.activate(vec![light, heavy], ActivationMode::Session);
    assert_eq!(statuses(&out), vec![Status::Done, Status::Done]);
    assert_eq!(fx.registered(), vec![(ttc.clone(), Mode::Session)]);

    // The file stays registered while one of its faces is active.
    assert_eq!(statuses(&fx.deactivate(vec![light])), vec![Status::Done]);
    assert_eq!(fx.active(), vec![(heavy, ActivationMode::Session)]);
    assert_eq!(fx.registered(), vec![(ttc, Mode::Session)]);

    assert_eq!(statuses(&fx.deactivate(vec![heavy])), vec![Status::Done]);
    assert_eq!(fx.registered(), vec![]);

    // A face of a file active in session mode cannot be made permanent alone.
    fx.activate(vec![light], ActivationMode::Session);
    let out = fx.activate(vec![heavy], ActivationMode::Permanent);
    assert!(matches!(out[0].outcome, Status::Skipped { .. }));
    let out = fx.activate(vec![light], ActivationMode::Permanent);
    assert_eq!(
        statuses(&out),
        vec![Status::Done],
        "the only active face switches"
    );
    assert_eq!(
        fx.registered(),
        vec![(PathBuf::from("/fonts/Pair.ttc"), Mode::Permanent)]
    );
}

#[test]
fn an_active_face_switches_mode() {
    let fx = Fixture::new();
    let regular = fx.regular();
    fx.activate(vec![regular], ActivationMode::Session);
    fx.events();
    let out = fx.activate(vec![regular], ActivationMode::Permanent);
    assert_eq!(statuses(&out), vec![Status::Done]);
    assert_eq!(fx.active(), vec![(regular, ActivationMode::Permanent)]);
    assert_eq!(
        fx.registered(),
        vec![(PathBuf::from("/fonts/Roboto-Regular.ttf"), Mode::Permanent)]
    );
    assert_eq!(fx.events(), vec![changed(vec![regular], vec![])]);
}

#[test]
fn faces_that_cannot_be_activated_are_skipped_with_a_reason() {
    let fx = Fixture::new();
    let web = fx.face("Web-Regular", "/fonts/Web.woff2");
    let broken = fx.face("Broken-Regular", "/fonts/Broken.ttf");
    let inter = fx.inter();
    fx.index
        .lock()
        .expect("index")
        .conn()
        .execute(
            "UPDATE font_file SET gone = 1 WHERE path = '/fonts/Inter.otf'",
            [],
        )
        .expect("gone");
    let out = fx.activate(vec![web, broken, inter], ActivationMode::Session);
    assert!(skipped_reason(&out[0].outcome).contains("WOFF"));
    assert_eq!(
        skipped_reason(&out[1].outcome),
        "The glyph table is truncated."
    );
    assert_eq!(skipped_reason(&out[2].outcome), "The font file is missing.");
    assert_eq!(fx.active(), vec![]);
    assert_eq!(fx.registered(), vec![]);
    assert_eq!(fx.events(), vec![]);
}

#[test]
fn an_os_failure_fails_one_face_and_not_the_others() {
    let fx = Fixture::new();
    let (regular, bold) = (fx.regular(), fx.bold());
    fx.os.fail_on(
        "/fonts/Roboto-Bold.ttf",
        fm_platform::ActivationErrorKind::PermissionDenied,
    );
    let out = fx.activate(vec![regular, bold], ActivationMode::Session);
    assert_eq!(out[0].outcome, Status::Done);
    match &out[1].outcome {
        Status::Failed { error } => assert_eq!(error.code, ErrorCode::PermissionDenied),
        other => panic!("expected Failed, got {other:?}"),
    }
    assert_eq!(fx.active(), vec![(regular, ActivationMode::Session)]);
    assert_eq!(fx.events(), vec![changed(vec![regular], vec![])]);
}

#[test]
fn an_unavailable_activator_is_an_error_and_changes_nothing() {
    let fx = Fixture::new();
    let regular = fx.regular();
    fx.os.set_unavailable(true);
    let err = fx
        .service
        .activate(ActivateRequest {
            target: Target::Faces {
                faces: vec![regular],
            },
            mode: ActivationMode::Session,
            on_conflict: ConflictPolicy::Replace,
        })
        .expect_err("unavailable");
    assert_eq!(err.code, ErrorCode::Unsupported);
    assert_eq!(fx.active(), vec![]);
    assert_eq!(fx.events(), vec![]);
}

#[test]
fn a_clash_keeps_the_face_inactive_unless_replace_is_asked() {
    let fx = Fixture::new();
    let inter = fx.inter();
    fx.os.install(
        "Inter-Regular",
        "/System/Library/Fonts/Inter.otf",
        ConflictKind::System,
    );
    let target = Target::Faces { faces: vec![inter] };
    let conflicts = fx.service.conflicts(target.clone()).expect("conflicts");
    assert_eq!(
        conflicts,
        vec![Conflict {
            face: inter,
            postscript_name: "Inter-Regular".into(),
            with: ConflictWith::System {
                path: Path::new("/System/Library/Fonts/Inter.otf")
                    .display()
                    .to_string()
            },
        }]
    );
    let out = fx.activate(vec![inter], ActivationMode::Session);
    assert!(matches!(
        out[0].outcome,
        Status::Conflict {
            with: ConflictWith::System { .. }
        }
    ));
    assert_eq!(fx.active(), vec![]);

    let out = fx
        .service
        .activate(ActivateRequest {
            target,
            mode: ActivationMode::Session,
            on_conflict: ConflictPolicy::Replace,
        })
        .expect("replace");
    assert_eq!(statuses(&out), vec![Status::Done]);
}

#[test]
fn a_clash_with_a_face_the_app_activated_names_that_face() {
    let fx = Fixture::new();
    let regular = fx.regular();
    let copy = fx.face("Roboto-Regular", "/fonts/copy/Roboto-Regular.ttf");
    fx.activate(vec![regular], ActivationMode::Session);
    let out = fx.activate(vec![copy], ActivationMode::Session);
    assert_eq!(
        statuses(&out),
        vec![Status::Conflict {
            with: ConflictWith::Active { face: regular }
        }]
    );
}

fn replace(fx: &Fixture, faces: Vec<FaceId>, mode: ActivationMode) -> Vec<ActivationOutcome> {
    fx.service
        .activate(ActivateRequest {
            target: Target::Faces { faces },
            mode,
            on_conflict: ConflictPolicy::Replace,
        })
        .expect("replace")
}

fn set_since(fx: &Fixture, face: FaceId, since: &str) {
    fx.index
        .lock()
        .expect("index")
        .conn()
        .execute(
            "UPDATE activation SET since = ?1 WHERE face_id = ?2",
            rusqlite::params![since, face.0],
        )
        .expect("since");
}

fn system_inter(fx: &Fixture) -> ConflictWith {
    let path = "/System/Library/Fonts/Inter.otf";
    fx.os.install("Inter-Regular", path, ConflictKind::System);
    ConflictWith::System {
        path: Path::new(path).display().to_string(),
    }
}

#[test]
fn replace_activates_a_clashing_face_for_the_session_only() {
    let fx = Fixture::new();
    let (bold, inter) = (fx.bold(), fx.inter());
    system_inter(&fx);

    // Permanent is asked; the clashing face replaces the system font for
    // this session only, the other face is permanent.
    let out = replace(&fx, vec![inter, bold], ActivationMode::Permanent);
    assert_eq!(statuses(&out), vec![Status::Done, Status::Done]);
    assert_eq!(out[0].face, inter);
    assert_eq!(
        fx.active(),
        vec![
            (bold, ActivationMode::Permanent),
            (inter, ActivationMode::Session)
        ]
    );
    assert_eq!(
        fx.registered(),
        vec![
            (PathBuf::from("/fonts/Inter.otf"), Mode::Session),
            (PathBuf::from("/fonts/Roboto-Bold.ttf"), Mode::Permanent),
        ]
    );
    assert_eq!(
        fx.events(),
        vec![changed(vec![bold], vec![]), changed(vec![inter], vec![])]
    );

    // Asked again, nothing changes; after a restart the system font is back.
    let again = replace(&fx, vec![inter, bold], ActivationMode::Permanent);
    assert_eq!(statuses(&again), vec![Status::Unchanged, Status::Unchanged]);
    fx.service.shutdown().expect("quit");
    let next = fx.restart();
    next.service
        .start(false, ConflictPolicy::KeepExisting)
        .expect("start");
    assert_eq!(next.active(), vec![(bold, ActivationMode::Permanent)]);
}

#[test]
fn the_conflicts_view_lists_every_clash_with_its_winner() {
    let fx = Fixture::new();
    let (regular, inter) = (fx.regular(), fx.inter());
    let copy = fx.face("Roboto-Regular", "/fonts/copy/Roboto-Regular.ttf");
    assert_eq!(fx.service.list_conflicts().expect("list"), vec![]);
    let system = system_inter(&fx);

    replace(&fx, vec![inter], ActivationMode::Session);
    fx.activate(vec![regular], ActivationMode::Session);
    replace(&fx, vec![copy], ActivationMode::Session);
    // Replacing an app face does not deactivate it.
    assert_eq!(fx.active().len(), 3);
    set_since(&fx, regular, "2026-10-19T10:00:00.000Z");
    set_since(&fx, copy, "2026-10-19T10:00:01.000Z");
    let registered = fx.registered();

    let pair = |winner| Clash {
        face: regular,
        postscript_name: "Roboto-Regular".into(),
        with: ConflictWith::Active { face: copy },
        winner,
    };
    let inter_clash = Clash {
        face: inter,
        postscript_name: "Inter-Regular".into(),
        with: system,
        winner: ClashWinner::Face,
    };
    // The pair once, under the lower id; the later activation wins.
    assert_eq!(
        fx.service.list_conflicts().expect("list"),
        vec![pair(ClashWinner::With), inter_clash.clone()]
    );
    set_since(&fx, regular, "2026-10-19T10:00:02.000Z");
    assert_eq!(
        fx.service.list_conflicts().expect("list"),
        vec![pair(ClashWinner::Face), inter_clash.clone()]
    );
    // Reading the view changes nothing.
    assert_eq!(fx.registered(), registered);
    assert_eq!(fx.events().len(), 3);

    fx.deactivate(vec![copy]);
    assert_eq!(
        fx.service.list_conflicts().expect("list"),
        vec![inter_clash]
    );
}

#[test]
fn activation_conflicts_are_problems_entries_with_both_files() {
    let fx = Fixture::new();
    let (regular, inter) = (fx.regular(), fx.inter());
    let copy = fx.face("Roboto-Regular", "/fonts/copy/Roboto-Regular.ttf");
    assert_eq!(fx.service.problems().expect("none"), vec![]);
    system_inter(&fx);
    replace(&fx, vec![inter], ActivationMode::Session);
    fx.activate(vec![regular], ActivationMode::Session);
    replace(&fx, vec![copy], ActivationMode::Session);
    let registered = fx.registered();

    let shown = |p: &str| Path::new(p).display().to_string();
    let rows: Vec<_> = fx
        .service
        .problems()
        .expect("problems")
        .into_iter()
        .map(|p| (p.kind, p.face, p.path, p.other))
        .collect();
    assert_eq!(
        rows,
        vec![
            (
                ProblemKind::ActiveConflict,
                Some(regular),
                shown("/fonts/Roboto-Regular.ttf"),
                Some(shown("/fonts/copy/Roboto-Regular.ttf")),
            ),
            (
                ProblemKind::SystemConflict,
                Some(inter),
                shown("/fonts/Inter.otf"),
                Some(shown("/System/Library/Fonts/Inter.otf")),
            ),
        ]
    );
    // Reading the entries changes nothing.
    assert_eq!(fx.registered(), registered);
}

#[test]
fn the_conflicts_view_is_an_error_when_the_activator_is_unavailable() {
    let fx = Fixture::new();
    fx.activate(vec![fx.regular()], ActivationMode::Session);
    fx.os.set_unavailable(true);
    let err = fx.service.list_conflicts().expect_err("unavailable");
    assert_eq!(err.code, ErrorCode::Unsupported);
}

/// An activator that reports the app's own fonts by their OS record, as
/// fontconfig does on Linux (it lists the link, not the file it points at).
struct ByHandle(Arc<MockActivator>);

impl FontActivator for ByHandle {
    fn activate(&self, faces: &[FaceFile], mode: Mode) -> Result<Vec<Outcome>, ActivationError> {
        self.0.activate(faces, mode)
    }

    fn deactivate(&self, regs: &[Registration]) -> Result<Vec<Outcome>, ActivationError> {
        self.0.deactivate(regs)
    }

    fn list_active(&self) -> Result<Vec<Registration>, ActivationError> {
        self.0.list_active()
    }

    fn conflicts(&self, faces: &[FaceFile]) -> Result<Vec<fm_platform::Conflict>, ActivationError> {
        let active = self.0.list_active()?;
        let mut found = self.0.conflicts(faces)?;
        for c in &mut found {
            if let Some(reg) = active.iter().find(|r| r.path == c.with) {
                c.with = PathBuf::from(&reg.handle);
            }
        }
        Ok(found)
    }
}

#[test]
fn an_app_font_the_os_reports_by_its_link_is_still_the_apps_face() {
    let fx = Fixture::new();
    let (regular, copy) = (
        fx.regular(),
        fx.face("Roboto-Regular", "/fonts/copy/Roboto-Regular.ttf"),
    );
    let activator: Arc<dyn FontActivator> = Arc::new(ByHandle(Arc::clone(&fx.os)));
    let service = ActivationManager::new(Arc::clone(&fx.index), activator, |_| {});
    let activate = |face, on_conflict| {
        service
            .activate(ActivateRequest {
                target: Target::Faces { faces: vec![face] },
                mode: ActivationMode::Session,
                on_conflict,
            })
            .expect("activate")
    };
    activate(regular, ConflictPolicy::KeepExisting);
    // The clash names the app's face, not a "system font" at the link.
    assert_eq!(
        statuses(&activate(copy, ConflictPolicy::KeepExisting)),
        vec![Status::Conflict {
            with: ConflictWith::Active { face: regular }
        }]
    );
    activate(copy, ConflictPolicy::Replace);
    set_since(&fx, regular, "2026-10-19T10:00:00.000Z");
    set_since(&fx, copy, "2026-10-19T10:00:01.000Z");
    // One row for the pair, with one winner.
    assert_eq!(
        service.list_conflicts().expect("list"),
        vec![Clash {
            face: regular,
            postscript_name: "Roboto-Regular".into(),
            with: ConflictWith::Active { face: copy },
            winner: ClashWinner::With,
        }]
    );
}

#[test]
fn replacing_a_font_never_deregisters_it() {
    let fx = Fixture::new();
    let inter = fx.inter();
    // A font the user installed, not the app.
    fx.os.install(
        "Inter-Regular",
        "/Users/ada/Library/Fonts/Inter.otf",
        ConflictKind::User,
    );
    replace(&fx, vec![inter], ActivationMode::Permanent);
    fx.service
        .deactivate(DeactivateRequest::default())
        .expect("all");
    fx.service.shutdown().expect("quit");

    // The user's font is still there: activating again still clashes with it.
    let out = fx.activate(vec![inter], ActivationMode::Session);
    assert_eq!(
        statuses(&out),
        vec![Status::Conflict {
            with: ConflictWith::System {
                path: Path::new("/Users/ada/Library/Fonts/Inter.otf")
                    .display()
                    .to_string()
            }
        }]
    );
    assert_eq!(fx.registered(), vec![]);
}

#[test]
fn quit_and_start_restore_the_last_session_when_asked() {
    let fx = Fixture::new();
    let (regular, bold, inter) = (fx.regular(), fx.bold(), fx.inter());
    fx.activate(vec![regular, bold], ActivationMode::Session);
    fx.activate(vec![inter], ActivationMode::Permanent);
    fx.events();

    let out = fx.service.shutdown().expect("quit");
    assert_eq!(statuses(&out), vec![Status::Done, Status::Done]);
    assert_eq!(fx.active(), vec![(inter, ActivationMode::Permanent)]);
    assert_eq!(
        fx.registered(),
        vec![(PathBuf::from("/fonts/Inter.otf"), Mode::Permanent)],
        "quit deregisters session fonts only"
    );
    assert_eq!(fx.events(), vec![changed(vec![], vec![regular, bold])]);

    let next = fx.restart();
    let out = next
        .service
        .start(true, ConflictPolicy::KeepExisting)
        .expect("start");
    assert_eq!(statuses(&out), vec![Status::Done, Status::Done]);
    assert_eq!(
        next.active(),
        vec![
            (regular, ActivationMode::Session),
            (bold, ActivationMode::Session),
            (inter, ActivationMode::Permanent)
        ]
    );
    assert_eq!(next.events(), vec![changed(vec![regular, bold], vec![])]);
}

#[test]
fn start_without_reactivate_on_launch_restores_nothing() {
    let fx = Fixture::new();
    let regular = fx.regular();
    fx.activate(vec![regular], ActivationMode::Session);
    fx.service.shutdown().expect("quit");

    let next = fx.restart();
    let out = next
        .service
        .start(false, ConflictPolicy::KeepExisting)
        .expect("start");
    assert_eq!(out, vec![]);
    assert_eq!(next.active(), vec![]);
    assert_eq!(next.registered(), vec![]);

    // The last session is still remembered for a later start that asks.
    let later = next.restart();
    let out = later
        .service
        .start(true, ConflictPolicy::KeepExisting)
        .expect("start");
    assert_eq!(statuses(&out), vec![Status::Done]);
}

#[test]
fn start_after_a_crash_cleans_up_and_restores_once() {
    let fx = Fixture::new();
    let (regular, inter) = (fx.regular(), fx.inter());
    fx.activate(vec![regular], ActivationMode::Session);
    fx.activate(vec![inter], ActivationMode::Permanent);
    // No shutdown: the app crashed. The OS still holds the session font.

    let next = fx.restart();
    next.service
        .start(false, ConflictPolicy::KeepExisting)
        .expect("start");
    assert_eq!(next.active(), vec![(inter, ActivationMode::Permanent)]);
    assert_eq!(
        next.registered(),
        vec![(PathBuf::from("/fonts/Inter.otf"), Mode::Permanent)],
        "the crashed session's registration is gone"
    );

    let later = next.restart();
    let out = later
        .service
        .start(true, ConflictPolicy::KeepExisting)
        .expect("start");
    assert_eq!(statuses(&out), vec![Status::Done]);
    assert_eq!(
        later.registered(),
        vec![
            (PathBuf::from("/fonts/Inter.otf"), Mode::Permanent),
            (PathBuf::from("/fonts/Roboto-Regular.ttf"), Mode::Session),
        ]
    );
}

#[test]
fn a_session_emptied_before_quit_restores_nothing() {
    let fx = Fixture::new();
    let regular = fx.regular();
    fx.activate(vec![regular], ActivationMode::Session);
    fx.service.shutdown().expect("quit");
    let next = fx.restart();
    next.service
        .start(true, ConflictPolicy::KeepExisting)
        .expect("start");
    next.deactivate(vec![regular]);
    next.service.shutdown().expect("quit");

    let later = next.restart();
    let out = later
        .service
        .start(true, ConflictPolicy::KeepExisting)
        .expect("start");
    assert_eq!(out, vec![]);
    assert_eq!(later.active(), vec![]);
}

#[test]
fn a_hundred_faces_activate_within_the_budget() {
    // The service's own cost over the mock activator; the bench job times
    // the OS activators against the reference library (`fm-bench activate`).
    let files: Vec<IndexedFile> = (0..100)
        .map(|i| {
            let path = format!("/fonts/bench/Face{i:03}.ttf");
            let (ps, style) = (format!("Bench-Face{i:03}"), format!("Style{i:03}"));
            file(
                &path,
                FontFormat::Ttf,
                &[(&ps, "Bench", &style)],
                Health::Ok,
            )
        })
        .collect();
    let mut index = Index::open_in_memory().expect("open");
    seed(&mut index, &files);
    let fx = Fixture::over(Arc::new(Mutex::new(index)), Arc::new(MockActivator::new()));
    let faces: Vec<FaceId> = {
        let index = fx.index.lock().expect("index");
        let mut stmt = index
            .conn()
            .prepare("SELECT id FROM face")
            .expect("prepare");
        stmt.query_map([], |r| r.get(0).map(FaceId))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("ids")
    };
    assert_eq!(faces.len(), 100);
    let begin = Instant::now();
    let out = fx.activate(faces, ActivationMode::Session);
    let took = begin.elapsed();
    assert!(out.iter().all(|o| o.outcome == Status::Done));
    assert_eq!(fx.active().len(), 100);
    assert!(
        took.as_millis() < 500,
        "100 faces took {} ms",
        took.as_millis()
    );
    assert_eq!(fx.events().len(), 1, "one event for the whole request");
}

#[test]
fn a_virtual_style_activates_its_face_once_with_the_collection_or_set() {
    let fx = Fixture::new();
    let (regular, bold) = (fx.regular(), fx.bold());
    let (collection, set) = {
        let mut index = fx.index.lock().expect("index");
        let collection = index
            .create_collection(&CreateCollectionRequest {
                rule_group: None,
                name: "Brand".into(),
                parent: None,
                rules: None,
            })
            .expect("collection")
            .id;
        index
            .add_to_collection(&CollectionFacesRequest {
                collection,
                faces: vec![regular],
                families: vec![],
            })
            .expect("add");
        let set = index
            .create_set(&CreateSetRequest {
                name: "Client".into(),
                faces: vec![],
            })
            .expect("set")
            .id;
        // Two instances of the member face and one of a face that is not a
        // member, in the collection; two instances of one face in the set.
        // The service reads the rows; how a pin is checked is tested in
        // `index/virtual_styles`.
        let conn = index.conn();
        for (face, name, collection, set) in [
            (regular, "Wide", Some(collection.0), None),
            (regular, "Narrow", Some(collection.0), None),
            (bold, "Heavy", Some(collection.0), None),
            (bold, "Heavy", None, Some(set.0)),
            (bold, "Light", None, Some(set.0)),
        ] {
            conn.execute(
                "INSERT INTO virtual_style (face_id, name, coordinates, collection_id, set_id)
                 VALUES (?1, ?2, '[]', ?3, ?4)",
                rusqlite::params![face.0, name, collection, set],
            )
            .expect("style");
        }
        (collection, set)
    };

    for (target, want) in [
        (Target::Collection { collection }, vec![regular, bold]),
        (Target::Set { set }, vec![bold]),
    ] {
        let out = fx
            .service
            .activate(ActivateRequest {
                target: target.clone(),
                mode: ActivationMode::Session,
                on_conflict: ConflictPolicy::KeepExisting,
            })
            .expect("activate");
        let mut faces: Vec<FaceId> = out.iter().map(|o| o.face).collect();
        faces.sort();
        let mut want = want;
        want.sort();
        assert_eq!(faces, want, "each face once");
        assert!(out.iter().all(|o| o.outcome == Status::Done), "{out:?}");
        // The OS holds the file once per face, not once per instance.
        assert_eq!(fx.registered().len(), want.len());
        fx.service
            .deactivate(DeactivateRequest {
                target: Some(target),
                except: None,
            })
            .expect("deactivate");
        assert_eq!(fx.active(), vec![]);
        assert_eq!(fx.registered(), vec![]);
    }
}

/// E29.3: `fonts_needed` for `document` by PostScript name.
fn need(fx: &Fixture, document: &str, names: &[&str]) -> Vec<ActivationOutcome> {
    fx.service
        .fonts_needed(FontsNeededRequest {
            fonts: names
                .iter()
                .map(|n| fm_api::activation::NeededFont {
                    postscript_name: Some((*n).to_owned()),
                    ..Default::default()
                })
                .collect(),
            document: Some(document.to_owned()),
            activate: true,
        })
        .expect("fonts needed")
        .outcomes
}

fn closed(fx: &Fixture, document: &str) -> Vec<FaceId> {
    fx.service
        .document_closed(DocumentClosedRequest {
            document: document.to_owned(),
        })
        .expect("document closed")
        .offer
}

/// Each face with its mode and the document it shows.
fn documents(fx: &Fixture) -> Vec<(FaceId, ActivationMode, Option<String>)> {
    fx.service
        .list_active()
        .expect("list")
        .into_iter()
        .map(|a| (a.face, a.mode, a.document))
        .collect()
}

/// The face-to-document table, by face then document.
fn tags(fx: &Fixture) -> Vec<(FaceId, String)> {
    let index = fx.index.lock().expect("index");
    let mut stmt = index
        .conn()
        .prepare("SELECT face_id, document FROM activation_document ORDER BY face_id, document")
        .expect("prepare");
    stmt.query_map([], |r| Ok((FaceId(r.get(0)?), r.get(1)?)))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows")
}

/// E29.3 criterion 1: a document's faces are session faces tagged with it;
/// a face belongs to every open document that asks for it, shows the one
/// that asked last, and a repeated request adds no tag.
#[test]
fn a_document_tags_its_session_faces_once_each() {
    let fx = Fixture::new();
    let (regular, bold) = (fx.regular(), fx.bold());
    let out = need(&fx, "poster.indd", &["Roboto-Regular", "Roboto-Bold"]);
    assert_eq!(statuses(&out), vec![Status::Done, Status::Done]);
    let poster = Some("poster.indd".to_owned());
    assert_eq!(
        documents(&fx),
        vec![
            (regular, ActivationMode::Session, poster.clone()),
            (bold, ActivationMode::Session, poster.clone()),
        ]
    );

    let again = need(&fx, "poster.indd", &["Roboto-Regular", "Roboto-Bold"]);
    assert_eq!(statuses(&again), vec![Status::Unchanged, Status::Unchanged]);
    need(&fx, "flyer.indd", &["Roboto-Bold"]);
    assert_eq!(
        tags(&fx),
        vec![
            (regular, "poster.indd".to_owned()),
            (bold, "flyer.indd".to_owned()),
            (bold, "poster.indd".to_owned()),
        ],
        "no duplicate tag"
    );
    assert_eq!(
        documents(&fx),
        vec![
            (regular, ActivationMode::Session, poster.clone()),
            (bold, ActivationMode::Session, Some("flyer.indd".to_owned())),
        ],
        "the document that asked last"
    );
    // The poster asks again: it is the last again, still one tag.
    need(&fx, "poster.indd", &["Roboto-Bold"]);
    assert_eq!(documents(&fx)[1].2, poster);
    assert_eq!(tags(&fx).len(), 3);
    assert_eq!(
        fx.registered(),
        vec![
            (PathBuf::from("/fonts/Roboto-Bold.ttf"), Mode::Session),
            (PathBuf::from("/fonts/Roboto-Regular.ttf"), Mode::Session),
        ]
    );
}

/// E29.3 criterion 2: closing a document drops its tags and offers exactly
/// the faces no other open document needs; it deactivates nothing.
#[test]
fn closing_a_document_offers_the_faces_no_open_document_needs() {
    let fx = Fixture::new();
    let (regular, bold, inter) = (fx.regular(), fx.bold(), fx.inter());
    need(&fx, "poster.indd", &["Roboto-Regular", "Roboto-Bold"]);
    need(&fx, "flyer.indd", &["Roboto-Bold", "Inter-Regular"]);
    let registered = fx.registered();
    fx.events();

    assert_eq!(closed(&fx, "poster.indd"), vec![regular]);
    assert_eq!(fx.registered(), registered, "nothing is deregistered");
    assert_eq!(fx.events(), vec![], "nothing changed");
    assert_eq!(
        tags(&fx),
        vec![
            (bold, "flyer.indd".to_owned()),
            (inter, "flyer.indd".to_owned()),
        ]
    );
    assert_eq!(
        documents(&fx),
        vec![
            (regular, ActivationMode::Session, Some("poster.indd".into())),
            (bold, ActivationMode::Session, Some("flyer.indd".into())),
            (inter, ActivationMode::Session, Some("flyer.indd".into())),
        ],
        "a face with an open document shows it"
    );
    // Closed again, or never open: nothing to offer.
    assert_eq!(closed(&fx, "poster.indd"), vec![]);
    assert_eq!(closed(&fx, "unknown.indd"), vec![]);

    assert_eq!(closed(&fx, "flyer.indd"), vec![bold, inter]);
    assert_eq!(tags(&fx), vec![]);
    assert_eq!(fx.registered(), registered);
}

/// E29.3 criterion 3: a face the user activated, before or after the
/// document asked, and a permanent face are never offered; deactivating the
/// offer deregisters only those faces, with one event. Two documents share
/// a face.
#[test]
fn the_users_and_permanent_faces_are_never_offered() {
    let fx = Fixture::new();
    let (regular, bold, inter) = (fx.regular(), fx.bold(), fx.inter());
    let light = fx.face("Pair-Light", "/fonts/Pair.ttc");
    // Before the documents ask: Inter for the session, Pair-Light for good.
    fx.activate(vec![inter], ActivationMode::Session);
    fx.activate(vec![light], ActivationMode::Permanent);
    let all = [
        "Roboto-Regular",
        "Roboto-Bold",
        "Inter-Regular",
        "Pair-Light",
    ];
    let out = need(&fx, "poster.indd", &all);
    assert_eq!(
        statuses(&out),
        vec![
            Status::Done,
            Status::Done,
            Status::Unchanged,
            Status::Unchanged
        ]
    );
    need(&fx, "flyer.indd", &["Roboto-Regular", "Roboto-Bold"]);
    // After they asked: the user activates Bold (already active) directly.
    let out = fx.activate(vec![bold], ActivationMode::Session);
    assert_eq!(statuses(&out), vec![Status::Unchanged]);
    assert_eq!(
        documents(&fx),
        vec![
            (regular, ActivationMode::Session, Some("flyer.indd".into())),
            (bold, ActivationMode::Session, None),
            (inter, ActivationMode::Session, None),
            (light, ActivationMode::Permanent, None),
        ],
        "only the document-scoped face shows a document"
    );

    assert_eq!(
        closed(&fx, "poster.indd"),
        vec![],
        "the flyer needs Regular"
    );
    let offer = closed(&fx, "flyer.indd");
    assert_eq!(offer, vec![regular]);
    fx.events();

    let out = fx.deactivate(offer);
    assert_eq!(statuses(&out), vec![Status::Done]);
    assert_eq!(
        fx.events(),
        vec![changed(vec![], vec![regular])],
        "one event for the offered face"
    );
    assert_eq!(
        fx.active(),
        vec![
            (bold, ActivationMode::Session),
            (inter, ActivationMode::Session),
            (light, ActivationMode::Permanent),
        ]
    );
    assert_eq!(
        fx.registered(),
        vec![
            (PathBuf::from("/fonts/Inter.otf"), Mode::Session),
            (PathBuf::from("/fonts/Pair.ttc"), Mode::Permanent),
            (PathBuf::from("/fonts/Roboto-Bold.ttf"), Mode::Session),
        ]
    );

    // A document face the user makes permanent is never offered either.
    need(&fx, "menu.indd", &["Roboto-Regular"]);
    fx.activate(vec![regular], ActivationMode::Permanent);
    assert_eq!(closed(&fx, "menu.indd"), vec![]);
    assert_eq!(tags(&fx), vec![]);
}

/// E29.3 criterion 4: quitting deregisters document-scoped faces like any
/// session face, and neither "reactivate on launch" nor the resume after a
/// quit brings them back; the user's session faces come back.
#[test]
fn document_faces_end_at_quit_and_are_not_restored() {
    let fx = Fixture::new();
    let (regular, bold, inter) = (fx.regular(), fx.bold(), fx.inter());
    fx.activate(vec![regular], ActivationMode::Session);
    need(&fx, "poster.indd", &["Roboto-Bold", "Inter-Regular"]);
    // The poster closed and the offer was declined: Inter stays scoped.
    need(&fx, "flyer.indd", &["Roboto-Bold"]);
    assert_eq!(closed(&fx, "poster.indd"), vec![inter]);
    fx.events();

    // The update path: what it will resume, then the quit.
    let resumable = crate::lifecycle::resumable_faces(&fx.service.list_active().expect("list"));
    assert_eq!(resumable, vec![regular]);
    let out = fx.service.shutdown().expect("quit");
    assert_eq!(statuses(&out), vec![Status::Done; 3]);
    assert_eq!(
        fx.registered(),
        vec![],
        "every session face is deregistered"
    );
    assert_eq!(
        fx.events(),
        vec![changed(vec![], vec![regular, bold, inter])]
    );
    assert_eq!(tags(&fx), vec![]);

    // "Reactivate on launch" straight after that quit: only the user's.
    let next = fx.restart();
    let out = next
        .service
        .start(true, ConflictPolicy::KeepExisting)
        .expect("start");
    assert_eq!(statuses(&out), vec![Status::Done]);
    assert_eq!(next.active(), vec![(regular, ActivationMode::Session)]);
    assert_eq!(
        next.registered(),
        vec![(PathBuf::from("/fonts/Roboto-Regular.ttf"), Mode::Session)]
    );

    // The resume after a quit (an update that did not install) on the same
    // run: the document asks again, the quit ends it, the resume skips it.
    need(&next, "poster.indd", &["Roboto-Bold"]);
    assert_eq!(resumable, vec![regular]);
    let again = crate::lifecycle::resumable_faces(&next.service.list_active().expect("list"));
    assert_eq!(again, vec![regular]);
    next.service.shutdown().expect("quit");
    let out = crate::lifecycle::restore_session(&next.service, again, ConflictPolicy::KeepExisting)
        .expect("resume");
    assert_eq!(statuses(&out), vec![Status::Done]);
    assert_eq!(next.active(), vec![(regular, ActivationMode::Session)]);
    assert!(!next.active().iter().any(|(f, _)| *f == bold));
}

/// The last session a quit records holds the user's session faces only.
#[test]
fn a_quit_remembers_only_the_users_session_faces() {
    let fx = Fixture::new();
    let regular = fx.regular();
    fx.activate(vec![regular], ActivationMode::Session);
    need(&fx, "poster.indd", &["Roboto-Bold", "Inter-Regular"]);
    fx.service.shutdown().expect("quit");
    let last = fx
        .index
        .lock()
        .expect("index")
        .last_session()
        .expect("last");
    assert_eq!(last, vec![regular]);
}

/// E29.3 criterion 4 after a crash: the next start cleans up the document
/// faces too and restores only the user's.
#[test]
fn a_crash_does_not_bring_document_faces_back() {
    let fx = Fixture::new();
    let regular = fx.regular();
    fx.activate(vec![regular], ActivationMode::Session);
    need(&fx, "poster.indd", &["Roboto-Bold"]);
    assert_eq!(fx.active().len(), 2);
    // No shutdown: the app crashed.
    let next = fx.restart();
    next.service
        .start(true, ConflictPolicy::KeepExisting)
        .expect("start");
    assert_eq!(next.active(), vec![(regular, ActivationMode::Session)]);
    assert_eq!(
        next.registered(),
        vec![(PathBuf::from("/fonts/Roboto-Regular.ttf"), Mode::Session)]
    );
    assert_eq!(tags(&next), vec![]);
}
