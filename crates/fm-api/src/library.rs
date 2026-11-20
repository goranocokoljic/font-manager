//! Library service: sources, scanning, families, faces and problems.

use serde::{Deserialize, Serialize};

use crate::activation::ActivationMode;
use crate::common::{
    Axis, Classification, Embedding, FeatureInfo, FontFormat, Health, Metrics, NamedInstance, Page,
    Paged, Slant, Timestamp,
};
use crate::search::{DetectedTrait, WeightRange};
use crate::{
    ApiError, ApiResult, ApiVersion, CollectionId, ErrorCode, FaceId, FamilyId, JobId,
    ResolutionId, SidecarConflictId, SourceId,
};

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
    /// What works differently on this system, one sentence each (Settings →
    /// About → Platform notes). Empty when nothing does.
    #[serde(default)]
    pub platform_notes: Vec<String>,
    /// Whether font files can be dragged out of the window into another app
    /// (E44.4, ADR 0022), and why not when they cannot. Absent from an older
    /// build: not available.
    #[serde(default)]
    pub file_drag: FileDragSupport,
}

/// Whether this system can drag font files out of the window (E44.4).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileDragSupport {
    /// The drag works here.
    pub available: bool,
    /// Why it does not, one sentence for a tooltip; absent when it works.
    #[serde(default)]
    pub reason: Option<String>,
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
    /// When a scan of it last finished (a full scan or a watcher rescan);
    /// `None` before its first scan (E50.13).
    #[serde(default)]
    pub last_scanned: Option<Timestamp>,
    /// The folder is not there or is not a folder (removed, renamed, an
    /// unplugged drive); its faces stay until the source is removed (E50.13).
    #[serde(default)]
    pub missing: bool,
    /// The folder is inside a cloud sync client's folder (Dropbox, OneDrive,
    /// iCloud Drive, Google Drive, Box, pCloud), judged from the path (E50.13).
    #[serde(default)]
    pub cloud: bool,
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
    /// Only families with a face in this collection (a smart one applies
    /// its rule); an unknown collection is `NotFound`.
    #[serde(default)]
    pub collection: Option<CollectionId>,
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
        self.sort_by_family(rows, |f| f);
    }

    /// Sorts `rows`, each with a family row, in this order (the same one as [`Self::sort`]).
    pub fn sort_by_family<T>(&self, rows: &mut [T], family: impl Fn(&T) -> &FamilySummary) {
        rows.sort_by_cached_key(|row| {
            let f = family(row);
            (self.key(f), f.name.to_lowercase(), f.id.0)
        });
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

/// The format a path's extension names, without case: what
/// [`DuplicateGroup::choose_keeper`] knows of a copy's format, and what the
/// keeper policy's format rule ranks, so the two agree (E56.4).
pub fn format_from_extension(path: &str) -> Option<FontFormat> {
    let ext = std::path::Path::new(path)
        .extension()?
        .to_str()?
        .to_ascii_lowercase();
    Some(match ext.as_str() {
        "ttf" => FontFormat::Ttf,
        "otf" => FontFormat::Otf,
        "ttc" => FontFormat::Ttc,
        "otc" => FontFormat::Otc,
        "woff" => FontFormat::Woff,
        "woff2" => FontFormat::Woff2,
        "pfb" | "pfa" => FontFormat::Type1,
        "dfont" => FontFormat::Dfont,
        _ => return None,
    })
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
    /// The user's notes on the family (E13.5), not those of its faces.
    #[serde(default)]
    pub notes: Option<String>,
    /// How its active faces are active (E50.5): `Permanent` when any face is
    /// permanently active, else `Session` when any is active; absent when no
    /// face is active. The activation dot draws it.
    #[serde(default)]
    pub active_mode: Option<ActivationMode>,
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
    /// The face that stands for the family (E42.2): closest to weight 400,
    /// then upright, then normal width, then the lowest id.
    #[serde(default)]
    pub default_face: Option<FaceId>,
    /// The earliest year the copyright of a face states (E42.2).
    #[serde(default)]
    pub year: Option<u16>,
    /// The collections, manual and smart, that hold at least one face of the
    /// family now, in id order (E42.9: the family page's "Used in").
    #[serde(default)]
    pub collections: Vec<CollectionId>,
    /// The font files its faces are in; a collection file counts once
    /// (E42.9: the rail's "Format").
    #[serde(default)]
    pub file_count: u32,
    /// The size of those files in bytes, each counted once; `u32::MAX` when
    /// larger (E42.9).
    #[serde(default)]
    pub file_bytes: u32,
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
    /// Seats the user declared for the family (E15.2).
    #[serde(default)]
    pub seats: Option<u32>,
    /// Purchase date the user declared, `YYYY-MM-DD` (E15.2).
    #[serde(default)]
    pub purchase_date: Option<String>,
    /// Absolute path of the receipt copy the app keeps (E15.2).
    #[serde(default)]
    pub receipt: Option<String>,
    /// Whether the face may be used on the web, and why (E15.3).
    #[serde(default)]
    pub web_use: Option<WebUse>,
    /// What the license lets the user do (E42.12); absent when nothing is known.
    #[serde(default)]
    pub permissions: Option<LicensePermissions>,
}

/// What a license lets the user do, in plain words (E42.12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LicensePermissions {
    /// What the user may do.
    pub may: Vec<String>,
    /// What the user may not do.
    pub may_not: Vec<String>,
    /// Where the lines come from.
    pub source: LicensePermissionsSource,
}

/// Where the lines of [`LicensePermissions`] come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum LicensePermissionsSource {
    /// The open license the name table states.
    OpenLicense,
    /// The license type the user declared.
    Declared,
    /// The embedding permission of `OS/2.fsType`.
    EmbeddingBits,
}

/// Whether a face may be used on the web, decided from its embedding
/// permission, recognized open license and declared license type (E15.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WebUse {
    /// The face is OK for web.
    pub ok: bool,
    /// Why, in plain words ("Open license (OFL)").
    pub reason: String,
}

/// One `CPAL` palette of a `COLR` color face (E16.3). The webview applies it
/// with CSS `font-palette` (`base-palette: index`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    /// Index of the palette in the `CPAL` table (0 is the default).
    pub index: u16,
    /// The palette's name from the `name` table, if the font gives one.
    pub name: Option<String>,
    /// Its colors as CSS hex, in palette-entry order: `#rrggbb`, or
    /// `#rrggbbaa` when the color is not opaque.
    pub colors: Vec<String>,
    /// The font marks it for use on a light background.
    pub light_background: bool,
    /// The font marks it for use on a dark background.
    pub dark_background: bool,
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
    /// The user's notes on the face's family (E13.5); `notes` are the face's.
    #[serde(default)]
    pub family_notes: Option<String>,
    /// The `CPAL` palettes of a `COLR` color face (E16.3); empty for a face
    /// without them.
    #[serde(default)]
    pub palettes: Vec<Palette>,
    /// What the font says about its design (name ID 10), for the family
    /// page's Info section (E42.8).
    #[serde(default)]
    pub description: Option<String>,
    /// The tags of the tables in the font file, in directory order (E42.8).
    #[serde(default)]
    pub tables: Vec<String>,
    /// The glyph traits detected on the face with confidence, of the traits
    /// the catalog offers, in catalog order (E49.10: the Characters
    /// section's trait tags). Empty when the face lacks the glyphs or is not
    /// measured yet.
    #[serde(default)]
    pub glyph_traits: Vec<DetectedTrait>,
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
    /// A source folder that is not there (removed, renamed, an unplugged
    /// drive). Its faces stay in the index until the user removes the source.
    MissingSource,
    /// The library index file was damaged and a fresh one replaced it; the
    /// damaged file was kept aside and the sources are scanned again.
    IndexRebuilt,
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

