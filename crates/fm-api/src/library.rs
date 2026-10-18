//! Library service: sources, scanning, families, faces and problems.

use serde::{Deserialize, Serialize};

use crate::activation::ActivationMode;
use crate::common::{
    Axis, Classification, Embedding, FeatureInfo, FontFormat, Health, Metrics, NamedInstance, Page,
    Paged, Slant, Timestamp,
};
use crate::search::WeightRange;
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
    /// Order by any column of the detail list; when set, it replaces `sort`.
    #[serde(default)]
    pub order: Option<FamilyOrder>,
}

/// What the family list can be ordered by: the columns of the detail list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FamilySortKey {
    /// Inactive, then partly active, then active.
    Activation,
    /// Family name, A to Z.
    Name,
    /// Number of styles.
    StyleCount,
    /// Lightest weight, then heaviest.
    Weight,
    /// Format badge: the formats, then variable, then color.
    Format,
    /// Version, digit runs compared as numbers.
    Version,
    /// Foundry or manufacturer.
    Manufacturer,
    /// Source folder.
    Location,
    /// Declared license, then the recognized open license.
    License,
    /// When the family was added, oldest first.
    DateAdded,
    /// Tags.
    Tags,
}

/// An order for the family list: a key and a direction. Families without a
/// value for the key come last in both directions; ties go by name, then id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FamilyOrder {
    /// The key.
    pub by: FamilySortKey,
    /// Largest (or Z, or newest) first.
    #[serde(default)]
    pub descending: bool,
}

impl FamilyOrder {
    /// Sorts `rows` in this order. Every implementation of
    /// [`LibraryService::list_families`] uses this, so the order is one.
    pub fn sort(&self, rows: &mut [FamilySummary]) {
        rows.sort_by_cached_key(|f| (self.key(f), f.name.to_lowercase(), f.id.0));
    }

    /// The sort key of one row: values first (reversed when descending), then
    /// rows without a value.
    fn key(&self, f: &FamilySummary) -> (bool, Directed) {
        let value = self.value(f);
        let missing = value.is_none();
        let value = value.unwrap_or(SortValue::Number(0));
        (
            missing,
            if self.descending {
                Directed::Down(std::cmp::Reverse(value))
            } else {
                Directed::Up(value)
            },
        )
    }

    /// The value one row has for the key; `None` when it has none.
    fn value(&self, f: &FamilySummary) -> Option<SortValue> {
        let text = |s: &Option<String>| {
            s.as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| SortValue::Text(s.to_lowercase()))
        };
        match self.by {
            FamilySortKey::Activation => Some(SortValue::Number(match f.activation {
                FamilyActivation::Inactive => 0,
                FamilyActivation::Partial => 1,
                FamilyActivation::Active => 2,
            })),
            FamilySortKey::Name => Some(SortValue::Text(f.name.to_lowercase())),
            FamilySortKey::StyleCount => Some(SortValue::Number(f.style_count.into())),
            FamilySortKey::Weight => f
                .weight_range
                .map(|w| SortValue::Number((u64::from(w.min) << 16) | u64::from(w.max))),
            FamilySortKey::Format => (!f.formats.is_empty()).then(|| {
                let mut names: Vec<&str> = f.formats.iter().map(|x| format_name(*x)).collect();
                names.sort_unstable();
                SortValue::Text(format!(
                    "{} {} {}",
                    names.join(","),
                    u8::from(f.is_variable),
                    u8::from(f.is_color)
                ))
            }),
            FamilySortKey::Version => f
                .version
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(|v| SortValue::Natural(natural(v))),
            FamilySortKey::Manufacturer => text(&f.foundry),
            FamilySortKey::Location => text(&f.location),
            FamilySortKey::License => {
                let declared = f.declared_license.map(license_type_name);
                let open = f.open_license.map(open_license_name);
                (declared.is_some() || open.is_some()).then(|| {
                    SortValue::Text(format!(
                        "{} {}",
                        declared.unwrap_or("~"),
                        open.unwrap_or("~")
                    ))
                })
            }
            FamilySortKey::DateAdded => {
                (!f.added.is_empty()).then(|| SortValue::Text(f.added.clone()))
            }
            FamilySortKey::Tags => (!f.tags.is_empty()).then(|| {
                let mut tags: Vec<String> = f.tags.iter().map(|t| t.to_lowercase()).collect();
                tags.sort();
                SortValue::Text(tags.join("\u{1}"))
            }),
        }
    }
}

