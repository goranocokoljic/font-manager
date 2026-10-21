//! Search filters (E3.4): which faces pass a [`Filters`], and the result
//! count of every filter value.
//!
//! Filters apply to faces and combine with AND: a face passes when it passes
//! every filter that is set, and a family is a result when one of its faces
//! passes (and matches the text). [`failed`] gives the filters a face fails
//! as a bit mask, so a count can leave one filter out: the count of a value
//! of filter `D` is the families with a face that has the value and passes
//! every filter but `D`. `features` asks for every listed tag, so its counts
//! keep its own selection.
//!
//! The values a face can be counted under are interned once per search
//! cache ([`Slots`]); a face holds their slot numbers, and a [`Tally`] counts
//! each slot once per family with two plain arrays.

use std::collections::HashMap;

use fm_api::SourceId;
use fm_api::common::{Classification, Embedding, FontFormat, Slant};
use fm_api::library::{LicenseType, OpenLicense};
use fm_api::search::{FilterCount, FilterCounts, Filters};
use fm_api::{CollectionId, TagId};

use crate::coverage::Fit;
use crate::license;
use crate::search::{FaceText, reaches_slant, reaches_weight, reaches_width};

/// A four-letter OpenType tag.
pub(crate) type Tag = [u8; 4];

/// `text` as a tag; `None` unless it is four bytes long.
pub(crate) fn tag(text: &str) -> Option<Tag> {
    text.as_bytes().try_into().ok()
}

/// What a face offers to the filters beyond its style ([`FaceText`]).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FaceFacts {
    /// Design class; `None` until classified (E3.5).
    pub classification: Option<Classification>,
    pub is_variable: bool,
    pub color: bool,
    pub format: Option<FontFormat>,
    /// OpenType feature tags, sorted.
    pub features: Vec<Tag>,
    /// The license type the user declared.
    pub license: Option<LicenseType>,
    pub foundry: Option<String>,
    pub source: SourceId,
    pub tags: Vec<TagId>,
    /// Manual collections the face is in.
    pub collections: Vec<CollectionId>,
    pub active: bool,
    pub favorite: bool,
    pub rating: Option<u8>,
    /// Health is not `Ok`.
    pub problems: bool,
    /// The embedding permission of `OS/2.fsType`.
    pub embedding: Embedding,
    /// The open license the name table states.
    pub open_license: Option<OpenLicense>,
}

impl FaceFacts {
    /// The face is OK for web ([`license::web_use`]).
    pub(crate) fn web_ok(&self) -> bool {
        license::web_use(self.embedding, self.open_license, self.license).ok()
    }
}

impl FaceFacts {
    /// A face with no facts but its source.
    #[cfg(test)]
    pub(crate) fn new(source: SourceId) -> Self {
        Self {
            classification: None,
            is_variable: false,
            color: false,
            format: None,
            features: Vec::new(),
            license: None,
            foundry: None,
            source,
            tags: Vec::new(),
            collections: Vec::new(),
            active: false,
            favorite: false,
            rating: None,
            problems: false,
            embedding: Embedding::Installable,
            open_license: None,
        }
    }
}

/// One filter of [`Filters`]; its bit in a [`failed`] mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dim {
    Classification,
    Weight,
    Width,
    Slant,
    Variable,
    Color,
    Format,
    Features,
    License,
    Foundry,
    Sources,
    Tags,
    Collection,
    Active,
    Favorite,
    Rating,
    Problems,
    Embedding,
    OpenLicense,
    WebOk,
    /// Language or character coverage: checked against a [`Need`](crate::coverage::Need) built
    /// once per search, so [`failed`] leaves it to [`failed_supports`].
    Supports,
    /// Added within some days: checked against a cutoff time taken once per
    /// search, so [`failed`] leaves it to [`failed_added`].
    Added,
}

impl Dim {
    const fn bit(self) -> u32 {
        1 << self as u32
    }
}

/// The filters [`failed`] and [`passes`] check, in [`Dim`] order.
const CHECKED: [Dim; 20] = [
    Dim::Classification,
    Dim::Weight,
    Dim::Width,
    Dim::Slant,
    Dim::Variable,
    Dim::Color,
    Dim::Format,
    Dim::Features,
    Dim::License,
    Dim::Foundry,
    Dim::Sources,
    Dim::Tags,
    Dim::Collection,
    Dim::Active,
    Dim::Favorite,
    Dim::Rating,
    Dim::Problems,
    Dim::Embedding,
    Dim::OpenLicense,
    Dim::WebOk,
];