/// The field of a sidecar conflict (E28.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SidecarField {
    /// The face is a favorite.
    Favorite,
    /// The face's rating.
    Rating,
    /// The face's notes.
    Notes,
    /// The family's notes.
    FamilyNotes,
    /// The family's declared license.
    FamilyLicense,
}

/// A field this machine and another machine that syncs the same folder set
/// to different values (E28.1). The index keeps this machine's value until
/// the user resolves it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SidecarConflict {
    /// Pass to `resolve_sidecar_conflict`.
    pub id: SidecarConflictId,
    /// The face; for a family field, the family's first face.
    pub face: FaceId,
    /// The face's family.
    pub family: String,
    /// Which field.
    pub field: SidecarField,
    /// This machine's value, in words ("4 stars", "a favorite").
    pub mine: String,
    /// The other machine's value, in words.
    pub theirs: String,
    /// The sidecar file the other value came from.
    pub sidecar_path: String,
}

/// Which value of a sidecar conflict to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ConflictSide {
    /// This machine's value.
    Mine,
    /// The other machine's value.
    Theirs,
}

/// Resolves one sidecar conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ResolveSidecarConflictRequest {
    /// The conflict.
    pub id: SidecarConflictId,
    /// The value to keep.
    pub keep: ConflictSide,
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
    /// Orders the copies keeper first and marks the keeper by the default
    /// [`KeeperPolicy`] with no language set (E56.4): the newest version
    /// (digit runs compared as numbers, no version last), then the format
    /// in the default order (by the file's extension), then a copy in the
    /// library folder, then the path A to Z. A service with an index
    /// applies the saved policy and the language guard instead, which gives
    /// this keeper while no language is set and the policy is the default.
    pub fn choose_keeper(&mut self) {
        let policy = KeeperPolicy::default();
        self.copies.sort_by_cached_key(|c| {
            let version = VersionKey::of(c.version.as_deref());
            (
                version.is_none(),
                std::cmp::Reverse(version),
                policy.format_rank(format_from_extension(&c.path)),
                !c.in_library_folder,
                c.path.clone(),
            )
        });
        for (i, copy) in self.copies.iter_mut().enumerate() {
            copy.keep = i == 0;
        }
    }

    /// The Problems entries of the group: one per copy not marked `keep`
    /// (a copy the language guard protects, or a system copy, is marked
    /// `keep` too and is no entry), with the keeper as `other`. Exact groups
    /// are `DuplicateFile`, near groups `DuplicateFace`. Every implementation
    /// of [`LibraryService::list_problems`] uses this, so the rule is one.
    pub fn problems(&self) -> Vec<Problem> {
        let Some(keeper) = self
            .copies
            .iter()
            .find(|c| c.keep)
            .or_else(|| self.copies.first())
        else {
            return Vec::new();
        };
        self.copies
            .iter()
            .filter(|c| !c.keep && !std::ptr::eq(*c, keeper))
            .map(|copy| {
                let (kind, message) = match self.kind {
                    DuplicateKind::Exact => (
                        ProblemKind::DuplicateFile,
                        format!("The same file is also at {}.", keeper.path),
                    ),
                    DuplicateKind::Near => {
                        let name = self.postscript_name.as_deref().unwrap_or("This font");
                        let version = copy
                            .version
                            .as_deref()
                            .map(str::trim)
                            .filter(|v| !v.is_empty())
                            .map(|v| format!(" ({v})"))
                            .unwrap_or_default();
                        (
                            ProblemKind::DuplicateFace,
                            format!("{name}{version} is also in {}.", keeper.path),
                        )
                    }
                };
                Problem {
                    kind,
                    face: copy.face,
                    path: copy.path.clone(),
                    other: Some(keeper.path.clone()),
                    message,
                }
            })
            .collect()
    }
}

/// How much the copies of a duplicate set are the same (E56.1, ADR 0027).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DuplicateLevel {
    /// The same file (the same hash) at different paths.
    Identical,
    /// The same PostScript name and the same version in files that differ.
    SameVersion,
    /// The same PostScript name in files whose versions differ.
    OtherVersion,
}

/// Which files a duplicate listing looks at.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum DuplicateScope {
    /// Every file the library shows.
    #[default]
    Library,
    /// The files of one source.
    Source {
        /// The source.
        source: SourceId,
    },
    /// The files in one folder and its subfolders.
    Folder {
        /// Absolute folder path.
        path: String,
    },
}

/// One page of duplicate sets ([`LibraryService::list_duplicate_sets`]).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ListDuplicateSetsRequest {
    /// The files to look at; the whole library by default. A set is listed
    /// when one of its copies is in the scope, with all its copies.
    #[serde(default)]
    pub scope: DuplicateScope,
    /// The window into the sets.
    #[serde(default)]
    pub page: Page,
}

/// One copy of a duplicate set: one file, for the face the set is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateSetCopy {
    /// Absolute file path.
    pub path: String,
    /// File size in bytes.
    pub size: u32,
    /// blake3 hash of the file, hex; equal hashes are identical files.
    pub hash: String,
    /// The face the set is about (the first face for an identical set),
    /// when the file parsed far enough to have one.
    pub face: Option<FaceId>,
    /// Version string (name ID 5) of that face.
    pub version: Option<String>,
    /// The file format, when the file is a font.
    pub format: Option<FontFormat>,
    /// The source the file was found in.
    pub source: SourceId,
    /// The source is the library folder.
    pub in_library_folder: bool,
    /// The file is in a system font folder: it is listed, and it can never
    /// be chosen for removal (CLAUDE.md rule 4).
    pub system: bool,
    /// The file is in the scope of the request.
    pub in_scope: bool,
    /// The other copies of this set that are the same file (the same hash).
    pub identical_with: Vec<String>,
    /// The ids of the other sets this file takes part in (a collection file
    /// with several faces): a plan must not trash it for one set while
    /// another keeps it.
    pub other_sets: Vec<String>,
    /// What a user weighs when choosing between the copies (E56.2), from
    /// the index; `None` when the copy has no face (data that is no font)
    /// or the service has no index to read them from (the mocks).
    #[serde(default)]
    pub facts: Option<CopyFacts>,
}

/// The facts of one copy of a duplicate set that a decision needs (E56.2),
/// as the index holds them for the copy's face and file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CopyFacts {
    /// When the file was last modified, when the scan read its time.
    pub modified: Option<Timestamp>,
    /// Glyphs in the font (`maxp`); `None` until a scan by this version
    /// reads the file again.
    pub glyph_count: Option<u32>,
    /// The face is a variable font.
    pub variable: bool,
    /// Its variation axes, by tag (`wght`), in the font's order.
    pub axes: Vec<String>,
    /// Its OpenType feature tags (GSUB and GPOS), sorted.
    pub features: Vec<String>,
    /// How the app activated it, when it did.
    pub activation: Option<ActivationMode>,
    /// The open documents that asked for it (`activation_document`), the
    /// last to ask first.
    pub activation_documents: Vec<String>,
    /// The user marked it a favorite.
    pub favorite: bool,
    /// The user's rating, 1 to 5.
    pub rating: Option<u8>,
    /// The user's tags on it, by name.
    pub tags: Vec<String>,
    /// The user wrote notes on it.
    pub has_notes: bool,
    /// The collections it is in, by name.
    pub collections: Vec<String>,
    /// The project sets it is in, by name.
    pub sets: Vec<String>,
    /// The language presets (`data/charsets` codes) it covers in full, from
    /// the index's coverage.
    pub languages_full: Vec<String>,
    /// The language presets it has `locl` forms for.
    pub languages_locl: Vec<String>,
}

