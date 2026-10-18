//! An in-memory [`FontActivator`] for tests. It touches no file and no OS
//! setting, so the core and adapter tests run the same on every host.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use super::{
    ActivationError, ActivationErrorKind, Conflict, ConflictKind, FaceFile, FontActivator, Mode,
    Outcome, OutcomeStatus, Registration,
};

/// A [`FontActivator`] that keeps its registrations in memory.
///
/// It behaves like a real activator: one registration per file, rule 3 on
/// deactivate, conflicts by PostScript name. Tests script the rest: fonts the
/// "OS" already has ([`MockActivator::install`]), per-file failures
/// ([`MockActivator::fail_on`]), a whole-call failure
/// ([`MockActivator::set_unavailable`]) and a registration removed behind the
/// app's back ([`MockActivator::remove_externally`]).
#[derive(Debug, Default)]
pub struct MockActivator {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    /// Active registrations with the PostScript name they were made for.
    active: Vec<(Registration, String)>,
    /// Every handle this activator has issued.
    issued: HashSet<OsString>,
    /// Fonts the "OS" has that this activator did not register.
    installed: Vec<(String, PathBuf, ConflictKind)>,
    /// Files whose activation or deactivation fails.
    failures: HashMap<PathBuf, ActivationErrorKind>,
    unavailable: bool,
    next_handle: u64,
}

impl MockActivator {
    /// An activator with nothing registered and nothing installed.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a font the OS already has, for [`FontActivator::conflicts`].
    pub fn install(&self, postscript_name: &str, path: impl Into<PathBuf>, kind: ConflictKind) {
        self.state()
            .installed
            .push((postscript_name.to_owned(), path.into(), kind));
    }

    /// Makes every later activation or deactivation of `path` fail with `kind`.
    pub fn fail_on(&self, path: impl Into<PathBuf>, kind: ActivationErrorKind) {
        self.state().failures.insert(path.into(), kind);
    }

    /// When `true`, every call returns `Err` (the OS service is not there).
    pub fn set_unavailable(&self, unavailable: bool) {
        self.state().unavailable = unavailable;
    }

    /// Drops the registration of `path` as if the user or the OS removed it.
    pub fn remove_externally(&self, path: &Path) {
        self.state().active.retain(|(r, _)| r.path != path);
    }

    fn state(&self) -> MutexGuard<'_, State> {
        // A panic in another test thread must not hide this activator's state.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl State {
    fn check_available(&self) -> Result<(), ActivationError> {
        if self.unavailable {
            Err(ActivationError::new(
                ActivationErrorKind::Unsupported,
                "mock activator set unavailable",
            ))
        } else {
            Ok(())
        }
    }

    fn failure(&self, path: &Path) -> Option<ActivationError> {
        self.failures.get(path).map(|&kind| {
            ActivationError::new(kind, format!("scripted failure for {}", path.display()))
        })
    }

    fn activate_one(&mut self, face: &FaceFile, mode: Mode) -> OutcomeStatus {
        if let Some((existing, _)) = self.active.iter().find(|(r, _)| r.path == face.path) {
            return OutcomeStatus::AlreadyActive(existing.clone());
        }
        if let Some(err) = self.failure(&face.path) {
            return OutcomeStatus::Failed(err);
        }
        self.next_handle += 1;
        let handle = OsString::from(format!("mock-{}", self.next_handle));
        let registration = Registration {
            path: face.path.clone(),
            mode,
            handle: handle.clone(),
        };
        self.issued.insert(handle);
        self.active
            .push((registration.clone(), face.postscript_name.clone()));
        OutcomeStatus::Activated(registration)
    }

    fn deactivate_one(&mut self, registration: &Registration) -> OutcomeStatus {
        if !self.issued.contains(&registration.handle) {
            return OutcomeStatus::NotOurs;
        }
        let Some(at) = self
            .active
            .iter()
            .position(|(r, _)| r.handle == registration.handle)
        else {
            return OutcomeStatus::NotActive;
        };
        if let Some(err) = self.failure(&registration.path) {
            return OutcomeStatus::Failed(err);
        }
        self.active.remove(at);
        OutcomeStatus::Deactivated
    }
}