/// A sort value in ascending or descending direction.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Directed {
    Up(SortValue),
    Down(std::cmp::Reverse<SortValue>),
}

/// A comparable sort value.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum SortValue {
    Number(u64),
    Text(String),
    Natural(Vec<NaturalPart>),
}

/// A piece of a natural-order key: a run of digits by value, or other text.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum NaturalPart {
    Number(u64),
    Text(String),
}

/// `s` split into digit runs (compared as numbers) and other text, so
/// "Version 2.10" sorts after "Version 2.9".
fn natural(s: &str) -> Vec<NaturalPart> {
    let mut parts = Vec::new();
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        let digit = c.is_ascii_digit();
        let end = rest
            .find(|ch: char| ch.is_ascii_digit() != digit)
            .unwrap_or(rest.len());
        let (run, tail) = rest.split_at(end);
        parts.push(if digit {
            NaturalPart::Number(run.parse().unwrap_or(u64::MAX))
        } else {
            NaturalPart::Text(run.to_lowercase())
        });
        rest = tail;
    }
    parts
}

fn format_name(f: FontFormat) -> &'static str {
    match f {
        FontFormat::Ttf => "ttf",
        FontFormat::Otf => "otf",
        FontFormat::Ttc => "ttc",
        FontFormat::Otc => "otc",
        FontFormat::Woff => "woff",
        FontFormat::Woff2 => "woff2",
        FontFormat::Type1 => "type1",
        FontFormat::Dfont => "dfont",
        FontFormat::Bitmap => "bitmap",
    }
}

fn license_type_name(l: LicenseType) -> &'static str {
    match l {
        LicenseType::App => "app",
        LicenseType::Desktop => "desktop",
        LicenseType::PersonalOnly => "personalOnly",
        LicenseType::Trial => "trial",
        LicenseType::Unknown => "unknown",
        LicenseType::Web => "web",
    }
}

fn open_license_name(l: OpenLicense) -> &'static str {
    match l {
        OpenLicense::Apache => "apache",
        OpenLicense::Mit => "mit",
        OpenLicense::Ofl => "ofl",
        OpenLicense::Ufl => "ufl",
    }
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
    /// Every style on the weight line, lightest first, upright before italic.
    #[serde(default)]
    pub styles: Vec<StyleMark>,
    /// Lightest to heaviest weight, a variable font's `wght` axis included.
    #[serde(default)]
    pub weight_range: Option<WeightRange>,
    /// At least one face is a color font.
    #[serde(default)]
    pub is_color: bool,
    /// Version string (name ID 5) of the first face.
    #[serde(default)]
    pub version: Option<String>,
    /// The source folder the family was found in.
    #[serde(default)]
    pub location: Option<String>,
    /// The open license recognized in the first face that has one.
    #[serde(default)]
    pub open_license: Option<OpenLicense>,
    /// The license type the user declared, if any.
    #[serde(default)]
    pub declared_license: Option<LicenseType>,
    /// User tags on any of its faces, by name, A to Z.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// One style on a family's weight line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StyleMark {
    /// `OS/2.usWeightClass` (100–1000).
    pub weight: u16,
    /// Italic or oblique.
    pub italic: bool,
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
    /// Family name as written in name ID 1 (the typographic one is
    /// `face.family_name`).
    #[serde(default)]
    pub legacy_family: Option<String>,
    /// Style name as written in name ID 2 (the typographic one is `face.style`).
    #[serde(default)]
    pub legacy_style: Option<String>,
    /// Foundry or manufacturer (name ID 8).
    #[serde(default)]
    pub foundry: Option<String>,
    /// `post.italicAngle` in degrees (0 for upright, negative leans right).
    #[serde(default)]
    pub italic_angle: Option<f32>,
    /// Code points the Unicode `cmap` maps.
    #[serde(default)]
    pub code_point_count: Option<u32>,
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
    /// Sidecar metadata (`<source>/.fontlibrary/`) that could not be merged:
    /// another machine set a field to a different value, or the sidecar file
    /// cannot be read. Nothing is overwritten until the user decides.
    SidecarConflict,
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