/// Compare two copies of a duplicate set ([`LibraryService::compare_copies`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CompareCopiesRequest {
    /// The first copy's face ([`DuplicateSetCopy::face`]).
    pub a: FaceId,
    /// The second copy's face; the differences read from `a` to `b`.
    pub b: FaceId,
}

/// One side of a [`CopyComparison`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ComparedCopy {
    /// The face.
    pub face: FaceId,
    /// Absolute file path.
    pub path: String,
    /// The file format read now, when the file is a font.
    pub format: Option<FontFormat>,
    /// Whether the file could be read now: `Broken` with the reason when it
    /// could not (gone, cut off, malformed).
    pub health: Health,
}

/// Glyphs one copy has and the other does not.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GlyphChange {
    /// How many.
    pub count: u32,
    /// The first of them (at most 10), in Unicode order, then by glyph
    /// name: `ä (U+00E4)` for a character, the glyph name otherwise.
    pub first: Vec<String>,
}

/// What differs from one copy to the other (E56.2). Glyphs are matched by
/// the character they map, else by glyph name; metrics and advance widths
/// are compared per em.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CopyDifferences {
    /// Language presets the second copy covers in full and the first does not.
    pub languages_gained: Vec<String>,
    /// Language presets the first copy covers in full and the second does not.
    pub languages_lost: Vec<String>,
    /// Glyphs the second copy has and the first does not.
    pub glyphs_added: GlyphChange,
    /// Glyphs the first copy has and the second does not.
    pub glyphs_removed: GlyphChange,
    /// OpenType features only the second copy has.
    pub features_added: Vec<String>,
    /// OpenType features only the first copy has.
    pub features_removed: Vec<String>,
    /// Variation axes only the second copy has.
    pub axes_added: Vec<String>,
    /// Variation axes only the first copy has.
    pub axes_removed: Vec<String>,
    /// The vertical metrics (`hhea`, `OS/2` typographic and Windows) differ.
    pub vertical_metrics_changed: bool,
    /// The units per em differ.
    pub units_per_em_changed: bool,
    /// Glyphs both copies have whose advance width differs.
    pub advance_widths_changed: u32,
    /// Text set in one copy may reflow in the other: the vertical metrics
    /// or advance widths changed.
    pub may_reflow: bool,
    /// Nothing but the container differs: the same tables and outlines, in
    /// another format (TTF or OTF and WOFF or WOFF2, a collection member).
    pub container_only: bool,
    /// The font tables whose data differs, by tag; empty when
    /// `container_only` or the files are the same.
    pub tables_changed: Vec<String>,
}

/// Two copies of a duplicate set side by side (E56.2): read from their files
/// on demand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CopyComparison {
    /// The first copy.
    pub a: ComparedCopy,
    /// The second copy.
    pub b: ComparedCopy,
    /// What differs; `None` when a side could not be read (its `health`).
    pub differences: Option<CopyDifferences>,
    /// The answer in sentences a user reads, most important first.
    pub summary: Vec<String>,
}

/// Files that hold the same font, at one [`DuplicateLevel`] (E56.1). The
/// set itself calls no copy older or newer; which copy to keep is the keeper
/// policy's answer, [`Self::recommendation`] (E56.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateSet {
    /// Stable id: `identical:<hash>` or `name:<PostScript name>`.
    pub id: String,
    /// How much the copies are the same.
    pub level: DuplicateLevel,
    /// The PostScript name the copies share (for an identical set, the first
    /// face's), when there is one.
    pub postscript_name: Option<String>,
    /// The copies are not all of one file format.
    pub formats_differ: bool,
    /// The copies, two or more, by path.
    pub copies: Vec<DuplicateSetCopy>,
    /// What the saved keeper policy and the language guard recommend for
    /// this set (E56.4); `None` from a service without an index to read the
    /// copies' facts from (the mocks).
    #[serde(default)]
    pub recommendation: Option<KeeperRecommendation>,
}

/// One rule of the keeper policy (E56.4, ADR 0027). A copy that wins a rule
/// wins the set; a tie goes to the next rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum KeeperRule {
    /// Covers more of my languages in full.
    MyLanguages,
    /// Has `locl` forms for more of my languages.
    MyLanguagesLocl,
    /// The newest version (digit runs as numbers; no version last).
    NewestVersion,
    /// The format that stands first in [`KeeperPolicy::format_order`].
    Format,
    /// A variable font over a static one.
    Variable,
    /// The most glyphs.
    MostGlyphs,
    /// In the folder that stands first in [`KeeperPolicy::preferred_folders`].
    PreferredFolder,
    /// In the library folder.
    LibraryFolder,
    /// Carries more kinds of the user's metadata (favorite, rating, tags,
    /// notes, collections, sets).
    UserMetadata,
    /// Active, or asked for by an open document.
    ActiveOrRequested,
    /// The path A to Z. Always the last rule, so the answer never depends
    /// on chance.
    Path,
}

/// The keeper policy (E56.4, ADR 0027): an ordered list of rules, stored in
/// `config.json` as [`crate::settings::Settings::keeper_policy`]. Values a
/// newer version wrote and this one does not know are dropped on read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KeeperPolicy {
    /// The rules, most important first; [`KeeperRule::Path`] always ends the
    /// list ([`Self::normalized`]).
    #[serde(
        default = "KeeperPolicy::default_rules",
        deserialize_with = "known_values"
    )]
    #[specta(type = Vec<KeeperRule>)]
    pub rules: Vec<KeeperRule>,
    /// The order of formats for [`KeeperRule::Format`], best first; a format
    /// not listed ranks after every listed one.
    #[serde(
        default = "KeeperPolicy::default_format_order",
        deserialize_with = "known_values"
    )]
    #[specta(type = Vec<FontFormat>)]
    pub format_order: Vec<FontFormat>,
    /// The folders for [`KeeperRule::PreferredFolder`], best first, as
    /// absolute paths.
    #[serde(default)]
    pub preferred_folders: Vec<String>,
}

impl Default for KeeperPolicy {
    /// Covers my languages, newest version, format, the library folder,
    /// path; formats OpenType CFF, TrueType, WOFF2, WOFF, Type 1.
    fn default() -> Self {
        Self {
            rules: Self::default_rules(),
            format_order: Self::default_format_order(),
            preferred_folders: Vec::new(),
        }
    }
}

impl KeeperPolicy {
    fn default_rules() -> Vec<KeeperRule> {
        vec![
            KeeperRule::MyLanguages,
            KeeperRule::NewestVersion,
            KeeperRule::Format,
            KeeperRule::LibraryFolder,
            KeeperRule::Path,
        ]
    }

    fn default_format_order() -> Vec<FontFormat> {
        vec![
            FontFormat::Otf,
            FontFormat::Ttf,
            FontFormat::Woff2,
            FontFormat::Woff,
            FontFormat::Type1,
        ]
    }

    /// This policy as every rule reads it: each rule, format and folder once
    /// where it first stands, blank folders left out, and
    /// [`KeeperRule::Path`] moved to the end. Every implementation stores and
    /// applies the normalized policy, so the rule is one.
    pub fn normalized(&self) -> Self {
        let mut rules: Vec<KeeperRule> = Vec::with_capacity(self.rules.len() + 1);
        for &rule in &self.rules {
            if rule != KeeperRule::Path && !rules.contains(&rule) {
                rules.push(rule);
            }
        }
        rules.push(KeeperRule::Path);
        let mut format_order: Vec<FontFormat> = Vec::with_capacity(self.format_order.len());
        for &f in &self.format_order {
            if !format_order.contains(&f) {
                format_order.push(f);
            }
        }
        let mut preferred_folders: Vec<String> = Vec::new();
        for folder in &self.preferred_folders {
            let folder = folder.trim();
            if !folder.is_empty() && !preferred_folders.iter().any(|f| f == folder) {
                preferred_folders.push(folder.to_owned());
            }
        }
        Self {
            rules,
            format_order,
            preferred_folders,
        }
    }

