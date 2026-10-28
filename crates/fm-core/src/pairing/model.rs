//! The local pairing model (E33.2): a small logistic model trained on the
//! curated pairings of `data/pairings`, blended into the rules' ranking.
//!
//! - [`curated`] reads `pairings.json`: pairs of family names that go well
//!   together, each with a short source note.
//! - [`features`] describes one pair of faces as [`FEATURES`] numbers: the
//!   classification pair, the x-height ratio, the weight and width gaps,
//!   the same designer or superfamily, and the fingerprint distance when
//!   both faces have one.
//! - [`fit`] fits the weights by gradient descent from zero, a fixed number
//!   of steps: the same examples give the same model on every OS.
//!   [`Index::train_pairing_model`](crate::index::Index::train_pairing_model)
//!   builds the examples from an index: the curated pairs are the positives,
//!   non-pairs drawn with a fixed seed ([`SEED`]) the negatives.
//! - [`embedded`] is the committed `model.json`, inside the binary: no
//!   download (rule 7). `cargo run -p fm-core --example train_pairings`
//!   writes it from `fixtures/fonts/`; a test fails when it is stale.
//!
//! Every number comes from IEEE `+ - * /` and a series [`exp`], in a fixed
//! order, so one corpus trains the same weights on every OS.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::OnceLock;

use fm_api::common::Classification;
use serde::Deserialize;

use super::{PairFacts, contrast, same_text, superfamily};
use crate::error::{FmError, FmResult};

/// The value `pairings.json` has in `$schema`.
const PAIRINGS_SCHEMA_REF: &str = "./pairings.schema.json";
/// The value `model.json` has in `$schema`.
const MODEL_SCHEMA_REF: &str = "./model.schema.json";
/// The format version of `model.json`.
const MODEL_VERSION: u32 = 1;

/// The curated pairings, embedded at build time.
const PAIRINGS_FILE: (&str, &str) = (
    "pairings.json",
    include_str!("../../../../data/pairings/pairings.json"),
);
/// The trained model, embedded at build time.
const MODEL_FILE: (&str, &str) = (
    "model.json",
    include_str!("../../../../data/pairings/model.json"),
);

/// The seed of the negative sampling.
pub const SEED: u64 = 0xE332;
/// Non-pairs drawn per curated pair and direction.
pub const NEGATIVES_PER_POSITIVE: usize = 6;
/// The model's share of a blended score; the rules have the rest.
pub const MODEL_SHARE: f32 = 0.5;
/// The model weighs in, and says so, from this probability on.
pub const MODEL_SAYS_PAIR: f32 = 0.5;
/// The reason a suggestion gains when the model weighs in.
pub const MODEL_REASON: &str = "often paired in curated pairings";

/// Gradient-descent steps, step size and L2 penalty of [`fit`].
const STEPS: usize = 3000;
const STEP_SIZE: f64 = 0.5;
const L2: f64 = 0.001;

/// The pair features, in the order [`features`] computes them.
pub const FEATURES: [&str; 11] = [
    "bias",
    "text_contrast",
    "accent_contrast",
    "same_class",
    "x_height_closeness",
    "weight_gap",
    "width_gap",
    "same_designer",
    "same_superfamily",
    "fingerprint_closeness",
    "fingerprints",
];

/// One pair's feature values, in [`FEATURES`] order.
pub type Features = [f64; FEATURES.len()];

/// What the model looks at in one face.
#[derive(Debug, Clone, Copy)]
pub struct FaceFeatures<'a> {
    /// What the rules look at.
    pub facts: PairFacts<'a>,
    /// OS/2 weight class.
    pub weight: u16,
    /// OS/2 width class (1–9).
    pub width: u16,
    /// Its glyph fingerprint vector, when it has one.
    pub fingerprint: Option<&'a [f32]>,
}

/// The features of the pair `a` / `b`; each in `0.0..=1.0`. Symmetric.
pub fn features(a: &FaceFeatures<'_>, b: &FaceFeatures<'_>) -> Features {
    use Classification::{Display, Sans, Script, Serif, Slab};
    let flag = |held: bool| if held { 1.0 } else { 0.0 };
    let (class_a, class_b) = (a.facts.class, b.facts.class);
    let (text, accent) = match (class_a, class_b) {
        (Some(x), Some(y)) if contrast(x, y).is_some() => {
            let text = matches!((x, y), (Serif | Slab, Sans) | (Sans, Serif | Slab));
            let accent = matches!((x, y), (Display | Script, _) | (_, Display | Script));
            (flag(text), flag(accent && !text))
        }
        _ => (0.0, 0.0),
    };
    let x_height = match (a.facts.x_height, b.facts.x_height) {
        (Some(x), Some(y)) if x > 0.0 && y > 0.0 => f64::from(x.min(y)) / f64::from(x.max(y)),
        _ => 0.0,
    };
    let gap = |x: u16, y: u16, span: f64| (f64::from(x.abs_diff(y)) / span).min(1.0);
    let shapes = match (a.fingerprint, b.fingerprint) {
        (Some(x), Some(y)) if x.len() == y.len() && !x.is_empty() => {
            let total: f64 = x.iter().zip(y).map(|(p, q)| f64::from((p - q).abs())).sum();
            Some((1.0 - total / x.len() as f64).clamp(0.0, 1.0))
        }
        _ => None,
    };
    [
        1.0,
        text,
        accent,
        flag(class_a.is_some() && class_a == class_b),
        x_height,
        gap(a.weight, b.weight, 1000.0),
        gap(a.width, b.width, 8.0),
        flag(shared_designer(a.facts.designer, b.facts.designer)),
        flag(related_families(a.facts.family, b.facts.family)),
        shapes.unwrap_or(0.0),
        flag(shapes.is_some()),
    ]
}

