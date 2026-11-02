//! Activation service. The core activates through `fm-platform`'s
//! `FontActivator`; this is the contract adapters see. Results are per face,
//! never all-or-nothing. The app only deactivates faces it registered.

use serde::{Deserialize, Serialize};

use crate::common::Timestamp;
use crate::library::{Problem, ProblemKind};
use crate::{ApiResult, CollectionId, FaceId, FamilyId, SetId};

/// How long an activation lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ActivationMode {
    /// Until the app quits or the user deactivates (the default).
    Session,
    /// Survives reboot; per-user, pointing at the file in place.
    Permanent,
}

/// What to act on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Target {
    /// These faces.
    #[serde(rename_all = "camelCase")]
    Faces {
        /// Face ids.
        faces: Vec<FaceId>,
    },
    /// Every face of a family.
    #[serde(rename_all = "camelCase")]
    Family {
        /// Family id.
        family: FamilyId,
    },
    /// Every face in a collection.
    #[serde(rename_all = "camelCase")]
    Collection {
        /// Collection id.
        collection: CollectionId,
    },
    /// Every face in a project set.
    #[serde(rename_all = "camelCase")]
    Set {
        /// Set id.
        set: SetId,
    },
}

/// What to do when a face clashes with a system or an already-active font.
/// The API never prompts; the UI asks the user and sends the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ConflictPolicy {
    /// Leave the clashing face inactive and report the conflict.
    #[default]
    KeepExisting,
    /// Activate anyway, for this session only: a clashing face is activated
    /// in session mode even when permanent was asked.
    Replace,
}

/// Activate faces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivateRequest {
    /// What to activate.
    pub target: Target,
    /// Session or permanent.
    #[serde(default = "session")]
    pub mode: ActivationMode,
    /// Conflict handling.
    #[serde(default)]
    pub on_conflict: ConflictPolicy,
}

fn session() -> ActivationMode {
    ActivationMode::Session
}

/// Deactivate faces the app registered.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DeactivateRequest {
    /// What to deactivate; everything the app activated when absent.
    #[serde(default)]
    pub target: Option<Target>,
    /// Faces to leave active, e.g. a project set for "Deactivate everything
    /// not in the current set". Nothing is kept when absent.
    #[serde(default)]
    pub except: Option<Target>,
}

/// The other side of a conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ConflictWith {
    /// A system font.
    #[serde(rename_all = "camelCase")]
    System {
        /// Its file.
        path: String,
    },
    /// A face already active from another source.
    #[serde(rename_all = "camelCase")]
    Active {
        /// That face.
        face: FaceId,
    },
}

/// What happened to one face.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum OutcomeStatus {
    /// Now active (or now inactive, for a deactivation).
    Done,
    /// Already in the requested state.
    Unchanged,
    /// Not done because of a clash (with `KeepExisting`).
    #[serde(rename_all = "camelCase")]
    Conflict {
        /// What it clashes with.
        with: ConflictWith,
    },
    /// Not done: the app did not register it, or the format cannot be activated.
    #[serde(rename_all = "camelCase")]
    Skipped {
        /// Why, in plain words.
        reason: String,
    },
    /// The OS call failed.
    #[serde(rename_all = "camelCase")]
    Failed {
        /// The error.
        error: crate::ApiError,
    },
}

/// One face's result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivationOutcome {
    /// The face.
    pub face: FaceId,
    /// What happened.
    pub outcome: OutcomeStatus,
}

/// A face the app has active.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActiveFace {
    /// The face.
    pub face: FaceId,
    /// Session or permanent.
    pub mode: ActivationMode,
    /// Since when.
    pub since: Timestamp,
    /// The document that asked for it (auto-activation), if any: of the
    /// open documents that need the face, the one that asked last (E29.3).
    pub document: Option<String>,
}

/// A clash found before activating.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    /// The face that would be activated.
    pub face: FaceId,
    /// The shared PostScript name.
    pub postscript_name: String,
    /// What it clashes with.
    pub with: ConflictWith,
}