    /// Where `format` stands in [`Self::format_order`]: 0 is best; a format
    /// not listed (and no format) is `format_order.len()`.
    pub fn format_rank(&self, format: Option<FontFormat>) -> usize {
        format
            .and_then(|f| self.format_order.iter().position(|&o| o == f))
            .unwrap_or(self.format_order.len())
    }

    /// `InvalidRequest` when a preferred folder is not an absolute path.
    pub fn check(&self) -> ApiResult<()> {
        if let Some(folder) = self
            .preferred_folders
            .iter()
            .map(|f| f.trim())
            .find(|f| !f.is_empty() && !std::path::Path::new(f).has_root())
        {
            return Err(
                ApiError::invalid("Choose each preferred folder with its full path.")
                    .with_detail(format!("preferredFolders {folder:?}")),
            );
        }
        Ok(())
    }
}

/// A list of values, each one this version does not know dropped (a file
/// from a newer version), so the rest of the file still reads.
fn known_values<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Maybe<T> {
        Known(T),
        Other(serde::de::IgnoredAny),
    }
    let items: Vec<Maybe<T>> = Vec::deserialize(d)?;
    Ok(items
        .into_iter()
        .filter_map(|m| match m {
            Maybe::Known(t) => Some(t),
            Maybe::Other(_) => None,
        })
        .collect())
}

/// The ordering key of a version string for [`KeeperRule::NewestVersion`]
/// and [`DuplicateGroup::choose_keeper`]: trimmed, digit runs as numbers,
/// other text without case; `None` for no version. A greater key is newer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct VersionKey(Vec<NaturalPart>);

impl VersionKey {
    /// The key of `version`, or `None` when it is missing or blank.
    pub fn of(version: Option<&str>) -> Option<Self> {
        version
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| Self(natural(v)))
    }
}

/// What the keeper policy answers for a set (E56.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum KeeperOutcome {
    /// One copy is the keeper; the others are recommended for removal
    /// unless something protects them ([`KeptCopy::because`]).
    Keeper,
    /// No copy covers every one of my languages the others cover: keep the
    /// copies that together cover them, each with the languages it alone has.
    KeepBoth,
    /// Every copy is in a system font folder: the policy does not apply.
    AllSystem,
}

/// Why a copy is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum KeptBecause {
    /// The policy's rules chose it.
    Policy,
    /// The language guard: it covers one of my languages in full that the
    /// other kept copies do not.
    Languages,
    /// It is in a system font folder; the policy never applies to it.
    System,
    /// Its file holds a font no other file holds (a collection file).
    LastCopy,
}

/// The rule that decided between two copies and the values it compared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KeeperReason {
    /// The rule.
    pub rule: KeeperRule,
    /// The kept copy's value(s): language preset ids, a version, a format,
    /// a glyph count, a folder, the kinds of metadata, a path.
    pub kept: Vec<String>,
    /// The other copy's value(s), of the same kind.
    pub other: Vec<String>,
    /// The other copy.
    pub other_path: String,
    /// The reason in one sentence, the same in the UI, the CLI and MCP:
    /// "Kept: covers Serbian Cyrillic, the other copy does not."
    pub sentence: String,
}

/// One copy the recommendation keeps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KeptCopy {
    /// Absolute file path.
    pub path: String,
    /// Why.
    pub because: KeptBecause,
    /// My languages (preset ids) it covers in full that no other kept copy
    /// covers.
    pub languages_only_here: Vec<String>,
    /// For [`KeptBecause::LastCopy`]: the fonts (PostScript names) only its
    /// file holds.
    pub fonts_only_here: Vec<String>,
    /// Why, in one sentence.
    pub sentence: String,
}

/// One copy the recommendation would remove, with the rule it lost by
/// against the keeper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemovedCopy {
    /// Absolute file path.
    pub path: String,
    /// The rule that put the keeper before it.
    pub reason: KeeperReason,
}

/// What the keeper policy and the language guard recommend for one set
/// (E56.4). Every copy of the set is in exactly one of `keep` and `remove`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KeeperRecommendation {
    /// Keeper, keep both, or every copy a system copy.
    pub outcome: KeeperOutcome,
    /// The copy the rules put first; `None` when every copy is a system copy.
    pub keeper: Option<String>,
    /// Why the keeper stands before the next copy the rules rank; `None`
    /// when no other copy is ranked.
    pub reason: Option<KeeperReason>,
    /// The copies kept, the keeper first.
    pub keep: Vec<KeptCopy>,
    /// The copies recommended for removal.
    pub remove: Vec<RemovedCopy>,
    /// Whether my languages were set; `NotSet` means the language rules and
    /// the guard did nothing.
    pub my_languages: crate::settings::MyLanguagesStatus,
}

/// Preview a keeper policy without saving it
/// ([`LibraryService::preview_keeper_policy`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PreviewKeeperPolicyRequest {
    /// The sets to look at, as [`ListDuplicateSetsRequest::scope`].
    #[serde(default)]
    pub scope: DuplicateScope,
    /// The policy to try.
    pub policy: KeeperPolicy,
}

/// What a keeper policy would do over a scope, against the saved one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KeeperPolicyPreview {
    /// The duplicate sets in the scope.
    pub sets: u32,
    /// Sets that get one keeper ([`KeeperOutcome::Keeper`]).
    pub with_keeper: u32,
    /// Sets answered "keep both" ([`KeeperOutcome::KeepBoth`]).
    pub keep_both: u32,
    /// Sets whose recommendation (the keeper, or the copies kept) differs
    /// from the saved policy's.
    pub changed: u32,
    /// Whether my languages were set.
    pub my_languages: crate::settings::MyLanguagesStatus,
}

/// "Keep this one": keep one copy of a duplicate font and move the other
/// copies to the OS trash (E8.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KeepCopyRequest {
    /// The copy to keep ([`DuplicateCopy::path`]).
    pub keep: String,
    /// The copies to move to the trash; each must be listed with `keep` in a
    /// group of [`LibraryService::list_duplicates`].
    pub trash: Vec<String>,
    /// Only say what would move; touch nothing.
    #[serde(default)]
    pub dry_run: bool,
}

impl KeepCopyRequest {
    /// Checks the request against the duplicate groups `groups`: something
    /// to keep and something to trash, no path twice, and every copy to
    /// trash listed with `keep` in one group. So only a duplicate of a copy
    /// that stays can ever be moved. Every implementation of
    /// [`LibraryService::keep_copy`] uses this, so the rule is one.
    pub fn check(&self, groups: &[DuplicateGroup]) -> ApiResult<()> {
        let invalid = |m: String| Err(ApiError::new(ErrorCode::InvalidRequest, m));
        if self.keep.trim().is_empty() {
            return invalid("Choose the copy to keep.".to_owned());
        }
        if self.trash.is_empty() {
            return invalid("Choose at least one copy to move to the trash.".to_owned());
        }
        let mut seen = std::collections::HashSet::new();
        for path in &self.trash {
            if *path == self.keep {
                return invalid("The copy to keep cannot also move to the trash.".to_owned());
            }
            if !seen.insert(path) {
                return invalid(format!("{path} is listed twice."));
            }
        }
        let with_keep: Vec<&DuplicateGroup> = groups
            .iter()
            .filter(|g| g.copies.iter().any(|c| c.path == self.keep))
            .collect();
        if with_keep.is_empty() {
            return invalid(format!("{} is not a duplicate any more.", self.keep));
        }
        for path in &self.trash {
            if !with_keep
                .iter()
                .any(|g| g.copies.iter().any(|c| c.path == *path))
            {
                return invalid(format!("{path} is not a copy of the font you keep."));
            }
        }
        Ok(())
    }
}