/// The designers share a name: "Eduardo Tunni / Pablo Cosgaya" and
/// "Pablo Cosgaya" do.
fn shared_designer(a: Option<&str>, b: Option<&str>) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        return false;
    };
    let names = |s: &str| -> BTreeSet<String> {
        s.to_lowercase()
            .replace(" and ", ",")
            .replace(" with ", ",")
            .replace(" - ", ",")
            .split([',', '&', '/', '|', ';'])
            .map(|n| n.trim().trim_end_matches('.').to_owned())
            .filter(|n| n.chars().count() >= 3)
            .collect()
    };
    !names(a).is_disjoint(&names(b))
}

/// Two different families of one superfamily, or whose names start with the
/// same word ("Cossette Titre" / "Cossette Texte").
fn related_families(a: &str, b: &str) -> bool {
    if same_text(a, b) {
        return false;
    }
    if superfamily(a).is_some_and(|s| superfamily(b) == Some(s)) {
        return true;
    }
    let first = |s: &str| s.split_whitespace().next().map(str::to_lowercase);
    first(a).is_some_and(|w| w.chars().count() >= 3 && first(b) == Some(w))
}

/// The pairing model: one weight per feature.
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    /// The seed the negatives were drawn with.
    pub seed: u64,
    /// One weight per [`FEATURES`] entry.
    pub weights: Features,
}

impl Model {
    /// The probability that the pair `x` describes is a pairing.
    pub fn probability(&self, x: &Features) -> f32 {
        sigmoid(dot(&self.weights, x)) as f32
    }

    /// The model as `model.json` holds it: weights to six decimals.
    pub fn to_json(&self) -> String {
        let quoted: Vec<String> = FEATURES.iter().map(|f| format!("\"{f}\"")).collect();
        let weights: Vec<String> = self.weights.iter().map(|w| format!("{w:.6}")).collect();
        format!(
            "{{\n  \"$schema\": \"{MODEL_SCHEMA_REF}\",\n  \"version\": {MODEL_VERSION},\n  \
             \"seed\": {},\n  \"features\": [{}],\n  \"weights\": [{}]\n}}\n",
            self.seed,
            quoted.join(", "),
            weights.join(", ")
        )
    }

    /// Parses and checks a model file: the features the core computes, in
    /// its order, and a finite weight each.
    pub fn from_json(file: &str, json: &str) -> FmResult<Self> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            #[serde(rename = "$schema")]
            schema: String,
            version: u32,
            seed: u64,
            features: Vec<String>,
            weights: Vec<f64>,
        }
        let raw: Raw = serde_json::from_str(json).map_err(|e| damaged(file, &e.to_string()))?;
        if raw.schema != MODEL_SCHEMA_REF {
            return Err(damaged(
                file,
                &format!("$schema must be \"{MODEL_SCHEMA_REF}\""),
            ));
        }
        if raw.version != MODEL_VERSION {
            return Err(damaged(file, &format!("version must be {MODEL_VERSION}")));
        }
        if raw.features != FEATURES {
            return Err(damaged(
                file,
                &format!("features must be {FEATURES:?}; retrain the model"),
            ));
        }
        if raw.weights.iter().any(|w| !w.is_finite()) {
            return Err(damaged(file, "a weight is not a finite number"));
        }
        let weights: Features = raw
            .weights
            .try_into()
            .map_err(|_| damaged(file, "one weight per feature"))?;
        Ok(Self {
            seed: raw.seed,
            weights,
        })
    }
}

/// The committed model, parsed once.
pub fn embedded() -> FmResult<&'static Model> {
    static MODEL: OnceLock<Result<Model, FmError>> = OnceLock::new();
    MODEL
        .get_or_init(|| Model::from_json(MODEL_FILE.0, MODEL_FILE.1))
        .as_ref()
        .map_err(Clone::clone)
}

/// Fits a model to labelled examples (`true`: a pairing): gradient descent
/// on the mean log loss from zero weights, [`STEPS`] steps, an L2 penalty
/// on every weight but the bias. The same examples in the same order give
/// the same weights.
pub fn fit(examples: &[(Features, bool)], seed: u64) -> Model {
    let mut weights: Features = [0.0; FEATURES.len()];
    if examples.is_empty() {
        return Model { seed, weights };
    }
    let n = examples.len() as f64;
    for _ in 0..STEPS {
        let mut grad: Features = [0.0; FEATURES.len()];
        for (x, label) in examples {
            let err = sigmoid(dot(&weights, x)) - if *label { 1.0 } else { 0.0 };
            for (g, v) in grad.iter_mut().zip(x) {
                *g += err * v;
            }
        }
        for (i, (w, g)) in weights.iter_mut().zip(grad).enumerate() {
            let penalty = if i == 0 { 0.0 } else { L2 * *w };
            *w -= STEP_SIZE * (g / n + penalty);
        }
    }
    Model { seed, weights }
}