/// Which of the two fonts of a [`Clash`] the app puts in front.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ClashWinner {
    /// The clash's `face`.
    Face,
    /// The font in the clash's `with`.
    With,
}

/// Two active fonts with one PostScript name: one row of the Conflicts view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Clash {
    /// A face the app has active.
    pub face: FaceId,
    /// The shared PostScript name.
    pub postscript_name: String,
    /// What it clashes with: a system or user font, or another face the app
    /// has active (then `face` is the lower face id of the two).
    pub with: ConflictWith,
    /// The winner as the app decided it. The app's face wins over a system
    /// or user font: it is active over that font only because the user chose
    /// "replace for this session" (or it was active before that font came).
    /// Between two of the app's faces the later activation wins.
    pub winner: ClashWinner,
}

impl Clash {
    /// The Problems entry of the clash: `path` is the file of `face`,
    /// `other_path` the file of the other face of an `Active` clash (a
    /// `System` clash carries its own). Every implementation of
    /// [`LibraryService::list_problems`](crate::library::LibraryService::list_problems)
    /// uses this, so the rule is one.
    pub fn problem(&self, path: String, other_path: Option<String>) -> Problem {
        let name = &self.postscript_name;
        let (kind, other, message) = match &self.with {
            ConflictWith::System { path: system } => (
                ProblemKind::SystemConflict,
                Some(system.clone()),
                format!("{name} is active and has the same PostScript name as an installed font."),
            ),
            ConflictWith::Active { .. } => (
                ProblemKind::ActiveConflict,
                other_path,
                format!("{name} is active twice, from two different files."),
            ),
        };
        Problem {
            kind,
            face: Some(self.face),
            path,
            other,
            message,
        }
    }
}

/// A font a document uses, as a design app names it (E29.2). Every name is
/// optional; the names given are tried in the order of [`NeededFontRule`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NeededFont {
    /// The PostScript name (name ID 6).
    #[serde(default)]
    pub postscript_name: Option<String>,
    /// The full name (name ID 4).
    #[serde(default)]
    pub full_name: Option<String>,
    /// The family name.
    #[serde(default)]
    pub family: Option<String>,
    /// The style within the family; "Regular" when a family comes without one.
    #[serde(default)]
    pub style: Option<String>,
}

/// The fonts a document needs, to find in the library and activate (the
/// plugins' auto-activation, E29).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FontsNeededRequest {
    /// The fonts, in the document's order.
    pub fonts: Vec<NeededFont>,
    /// The document that needs them, recorded with the faces it activates.
    #[serde(default)]
    pub document: Option<String>,
    /// Session-activate the matched faces; when false, only match.
    #[serde(default = "yes")]
    pub activate: bool,
    /// Conflict handling when activating; a face that clashes is left
    /// inactive and reported unless this is `Replace` (the user chose to
    /// replace the installed font, E30.4).
    #[serde(default)]
    pub on_conflict: ConflictPolicy,
}

fn yes() -> bool {
    true
}

/// Which name found a needed font, in the order the names are tried. Names
/// compare ignoring case and surrounding space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum NeededFontRule {
    /// The PostScript name (name ID 6).
    PostscriptName,
    /// The full name (name ID 4).
    FullName,
    /// The typographic family and subfamily (name IDs 16 and 17; 17 is 2
    /// when the font has no 17).
    TypographicFamily,
    /// The legacy family and subfamily (name IDs 1 and 2).
    Family,
}

/// A needed font the library has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NeededFontMatch {
    /// The needed font, as asked.
    pub font: NeededFont,
    /// The face that stands for it. Of several faces that match, an active
    /// face wins, then a healthy one, then the lowest face id; a broken face
    /// is never matched.
    pub face: FaceId,
    /// The name that found it.
    pub matched_by: NeededFontRule,
}

