//! The `FontActivator` trait: make a font file usable by other apps, and undo it.
//!
//! One implementation per OS lives under `src/{windows,macos,linux}/` (ADR 0008);
//! [`MockActivator`] stands in for them in tests. The activator knows files and
//! PostScript names, not library ids: the core maps faces to [`FaceFile`]s and
//! stores the [`Registration`]s it gets back in the activation table.
//!
//! Results are per face. A call returns `Err` only when the activator cannot
//! work at all (for example the OS service is not there); a face that fails
//! comes back as [`OutcomeStatus::Failed`] next to the faces that worked.
//!
//! Rule 3: an activator only removes what it registered. [`FontActivator::deactivate`]
//! takes the registrations [`FontActivator::activate`] returned, and answers
//! [`OutcomeStatus::NotOurs`] for anything else.

mod mock;

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

pub use mock::MockActivator;

use crate::AppPaths;

/// The activator of this OS (ADR 0008), for the app folders `paths`: the
/// per-user registry on Windows, CoreText on macOS, symbolic links on Linux.
/// Any other OS is [`ActivationErrorKind::Unsupported`].
#[cfg_attr(
    not(any(windows, target_os = "macos", target_os = "linux")),
    allow(unused_variables)
)]
pub fn system(paths: &AppPaths) -> Result<Arc<dyn FontActivator>, ActivationError> {
    #[cfg(windows)]
    {
        let _ = paths;
        Ok(Arc::new(crate::WindowsActivator::new()))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(Arc::new(crate::MacosActivator::new(paths)))
    }
    #[cfg(target_os = "linux")]
    {
        Ok(Arc::new(crate::LinuxActivator::new(paths)))
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        Err(ActivationError::new(
            ActivationErrorKind::Unsupported,
            "font activation is implemented for Windows, macOS and Linux only",
        ))
    }
}

/// How long an activation lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Until the app quits; cleaned up at the next start after a crash.
    Session,
    /// Survives reboot; per-user, pointing at the file in place.
    Permanent,
}

/// A face to activate: the file where it is and the face's PostScript name.
///
/// The OS registers whole files, so faces of one collection file (`.ttc`)
/// share one registration: the second comes back [`OutcomeStatus::AlreadyActive`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FaceFile {
    /// The font file, in place (never a copy, rule 2).
    pub path: PathBuf,
    /// The PostScript name; conflicts match on it.
    pub postscript_name: String,
}

/// What the OS holds for one activated file. The core stores it and gives it
/// back to [`FontActivator::deactivate`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Registration {
    /// The font file the registration points at.
    pub path: PathBuf,
    /// Session or permanent.
    pub mode: Mode,
    /// The OS record the activator wrote: the registry value name on Windows,
    /// the symbolic link on Linux, the file URL on macOS. Store it as bytes with
    /// [`crate::fs::path_to_bytes`]; it may not be valid UTF-8.
    pub handle: OsString,
}

/// What happened to one face or one registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutcomeStatus {
    /// Activated now.
    Activated(Registration),
    /// The file was active already; this is its registration.
    AlreadyActive(Registration),
    /// Deactivated now.
    Deactivated,
    /// The registration is gone already (the user or the OS removed it).
    NotActive,
    /// This activator did not make the registration, so it does not touch it (rule 3).
    NotOurs,
    /// The OS call failed for this face; the other faces are not affected.
    Failed(ActivationError),
}

/// One result, for the file at `path`. Outcomes come back in the order of the
/// input, one per item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The font file.
    pub path: PathBuf,
    /// What happened.
    pub status: OutcomeStatus,
}

/// Who owns the font a face clashes with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConflictKind {
    /// A font installed in a system font folder.
    System,
    /// A per-user font: installed by the user, or registered by an app
    /// (this one included; the core tells them apart by path).
    User,
}

/// A font the OS already knows with the same PostScript name as a face.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// The face's file.
    pub path: PathBuf,
    /// The shared PostScript name.
    pub postscript_name: String,
    /// The file of the font it clashes with.
    pub with: PathBuf,
    /// Who owns that font.
    pub kind: ConflictKind,
}

/// Why an activation call failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActivationErrorKind {
    /// The font file is not there.
    FileNotFound,
    /// The OS refused access to the file or to the registration store.
    PermissionDenied,
    /// The OS cannot activate this file (format, location) or this activator
    /// cannot work on this host.
    Unsupported,
    /// Any other OS failure.
    Os,
}

/// A failed activation call: what kind, and the developer detail (the OS
/// error, the path). [`fmt::Display`] is the sentence for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationError {
    /// What kind of failure.
    pub kind: ActivationErrorKind,
    /// The developer detail.
    pub detail: String,
}

impl ActivationError {
    /// An error of `kind` with the developer `detail`.
    pub fn new(kind: ActivationErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for ActivationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            ActivationErrorKind::FileNotFound => "The font file was not found.",
            ActivationErrorKind::PermissionDenied => {
                "The system did not allow access to the font file or the font settings."
            }
            ActivationErrorKind::Unsupported => "This font cannot be activated on this system.",
            ActivationErrorKind::Os => "The system could not change the font's activation.",
        })
    }
}

impl std::error::Error for ActivationError {}

/// Makes font files usable by other apps without copying them, and undoes it.
///
/// Every method returns `Err` only when the activator cannot work at all;
/// per-face failures are [`OutcomeStatus::Failed`] items in the result.
pub trait FontActivator: Send + Sync {
    /// Activates each face in `mode`. One [`Outcome`] per face, in order.
    fn activate(&self, faces: &[FaceFile], mode: Mode) -> Result<Vec<Outcome>, ActivationError>;

    /// Removes each registration. One [`Outcome`] per registration, in order;
    /// a registration this activator did not make is [`OutcomeStatus::NotOurs`]
    /// and stays untouched.
    fn deactivate(&self, registrations: &[Registration]) -> Result<Vec<Outcome>, ActivationError>;

    /// The registrations this activator made that the OS still holds. The core
    /// compares them with its activation table, for example to clean up
    /// session activations left by a crash (ADR 0008).
    fn list_active(&self) -> Result<Vec<Registration>, ActivationError>;

    /// Fonts the OS already knows whose PostScript name matches one of `faces`,
    /// without activating anything. A file never conflicts with itself.
    fn conflicts(&self, faces: &[FaceFile]) -> Result<Vec<Conflict>, ActivationError>;

    /// Removes every session registration this activator made that the OS
    /// still holds: on quit, and at start after a crash (ADR 0008). One
    /// [`Outcome`] per session registration.
    fn clear_session(&self) -> Result<Vec<Outcome>, ActivationError> {
        let session: Vec<Registration> = self
            .list_active()?
            .into_iter()
            .filter(|r| r.mode == Mode::Session)
            .collect();
        self.deactivate(&session)
    }
}
