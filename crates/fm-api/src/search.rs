//! Search service: text search, filters, language coverage and similarity.

use serde::{Deserialize, Serialize};

use crate::common::{Classification, Embedding, FontFormat, Page, Slant};
use crate::library::{FaceSummary, FamilyOrder, FamilySummary, LicenseType};
use crate::organization::RuleGroup;
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
    /// A smart-collection rule a face must pass as well, as a smart
    /// collection applies it (E14.1: the rule editor's live count of a rule
    /// not saved yet).
    #[serde(default)]
    pub rule: Option<RuleGroup>,
    /// The order of the family groups; `None`: best match first.
    #[serde(default)]
    pub order: Option<FamilyOrder>,
    /// Window over the family groups.
    #[serde(default)]
    pub page: Page,
    /// Glyph traits every result face must have (E49.5), from
    /// `list_glyph_traits`; they combine with AND, with each other and with
    /// the filters. A face without a detection of the trait (it lacks the
    /// glyph, or it is not measured yet) passes no trait filter.
    #[serde(default)]
    pub glyph_traits: Option<Vec<TraitFilter>>,
}

/// One glyph-trait filter of a search (E49.5).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum TraitFilter {
    /// The face's detected value of the trait is `value`, detected with
    /// confidence (a guess between two values does not match).
    #[serde(rename_all = "camelCase")]
    Value {
        /// The trait id (`R.leg`).
        trait_id: String,
        /// The value id (`straight`).
        value: String,
    },
    /// A scalar trait's measure lies in `min..=max`; a bound left out is open.
    #[serde(rename_all = "camelCase")]
    Range {
        /// The trait id (`O.roundness`).
        trait_id: String,
        /// The measure id; `None`: the trait's first measure.
        #[serde(default)]
        measure: Option<String>,
        /// Inclusive lower bound.
        #[serde(default)]
        min: Option<f64>,
        /// Inclusive upper bound.
        #[serde(default)]
        max: Option<f64>,
    },
}

impl TraitFilter {
    /// The trait the filter is on.
    pub fn trait_id(&self) -> &str {
        match self {
            Self::Value { trait_id, .. } | Self::Range { trait_id, .. } => trait_id,
        }
    }
}

/// Bounds compare by their bits, so a filter equals itself (`SearchRequest`
/// is `Eq`).
impl PartialEq for TraitFilter {
    fn eq(&self, other: &Self) -> bool {
        let bits = |v: &Option<f64>| v.map(f64::to_bits);
        match (self, other) {
            (
                Self::Value { trait_id, value },
                Self::Value {
                    trait_id: t,
                    value: v,
                },
            ) => trait_id == t && value == v,
            (
                Self::Range {
                    trait_id,
                    measure,
                    min,
                    max,
                },
                Self::Range {
                    trait_id: t,
                    measure: m,
                    min: lo,
                    max: hi,
                },
            ) => trait_id == t && measure == m && bits(min) == bits(lo) && bits(max) == bits(hi),
            _ => false,
        }
    }
}

impl Eq for TraitFilter {}

/// The text form of a trait filter, as `fm search --trait` takes it:
/// `R.leg=straight` (a value), `O.roundness>=0.95` or `O.roundness<=0.9`
/// (a bound on the first measure), `O.roundness:bulge<=0.1` (a bound on a
/// named measure). The text is only split, not checked against the catalog;
/// the search answers an unknown trait or value.
impl std::str::FromStr for TraitFilter {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim();
        let shape =
            || format!("{text:?} is not <trait>=<value>, <trait>>=<number> or <trait><=<number>");
        let at = text.find(['=', '<', '>']).ok_or_else(shape)?;
        let (left, rest) = text.split_at(at);
        let (trait_part, measure) = match left.split_once(':') {
            Some((t, m)) => (t.trim(), Some(m.trim())),
            None => (left.trim(), None),
        };
        if trait_part.is_empty() || measure.is_some_and(str::is_empty) {
            return Err(format!("{text:?} names no trait"));
        }
        let number = |s: &str| {
            s.trim()
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| format!("{:?} in {text:?} is not a number", s.trim()))
        };
        let range = |min: Option<f64>, max: Option<f64>| Self::Range {
            trait_id: trait_part.to_owned(),
            measure: measure.map(str::to_owned),
            min,
            max,
        };
        if let Some(v) = rest.strip_prefix(">=") {
            Ok(range(Some(number(v)?), None))
        } else if let Some(v) = rest.strip_prefix("<=") {
            Ok(range(None, Some(number(v)?)))
        } else if rest.starts_with(['<', '>']) {
            Err(format!("{text:?}: use >= or <="))
        } else if let Some(v) = rest.strip_prefix('=') {
            let value = v.trim();
            if measure.is_some() {
                return Err(format!("{text:?}: a measure takes >= or <=, not ="));
            }
            if value.is_empty() || value.contains(['=', '<', '>']) {
                return Err(format!("{text:?} names no value"));
            }
            Ok(Self::Value {
                trait_id: trait_part.to_owned(),
                value: value.to_owned(),
            })
        } else {
            Err(shape())
        }
    }
}

