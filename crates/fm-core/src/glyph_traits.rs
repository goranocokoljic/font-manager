//! The glyph trait catalog: `data/classification/glyph-traits.json` (E49.2).
//!
//! A glyph trait is one distinctive feature of one letter or figure — the
//! leg of the R, the storeys of the g — that the detectors compute and the
//! letter picker offers. The catalog lists every trait with its values, their
//! labels in English and German, and a small drawn example per value, so the
//! picker never depends on the fonts the library holds.
//!
//! The file is embedded at build time. [`load`] parses it and checks every
//! rule of `glyph-traits.schema.json` plus the ones a schema cannot say: ids
//! unique, the code point and the glyph agree, the id starts with the glyph's
//! name, the scalar thresholds reference declared measures and end in a
//! catch-all value, and every example is a path inside the 56-unit box.
//!
//! `fixtures/glyph-traits-labels.json` hand-labels the traits for fonts of
//! the fixture corpus; the tests below check it against this catalog.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use serde::Deserialize;

use crate::error::{FmError, FmResult};

/// The value the data file has in `$schema`.
const SCHEMA_REF: &str = "./glyph-traits.schema.json";

/// The embedded catalog.
const FILE: &str = include_str!("../../../data/classification/glyph-traits.json");

/// The side of the square box the examples are drawn in.
pub const EXAMPLE_BOX: f64 = 56.0;

/// The languages every label must have.
const REQUIRED_LANGUAGES: [&str; 2] = ["en", "de"];

/// The longest example path, in bytes.
const MAX_EXAMPLE_LEN: usize = 400;

/// Text by two-letter language code (`en`, `de`).
pub type Labels = BTreeMap<String, String>;

/// How a detector reports a trait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TraitKind {
    /// The detector picks one of the values.
    Enum,
    /// The detector measures numbers; [`GlyphTrait::chip`] maps them to a value.
    Scalar,
}

/// One number a scalar trait measures.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measure {
    /// Unique within the trait (`roundness`).
    pub id: String,
    /// What the number is, in one line.
    pub description: String,
    /// The least value the detector reports.
    pub min: f64,
    /// The greatest value the detector reports.
    pub max: f64,
}

/// One threshold of a scalar value: `min <= measure < max`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Threshold {
    /// The id of the measure.
    pub measure: String,
    /// Inclusive lower bound.
    #[serde(default)]
    pub min: Option<f64>,
    /// Exclusive upper bound.
    #[serde(default)]
    pub max: Option<f64>,
}

impl Threshold {
    fn holds(&self, value: f64) -> bool {
        self.min.is_none_or(|min| value >= min) && self.max.is_none_or(|max| value < max)
    }
}

/// One value of a trait: a chip in the picker.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraitValue {
    /// Unique within the trait (`straight`).
    pub id: String,
    /// The chip text by language.
    pub label: Labels,
    /// What the value looks like, in one line.
    pub description: String,
    /// Scalar traits only: all must hold for this value. Empty on the last
    /// value of a scalar trait, which catches the rest; empty on enum values.
    #[serde(default)]
    pub when: Vec<Threshold>,
    /// The example glyph: an SVG path, stroked, in a [`EXAMPLE_BOX`] box.
    pub example: String,
}

/// One trait of one glyph.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphTrait {
    /// The glyph's name and the trait (`R.leg`, `one.flag`).
    pub id: String,
    /// ISO 15924 script of the glyph (`Latn`).
    pub script: String,
    /// The character the trait is measured on.
    pub glyph: char,
    /// How the detector reports it.
    pub kind: TraitKind,
    /// The trait's name by language.
    pub name: Labels,
    /// What the trait is, in one line.
    pub description: String,
    /// Scalar traits: the numbers measured. Empty for an enum trait.
    pub measures: Vec<Measure>,
    /// The values, in picker order (for a scalar trait, in threshold order).
    pub values: Vec<TraitValue>,
}

impl GlyphTrait {
    /// The value with this id.
    pub fn value(&self, id: &str) -> Option<&TraitValue> {
        self.values.iter().find(|v| v.id == id)
    }