/// What happened, or would happen, to one copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum TrashMoveStatus {
    /// Dry run: it would move to the trash.
    Planned,
    /// It is in the trash.
    Trashed,
    /// Undo put it back.
    Restored,
    /// It was not moved and stays where it is (`message` says why).
    Skipped,
    /// The move or the undo failed (`message` says why).
    Failed,
}

/// One copy of a [`Resolution`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TrashMove {
    /// The copy's path in the library.
    pub path: String,
    /// What happened to it.
    pub status: TrashMoveStatus,
    /// Why it was skipped or failed, in one sentence.
    pub message: Option<String>,
}

/// A "Keep this one" action: the copy kept and the copies moved to the
/// trash. Listed in the Problems history until undone or forgotten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    /// The action, for [`LibraryService::undo_resolution`]; `None` for a dry
    /// run or when nothing moved.
    pub id: Option<ResolutionId>,
    /// The copy kept.
    pub kept: String,
    /// When it happened.
    pub at: Timestamp,
    /// It was a dry run: nothing moved.
    pub dry_run: bool,
    /// Every moved copy was put back.
    pub undone: bool,
    /// The copies, in the order of the request.
    pub moves: Vec<TrashMove>,
}

/// One page of a face's glyph map (E11.1): its mapped characters by Unicode
/// block, optionally one block only or the ones a search finds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GlyphsRequest {
    /// The face.
    pub face: FaceId,
    /// Only this block (its name, as [`GlyphBlock::name`]); all when absent.
    #[serde(default)]
    pub block: Option<String>,
    /// Search: the character itself (`Ж`), a code point (`U+0416`, `0416`)
    /// or a glyph name (`uni0416`, `Zhe-cy`). All characters when absent or
    /// blank.
    #[serde(default)]
    pub query: Option<String>,
    /// The window into the list; `limit` is capped at 500.
    #[serde(default)]
    pub page: Page,
}

/// One mapped character of a face.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GlyphInfo {
    /// The Unicode code point.
    pub code_point: u32,
    /// The glyph `cmap` maps it to.
    pub glyph_id: u32,
    /// The glyph name from `post` or the CFF charset; absent when the font
    /// has none.
    pub name: Option<String>,
    /// The Unicode block it is in ([`GlyphBlock::name`]); "No block" for a
    /// code point outside every block.
    pub block: String,
}

/// A Unicode block and how many of the listed characters it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GlyphBlock {
    /// The block name from the Unicode block table (`Cyrillic`).
    pub name: String,
    /// Its first code point.
    pub first: u32,
    /// Its last code point.
    pub last: u32,
    /// Characters of the whole list (all pages) in this block.
    pub count: u32,
}

/// One page of a glyph map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GlyphPage {
    /// The characters in this page, by code point.
    pub items: Vec<GlyphInfo>,
    /// Characters in the whole list.
    pub total: u32,
    /// The blocks of the whole list, in code-point order, each with its
    /// count: the list is these blocks one after another.
    pub blocks: Vec<GlyphBlock>,
    /// `Broken` with a reason when the font's tables are malformed; the
    /// list is empty then.
    pub health: Health,
    /// The Unicode version of the block table (`17.0.0`).
    pub unicode_version: String,
}

/// One character of a face, for its glyph detail (E11.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GlyphRequest {
    /// The face.
    pub face: FaceId,
    /// The Unicode code point of the character.
    pub code_point: u32,
}

/// How many styles of a face's family map a character (E42.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GlyphStyles {
    /// The styles that map it.
    pub have: u32,
    /// All styles of the family: a variable face counts as its named
    /// instances (at least one), a static face as one.
    pub total: u32,
}

/// The detail of one mapped character of a face.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GlyphDetail {
    /// The Unicode code point.
    pub code_point: u32,
    /// The glyph `cmap` maps it to.
    pub glyph_id: u32,
    /// The glyph name from `post` or the CFF charset; absent when the font
    /// has none.
    pub name: Option<String>,
    /// The advance width in font units, at the default location of a
    /// variable face; absent when the font has no horizontal metrics.
    pub advance_width: Option<u32>,
    /// GSUB features that produce this glyph from other glyphs (`liga` for
    /// `ﬁ`), by tag.
    pub produced_by: Vec<FeatureInfo>,
    /// GSUB features that replace this glyph with another (`smcp`, `ss01`),
    /// by tag.
    pub replaced_by: Vec<FeatureInfo>,
    /// `Broken` with a reason when the font's tables are malformed; the
    /// glyph id is 0 and the rest is empty then. Malformed GSUB lookups only
    /// empty the feature lists.
    pub health: Health,
    /// The left side bearing in font units at the default instance (E50.8):
    /// `bounds.x_min`, or the `hmtx` bearing of a glyph with no outline.
    /// Absent when the font has no horizontal metrics or the outline cannot
    /// be read.
    #[serde(default)]
    pub left_side_bearing: Option<i32>,
    /// The right side bearing in font units at the default instance:
    /// the advance minus `bounds.x_max`, or minus the left side bearing of a
    /// glyph with no outline. Absent as the left side bearing is.
    #[serde(default)]
    pub right_side_bearing: Option<i32>,
    /// The ink box of the outline in font units at the default instance;
    /// absent for a glyph with no outline (a space) or one that cannot be
    /// read.
    #[serde(default)]
    pub bounds: Option<GlyphBounds>,
    /// The Unicode name of the character (`LATIN SMALL LETTER G`, E50.9),
    /// from the Unicode Character Database; absent for a control, a
    /// surrogate, private use or a code point Unicode has not assigned.
    #[serde(default)]
    pub unicode_name: Option<String>,
}

/// The ink box of a glyph's outline in font units (E50.8): the tight box of
/// its curves, rounded outwards to whole units. Font y grows up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GlyphBounds {
    /// Left edge.
    pub x_min: i32,
    /// Bottom edge.
    pub y_min: i32,
    /// Right edge.
    pub x_max: i32,
    /// Top edge.
    pub y_max: i32,
}

/// A `<font>` argument to look up (E26.1): a face id or a name.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ResolveFontRequest {
    /// A face id (`42`) or a name: a PostScript name, a full name, or a
    /// family with or without its style.
    pub font: String,
}

/// Which name a font argument matched, in the order they are tried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FontMatchKind {
    /// The argument is the id of a face in the library.
    Id,
    /// The PostScript name (name ID 6).
    PostscriptName,
    /// The full name (name ID 4).
    FullName,
    /// The family (name ID 1 or 16), alone or followed by the style (name
    /// ID 2 or 17).
    FamilyStyle,
}

/// One face a font argument matched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FontMatch {
    /// The face.
    pub face: FaceId,
    /// Its family as the library shows it.
    pub family: String,
    /// Its style as the library shows it.
    pub style: String,
    /// Its PostScript name; empty when the font has none.
    pub postscript_name: String,
    /// Its full name, when the font has one and was scanned since E26.1.
    pub full_name: Option<String>,
}