/// How a glyph trait is detected (E49.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum GlyphTraitKind {
    /// The detector picks one of the values.
    Enum,
    /// The detector measures numbers; the values are ranges of them.
    Scalar,
}

/// One number a scalar trait measures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GlyphTraitMeasure {
    /// The measure id (`roundness`), for a `TraitFilter::Range`.
    pub id: String,
    /// What the number is, in one line.
    pub description: String,
    /// The least value the detector reports.
    pub min: f64,
    /// The greatest value the detector reports.
    pub max: f64,
}

/// One bound of a scalar value: `min <= measure < max`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GlyphTraitThreshold {
    /// The measure id.
    pub measure: String,
    /// Inclusive lower bound.
    pub min: Option<f64>,
    /// Exclusive upper bound.
    pub max: Option<f64>,
}

/// One value of a glyph trait with its count in the library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GlyphTraitValue {
    /// The value id (`straight`), for a `TraitFilter::Value`.
    pub id: String,
    /// The English chip text.
    pub label: String,
    /// What the value looks like, in one line.
    pub description: String,
    /// Scalar traits: the bounds that all hold for this value; empty for the
    /// last value, which takes the rest, and for enum values.
    pub when: Vec<GlyphTraitThreshold>,
    /// A drawn example: an SVG path, stroked, in a 56-unit square box.
    pub example: String,
    /// Searchable faces detected with this value, with confidence: the faces
    /// a `TraitFilter::Value` of it finds.
    pub faces: u32,
    /// The family groups of those faces.
    pub families: u32,
}

/// One glyph trait of the catalog with its counts in the library (E49.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GlyphTraitInfo {
    /// The trait id (`R.leg`).
    pub id: String,
    /// ISO 15924 script of the glyph (`Latn`).
    pub script: String,
    /// The character the trait is measured on.
    pub glyph: String,
    /// How it is detected.
    pub kind: GlyphTraitKind,
    /// The English name ("R leg").
    pub name: String,
    /// What the trait is, in one line.
    pub description: String,
    /// Whether the picker offers it. A trait whose detector misses the
    /// accuracy bar is listed disabled; a search on it still works.
    pub enabled: bool,
    /// Scalar traits: the numbers measured; empty for an enum trait.
    pub measures: Vec<GlyphTraitMeasure>,
    /// The values, in picker order.
    pub values: Vec<GlyphTraitValue>,
    /// Searchable faces with a detection of this trait; the rest lack the
    /// glyph or are not measured yet, and count under no value.
    pub measured: u32,
    /// Of `measured`, the faces whose detection is a guess between two
    /// values: no value counts them.
    pub unsure: u32,
}

/// One glyph trait detected on a face with confidence (E49.10): a value a
/// `TraitFilter::Value` of it finds the face by.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DetectedTrait {
    /// The trait id (`R.leg`), as in `list_glyph_traits`.
    pub trait_id: String,
    /// The value id (`straight`).
    pub value: String,
    /// How sure the detector was, 0–1; at least the bar a value filter
    /// asks for.
    pub confidence: f32,
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
    /// Per foundry, A to Z (E14.1).
    #[serde(default)]
    pub foundry: Vec<FilterCount<String>>,
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
    /// A pangram, or a sample sentence where the language has no true
    /// pangram, from `data/pangrams`; `None` when there is none (E19.4).
    #[serde(default)]
    pub pangram: Option<String>,
    /// The language's distinctive letters (`Šđčćž` for `sr-Latn`), from
    /// `data/charsets`, for a short glyph line next to the pangram; every one
    /// is in the preset's characters. `None` when the preset has none (E42.10).
    #[serde(default)]
    pub sample_glyphs: Option<String>,
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

