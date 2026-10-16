//! Library service: sources, scanning, families, faces and problems.

use serde::{Deserialize, Serialize};

use crate::activation::ActivationMode;
use crate::common::{
    Axis, Classification, Embedding, FeatureInfo, FontFormat, Health, Metrics, NamedInstance, Page,
    Paged, Slant, Timestamp,
};
use crate::{ApiResult, ApiVersion, FaceId, FamilyId, JobId, SourceId};

/// The operating system the app runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Platform {
    /// Windows 10 or 11.
    Windows,
    /// macOS.
    Macos,
    /// Linux.
    Linux,
}

/// What the running app is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    /// Product name.
    pub name: String,
    /// App version (semver).
    pub version: String,
    /// The API contract this build serves.
    pub api_version: ApiVersion,
    /// The operating system.
    pub platform: Platform,
}

/// Where a source's fonts come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    /// A folder the user chose; indexed in place.
    Watched,
    /// The OS font folders; read-only, never deactivated by the app.
    System,
    /// The user-chosen folder that "Copy to library folder" imports into.
    LibraryFolder,
}

/// A source of fonts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    /// Id.
    pub id: SourceId,
    /// Absolute folder path.
    pub path: String,
    /// What kind of source.
    pub kind: SourceKind,
    /// Hidden from the library without being removed.
    pub paused: bool,
    /// Faces indexed from it.
    pub face_count: u32,
}

/// How dropped or added fonts join the library (ADR 0004).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ImportMode {
    /// Index the files where they are (the default).
    IndexInPlace,
    /// Copy the files into a user-chosen library folder first.
    #[serde(rename_all = "camelCase")]
    CopyTo {
        /// The library folder.
        folder: String,
    },
}

/// Add a folder as a source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AddSourceRequest {
    /// Absolute folder path.
    pub path: String,
    /// Index in place or copy.
    pub mode: ImportMode,
}

/// Pause or resume a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SetSourcePausedRequest {
    /// The source.
    pub source: SourceId,
    /// Paused (hidden) or not.
    pub paused: bool,
}

/// Start a scan. Returns at once; progress arrives as `Event::ScanProgress`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScanRequest {
    /// One source, or every source when absent.
    #[serde(default)]
    pub source: Option<SourceId>,
}

/// Family list order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FamilySort {
    /// A to Z.
    #[default]
    Name,
    /// Newest first.
    DateAdded,
    /// Most styles first.
    StyleCount,
}

/// List families, one page at a time (the UI list is virtualized).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ListFamiliesRequest {
    /// One source, or all unpaused sources when absent.
    #[serde(default)]
    pub source: Option<SourceId>,
    /// Order.
    #[serde(default)]
    pub sort: FamilySort,
    /// Window.
    #[serde(default)]
    pub page: Page,
}

/// Whether a family's faces are active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FamilyActivation {
    /// No face is active.
    Inactive,
    /// Some faces are active.
    Partial,
    /// Every face is active.
    Active,
}

/// One row of the family list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FamilySummary {
    /// Id.
    pub id: FamilyId,
    /// Typographic family name (name ID 16, falling back to 1).
    pub name: String,
    /// Number of styles.
    pub style_count: u32,
    /// Formats present in the family.
    pub formats: Vec<FontFormat>,
    /// Foundry or manufacturer.
    pub foundry: Option<String>,
    /// Design class, when known.
    pub classification: Option<Classification>,
    /// At least one face is a variable font.
    pub is_variable: bool,
    /// Activation state at a glance.
    pub activation: FamilyActivation,
    /// When the first face was indexed.
    pub added: Timestamp,
}

/// One face, as listed under its family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FaceSummary {
    /// Id.
    pub id: FaceId,
    /// Family id.
    pub family: FamilyId,
    /// Family name.
    pub family_name: String,
    /// Style name (name ID 17, falling back to 2).
    pub style: String,
    /// PostScript name; activation matches on it.
    pub postscript_name: String,
    /// `OS/2.usWeightClass` (100–1000).
    pub weight: u16,
    /// `OS/2.usWidthClass` (1–9).
    pub width: u16,
    /// Slant.
    pub slant: Slant,
    /// File format.
    pub format: FontFormat,
    /// Variable font.
    pub is_variable: bool,
    /// Active, and how; absent when inactive.
    pub active: Option<ActivationMode>,
    /// Usable or not.
    pub health: Health,
}

/// A family with its faces, sorted by weight, then width, then slant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Family {
    /// The list row.
    pub summary: FamilySummary,
    /// Designer, when declared.
    pub designer: Option<String>,
    /// Faces.
    pub faces: Vec<FaceSummary>,
}