/// The faces a font argument names: every match of the first kind of name
/// that matched, by face id. Never empty; more than one is ambiguous.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FontMatches {
    /// The kind of name that matched.
    pub matched_by: FontMatchKind,
    /// The faces it matched.
    pub faces: Vec<FontMatch>,
}

/// A face's names as [`ResolveFontRequest::resolve`] compares them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontNames {
    /// The face, as a match returns it.
    pub face: FontMatch,
    /// The family and style pairs the face is known by: name IDs 1 and 2,
    /// and 16 and 17 when the font has them.
    pub families: Vec<(String, String)>,
}

impl ResolveFontRequest {
    /// The faces among `faces` this argument names: the face whose id it
    /// is, else every face whose PostScript name, else full name, else
    /// family (alone or followed by the style) equals it, ignoring case and
    /// surrounding space. `NotFound` when none does; `InvalidRequest` for an
    /// empty argument.
    pub fn resolve(&self, faces: impl IntoIterator<Item = FontNames>) -> ApiResult<FontMatches> {
        let font = self.font.trim();
        if font.is_empty() {
            return Err(ApiError::new(
                ErrorCode::InvalidRequest,
                "Name a font: a face id or a font name.",
            ));
        }
        let faces: Vec<FontNames> = faces.into_iter().collect();
        let wanted = font.to_lowercase();
        let same = |name: &str| name.trim().to_lowercase() == wanted;
        let id = font.parse::<u32>().ok().map(FaceId);
        let matches = |kind: FontMatchKind, f: &FontNames| match kind {
            FontMatchKind::Id => Some(f.face.face) == id,
            FontMatchKind::PostscriptName => {
                !f.face.postscript_name.is_empty() && same(&f.face.postscript_name)
            }
            FontMatchKind::FullName => f.face.full_name.as_deref().is_some_and(same),
            FontMatchKind::FamilyStyle => f.families.iter().any(|(family, style)| {
                !family.trim().is_empty()
                    && (same(family) || same(&format!("{} {}", family.trim(), style.trim())))
            }),
        };
        let tiers = [
            FontMatchKind::Id,
            FontMatchKind::PostscriptName,
            FontMatchKind::FullName,
            FontMatchKind::FamilyStyle,
        ];
        for kind in tiers {
            let mut hits: Vec<FontMatch> = faces
                .iter()
                .filter(|f| matches(kind, f))
                .map(|f| f.face.clone())
                .collect();
            if !hits.is_empty() {
                hits.sort_by_key(|f| f.face);
                hits.dedup_by_key(|f| f.face);
                return Ok(FontMatches {
                    matched_by: kind,
                    faces: hits,
                });
            }
        }
        Err(not_called(font))
    }
}

/// No face in the library is called `font`.
fn not_called(font: &str) -> ApiError {
    ApiError::new(
        ErrorCode::NotFound,
        format!("No font in the library is called \"{font}\"."),
    )
}

impl FontMatches {
    /// The one face `font` names; `InvalidRequest` listing the candidates,
    /// one `id<TAB>PostScript name<TAB>family style` line each in the
    /// detail, when it names more than one.
    pub fn single(self, font: &str) -> ApiResult<FontMatch> {
        let count = self.faces.len();
        let mut faces = self.faces.into_iter();
        match (faces.next(), count) {
            (Some(face), 1) => Ok(face),
            (Some(first), _) => {
                let lines: Vec<String> = std::iter::once(first)
                    .chain(faces)
                    .map(|f| {
                        format!(
                            "{}	{}	{} {}",
                            f.face.0, f.postscript_name, f.family, f.style
                        )
                    })
                    .collect();
                Err(ApiError::new(
                    ErrorCode::InvalidRequest,
                    format!("\"{font}\" matches {count} fonts. Use one of their ids instead."),
                )
                .with_detail(lines.join(
                    "
",
                )))
            }
            (None, _) => Err(not_called(font)),
        }
    }
}

/// What a font drag to another app carries (E34.1): enough for a design
/// app's "Font Switch" drop to pick the face, by name or by file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FontSwitchPayload {
    /// PostScript name; design apps apply a font by it.
    pub postscript_name: String,
    /// Typographic family name.
    pub family: String,
    /// Style name.
    pub style: String,
    /// The font file as a `file://` URL (RFC 8089), percent-encoded UTF-8.
    pub file_url: String,
}

/// The fonts a file drag out of the window carries (E44.4): faces and whole
/// families, as a selection holds them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DragFilesRequest {
    /// Faces.
    #[serde(default)]
    pub faces: Vec<FaceId>,
    /// Every face of these families.
    #[serde(default)]
    pub families: Vec<FamilyId>,
}