/// Faces that look like the text in an image (E33.1): a screenshot of a line
/// of text, matched against the glyph fingerprints of the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImageMatchRequest {
    /// The image: a PNG of at most 4096 px a side. It is only read, never
    /// written or moved. The sample text "Hamburgefonstiv" matches best.
    pub path: String,
    /// Results to return.
    pub limit: u32,
}

/// Faces ranked by how close their drawing of one character is to the
/// reference face's (E49.8): "an R like this one".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SimilarGlyphRequest {
    /// The reference face. A face that is unknown, or does not draw the
    /// character, is `NotFound`.
    pub face: FaceId,
    /// The character compared, as a Unicode code point (`82` for `R`).
    pub code_point: u32,
    /// Results to return.
    pub limit: u32,
}

/// A face whose drawing of the character is close to the reference's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SimilarGlyphFace {
    /// The face.
    pub face: FaceSummary,
    /// How far its glyph is from the reference glyph, `0.0..=1.0`; lower is
    /// closer, 0.0 is the same drawing.
    pub distance: f32,
}

/// The part a suggested face plays in a layout (E42.13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum PairingRole {
    /// Large type: a title or a heading.
    Headline,
    /// Running text.
    Body,
    /// Small type: a note or a label.
    Caption,
}

/// Faces that pair well with this one (E23.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PairingRequest {
    /// The face to find partners for. With `partner_of` set, the face being
    /// replaced instead: its family is not suggested.
    pub face: FaceId,
    /// Suggestions to return.
    pub limit: u32,
    /// The role the suggested faces play, which changes how they are scored
    /// and the reasons given (ADR 0021). Absent: the plain rules, for no
    /// role in particular.
    #[serde(default)]
    pub role: Option<PairingRole>,
    /// The face the suggestions must go with, when that is not `face`:
    /// "given this body face, suggest headlines". A face that is not visible
    /// is `NotFound`.
    #[serde(default)]
    pub partner_of: Option<FaceId>,
}

/// A face suggested as a partner and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PairingSuggestion {
    /// The face: one per family, the style closest to the one asked about.
    pub face: FaceSummary,
    /// 0.0–1.0; higher pairs better.
    pub score: f32,
    /// Plain-word reasons, one per rule that holds ("same superfamily"); with
    /// a role in the request, those of the role first ("lower contrast for
    /// body", "x-height within 4 %").
    pub reasons: Vec<String>,
}

/// A text to find the likely languages of (E43.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DetectLanguagesRequest {
    /// The text, as the user typed it. Long text is read in part.
    pub text: String,
}

/// A language preset that a text may be in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LanguageHint {
    /// Preset code from `list_languages`.
    pub code: String,
    /// Above 0.0 and at most 1.0; higher fits the text better.
    pub confidence: f32,
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
    /// The language presets a text may be in, best first, from its letters
    /// outside ASCII. Empty for a text without such letters.
    fn detect_languages(&self, req: DetectLanguagesRequest) -> ApiResult<Vec<LanguageHint>>;
    /// Faces ranked by visual similarity.
    fn find_similar(&self, req: SimilarRequest) -> ApiResult<Vec<SimilarFace>>;
    /// Faces ranked by how close they look to the text in an image, closest
    /// first, at most `limit`; only faces with a glyph fingerprint are
    /// candidates. An image that is not a readable PNG, or shows no text, is
    /// an error.
    fn find_by_image(&self, req: ImageMatchRequest) -> ApiResult<Vec<SimilarFace>>;
    /// Faces ranked by how close their glyph of `req.code_point` is to the
    /// reference face's, closest first, at most `limit`, each with the
    /// distance (E49.8). The reference face itself is not listed; the other
    /// faces of its family are. A face that does not draw the character is
    /// not a candidate. The whole-face [`Self::find_similar`] is unchanged.
    fn find_similar_glyph(&self, req: SimilarGlyphRequest) -> ApiResult<Vec<SimilarGlyphFace>>;
    /// Faces of other families that pair with this one by rule: contrasting
    /// classes, a matching x-height, the same designer or superfamily. Best
    /// first; a face no rule holds for is not suggested. A role in the request
    /// also weighs stroke contrast, x-height, width and era for that role.
    fn suggest_pairings(&self, req: PairingRequest) -> ApiResult<Vec<PairingSuggestion>>;
    /// The glyph trait catalog, in catalog order, each value with how many
    /// searchable faces and family groups have it (E49.5).
    fn list_glyph_traits(&self) -> ApiResult<Vec<GlyphTraitInfo>>;
}