/// What [`ActivationService::fonts_needed`] found and did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FontsNeededResponse {
    /// The fonts the library has, in request order.
    pub matched: Vec<NeededFontMatch>,
    /// The fonts it does not have, in request order.
    pub unmatched: Vec<NeededFont>,
    /// How many fonts are unmatched.
    pub missing: u32,
    /// One outcome per matched face (a face two fonts matched comes once);
    /// empty when the request did not activate.
    pub outcomes: Vec<ActivationOutcome>,
    /// The fonts the library did not have whose family this request
    /// downloaded from an online catalog the user turned on (E29.5), in
    /// request order; empty unless `Settings::fetch_missing_fonts` is on and
    /// the request activates.
    #[serde(default)]
    pub fetched: Vec<FetchedFont>,
}

/// A needed font whose family `fonts_needed` downloaded from an online
/// catalog (E29.5), so a plugin can say what was downloaded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FetchedFont {
    /// The needed font, as asked.
    pub font: NeededFont,
    /// The catalog the family came from.
    pub catalog: crate::catalog::CatalogId,
    /// The family's name, as the catalog lists it.
    pub family: String,
    /// The faces of the family's files in the library, by id.
    pub faces: Vec<FaceId>,
}

/// A design app closed a document (E29.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DocumentClosedRequest {
    /// The document, as [`FontsNeededRequest::document`] named it.
    pub document: String,
}

/// What [`ActivationService::document_closed`] offers to deactivate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DocumentClosedResponse {
    /// The session faces only documents activated and no open document
    /// needs any more, in id order. Deactivate them with
    /// [`ActivationService::deactivate`] when the user accepts.
    pub offer: Vec<FaceId>,
    /// The names of the faces in `offer`, one per face in the same order
    /// (E30.5), so a plugin can name them in its question. Empty from an
    /// app older than E30.5.
    #[serde(default)]
    pub offered: Vec<OfferedFace>,
}

/// A face [`ActivationService::document_closed`] offers, with the names a
/// plugin shows for it (E30.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OfferedFace {
    /// The face.
    pub face: FaceId,
    /// Its family name, as the library groups it.
    pub family: String,
    /// Its style name.
    pub style: String,
    /// Its PostScript name; `None` when the font declares none.
    pub postscript_name: Option<String>,
}

/// Activation and deactivation.
pub trait ActivationService {
    /// Activate; one outcome per face.
    fn activate(&self, req: ActivateRequest) -> ApiResult<Vec<ActivationOutcome>>;
    /// Deactivate; faces the app did not register come back `Skipped`.
    fn deactivate(&self, req: DeactivateRequest) -> ApiResult<Vec<ActivationOutcome>>;
    /// Every face the app has active.
    fn list_active(&self) -> ApiResult<Vec<ActiveFace>>;
    /// Clashes the target would cause, without activating anything.
    fn conflicts(&self, target: Target) -> ApiResult<Vec<Conflict>>;
    /// Every clash of a face the app has active, with its winner (the
    /// Conflicts view). Changes nothing.
    fn list_conflicts(&self) -> ApiResult<Vec<Clash>>;
    /// Finds the fonts a document needs and, unless the request says not
    /// to, session-activates them (E29.2). Fonts the library does not have
    /// are reported, not an error.
    fn fonts_needed(&self, req: FontsNeededRequest) -> ApiResult<FontsNeededResponse>;
    /// A document closed (E29.3): forgets that it needs its faces and
    /// offers the faces no other open document needs. A face the user
    /// activated, or one active permanently, is never offered. Deactivates
    /// nothing.
    fn document_closed(&self, req: DocumentClosedRequest) -> ApiResult<DocumentClosedResponse>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fonts_needed_request_activates_unless_it_says_not_to() {
        let req: FontsNeededRequest =
            serde_json::from_str(r#"{"fonts":[{"postscriptName":"Inter-Bold"}]}"#).expect("json");
        assert!(req.activate);
        assert_eq!(req.document, None);
        assert_eq!(
            req.fonts,
            [NeededFont {
                postscript_name: Some("Inter-Bold".into()),
                ..NeededFont::default()
            }]
        );
        let req: FontsNeededRequest = serde_json::from_str(
            r#"{"fonts":[{"family":"Inter","style":"Bold","fullName":"Inter Bold"}],
                "document":"poster.indd","activate":false}"#,
        )
        .expect("json");
        assert!(!req.activate);
        assert_eq!(req.document.as_deref(), Some("poster.indd"));
        assert_eq!(req.fonts[0].full_name.as_deref(), Some("Inter Bold"));
        assert_eq!(req.fonts[0].style.as_deref(), Some("Bold"));
    }