fn dot(w: &Features, x: &Features) -> f64 {
    w.iter().zip(x).map(|(a, b)| a * b).sum()
}

fn sigmoid(z: f64) -> f64 {
    1.0 / (1.0 + exp(-z))
}

/// `e^x` from `+ - * /` only, the same on every OS: a Taylor series of
/// `e^(x/64)`, squared six times. Clamped to `x` in `-40..=40`.
fn exp(x: f64) -> f64 {
    let r = x.clamp(-40.0, 40.0) / 64.0;
    let (mut term, mut sum) = (1.0, 1.0);
    for k in 1..=16 {
        term = term * r / f64::from(k);
        sum += term;
    }
    (0..6).fold(sum, |y, _| y * y)
}

/// A small deterministic generator (SplitMix64) for the negative sampling.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// A generator from `seed`.
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next number.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Up to `k` distinct items of `items`, in draw order.
    pub fn sample<T: Copy>(&mut self, items: &[T], k: usize) -> Vec<T> {
        let mut pool = items.to_vec();
        let k = k.min(pool.len());
        for i in 0..k {
            let span = (pool.len() - i) as u64;
            let j = i + (self.next_u64() % span) as usize;
            pool.swap(i, j);
        }
        pool.truncate(k);
        pool
    }
}

/// One curated pairing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Curated {
    /// The two family names.
    pub families: [String; 2],
    /// Where the pairing comes from.
    pub source: String,
}

/// Every curated pairing, in file order.
pub fn curated() -> FmResult<Vec<Curated>> {
    parse_curated(PAIRINGS_FILE.0, PAIRINGS_FILE.1)
}

/// Parses and checks a curated-pairings file: every rule of
/// `pairings.schema.json`, plus no pair of one family and no pair twice.
fn parse_curated(file: &str, json: &str) -> FmResult<Vec<Curated>> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RawFile {
        #[serde(rename = "$schema")]
        schema: String,
        pairings: Vec<RawPairing>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RawPairing {
        families: [String; 2],
        source: String,
    }
    let raw: RawFile = serde_json::from_str(json).map_err(|e| damaged(file, &e.to_string()))?;
    if raw.schema != PAIRINGS_SCHEMA_REF {
        return Err(damaged(
            file,
            &format!("$schema must be \"{PAIRINGS_SCHEMA_REF}\""),
        ));
    }
    if raw.pairings.is_empty() {
        return Err(damaged(file, "pairings is empty"));
    }
    let trimmed = |s: &str| !s.is_empty() && s.trim() == s;
    let mut seen = BTreeSet::new();
    raw.pairings
        .into_iter()
        .map(|p| {
            let [a, b] = &p.families;
            let label = format!("{a} / {b}");
            if !trimmed(a) || !trimmed(b) {
                return Err(damaged(
                    file,
                    &format!("{label}: a family name is empty or has white space at an end"),
                ));
            }
            if same_text(a, b) {
                return Err(damaged(file, &format!("{label}: one family twice")));
            }
            let key = {
                let (x, y) = (a.to_lowercase(), b.to_lowercase());
                if x < y { (x, y) } else { (y, x) }
            };
            if !seen.insert(key) {
                return Err(damaged(file, &format!("{label}: listed twice")));
            }
            if !trimmed(&p.source) || p.source.chars().count() > 120 {
                return Err(damaged(
                    file,
                    &format!(
                        "{label}: source is empty, longer than 120 characters or has white space at an end"
                    ),
                ));
            }
            Ok(Curated {
                families: p.families,
                source: p.source,
            })
        })
        .collect()
}

/// Trains the model on the font files under `root`, as the
/// `train_pairings` example and the staleness test do: the files in path
/// order, one in-memory index, the curated pairings, [`SEED`].
pub fn train_on_folder(root: &Path) -> FmResult<Model> {
    use crate::scanner::{IndexedFile, discover, hash_and_parse};
    let found = discover(root)?;
    let files = found
        .files
        .iter()
        .map(|f| {
            let (hash, parsed) = hash_and_parse(&f.path)?;
            Ok(IndexedFile {
                path: f.path.clone(),
                size: f.size,
                modified: f.modified,
                hash,
                parsed,
                gone: false,
            })
        })
        .collect::<FmResult<Vec<_>>>()?;
    let mut index = crate::index::Index::open_in_memory()?;
    let source = index
        .add_source(
            &root.to_string_lossy(),
            fm_api::library::SourceKind::Watched,
        )?
        .id;
    index.persist_files(source, &files)?;
    index.train_pairing_model(&curated()?, SEED)
}

fn damaged(file: &str, detail: &str) -> FmError {
    FmError::internal(format!("data/pairings/{file}: {detail}"))
}

#[cfg(test)]
mod tests;