/// Why files are listed together as duplicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DuplicateKind {
    /// The same file (same hash) at different paths.
    Exact,
    /// The same PostScript name and version in files that differ.
    Near,
}

/// One file of a duplicate group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateCopy {
    /// Absolute file path.
    pub path: String,
    /// File size in bytes.
    pub size: u32,
    /// blake3 hash of the file, hex; equal hashes are identical files.
    pub hash: String,
    /// The face the group is about (the first face for an exact group), when
    /// the file parsed far enough to have one.
    pub face: Option<FaceId>,
    /// Version string (name ID 5) of that face.
    pub version: Option<String>,
    /// The source the file was found in.
    pub source: SourceId,
    /// The source is the library folder.
    pub in_library_folder: bool,
    /// The recommended keeper of the group.
    pub keep: bool,
}

/// Files that hold the same font. The recommended keeper comes first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    /// Exact or near.
    pub kind: DuplicateKind,
    /// The PostScript name the copies share (for an exact group, the first
    /// face's), when there is one.
    pub postscript_name: Option<String>,
    /// The copies, two or more, keeper first.
    pub copies: Vec<DuplicateCopy>,
}

impl DuplicateGroup {
    /// Orders the copies keeper first and marks the keeper: the newest
    /// version (digit runs compared as numbers, no version last), then a copy
    /// in the library folder, then the path A to Z. Every implementation of
    /// [`LibraryService::list_duplicates`] uses this, so the rule is one.
    pub fn choose_keeper(&mut self) {
        self.copies.sort_by_cached_key(|c| {
            let version = c
                .version
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(natural);
            (
                version.is_none(),
                std::cmp::Reverse(version),
                !c.in_library_folder,
                c.path.clone(),
            )
        });
        for (i, copy) in self.copies.iter_mut().enumerate() {
            copy.keep = i == 0;
        }
    }
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
    /// Rebuild the index from the sources: every file of every source is read
    /// again by a full scan, reported like `scan`. Face ids, tags, ratings,
    /// collections, sets and activation state are kept. `Busy` when a scan is
    /// running.
    fn rebuild_index(&self) -> ApiResult<JobId>;
    /// One page of families.
    fn list_families(&self, req: ListFamiliesRequest) -> ApiResult<Paged<FamilySummary>>;
    /// One family with its faces.
    fn get_family(&self, family: FamilyId) -> ApiResult<Family>;
    /// Full detail of one face.
    fn get_face(&self, face: FaceId) -> ApiResult<FaceDetail>;
    /// Duplicates, broken files and conflicts.
    fn list_problems(&self) -> ApiResult<Vec<Problem>>;
    /// Exact duplicates (same hash) and near duplicates (same PostScript name
    /// and version in files that differ) among the files the library shows,
    /// in groups, each with its recommended keeper first.
    fn list_duplicates(&self) -> ApiResult<Vec<DuplicateGroup>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: u32, name: &str, version: Option<&str>) -> FamilySummary {
        FamilySummary {
            id: FamilyId(id),
            name: name.into(),
            style_count: 1,
            formats: vec![FontFormat::Ttf],
            foundry: None,
            classification: None,
            is_variable: false,
            activation: FamilyActivation::Inactive,
            added: String::new(),
            styles: Vec::new(),
            weight_range: None,
            is_color: false,
            version: version.map(Into::into),
            location: None,
            open_license: None,
            declared_license: None,
            tags: Vec::new(),
        }
    }

    fn sorted(by: FamilySortKey, descending: bool, mut rows: Vec<FamilySummary>) -> Vec<u32> {
        FamilyOrder { by, descending }.sort(&mut rows);
        rows.iter().map(|r| r.id.0).collect()
    }

    #[test]
    fn versions_compare_digit_runs_as_numbers() {
        let rows = vec![
            row(1, "a", Some("Version 2.10")),
            row(2, "b", Some("Version 2.9")),
            row(3, "c", Some("version 10.0")),
            row(4, "d", None),
            row(5, "e", Some("  ")),
        ];
        assert_eq!(
            sorted(FamilySortKey::Version, false, rows.clone()),
            [2, 1, 3, 4, 5]
        );
        // Descending reverses the values only: rows without one stay last.
        assert_eq!(sorted(FamilySortKey::Version, true, rows), [3, 1, 2, 4, 5]);
    }

    #[test]
    fn ties_go_by_name_then_id_in_both_directions() {
        let rows = vec![
            row(3, "Beta", None),
            row(2, "alpha", None),
            row(1, "Alpha", None),
        ];
        assert_eq!(
            sorted(FamilySortKey::StyleCount, false, rows.clone()),
            [1, 2, 3]
        );
        assert_eq!(sorted(FamilySortKey::StyleCount, true, rows), [1, 2, 3]);
    }

    #[test]
    fn formats_sort_by_badge_then_variable_then_color() {
        let mut otf = row(1, "a", None);
        otf.formats = vec![FontFormat::Otf];
        let ttf = row(2, "b", None);
        let mut var = row(3, "c", None);
        var.is_variable = true;
        let mut none = row(4, "d", None);
        none.formats.clear();
        assert_eq!(
            sorted(FamilySortKey::Format, false, vec![none, var, ttf, otf]),
            [1, 2, 3, 4]
        );
    }

    fn copy(path: &str, version: Option<&str>, in_library_folder: bool) -> DuplicateCopy {
        DuplicateCopy {
            path: path.into(),
            size: 1,
            hash: String::new(),
            face: None,
            version: version.map(Into::into),
            source: SourceId(1),
            in_library_folder,
            keep: false,
        }
    }

    fn keeper_order(copies: Vec<DuplicateCopy>) -> Vec<String> {
        let mut group = DuplicateGroup {
            kind: DuplicateKind::Near,
            postscript_name: None,
            copies,
        };
        group.choose_keeper();
        assert!(group.copies[0].keep);
        assert_eq!(group.copies.iter().filter(|c| c.keep).count(), 1);
        group.copies.into_iter().map(|c| c.path).collect()
    }

    #[test]
    fn the_newest_version_is_the_keeper_even_outside_the_library_folder() {
        let order = keeper_order(vec![
            copy("/lib/a.ttf", Some("Version 2.9"), true),
            copy("/b.ttf", Some("Version 2.10"), false),
            copy("/c.ttf", None, true),
            copy("/d.ttf", Some(" "), true),
        ]);
        assert_eq!(order, ["/b.ttf", "/lib/a.ttf", "/c.ttf", "/d.ttf"]);
    }

    #[test]
    fn on_equal_versions_the_library_folder_copy_is_the_keeper_then_the_path() {
        let order = keeper_order(vec![
            copy("/z.ttf", Some("1.0"), false),
            copy("/y.ttf", Some("1.0"), true),
            copy("/a.ttf", Some("1.0"), false),
        ]);
        assert_eq!(order, ["/y.ttf", "/a.ttf", "/z.ttf"]);
    }

    #[test]
    fn a_marked_keeper_that_loses_is_unmarked() {
        let mut stale = copy("/b.ttf", Some("1.0"), false);
        stale.keep = true;
        let order = keeper_order(vec![stale, copy("/a.ttf", Some("2.0"), false)]);
        assert_eq!(order, ["/a.ttf", "/b.ttf"]);
    }

    #[test]
    fn tags_sort_case_insensitively_and_untagged_last() {
        let mut a = row(1, "a", None);
        a.tags = vec!["zeta".into()];
        let mut b = row(2, "b", None);
        b.tags = vec!["Alpha".into(), "beta".into()];
        let c = row(3, "c", None);
        assert_eq!(sorted(FamilySortKey::Tags, false, vec![c, a, b]), [2, 1, 3]);
    }
}