/// The font files a file drag carries, where the user keeps them; never a
/// copy (rule 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DragFiles {
    /// Absolute paths, each once, in the order of the request's faces.
    pub paths: Vec<String>,
    /// Files of the request's faces that are not on disk any more; left out.
    pub missing: u32,
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
    /// One page of families. A `collection` that does not exist is
    /// `NotFound`.
    fn list_families(&self, req: ListFamiliesRequest) -> ApiResult<Paged<FamilySummary>>;
    /// One family with its faces.
    fn get_family(&self, family: FamilyId) -> ApiResult<Family>;
    /// Full detail of one face.
    fn get_face(&self, face: FaceId) -> ApiResult<FaceDetail>;
    /// Duplicates, broken files and conflicts.
    fn list_problems(&self) -> ApiResult<Vec<Problem>>;
    /// Exact duplicates (same hash) and near duplicates (same PostScript name
    /// and version in files that differ) among the files the library shows,
    /// in groups, each with its recommended keeper first. The keeper comes
    /// from the saved keeper policy and the language guard (E56.4): copies
    /// the guard protects and system copies are marked `keep` as well.
    fn list_duplicates(&self) -> ApiResult<Vec<DuplicateGroup>>;
    /// One page of duplicate sets (E56.1, ADR 0027) among the files the
    /// library shows: identical files, the same version in files that
    /// differ, and other versions of one PostScript name; a file is listed
    /// once per font it duplicates. Sets by PostScript name, copies by path.
    /// Computed on each call, never at start. A `source` scope that does not
    /// exist is `NotFound`.
    fn list_duplicate_sets(&self, req: ListDuplicateSetsRequest) -> ApiResult<Paged<DuplicateSet>>;
    /// What differs between two copies of a duplicate set (E56.2): reads
    /// both files on demand. A file that cannot be read gives
    /// `Health::Broken` for its side and no differences, never an error.
    /// `NotFound` when a face is not in the library; `InvalidRequest` when
    /// `a` and `b` are the same face.
    fn compare_copies(&self, req: CompareCopiesRequest) -> ApiResult<CopyComparison>;
    /// What a keeper policy would do over a scope, without saving it
    /// (E56.4): how many sets get a keeper, how many are "keep both", and how
    /// many recommendations change against the saved policy. A `source`
    /// scope that does not exist is `NotFound`; a preferred folder that is
    /// not absolute is `InvalidRequest`.
    fn preview_keeper_policy(
        &self,
        req: PreviewKeeperPolicyRequest,
    ) -> ApiResult<KeeperPolicyPreview>;
    /// "Keep this one" (E8.3): moves the other copies to the OS trash, never
    /// deleting them, and records the action for undo. With `dry_run`, only
    /// says what would move. A copy that cannot move (the app activated it,
    /// it is in a system font folder, its drive has no trash) is `Skipped`
    /// and stays where it is.
    /// `InvalidRequest` when a copy is not a duplicate of `keep`;
    /// `NotFound` when the copy to keep is not there.
    fn keep_copy(&self, req: KeepCopyRequest) -> ApiResult<Resolution>;
    /// The "Keep this one" actions, newest first, for the Problems history.
    fn list_resolutions(&self) -> ApiResult<Vec<Resolution>>;
    /// Puts the copies an action moved back from the trash. A copy that
    /// cannot come back (another file is at its path, the trash was emptied)
    /// is `Failed` and the others still come back; calling again retries it.
    fn undo_resolution(&self, resolution: ResolutionId) -> ApiResult<Resolution>;
    /// The sidecar conflicts that wait for the user (E28.1): fields this
    /// machine and another set to different values, oldest first.
    fn list_sidecar_conflicts(&self) -> ApiResult<Vec<SidecarConflict>>;
    /// Keeps one value of a conflict: it is written to the index and to the
    /// sidecar with a new version, so the other machine takes it without a
    /// new conflict. `NotFound` for a conflict that is not listed (any more).
    fn resolve_sidecar_conflict(&self, req: ResolveSidecarConflictRequest) -> ApiResult<()>;
    /// One page of a face's glyph map (E11.1): the characters its `cmap`
    /// maps, by code point and grouped by Unicode block, with glyph ids and
    /// names. `NotFound` for an unknown face; a malformed font is a page with
    /// `Broken` health, never an error.
    fn list_glyphs(&self, req: GlyphsRequest) -> ApiResult<GlyphPage>;
    /// One character of a face in detail (E11.2): glyph id and name, advance
    /// width and the GSUB features that produce or replace its glyph.
    /// `NotFound` for an unknown face or a character the face does not map;
    /// a malformed font is a detail with `Broken` health, never an error.
    fn get_glyph(&self, req: GlyphRequest) -> ApiResult<GlyphDetail>;
    /// How many styles of the family of `req.face` map `req.code_point`
    /// (E42.5), from each face's stored `cmap`, so a space or a zero-width
    /// character counts like any other. A face whose stored `cmap` cannot be
    /// read counts as not mapping it. `NotFound` for an unknown face.
    fn glyph_styles(&self, req: GlyphRequest) -> ApiResult<GlyphStyles>;
    /// The faces a `<font>` argument names (E26.1): a face id, else every
    /// face whose PostScript name, else full name, else family (alone or
    /// with the style) equals it, ignoring case. Only faces the library
    /// shows match. `NotFound` when none does; `InvalidRequest` for an
    /// empty argument.
    fn resolve_font(&self, req: ResolveFontRequest) -> ApiResult<FontMatches>;
    /// What dragging `face` to another app carries (E34.1): its PostScript
    /// name, family, style and file URL. `NotFound` for an unknown face.
    fn font_switch_payload(&self, face: FaceId) -> ApiResult<FontSwitchPayload>;
    /// The font files of `req`'s faces and families for a file drag out of
    /// the window (E44.4): each file once (a collection file holds several
    /// faces), files no longer on disk counted in `missing`. An unknown face
    /// or family is `NotFound`.
    fn drag_files(&self, req: DragFilesRequest) -> ApiResult<DragFiles>;
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
            notes: None,
            active_mode: None,
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
    fn on_equal_versions_the_format_comes_before_the_library_folder() {
        let order = keeper_order(vec![
            copy("/lib/a.woff2", Some("1.0"), true),
            copy("/z.TTF", Some("1.0"), false),
            copy("/b.otf", Some("1.0"), false),
            copy("/c.dat", Some("1.0"), true),
        ]);
        assert_eq!(order, ["/b.otf", "/z.TTF", "/lib/a.woff2", "/c.dat"]);
    }

    #[test]
    fn a_policy_is_normalized_path_last_and_each_value_once() {
        let policy = KeeperPolicy {
            rules: vec![
                KeeperRule::Path,
                KeeperRule::Format,
                KeeperRule::Format,
                KeeperRule::Variable,
            ],
            format_order: vec![FontFormat::Ttf, FontFormat::Ttf, FontFormat::Otf],
            preferred_folders: vec![" /a ".into(), "".into(), "/a".into(), "/b".into()],
        }
        .normalized();
        assert_eq!(
            policy.rules,
            [KeeperRule::Format, KeeperRule::Variable, KeeperRule::Path]
        );
        assert_eq!(policy.format_order, [FontFormat::Ttf, FontFormat::Otf]);
        assert_eq!(policy.preferred_folders, ["/a", "/b"]);
        assert_eq!(policy.format_rank(Some(FontFormat::Otf)), 1);
        assert_eq!(policy.format_rank(Some(FontFormat::Woff)), 2);
        assert_eq!(policy.format_rank(None), 2);
    }

    #[test]
    fn a_stored_policy_drops_the_values_this_version_does_not_know() {
        let policy: KeeperPolicy = serde_json::from_value(serde_json::json!({
            "rules": ["newestVersion", "aRuleFromLater", "path"],
            "formatOrder": ["woff2", "svg", "ttf"],
        }))
        .expect("reads");
        assert_eq!(policy.rules, [KeeperRule::NewestVersion, KeeperRule::Path]);
        assert_eq!(policy.format_order, [FontFormat::Woff2, FontFormat::Ttf]);
        assert!(policy.preferred_folders.is_empty());
        let empty: KeeperPolicy = serde_json::from_value(serde_json::json!({})).expect("reads");
        assert_eq!(empty, KeeperPolicy::default());
        assert_eq!(
            KeeperPolicy::default().rules,
            [
                KeeperRule::MyLanguages,
                KeeperRule::NewestVersion,
                KeeperRule::Format,
                KeeperRule::LibraryFolder,
                KeeperRule::Path
            ]
        );
    }

    #[test]
    fn a_relative_preferred_folder_is_refused() {
        let policy = KeeperPolicy {
            preferred_folders: vec!["fonts".into()],
            ..KeeperPolicy::default()
        };
        assert_eq!(
            policy.check().expect_err("relative").code,
            ErrorCode::InvalidRequest
        );
        assert!(KeeperPolicy::default().check().is_ok());
    }

    #[test]
    fn every_copy_marked_keep_is_no_problem() {
        let mut group = DuplicateGroup {
            kind: DuplicateKind::Near,
            postscript_name: Some("Inter-Regular".into()),
            copies: vec![
                copy("/a.ttf", Some("1.0"), false),
                copy("/b.ttf", Some("1.0"), false),
                copy("/c.ttf", Some("1.0"), false),
            ],
        };
        group.copies[0].keep = true;
        group.copies[1].keep = true;
        let problems: Vec<String> = group.problems().into_iter().map(|p| p.path).collect();
        assert_eq!(problems, ["/c.ttf"]);
        group.copies[2].keep = true;
        assert!(group.problems().is_empty());
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

    #[test]
    fn every_copy_but_the_keeper_is_a_duplicate_problem_pointing_at_the_keeper() {
        let mut exact = DuplicateGroup {
            kind: DuplicateKind::Exact,
            postscript_name: Some("Inter-Regular".into()),
            copies: vec![
                copy("/z.ttf", Some("1.0"), false),
                copy("/lib/a.ttf", Some("1.0"), true),
                copy("/b.ttf", Some("1.0"), false),
            ],
        };
        exact.choose_keeper();
        let problems = exact.problems();
        let rows: Vec<_> = problems
            .iter()
            .map(|p| (p.kind, p.path.as_str(), p.other.as_deref()))
            .collect();
        assert_eq!(
            rows,
            [
                (ProblemKind::DuplicateFile, "/b.ttf", Some("/lib/a.ttf")),
                (ProblemKind::DuplicateFile, "/z.ttf", Some("/lib/a.ttf")),
            ]
        );
        assert_eq!(problems[0].message, "The same file is also at /lib/a.ttf.");
    }

    #[test]
    fn a_near_group_lists_duplicate_faces_with_name_and_version() {
        let mut near = DuplicateGroup {
            kind: DuplicateKind::Near,
            postscript_name: Some("Inter-Regular".into()),
            copies: vec![
                copy("/old.ttf", Some("2.0"), false),
                copy("/new.ttf", Some("2.0 "), false),
            ],
        };
        near.copies[0].face = Some(FaceId(7));
        near.choose_keeper();
        let problems = near.problems();
        assert_eq!(problems.len(), 1);
        let p = &problems[0];
        assert_eq!(p.kind, ProblemKind::DuplicateFace);
        assert_eq!(p.path, "/old.ttf");
        assert_eq!(p.face, Some(FaceId(7)));
        assert_eq!(p.other.as_deref(), Some("/new.ttf"));
        assert_eq!(p.message, "Inter-Regular (2.0) is also in /new.ttf.");
    }

    #[test]
    fn a_group_without_copies_has_no_problems() {
        let empty = DuplicateGroup {
            kind: DuplicateKind::Exact,
            postscript_name: None,
            copies: Vec::new(),
        };
        assert_eq!(empty.problems(), []);
    }

    fn groups() -> Vec<DuplicateGroup> {
        let group = |paths: &[&str]| DuplicateGroup {
            kind: DuplicateKind::Exact,
            postscript_name: None,
            copies: paths.iter().map(|p| copy(p, None, false)).collect(),
        };
        vec![
            group(&["/a.ttf", "/b.ttf", "/c.ttf"]),
            group(&["/x.ttf", "/y.ttf"]),
        ]
    }

    fn keep(keep: &str, trash: &[&str]) -> ApiResult<()> {
        KeepCopyRequest {
            keep: keep.into(),
            trash: trash.iter().map(|&p| p.into()).collect(),
            dry_run: false,
        }
        .check(&groups())
    }

    #[test]
    fn copies_of_the_kept_font_may_go_to_the_trash() {
        assert_eq!(keep("/b.ttf", &["/a.ttf", "/c.ttf"]), Ok(()));
        assert_eq!(keep("/y.ttf", &["/x.ttf"]), Ok(()));
    }

    #[test]
    fn only_duplicates_of_a_copy_that_stays_can_go_to_the_trash() {
        let code = |r: ApiResult<()>| r.expect_err("refused").code;
        // Another group's file, the kept copy itself, a path twice, a
        // keeper that is no duplicate, nothing to keep or to trash.
        assert_eq!(code(keep("/a.ttf", &["/x.ttf"])), ErrorCode::InvalidRequest);
        assert_eq!(code(keep("/a.ttf", &["/a.ttf"])), ErrorCode::InvalidRequest);
        assert_eq!(
            code(keep("/a.ttf", &["/b.ttf", "/b.ttf"])),
            ErrorCode::InvalidRequest
        );
        assert_eq!(
            code(keep("/home/notes.txt", &["/a.ttf"])),
            ErrorCode::InvalidRequest
        );
        assert_eq!(code(keep("", &["/a.ttf"])), ErrorCode::InvalidRequest);
        assert_eq!(code(keep("/a.ttf", &[])), ErrorCode::InvalidRequest);
    }

    #[test]
    fn a_dry_run_is_off_unless_asked() {
        let req: KeepCopyRequest =
            serde_json::from_str(r#"{"keep":"/a.ttf","trash":["/b.ttf"]}"#).expect("parses");
        assert!(!req.dry_run);
    }

    fn names(id: u32, ps: &str, full: Option<&str>, families: &[(&str, &str)]) -> FontNames {
        let (family, style) = families[0];
        FontNames {
            face: FontMatch {
                face: FaceId(id),
                family: family.to_owned(),
                style: style.to_owned(),
                postscript_name: ps.to_owned(),
                full_name: full.map(str::to_owned),
            },
            families: families
                .iter()
                .map(|(f, s)| ((*f).to_owned(), (*s).to_owned()))
                .collect(),
        }
    }

    /// Every name differs from the others, so a match proves which field
    /// it came from: face 3's PostScript name is face 1's full name, face
    /// 4's full name is "Sora Bold" (its family and style say otherwise).
    fn library() -> Vec<FontNames> {
        vec![
            names(
                1,
                "Sora-Light",
                Some("Sora Thin Display"),
                &[("Sora", "Light")],
            ),
            names(2, "Sora-Italic", None, &[("Sora", "Italic")]),
            names(3, "Sora Thin Display", None, &[("Teko", "Bold")]),
            names(
                4,
                "Agdasima-B",
                Some("Sora Bold"),
                &[("Agdasima", "Bold"), ("Agdasima Pro", "Bold Wide")],
            ),
            names(1942, "", None, &[("1942 Report", "Regular")]),
        ]
    }

    fn resolve(font: &str) -> ApiResult<(FontMatchKind, Vec<u32>)> {
        ResolveFontRequest { font: font.into() }
            .resolve(library())
            .map(|m| (m.matched_by, m.faces.iter().map(|f| f.face.0).collect()))
    }

    #[test]
    fn a_font_argument_is_an_id_then_a_postscript_full_or_family_name() {
        use FontMatchKind::*;
        assert_eq!(resolve("2"), Ok((Id, vec![2])));
        assert_eq!(resolve("sora-light"), Ok((PostscriptName, vec![1])));
        // The PostScript name wins over another face's full name.
        assert_eq!(resolve("Sora Thin Display"), Ok((PostscriptName, vec![3])));
        // The full name wins over another face's family and style.
        assert_eq!(resolve("  SORA bold "), Ok((FullName, vec![4])));
        assert_eq!(resolve("Sora Italic"), Ok((FamilyStyle, vec![2])));
        assert_eq!(
            resolve("Agdasima Pro Bold Wide"),
            Ok((FamilyStyle, vec![4]))
        );
        assert_eq!(resolve("agdasima pro"), Ok((FamilyStyle, vec![4])));
        // A family alone names every face of it.
        assert_eq!(resolve("Sora"), Ok((FamilyStyle, vec![1, 2])));
        // A number that is no face id is a name; an empty PostScript name
        // matches nothing.
        assert_eq!(resolve("1942 report"), Ok((FamilyStyle, vec![1942])));
        assert_eq!(resolve("1942"), Ok((Id, vec![1942])));
    }

    #[test]
    fn an_unknown_or_empty_font_argument_is_refused() {
        let code = |font: &str| resolve(font).expect_err("refused").code;
        assert_eq!(code("Inter"), ErrorCode::NotFound);
        assert_eq!(code("99"), ErrorCode::NotFound);
        assert_eq!(code("Sora Light Italic"), ErrorCode::NotFound);
        assert_eq!(code("  "), ErrorCode::InvalidRequest);
    }

    #[test]
    fn an_ambiguous_name_lists_every_candidate_with_its_id() {
        let matches = ResolveFontRequest {
            font: "Sora".into(),
        }
        .resolve(library())
        .expect("matches");
        let err = matches.single("Sora").expect_err("ambiguous");
        assert_eq!(err.code, ErrorCode::InvalidRequest);
        assert_eq!(
            err.message,
            "\"Sora\" matches 2 fonts. Use one of their ids instead."
        );
        assert_eq!(
            err.detail.as_deref(),
            Some(
                "1	Sora-Light	Sora Light
2	Sora-Italic	Sora Italic"
            )
        );
        let one = ResolveFontRequest {
            font: "Teko Bold".into(),
        }
        .resolve(library())
        .and_then(|m| m.single("Teko Bold"))
        .expect("one face");
        assert_eq!(one.face, FaceId(3));
    }
}