    #[test]
    fn a_fonts_needed_answer_without_fetched_has_none() {
        let res: FontsNeededResponse =
            serde_json::from_str(r#"{"matched":[],"unmatched":[],"missing":0,"outcomes":[]}"#)
                .expect("an answer from before E29.5");
        assert!(res.fetched.is_empty());
        let fetched = FetchedFont {
            font: NeededFont {
                family: Some("Alegreya".into()),
                ..NeededFont::default()
            },
            catalog: crate::catalog::CatalogId::GoogleFonts,
            family: "Alegreya".into(),
            faces: vec![FaceId(7)],
        };
        let json = serde_json::to_value(FontsNeededResponse {
            fetched: vec![fetched],
            ..res
        })
        .expect("json");
        assert_eq!(json["fetched"][0]["catalog"], "googleFonts");
        assert_eq!(json["fetched"][0]["family"], "Alegreya");
        assert_eq!(json["fetched"][0]["faces"][0], 7);
    }

    #[test]
    fn a_document_closed_answer_without_offered_has_none() {
        let res: DocumentClosedResponse =
            serde_json::from_str(r#"{"offer":[3]}"#).expect("an answer from before E30.5");
        assert_eq!(res.offer, vec![FaceId(3)]);
        assert!(res.offered.is_empty());
        let json = serde_json::to_value(DocumentClosedResponse {
            offered: vec![OfferedFace {
                face: FaceId(3),
                family: "Alegreya".into(),
                style: "Bold Italic".into(),
                postscript_name: Some("Alegreya-BoldItalic".into()),
            }],
            ..res
        })
        .expect("json");
        assert_eq!(json["offer"][0], 3);
        assert_eq!(json["offered"][0]["face"], 3);
        assert_eq!(json["offered"][0]["family"], "Alegreya");
        assert_eq!(json["offered"][0]["style"], "Bold Italic");
        assert_eq!(json["offered"][0]["postscriptName"], "Alegreya-BoldItalic");
    }

    #[test]
    fn a_system_clash_is_a_system_conflict_with_the_installed_file() {
        let clash = Clash {
            face: FaceId(6),
            postscript_name: "Helvetica".into(),
            with: ConflictWith::System {
                path: "/System/Library/Fonts/Helvetica.ttc".into(),
            },
            winner: ClashWinner::Face,
        };
        let p = clash.problem("/fonts/Helvetica.otf".into(), Some("ignored".into()));
        assert_eq!(p.kind, ProblemKind::SystemConflict);
        assert_eq!(p.face, Some(FaceId(6)));
        assert_eq!(p.path, "/fonts/Helvetica.otf");
        assert_eq!(
            p.other.as_deref(),
            Some("/System/Library/Fonts/Helvetica.ttc")
        );
        assert!(p.message.starts_with("Helvetica is active"));
    }

    #[test]
    fn an_active_clash_is_an_active_conflict_with_the_other_file() {
        let clash = Clash {
            face: FaceId(2),
            postscript_name: "Inter-Regular".into(),
            with: ConflictWith::Active { face: FaceId(9) },
            winner: ClashWinner::With,
        };
        let p = clash.problem("/a/Inter.ttf".into(), Some("/b/Inter.ttf".into()));
        assert_eq!(p.kind, ProblemKind::ActiveConflict);
        assert_eq!(p.path, "/a/Inter.ttf");
        assert_eq!(p.other.as_deref(), Some("/b/Inter.ttf"));
    }
}