/// Whether `face` passes the filter `dim` of `filters` (an unset filter
/// passes). `supports` and `added` pass here: see [`failed_supports`] and
/// [`failed_added`].
fn check(dim: Dim, f: &Filters, text: &FaceText, facts: &FaceFacts) -> bool {
    match dim {
        Dim::Classification => {
            f.classification.is_empty()
                || facts
                    .classification
                    .is_some_and(|c| f.classification.contains(&c))
        }
        Dim::Weight => f.weight.is_none_or(|w| reaches_weight(text, w.min, w.max)),
        Dim::Width => f.width.is_none_or(|w| reaches_width(text, w.min, w.max)),
        Dim::Slant => f.slant.is_none_or(|s| reaches_slant(text, s)),
        Dim::Variable => f.variable.is_none_or(|v| v == facts.is_variable),
        Dim::Color => f.color.is_none_or(|c| c == facts.color),
        Dim::Format => f.formats.is_empty() || facts.format.is_some_and(|x| f.formats.contains(&x)),
        Dim::Features => f
            .features
            .iter()
            .all(|t| tag(t).is_some_and(|t| facts.features.binary_search(&t).is_ok())),
        Dim::License => f.license.is_none_or(|l| facts.license == Some(l)),
        Dim::Foundry => f
            .foundry
            .as_deref()
            .is_none_or(|x| facts.foundry.as_deref() == Some(x)),
        Dim::Sources => f.sources.is_empty() || f.sources.contains(&facts.source),
        Dim::Tags => f.tags.iter().all(|t| facts.tags.contains(t)),
        Dim::Collection => f.collection.is_none_or(|c| facts.collections.contains(&c)),
        Dim::Active => f.active.is_none_or(|a| a == facts.active),
        Dim::Favorite => f.favorite.is_none_or(|v| v == facts.favorite),
        Dim::Rating => f
            .min_rating
            .is_none_or(|r| facts.rating.is_some_and(|x| x >= r)),
        Dim::Problems => f.problems.is_none_or(|p| p == facts.problems),
        Dim::Embedding => f.embedding.is_none_or(|e| e == facts.embedding),
        Dim::OpenLicense => f
            .open_license
            .is_none_or(|o| o == facts.open_license.is_some()),
        Dim::WebOk => f.web_ok.is_none_or(|w| w == facts.web_ok()),
        Dim::Supports | Dim::Added => true,
    }
}

/// The filters `face` fails, one [`Dim::bit`] each; 0 when it passes all.
/// `supports` is not looked at here: see [`failed_supports`].
pub(crate) fn failed(filters: &Filters, text: &FaceText, facts: &FaceFacts) -> u32 {
    CHECKED
        .iter()
        .filter(|dim| !check(**dim, filters, text, facts))
        .fold(0, |mask, dim| mask | dim.bit())
}

/// Whether `face` passes every filter [`failed`] checks; stops at the first
/// it fails (a smart collection's rule tests every face of the library).
pub(crate) fn passes(filters: &Filters, text: &FaceText, facts: &FaceFacts) -> bool {
    CHECKED.iter().all(|dim| check(*dim, filters, text, facts))
}

/// The [`Dim::Supports`] bit for a face that meets the search's [`Need`](crate::coverage::Need)
/// as `fit`: complete or a near miss passes; `None` (the face's coverage
/// is not known) fails.
pub(crate) fn failed_supports(fit: Option<&Fit>) -> u32 {
    if fit.is_some_and(Fit::passes) {
        0
    } else {
        Dim::Supports.bit()
    }
}

/// The [`Dim::Added`] bit for a face added at `added` (RFC 3339, UTC) when
/// the search keeps faces added at `cutoff` or later; 0 without a cutoff. A
/// face whose time is not known fails.
pub(crate) fn failed_added(cutoff: Option<&str>, added: Option<&str>) -> u32 {
    match cutoff {
        Some(cutoff) if added.is_none_or(|added| added < cutoff) => Dim::Added.bit(),
        _ => 0,
    }
}

/// `mask` with its [`Dim::Collection`] bit decided by `inside`: whether the
/// face is in the collection the filters name, which for a smart collection
/// only its rule can tell.
pub(crate) fn with_collection(mask: u32, inside: bool) -> u32 {
    if inside {
        mask & !Dim::Collection.bit()
    } else {
        mask | Dim::Collection.bit()
    }
}

/// A filter value a face can be counted under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Value {
    Classification(Classification),
    /// A weight class in hundreds.
    Weight(u16),
    /// A width class.
    Width(u16),
    Slant(Slant),
    Variable(bool),
    Color(bool),
    Format(FontFormat),
    Feature(Tag),
    /// A [`LICENSES`] position.
    License(u8),
    Source(SourceId),
    Problems(bool),
    Active(bool),
    /// A user tag.
    Tag(TagId),
    Favorite(bool),
    /// A `min_rating` the face's rating reaches (1–5).
    Rating(u8),
    /// An [`EMBEDDINGS`] position.
    Embedding(u8),
    OpenLicense(bool),
    WebOk(bool),
}