impl FontActivator for MockActivator {
    fn activate(&self, faces: &[FaceFile], mode: Mode) -> Result<Vec<Outcome>, ActivationError> {
        let mut state = self.state();
        state.check_available()?;
        Ok(faces
            .iter()
            .map(|face| Outcome {
                path: face.path.clone(),
                status: state.activate_one(face, mode),
            })
            .collect())
    }

    fn deactivate(&self, registrations: &[Registration]) -> Result<Vec<Outcome>, ActivationError> {
        let mut state = self.state();
        state.check_available()?;
        Ok(registrations
            .iter()
            .map(|registration| Outcome {
                path: registration.path.clone(),
                status: state.deactivate_one(registration),
            })
            .collect())
    }

    fn list_active(&self) -> Result<Vec<Registration>, ActivationError> {
        let state = self.state();
        state.check_available()?;
        Ok(state.active.iter().map(|(r, _)| r.clone()).collect())
    }

    fn conflicts(&self, faces: &[FaceFile]) -> Result<Vec<Conflict>, ActivationError> {
        let state = self.state();
        state.check_available()?;
        let known: Vec<(&str, &Path, ConflictKind)> = state
            .installed
            .iter()
            .map(|(name, path, kind)| (name.as_str(), path.as_path(), *kind))
            .chain(
                state
                    .active
                    .iter()
                    .map(|(r, name)| (name.as_str(), r.path.as_path(), ConflictKind::User)),
            )
            .collect();
        Ok(faces
            .iter()
            .flat_map(|face| {
                known
                    .iter()
                    .filter(|(name, path, _)| *name == face.postscript_name && *path != face.path)
                    .map(|(_, path, kind)| Conflict {
                        path: face.path.clone(),
                        postscript_name: face.postscript_name.clone(),
                        with: path.to_path_buf(),
                        kind: *kind,
                    })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn face(path: &str, postscript_name: &str) -> FaceFile {
        FaceFile {
            path: PathBuf::from(path),
            postscript_name: postscript_name.to_owned(),
        }
    }

    fn registration(outcome: &Outcome) -> Option<Registration> {
        match &outcome.status {
            OutcomeStatus::Activated(r) | OutcomeStatus::AlreadyActive(r) => Some(r.clone()),
            _ => None,
        }
    }

    fn activate(activator: &MockActivator, faces: &[FaceFile], mode: Mode) -> Vec<Registration> {
        activator
            .activate(faces, mode)
            .unwrap_or_default()
            .iter()
            .filter_map(registration)
            .collect()
    }

    fn active_paths(activator: &MockActivator) -> Vec<PathBuf> {
        activator
            .list_active()
            .unwrap_or_default()
            .into_iter()
            .map(|r| r.path)
            .collect()
    }

    #[test]
    fn is_usable_as_a_shared_trait_object_across_threads() {
        let activator: Arc<dyn FontActivator> = Arc::new(MockActivator::new());
        let worker = Arc::clone(&activator);
        let outcomes =
            std::thread::spawn(move || worker.activate(&[face("/f/a.ttf", "A")], Mode::Session))
                .join();
        assert!(matches!(outcomes, Ok(Ok(ref o)) if o.len() == 1));
        assert_eq!(activator.list_active().map(|l| l.len()), Ok(1));
    }

    #[test]
    fn activate_returns_one_outcome_per_face_in_order_with_the_mode() {
        let activator = MockActivator::new();
        let faces = [face("/f/b.ttf", "B"), face("/f/a.ttf", "A")];
        let outcomes = activator
            .activate(&faces, Mode::Permanent)
            .unwrap_or_default();

        let paths: Vec<_> = outcomes.iter().map(|o| o.path.clone()).collect();
        assert_eq!(
            paths,
            [PathBuf::from("/f/b.ttf"), PathBuf::from("/f/a.ttf")]
        );
        for o in &outcomes {
            assert!(
                matches!(&o.status, OutcomeStatus::Activated(r) if r.mode == Mode::Permanent && r.path == o.path),
                "{o:?}"
            );
        }
        let handles: HashSet<_> = outcomes
            .iter()
            .filter_map(registration)
            .map(|r| r.handle)
            .collect();
        assert_eq!(handles.len(), 2, "each file gets its own handle");
    }

    #[test]
    fn a_failing_face_does_not_stop_the_others() {
        let activator = MockActivator::new();
        activator.fail_on("/f/bad.ttf", ActivationErrorKind::PermissionDenied);
        let faces = [
            face("/f/a.ttf", "A"),
            face("/f/bad.ttf", "Bad"),
            face("/f/c.ttf", "C"),
        ];
        let outcomes = activator
            .activate(&faces, Mode::Session)
            .unwrap_or_default();

        assert_eq!(outcomes.len(), 3);
        assert!(matches!(outcomes[0].status, OutcomeStatus::Activated(_)));
        assert!(matches!(
            &outcomes[1].status,
            OutcomeStatus::Failed(e) if e.kind == ActivationErrorKind::PermissionDenied
        ));
        assert!(matches!(outcomes[2].status, OutcomeStatus::Activated(_)));
        assert_eq!(
            active_paths(&activator),
            [PathBuf::from("/f/a.ttf"), PathBuf::from("/f/c.ttf")]
        );
    }

    #[test]
    fn a_failing_deactivation_does_not_stop_the_others() {
        let activator = MockActivator::new();
        let regs = activate(
            &activator,
            &[face("/f/a.ttf", "A"), face("/f/b.ttf", "B")],
            Mode::Session,
        );
        activator.fail_on("/f/a.ttf", ActivationErrorKind::Os);

        let outcomes = activator.deactivate(&regs).unwrap_or_default();
        assert_eq!(outcomes.len(), 2);
        assert!(matches!(
            &outcomes[0].status,
            OutcomeStatus::Failed(e) if e.kind == ActivationErrorKind::Os
        ));
        assert_eq!(outcomes[1].status, OutcomeStatus::Deactivated);
        assert_eq!(activator.list_active(), Ok(vec![regs[0].clone()]));
    }

    #[test]
    fn faces_of_one_file_share_its_registration() {
        let activator = MockActivator::new();
        let outcomes = activator
            .activate(
                &[
                    face("/f/fam.ttc", "Fam-Regular"),
                    face("/f/fam.ttc", "Fam-Bold"),
                ],
                Mode::Session,
            )
            .unwrap_or_default();
        assert!(matches!(outcomes[0].status, OutcomeStatus::Activated(_)));
        assert_eq!(
            Some(outcomes[1].status.clone()),
            registration(&outcomes[0]).map(OutcomeStatus::AlreadyActive)
        );
        assert_eq!(activator.list_active().map(|l| l.len()), Ok(1));
    }

    #[test]
    fn deactivate_never_touches_a_registration_it_did_not_make() {
        let activator = MockActivator::new();
        let ours = activate(&activator, &[face("/f/a.ttf", "A")], Mode::Session);
        // Same file and mode, but a handle this activator never issued
        // (another app's registry value).
        let foreign = Registration {
            handle: OsString::from("Arial (TrueType)"),
            ..ours[0].clone()
        };

        let outcomes = activator.deactivate(&[foreign]).unwrap_or_default();
        assert_eq!(outcomes[0].status, OutcomeStatus::NotOurs);
        assert_eq!(activator.list_active(), Ok(ours));
    }

    #[test]
    fn deactivate_reports_a_registration_removed_behind_its_back() {
        let activator = MockActivator::new();
        let regs = activate(&activator, &[face("/f/a.ttf", "A")], Mode::Permanent);
        activator.remove_externally(Path::new("/f/a.ttf"));

        let outcomes = activator.deactivate(&regs).unwrap_or_default();
        assert_eq!(outcomes[0].status, OutcomeStatus::NotActive);
        assert_eq!(active_paths(&activator), Vec::<PathBuf>::new());
    }

    #[test]
    fn deactivating_twice_is_not_active_the_second_time() {
        let activator = MockActivator::new();
        let regs = activate(&activator, &[face("/f/a.ttf", "A")], Mode::Session);
        let first = activator.deactivate(&regs).unwrap_or_default();
        let second = activator.deactivate(&regs).unwrap_or_default();
        assert_eq!(first[0].status, OutcomeStatus::Deactivated);
        assert_eq!(second[0].status, OutcomeStatus::NotActive);
    }

    #[test]
    fn conflicts_match_postscript_names_but_not_the_same_file() {
        let activator = MockActivator::new();
        activator.install("Clash", "/sys/clash.ttf", ConflictKind::System);
        activate(
            &activator,
            &[face("/lib/other.ttf", "Other")],
            Mode::Session,
        );

        let faces = [
            face("/lib/clash.ttf", "Clash"),
            face("/lib/other-copy.ttf", "Other"),
            face("/lib/other.ttf", "Other"),
            face("/lib/fresh.ttf", "Fresh"),
        ];
        let conflicts = activator.conflicts(&faces).unwrap_or_default();
        assert_eq!(
            conflicts,
            [
                Conflict {
                    path: PathBuf::from("/lib/clash.ttf"),
                    postscript_name: "Clash".to_owned(),
                    with: PathBuf::from("/sys/clash.ttf"),
                    kind: ConflictKind::System,
                },
                Conflict {
                    path: PathBuf::from("/lib/other-copy.ttf"),
                    postscript_name: "Other".to_owned(),
                    with: PathBuf::from("/lib/other.ttf"),
                    kind: ConflictKind::User,
                },
            ]
        );
        assert_eq!(
            active_paths(&activator),
            [PathBuf::from("/lib/other.ttf")],
            "conflicts activates nothing"
        );
    }

    #[test]
    fn an_unavailable_activator_fails_every_call_as_a_whole() {
        let activator = MockActivator::new();
        activator.set_unavailable(true);
        let faces = [face("/f/a.ttf", "A")];
        let err = activator.activate(&faces, Mode::Session).err();
        assert_eq!(err.map(|e| e.kind), Some(ActivationErrorKind::Unsupported));
        assert!(activator.deactivate(&[]).is_err());
        assert!(activator.list_active().is_err());
        assert!(activator.conflicts(&faces).is_err());

        activator.set_unavailable(false);
        assert_eq!(activator.list_active(), Ok(vec![]), "nothing was activated");
    }

    #[test]
    fn errors_read_as_plain_sentences_and_keep_the_detail() {
        let err =
            ActivationError::new(ActivationErrorKind::FileNotFound, "/f/gone.ttf: os error 2");
        assert_eq!(err.to_string(), "The font file was not found.");
        assert_eq!(err.detail, "/f/gone.ttf: os error 2");
    }

    #[test]
    fn clear_session_removes_session_registrations_and_keeps_permanent_ones() {
        let activator = MockActivator::new();
        activate(&activator, &[face("/f/s.ttf", "S")], Mode::Session);
        activate(&activator, &[face("/f/p.ttf", "P")], Mode::Permanent);

        let cleared = activator.clear_session().unwrap_or_default();

        assert_eq!(cleared.len(), 1);
        assert_eq!(cleared[0].path, PathBuf::from("/f/s.ttf"));
        assert_eq!(cleared[0].status, OutcomeStatus::Deactivated);
        assert_eq!(active_paths(&activator), vec![PathBuf::from("/f/p.ttf")]);
    }

    #[test]
    fn clear_session_fails_when_the_activator_cannot_work() {
        let activator = MockActivator::new();
        activate(&activator, &[face("/f/s.ttf", "S")], Mode::Session);
        activator.set_unavailable(true);
        assert!(activator.clear_session().is_err());
    }
}