/// An open-source license recognized from the license text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum OpenLicense {
    /// SIL Open Font License.
    Ofl,
    /// Apache License 2.0.
    Apache,
    /// Ubuntu Font Licence.
    Ufl,
    /// MIT.
    Mit,
}

/// License type the user declared for a family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum LicenseType {
    /// Desktop use.
    Desktop,
    /// Web embedding.
    Web,
    /// App embedding.
    App,
    /// Trial.
    Trial,
    /// Personal use only.
    PersonalOnly,
    /// Not known.
    Unknown,
}

/// What the file says, and what the user declared, about where a font may be used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LicenseInfo {
    /// License description (name ID 13).
    pub description: Option<String>,
    /// License URL (name ID 14).
    pub url: Option<String>,
    /// Recognized open license.
    pub open_license: Option<OpenLicense>,
    /// `OS/2.fsType`.
    pub embedding: Embedding,
    /// What the user declared, if anything.
    pub declared: Option<LicenseType>,
}

/// Everything the info panel shows about one face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FaceDetail {
    /// The list row.
    pub face: FaceSummary,
    /// Absolute file path.
    pub path: String,
    /// Index inside a collection file (0 for a single-face file).
    pub collection_index: u32,
    /// File size in bytes.
    pub file_size: u32,
    /// blake3 hash of the file, hex.
    pub hash: String,
    /// Version string (name ID 5).
    pub version: Option<String>,
    /// Copyright (name ID 0).
    pub copyright: Option<String>,
    /// Designer (name ID 9).
    pub designer: Option<String>,
    /// Vendor id (`OS/2.achVendID`).
    pub vendor_id: Option<String>,
    /// License facts.
    pub license: LicenseInfo,
    /// Vertical metrics.
    pub metrics: Metrics,
    /// Variation axes (empty for a static font).
    pub axes: Vec<Axis>,
    /// Named instances (empty for a static font).
    pub instances: Vec<NamedInstance>,
    /// GSUB/GPOS features.
    pub features: Vec<FeatureInfo>,
    /// OpenType script tags the face declares (`latn`, `cyrl`).
    pub scripts: Vec<String>,
    /// User tags, by name.
    pub tags: Vec<String>,
    /// Favorite.
    pub favorite: bool,
    /// 1–5 stars, or absent.
    pub rating: Option<u8>,
    /// User notes.
    pub notes: Option<String>,
}

/// What kind of health problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ProblemKind {
    /// The same file (same hash) at two paths.
    DuplicateFile,
    /// The same family, style and version at different paths.
    DuplicateFace,
    /// Truncated or missing required tables.
    Broken,
    /// No Unicode `cmap`.
    NoUnicodeCmap,
    /// Same PostScript name as a system font.
    SystemConflict,
    /// Same PostScript name as a font already active from another source.
    ActiveConflict,
    /// A format the app cannot activate.
    Unsupported,
}

/// One entry of the Problems view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    /// What kind.
    pub kind: ProblemKind,
    /// The face, when the file parsed far enough to have one.
    pub face: Option<FaceId>,
    /// The file.
    pub path: String,
    /// The other face or file it clashes with, for duplicates and conflicts.
    pub other: Option<String>,
    /// One sentence for the user.
    pub message: String,
}

/// Sources, scans, families, faces and problems.
pub trait LibraryService {
    /// What the running app is (E1.5 round-trips this).
    fn app_info(&self) -> ApiResult<AppInfo>;
    /// Every source.
    fn list_sources(&self) -> ApiResult<Vec<Source>>;
    /// Add a folder; does not scan it (call `scan`).
    fn add_source(&self, req: AddSourceRequest) -> ApiResult<Source>;
    /// Forget a source and its faces. Never touches the files.
    fn remove_source(&self, source: SourceId) -> ApiResult<()>;
    /// Pause or resume a source.
    fn set_source_paused(&self, req: SetSourcePausedRequest) -> ApiResult<Source>;
    /// Start a scan; `Busy` when one is running.
    fn scan(&self, req: ScanRequest) -> ApiResult<JobId>;
    /// One page of families.
    fn list_families(&self, req: ListFamiliesRequest) -> ApiResult<Paged<FamilySummary>>;
    /// One family with its faces.
    fn get_family(&self, family: FamilyId) -> ApiResult<Family>;
    /// Full detail of one face.
    fn get_face(&self, face: FaceId) -> ApiResult<FaceDetail>;
    /// Duplicates, broken files and conflicts.
    fn list_problems(&self) -> ApiResult<Vec<Problem>>;
}