impl Value {
    const fn dim(self) -> Dim {
        match self {
            Self::Classification(_) => Dim::Classification,
            Self::Weight(_) => Dim::Weight,
            Self::Width(_) => Dim::Width,
            Self::Slant(_) => Dim::Slant,
            Self::Variable(_) => Dim::Variable,
            Self::Color(_) => Dim::Color,
            Self::Format(_) => Dim::Format,
            Self::Feature(_) => Dim::Features,
            Self::License(_) => Dim::License,
            Self::Source(_) => Dim::Sources,
            Self::Problems(_) => Dim::Problems,
            Self::Active(_) => Dim::Active,
            Self::Tag(_) => Dim::Tags,
            Self::Favorite(_) => Dim::Favorite,
            Self::Rating(_) => Dim::Rating,
            Self::Embedding(_) => Dim::Embedding,
            Self::OpenLicense(_) => Dim::OpenLicense,
            Self::WebOk(_) => Dim::WebOk,
        }
    }
}

/// Every class, format, slant, license type and embedding permission, in
/// the order counts list them.
const CLASSES: [Classification; 7] = [
    Classification::Serif,
    Classification::Sans,
    Classification::Slab,
    Classification::Script,
    Classification::Display,
    Classification::Mono,
    Classification::Symbol,
];
const FORMATS: [FontFormat; 9] = [
    FontFormat::Ttf,
    FontFormat::Otf,
    FontFormat::Ttc,
    FontFormat::Otc,
    FontFormat::Woff,
    FontFormat::Woff2,
    FontFormat::Type1,
    FontFormat::Dfont,
    FontFormat::Bitmap,
];
const SLANTS: [Slant; 3] = [Slant::Upright, Slant::Italic, Slant::Oblique];
const LICENSES: [LicenseType; 6] = [
    LicenseType::Desktop,
    LicenseType::Web,
    LicenseType::App,
    LicenseType::Trial,
    LicenseType::PersonalOnly,
    LicenseType::Unknown,
];
const EMBEDDINGS: [Embedding; 4] = [
    Embedding::Installable,
    Embedding::Editable,
    Embedding::PreviewAndPrint,
    Embedding::Restricted,
];

/// The `min_rating` values counted.
const RATINGS: [u8; 5] = [1, 2, 3, 4, 5];

/// The weight classes counted, and the widths: 1 to 9.
const WEIGHTS: [u16; 9] = [100, 200, 300, 400, 500, 600, 700, 800, 900];
const WIDTHS: [u16; 9] = [1, 2, 3, 4, 5, 6, 7, 8, 9];

/// The weights a weight class counts: 50 below to 49 above; the ends reach
/// the ends of the OS/2 range (1, 1000).
fn weight_span(class: u16) -> (u16, u16) {
    let lo = if class <= 100 { 1 } else { class - 50 };
    let hi = if class >= 900 { 1000 } else { class + 49 };
    (lo, hi)
}

/// Every value `face` can be counted under.
pub(crate) fn values(text: &FaceText, facts: &FaceFacts) -> Vec<Value> {
    let mut out = Vec::with_capacity(27 + facts.features.len() + facts.tags.len());
    out.extend(facts.classification.map(Value::Classification));
    for class in WEIGHTS {
        let (lo, hi) = weight_span(class);
        if reaches_weight(text, lo, hi) {
            out.push(Value::Weight(class));
        }
    }
    for class in WIDTHS {
        if reaches_width(text, class, class) {
            out.push(Value::Width(class));
        }
    }
    for slant in SLANTS {
        if reaches_slant(text, slant) {
            out.push(Value::Slant(slant));
        }
    }
    out.push(Value::Variable(facts.is_variable));
    out.push(Value::Color(facts.color));
    out.extend(facts.format.map(Value::Format));
    out.extend(facts.features.iter().copied().map(Value::Feature));
    out.extend(
        facts
            .license
            .and_then(|l| LICENSES.iter().position(|x| *x == l))
            .and_then(|i| u8::try_from(i).ok())
            .map(Value::License),
    );
    out.push(Value::Source(facts.source));
    out.push(Value::Problems(facts.problems));
    out.push(Value::Active(facts.active));
    out.extend(facts.tags.iter().copied().map(Value::Tag));
    out.push(Value::Favorite(facts.favorite));
    let rating = facts.rating.unwrap_or(0);
    out.extend(
        RATINGS
            .into_iter()
            .filter(|r| *r <= rating)
            .map(Value::Rating),
    );
    out.extend(
        EMBEDDINGS
            .iter()
            .position(|e| *e == facts.embedding)
            .and_then(|i| u8::try_from(i).ok())
            .map(Value::Embedding),
    );
    out.push(Value::OpenLicense(facts.open_license.is_some()));
    out.push(Value::WebOk(facts.web_ok()));
    out
}

