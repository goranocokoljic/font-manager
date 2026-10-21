//! Search service: text search, filters, language coverage and similarity.

use serde::{Deserialize, Serialize};

use crate::common::{Classification, Embedding, FontFormat, Page, Slant};
use crate::library::{FaceSummary, FamilySummary, LicenseType};
use crate::{ApiResult, CollectionId, FaceId, SourceId, TagId};

/// An inclusive weight range (100–1000).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WeightRange {
    /// Lowest weight.
    pub min: u16,
    /// Highest weight.
    pub max: u16,
}

/// An inclusive OS/2 width-class range (1 ultra-condensed – 9 ultra-expanded).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WidthRange {
    /// Narrowest width class.
    pub min: u16,
    /// Widest width class.
    pub max: u16,
}

/// Characters a font must cover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Supports {
    /// Every listed language preset (AND), by code (`sr-Cyrl`, `pl`).
    #[serde(rename_all = "camelCase")]
    Languages {
        /// Preset codes from `list_languages`.
        codes: Vec<String>,
    },
    /// Every character of a pasted text.
    #[serde(rename_all = "camelCase")]
    Text {
        /// The text.
        text: String,
    },
}

/// Filters; every field is optional and they combine with AND. A smart
/// collection's rule is made of them (`organization::RuleGroup`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub struct Filters {
    /// Any of these classes.
    pub classification: Vec<Classification>,
    /// Weight range; a variable face passes when its `wght` axis reaches it.
    pub weight: Option<WeightRange>,
    /// Width-class range; a variable face passes when its `wdth` axis reaches it.
    pub width: Option<WidthRange>,
    /// Slant.
    pub slant: Option<Slant>,
    /// Variable (true) or static (false) only.
    pub variable: Option<bool>,
    /// Color fonts only (true) or none (false).
    pub color: Option<bool>,
    /// Any of these formats.
    pub formats: Vec<FontFormat>,
    /// Language or character coverage.
    pub supports: Option<Supports>,
    /// Every one of these OpenType feature tags.
    pub features: Vec<String>,
    /// Declared license type.
    pub license: Option<LicenseType>,
    /// Foundry, exact.
    pub foundry: Option<String>,
    /// Any of these sources.
    pub sources: Vec<SourceId>,
    /// Every one of these tags.
    pub tags: Vec<TagId>,
    /// Inside this collection; a smart collection applies its rule.
    pub collection: Option<CollectionId>,
    /// Active (true) or inactive (false) only.
    pub active: Option<bool>,
    /// Favorites only.
    pub favorite: Option<bool>,
    /// At least this many stars.
    pub min_rating: Option<u8>,
    /// Health: faces with problems (true) or healthy faces (false) only.
    pub problems: Option<bool>,
    /// Added to the library within the last this many days (each 24 hours).
    pub added_within_days: Option<u32>,
    /// Embedding permission (`OS/2.fsType`), exact.
    #[serde(default)]
    pub embedding: Option<Embedding>,
    /// Faces whose name table states an open license (true) or none (false) only.
    #[serde(default)]
    pub open_license: Option<bool>,
    /// Faces OK for web (true) or not (false) only; see `LicenseInfo::web_use` (E15.3).
    #[serde(default)]
    pub web_ok: Option<bool>,
    /// Activated within the last this many days (each 24 hours), active now
    /// or not (E14.3).
    #[serde(default)]
    pub activated_within_days: Option<u32>,
}

/// A search. An empty query with filters lists everything that passes them.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    /// Free text over family, style, PostScript name, foundry, designer, tags
    /// and file name; typo-tolerant; style words ("bold italic") narrow styles.
    #[serde(default)]
    pub query: String,
    /// Filters.
    #[serde(default)]
    pub filters: Filters,
    /// Window over the family groups.
    #[serde(default)]
    pub page: Page,
}

/// Near-miss detail for a coverage filter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Missing {
    /// The exact characters the face lacks.
    pub chars: Vec<String>,
    /// The face carries `locl` forms for the requested language.
    pub has_locl: bool,
}

/// One family in the results, with the faces that matched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FamilyHit {
    /// The family row.
    pub family: FamilySummary,
    /// The matching faces, best first.
    pub faces: Vec<FaceId>,
    /// Rank score; higher is better.
    pub score: f32,
    /// With a `supports` filter: what the best face misses (empty when full).
    pub missing: Option<Missing>,
}

/// Search results grouped by family.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    /// Family groups in this page.
    pub families: Vec<FamilyHit>,
    /// Family groups in all pages.
    pub total: u32,
    /// The result count of every filter value.
    #[serde(default)]
    pub counts: Option<FilterCounts>,
}

/// One filter value and how many family groups it would give.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FilterCount<T> {
    /// The value.
    pub value: T,
    /// Family groups the search returns with this value chosen.
    pub count: u32,
}