    /// The chip of a scalar trait for the measured numbers: the first value
    /// whose thresholds all hold. A threshold on a measure that is not given
    /// does not hold. `None` for an enum trait.
    pub fn chip(&self, measured: &[(&str, f64)]) -> Option<&TraitValue> {
        if self.kind != TraitKind::Scalar {
            return None;
        }
        self.values.iter().find(|value| {
            value.when.iter().all(|t| {
                measured
                    .iter()
                    .find(|(id, _)| *id == t.measure)
                    .is_some_and(|(_, v)| t.holds(*v))
            })
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(rename = "$schema")]
    schema: String,
    traits: Vec<RawTrait>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTrait {
    id: String,
    script: String,
    glyph: String,
    codepoint: String,
    kind: TraitKind,
    name: Labels,
    description: String,
    #[serde(default)]
    measures: Option<Vec<Measure>>,
    values: Vec<RawValue>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawValue {
    id: String,
    label: Labels,
    description: String,
    #[serde(default)]
    when: Option<Vec<Threshold>>,
    example: String,
}

static CATALOG: LazyLock<FmResult<Vec<GlyphTrait>>> = LazyLock::new(|| parse(FILE));

/// Every trait of the catalog, parsed once, in file order.
pub fn load() -> FmResult<&'static [GlyphTrait]> {
    CATALOG.as_ref().map(Vec::as_slice).map_err(Clone::clone)
}

/// The trait with this id.
pub fn find(id: &str) -> FmResult<Option<&'static GlyphTrait>> {
    Ok(load()?.iter().find(|t| t.id == id))
}

/// Parses and checks the catalog.
fn parse(json: &str) -> FmResult<Vec<GlyphTrait>> {
    let raw: RawFile = serde_json::from_str(json).map_err(|e| damaged(&e.to_string()))?;
    if raw.schema != SCHEMA_REF {
        return Err(damaged(&format!("$schema must be \"{SCHEMA_REF}\"")));
    }
    if raw.traits.is_empty() {
        return Err(damaged("traits is empty"));
    }
    let mut ids = BTreeSet::new();
    let mut traits = Vec::with_capacity(raw.traits.len());
    for raw in raw.traits {
        let id = raw.id.clone();
        if !ids.insert(id.clone()) {
            return Err(damaged(&format!("trait {id} is listed twice")));
        }
        traits.push(check_trait(raw).map_err(|why| damaged(&format!("{id}: {why}")))?);
    }
    Ok(traits)
}

fn check_trait(raw: RawTrait) -> Result<GlyphTrait, String> {
    let glyph = single_char(&raw.glyph).ok_or("glyph must be exactly one character")?;
    if raw.codepoint != format!("U+{:04X}", u32::from(glyph)) {
        return Err(format!(
            "codepoint {} is not the glyph {glyph:?}",
            raw.codepoint
        ));
    }
    let (prefix, suffix) = raw
        .id
        .split_once('.')
        .ok_or("id must be <glyph name>.<trait>")?;
    if !is_name(prefix, |c| c.is_ascii_alphabetic()) || !is_name(suffix, |c| c.is_ascii_lowercase())
    {
        return Err("id must be <glyph name>.<trait> in ASCII letters and digits".into());
    }
    if let Some(name) = ascii_glyph_name(glyph)
        && prefix != name
    {
        return Err(format!("id must start with {name}, the glyph's name"));
    }
    if !is_script(&raw.script) {
        return Err(format!("script {:?} is not an ISO 15924 code", raw.script));
    }
    check_labels(&raw.name).map_err(|why| format!("name: {why}"))?;
    check_line(&raw.description).map_err(|why| format!("description: {why}"))?;
    if raw.values.len() < 2 {
        return Err("a trait needs at least two values".into());
    }
    let measures = match (raw.kind, raw.measures) {
        (TraitKind::Enum, None) => Vec::new(),
        (TraitKind::Enum, Some(_)) => return Err("an enum trait has no measures".into()),
        (TraitKind::Scalar, None) => return Err("a scalar trait needs measures".into()),
        (TraitKind::Scalar, Some(m)) => check_measures(m)?,
    };
    let count = raw.values.len();
    let mut values: Vec<TraitValue> = Vec::with_capacity(count);
    for (at, raw_value) in raw.values.into_iter().enumerate() {
        let id = raw_value.id.clone();
        let value = check_value(raw_value, raw.kind, &measures, at + 1 == count)
            .map_err(|why| format!("value {id}: {why}"))?;
        if values.iter().any(|v| v.id == value.id) {
            return Err(format!("value {id} is listed twice"));
        }
        if values.iter().any(|v| v.example == value.example) {
            return Err(format!("value {id} has the example of another value"));
        }
        values.push(value);
    }
    Ok(GlyphTrait {
        id: raw.id,
        script: raw.script,
        glyph,
        kind: raw.kind,
        name: raw.name,
        description: raw.description,
        measures,
        values,
    })
}

fn check_measures(measures: Vec<Measure>) -> Result<Vec<Measure>, String> {
    if measures.is_empty() {
        return Err("a scalar trait needs measures".into());
    }
    for (at, m) in measures.iter().enumerate() {
        if !is_name(&m.id, |c| c.is_ascii_lowercase()) {
            return Err(format!("measure id {:?} is not a lowercase name", m.id));
        }
        if measures[..at].iter().any(|o| o.id == m.id) {
            return Err(format!("measure {} is listed twice", m.id));
        }
        check_line(&m.description).map_err(|why| format!("measure {}: {why}", m.id))?;
        if !(m.min.is_finite() && m.max.is_finite() && m.min < m.max) {
            return Err(format!("measure {}: min must be below max", m.id));
        }
    }
    Ok(measures)
}

fn check_value(
    raw: RawValue,
    kind: TraitKind,
    measures: &[Measure],
    last: bool,
) -> Result<TraitValue, String> {
    if !is_value_id(&raw.id) {
        return Err("id must be lowercase letters, digits and -".into());
    }
    check_labels(&raw.label).map_err(|why| format!("label: {why}"))?;
    check_line(&raw.description).map_err(|why| format!("description: {why}"))?;
    check_example(&raw.example).map_err(|why| format!("example: {why}"))?;
    let when = match (kind, raw.when) {
        (TraitKind::Enum, None) => Vec::new(),
        (TraitKind::Enum, Some(_)) => return Err("an enum value has no thresholds".into()),
        (TraitKind::Scalar, None) => return Err("a scalar value needs when".into()),
        (TraitKind::Scalar, Some(when)) => {
            if last != when.is_empty() {
                return Err(
                    "only the last value of a scalar trait has an empty when, and it must".into(),
                );
            }
            for t in &when {
                let measure = measures
                    .iter()
                    .find(|m| m.id == t.measure)
                    .ok_or_else(|| format!("threshold on unknown measure {}", t.measure))?;
                check_threshold(t, measure)?;
            }
            when
        }
    };
    Ok(TraitValue {
        id: raw.id,
        label: raw.label,
        description: raw.description,
        when,
        example: raw.example,
    })
}

fn check_threshold(t: &Threshold, measure: &Measure) -> Result<(), String> {
    if t.min.is_none() && t.max.is_none() {
        return Err(format!(
            "threshold on {} has neither min nor max",
            t.measure
        ));
    }
    for bound in [t.min, t.max].into_iter().flatten() {
        if !(bound.is_finite() && (measure.min..=measure.max).contains(&bound)) {
            return Err(format!(
                "threshold {bound} is outside {} ({}–{})",
                t.measure, measure.min, measure.max
            ));
        }
    }
    if let (Some(min), Some(max)) = (t.min, t.max)
        && min >= max
    {
        return Err(format!("threshold on {}: min must be below max", t.measure));
    }
    Ok(())
}

/// The numbers each SVG path command takes per segment.
fn arity(command: char) -> Option<usize> {
    Some(match command {
        'M' | 'L' | 'T' => 2,
        'H' | 'V' => 1,
        'C' => 6,
        'S' | 'Q' => 4,
        'A' => 7,
        'Z' => 0,
        _ => return None,
    })
}

/// Checks an example path: absolute commands only, the right count of
/// numbers for each, arc flags 0 or 1, every number inside the box.
fn check_example(path: &str) -> Result<(), String> {
    if path.len() > MAX_EXAMPLE_LEN {
        return Err(format!("longer than {MAX_EXAMPLE_LEN} bytes"));
    }
    if !path.starts_with('M') {
        return Err("must start with M".into());
    }
    let mut segments: Vec<(char, Vec<f64>)> = Vec::new();
    let mut number = String::new();
    let flush = |number: &mut String, segments: &mut Vec<(char, Vec<f64>)>| -> Result<(), String> {
        if number.is_empty() {
            return Ok(());
        }
        let value: f64 = number
            .parse()
            .map_err(|_| format!("{number:?} is not a number"))?;
        number.clear();
        segments
            .last_mut()
            .ok_or("a number before the first command")?
            .1
            .push(value);
        Ok(())
    };
    for c in path.chars() {
        match c {
            '0'..='9' | '.' => number.push(c),
            '-' => {
                flush(&mut number, &mut segments)?;
                number.push(c);
            }
            ' ' | ',' => flush(&mut number, &mut segments)?,
            _ if arity(c).is_some() => {
                flush(&mut number, &mut segments)?;
                segments.push((c, Vec::new()));
            }
            _ => return Err(format!("{c:?} is not an absolute path command")),
        }
    }
    flush(&mut number, &mut segments)?;
    for (command, numbers) in &segments {
        let n = arity(*command).unwrap_or(0);
        let fits = if n == 0 {
            numbers.is_empty()
        } else {
            !numbers.is_empty() && numbers.len() % n == 0
        };
        if !fits {
            return Err(format!("{command} has {} numbers", numbers.len()));
        }
        for (at, value) in numbers.iter().enumerate() {
            if !(0.0..=EXAMPLE_BOX).contains(value) {
                return Err(format!("{value} is outside the {EXAMPLE_BOX}-unit box"));
            }
            if *command == 'A' && matches!(at % 7, 3 | 4) && *value != 0.0 && *value != 1.0 {
                return Err(format!("arc flag {value} is not 0 or 1"));
            }
        }
    }
    Ok(())
}

fn check_labels(labels: &Labels) -> Result<(), String> {
    for language in REQUIRED_LANGUAGES {
        if !labels.contains_key(language) {
            return Err(format!("no {language} text"));
        }
    }
    for (language, text) in labels {
        if language.len() != 2 || !language.bytes().all(|b| b.is_ascii_lowercase()) {
            return Err(format!("{language:?} is not a two-letter language code"));
        }
        check_line(text).map_err(|why| format!("{language}: {why}"))?;
    }
    Ok(())
}

/// Non-empty, one line, no spaces around it.
fn check_line(text: &str) -> Result<(), String> {
    if text.is_empty() || text.trim() != text || text.contains('\n') {
        return Err("must be one non-empty line without spaces around it".into());
    }
    Ok(())
}

fn single_char(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let c = chars.next()?;
    chars.next().is_none().then_some(c)
}

/// The glyph name an id must start with, for ASCII letters and figures; other
/// glyphs (Cyrillic, Greek) choose their own ASCII name.
fn ascii_glyph_name(glyph: char) -> Option<String> {
    const FIGURES: [&str; 10] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
    ];
    if glyph.is_ascii_alphabetic() {
        Some(glyph.to_string())
    } else {
        glyph
            .to_digit(10)
            .filter(|_| glyph.is_ascii())
            .and_then(|d| FIGURES.get(d as usize))
            .map(|name| (*name).to_string())
    }
}

/// A name: a first character that passes `first`, then ASCII letters and digits.
fn is_name(text: &str, first: fn(&char) -> bool) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| first(&c)) && chars.all(|c| c.is_ascii_alphanumeric())
}

/// The schema's `^[a-z][a-z0-9-]*$`.
fn is_value_id(id: &str) -> bool {
    id.starts_with(|c: char| c.is_ascii_lowercase())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The schema's `^[A-Z][a-z]{3}$`.
fn is_script(script: &str) -> bool {
    let b = script.as_bytes();
    b.len() == 4 && b[0].is_ascii_uppercase() && b[1..].iter().all(u8::is_ascii_lowercase)
}

fn damaged(detail: &str) -> FmError {
    FmError::internal(format!("data/classification/glyph-traits.json: {detail}"))
}

#[cfg(test)]
mod tests;