/// The countable values of a library, each with a slot number.
#[derive(Debug, Default)]
pub(crate) struct Slots {
    values: Vec<Value>,
    by_value: HashMap<Value, u32>,
}

impl Slots {
    /// The slot numbers of `values`, adding the new ones.
    pub(crate) fn intern(&mut self, values: &[Value]) -> Vec<u32> {
        values
            .iter()
            .map(|v| {
                *self.by_value.entry(*v).or_insert_with(|| {
                    self.values.push(*v);
                    u32::try_from(self.values.len() - 1).unwrap_or(u32::MAX)
                })
            })
            .collect()
    }
}

/// Counts families per slot.
pub(crate) struct Tally<'a> {
    slots: &'a Slots,
    counts: Vec<u32>,
    /// The family last counted in each slot.
    last: Vec<u32>,
}

impl<'a> Tally<'a> {
    pub(crate) fn new(slots: &'a Slots) -> Self {
        let n = slots.values.len();
        Self {
            slots,
            counts: vec![0; n],
            last: vec![u32::MAX; n],
        }
    }

    /// Counts one face of family number `family`, whose [`failed`] mask is
    /// `failed` and slots `face_slots`. The faces of one family must be
    /// added one after another.
    pub(crate) fn add(&mut self, family: u32, failed: u32, face_slots: &[u32]) {
        for &slot in face_slots {
            let i = slot as usize;
            let Some(value) = self.slots.values.get(i) else {
                continue;
            };
            let dim = value.dim();
            let others = if matches!(dim, Dim::Features | Dim::Tags) {
                failed
            } else {
                failed & !dim.bit()
            };
            if others == 0 && self.last[i] != family {
                self.last[i] = family;
                self.counts[i] += 1;
            }
        }
    }

    /// The counts, every list in a fixed order.
    pub(crate) fn finish(&self) -> FilterCounts {
        let count = |v: Value| {
            self.slots
                .by_value
                .get(&v)
                .map_or(0, |&slot| self.counts[slot as usize])
        };
        fn list<T: Copy>(
            values: impl IntoIterator<Item = T>,
            as_value: impl Fn(T) -> Value,
            count: &impl Fn(Value) -> u32,
        ) -> Vec<FilterCount<T>> {
            values
                .into_iter()
                .map(|value| FilterCount {
                    value,
                    count: count(as_value(value)),
                })
                .filter(|c| c.count > 0)
                .collect()
        }
        let mut features: Vec<Tag> = Vec::new();
        let mut sources: Vec<SourceId> = Vec::new();
        let mut tags: Vec<TagId> = Vec::new();
        for value in &self.slots.values {
            match value {
                Value::Feature(tag) => features.push(*tag),
                Value::Source(id) => sources.push(*id),
                Value::Tag(id) => tags.push(*id),
                _ => {}
            }
        }
        features.sort_unstable();
        sources.sort_unstable();
        tags.sort_unstable();
        let bools = [true, false];
        FilterCounts {
            classification: list(CLASSES, Value::Classification, &count),
            weight: list(WEIGHTS, Value::Weight, &count),
            width: list(WIDTHS, Value::Width, &count),
            slant: list(SLANTS, Value::Slant, &count),
            variable: list(bools, Value::Variable, &count),
            color: list(bools, Value::Color, &count),
            formats: list(FORMATS, Value::Format, &count),
            features: list(features, Value::Feature, &count)
                .into_iter()
                .map(|c| FilterCount {
                    value: String::from_utf8_lossy(&c.value).into_owned(),
                    count: c.count,
                })
                .collect(),
            license: list(0..LICENSES.len(), license_value, &count)
                .into_iter()
                .map(|c| FilterCount {
                    value: LICENSES[c.value],
                    count: c.count,
                })
                .collect(),
            sources: list(sources, Value::Source, &count),
            problems: list(bools, Value::Problems, &count),
            active: list(bools, Value::Active, &count),
            tags: list(tags, Value::Tag, &count),
            favorite: list(bools, Value::Favorite, &count),
            min_rating: list(RATINGS, Value::Rating, &count),
            embedding: list(0..EMBEDDINGS.len(), embedding_value, &count)
                .into_iter()
                .map(|c| FilterCount {
                    value: EMBEDDINGS[c.value],
                    count: c.count,
                })
                .collect(),
            open_license: list(bools, Value::OpenLicense, &count),
            web_ok: list(bools, Value::WebOk, &count),
        }
    }
}

fn license_value(i: usize) -> Value {
    Value::License(u8::try_from(i).unwrap_or(u8::MAX))
}

fn embedding_value(i: usize) -> Value {
    Value::Embedding(u8::try_from(i).unwrap_or(u8::MAX))
}

#[cfg(test)]
mod tests;