/// Result counts for the filters, per value: the family groups the same
/// search returns with that value chosen and the other filters kept. A
/// filter that takes any of several values (classification, formats,
/// sources) is counted without its own selection; `features` and `tags`,
/// which take every one, are counted with their selection plus the value.
/// Values with no result are left out.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub struct FilterCounts {
    /// Per class.
    pub classification: Vec<FilterCount<Classification>>,
    /// Per weight class in hundreds (100–900): a face counts under the class
    /// its weight rounds to; a variable face under every class it reaches.
    pub weight: Vec<FilterCount<u16>>,
    /// Per width class (1–9); a variable face under every class it reaches.
    pub width: Vec<FilterCount<u16>>,
    /// Per slant.
    pub slant: Vec<FilterCount<Slant>>,
    /// Variable (true) and static (false).
    pub variable: Vec<FilterCount<bool>>,
    /// Color (true) and not (false).
    pub color: Vec<FilterCount<bool>>,
    /// Per format.
    pub formats: Vec<FilterCount<FontFormat>>,
    /// Per OpenType feature tag.
    pub features: Vec<FilterCount<String>>,
    /// Per declared license type.
    pub license: Vec<FilterCount<LicenseType>>,
    /// Per source.
    pub sources: Vec<FilterCount<SourceId>>,
    /// With problems (true) and healthy (false).
    pub problems: Vec<FilterCount<bool>>,
    /// Active (true) and inactive (false).
    pub active: Vec<FilterCount<bool>>,
    /// Per tag. Like `features`, counted with the tags already chosen.
    pub tags: Vec<FilterCount<TagId>>,
    /// Favorites (true) and the rest (false).
    pub favorite: Vec<FilterCount<bool>>,
    /// Per `min_rating` (1–5): a face rated 4 counts under 1, 2, 3 and 4.
    pub min_rating: Vec<FilterCount<u8>>,
    /// Per embedding permission.
    #[serde(default)]
    pub embedding: Vec<FilterCount<Embedding>>,
    /// With an open license (true) and without (false).
    #[serde(default)]
    pub open_license: Vec<FilterCount<bool>>,
    /// OK for web (true) and not (false).
    #[serde(default)]
    pub web_ok: Vec<FilterCount<bool>>,
}

/// How well one face covers some languages or text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CoverageRequest {
    /// The face.
    pub face: FaceId,
    /// What it must cover.
    pub target: Supports,
}

/// Coverage of one language preset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LanguageCoverage {
    /// Preset code.
    pub code: String,
    /// Characters covered.
    pub covered: u32,
    /// Characters the preset needs.
    pub total: u32,
    /// The characters missing.
    pub missing: Vec<String>,
    /// The face has `locl` forms for this language.
    pub has_locl: bool,
}

/// The answer to a coverage request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CoverageReport {
    /// The face.
    pub face: FaceId,
    /// Everything requested is covered.
    pub complete: bool,
    /// Characters missing overall.
    pub missing: Vec<String>,
    /// Per language, for a `Languages` target (empty for `Text`).
    pub languages: Vec<LanguageCoverage>,
}

/// A language preset from `data/charsets`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LanguagePreset {
    /// Code (`sr-Cyrl`).
    pub code: String,
    /// English name ("Serbian (Cyrillic)").
    pub name: String,
    /// ISO 15924 script (`Cyrl`).
    pub script: String,
}

/// How many characters of one Unicode block a face maps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BlockCoverage {
    /// The block name from the Unicode block table (`Latin Extended-A`), as
    /// the glyph map's `GlyphBlock::name`.
    pub name: String,
    /// Its first code point.
    pub first: u32,
    /// Its last code point.
    pub last: u32,
    /// Characters of the block the face maps (at most `assigned`).
    pub covered: u32,
    /// Assigned code points of the block.
    pub assigned: u32,
}

/// How much of one language preset a face covers, for the coverage list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FaceLanguageCoverage {
    /// Preset code (`sr-Cyrl`).
    pub code: String,
    /// English name ("Serbian (Cyrillic)").
    pub name: String,
    /// Base characters of the preset the face maps.
    pub covered: u32,
    /// Base characters the preset needs.
    pub total: u32,
    /// The face has `locl` forms for this language.
    pub has_locl: bool,
}

/// The coverage of one face: per Unicode block and per language preset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FaceCoverage {
    /// The face.
    pub face: FaceId,
    /// The blocks with at least one covered character, in Unicode order.
    pub blocks: Vec<BlockCoverage>,
    /// The presets the face covers anything of or has `locl` forms for:
    /// complete ones first (in preset order), then partial ones, most
    /// covered first. Missing characters come from `check_coverage`.
    pub languages: Vec<FaceLanguageCoverage>,
}

/// Faces that look like this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SimilarRequest {
    /// The face to compare against.
    pub face: FaceId,
    /// Results to return.
    pub limit: u32,
}

/// A similar face and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SimilarFace {
    /// The face.
    pub face: FaceSummary,
    /// 0.0–1.0; higher is closer.
    pub score: f32,
    /// Plain-word reasons ("matching x-height").
    pub reasons: Vec<String>,
}

/// Search, filters, coverage and similarity.
pub trait SearchService {
    /// Ranked results grouped by family.
    fn search(&self, req: SearchRequest) -> ApiResult<SearchResponse>;
    /// Coverage of one face.
    fn check_coverage(&self, req: CoverageRequest) -> ApiResult<CoverageReport>;
    /// Coverage of one face per Unicode block and per language preset, from
    /// the code points in the index.
    fn face_coverage(&self, face: FaceId) -> ApiResult<FaceCoverage>;
    /// Every language preset.
    fn list_languages(&self) -> ApiResult<Vec<LanguagePreset>>;
    /// Faces ranked by visual similarity.
    fn find_similar(&self, req: SimilarRequest) -> ApiResult<Vec<SimilarFace>>;
}
