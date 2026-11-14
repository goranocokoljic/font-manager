//! Foundry names (E50.11): the raw foundry strings of a font's name table
//! folded into one canonical name per foundry.
//!
//! A face's foundry is read from, in order: the manufacturer (name ID 8),
//! the trademark notice (name ID 7), the copyright notice (name ID 0) and
//! the designer (name ID 9). A notice gives a name only through one of the
//! data file's patterns ("Copyright (c) {years} {name}"); a name the data
//! file ignores ("The Karla Project Authors") gives none, and the next
//! string is tried.
//!
//! Names compare as keys ([`key`]): lower case, every run of characters that
//! are not letters or digits one space, the legal suffixes at the end
//! dropped, so `Google LLC`, `GOOGLE` and `google, inc.` are one foundry.
//! The data file's alias table folds the rest (`The Google Font Team` →
//! `Google`). A key the table does not know is its own foundry, named by the
//! spelling most of its faces use.
//!
//! The data is `data/foundries.json`, embedded at build time. The patterns
//! are a small syntax of the file's own, not regular expressions: text
//! matches without case, a space matches any run of white space or none,
//! `{years}` one or more years, `{*}` any text, `{name}` the name up to the
//! first stop.
//!
//! [`Table::build`] groups the faces of a library and applies the user's
//! merges (stored in the index, `foundry_merge`).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::LazyLock;

use serde::Deserialize;

use crate::error::{FmError, FmResult};

/// The value the data file has in `$schema`.
const SCHEMA_REF: &str = "./foundries.schema.json";

/// The data file, embedded at build time.
const FILE: (&str, &str) = (
    "foundries.json",
    include_str!("../../../data/foundries.json"),
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawFile {
    #[serde(rename = "$schema")]
    schema: String,
    suffixes: Vec<String>,
    ignore: Vec<String>,
    patterns: Vec<String>,
    name_stops: Vec<String>,
    notice_stops: Vec<String>,
    foundries: Vec<RawFoundry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFoundry {
    name: String,
    aliases: Vec<String>,
}

/// One piece of a notice pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    /// A character, matched without case.
    Char(char),
    /// Any run of white space, or none.
    Space,
    /// One or more years: `2011`, `2015-2022`, `2011, 2012`.
    Years,
    /// Any text.
    Any,
    /// The name: the rest of the text, up to the first stop.
    Name,
}

/// The loaded data file.
#[derive(Debug)]
struct Catalog {
    suffixes: BTreeSet<String>,
    /// Ignored keys: `(true, x)` ignores every key ending in `x`.
    ignore: Vec<(bool, String)>,
    patterns: Vec<Vec<Token>>,
    name_stops: Vec<String>,
    /// The name stops and the notice stops.
    notice_stops: Vec<String>,
    /// The canonical name of every name and alias, by key.
    canonical: HashMap<String, String>,
}

/// The catalog, loaded once. The data is embedded and its tests reject a
/// broken file, so a load error is a build defect: it is logged, and every
/// name is its own foundry.
static CATALOG: LazyLock<Catalog> = LazyLock::new(|| match parse(FILE.0, FILE.1) {
    Ok(catalog) => catalog,
    Err(e) => {
        tracing::error!(error = %e, "foundry names could not be loaded");
        Catalog {
            suffixes: BTreeSet::new(),
            ignore: Vec::new(),
            patterns: Vec::new(),
            name_stops: Vec::new(),
            notice_stops: Vec::new(),
            canonical: HashMap::new(),
        }
    }
});

fn damaged(file: &str, detail: &str) -> FmError {
    FmError::internal(format!("data/{file}: {detail}"))
}

/// Parses and checks the data file: every rule of `foundries.schema.json`,
/// plus the sort orders and the one-foundry-per-key rule.
fn parse(file: &str, json: &str) -> FmResult<Catalog> {
    let raw: RawFile = serde_json::from_str(json).map_err(|e| damaged(file, &e.to_string()))?;
    if raw.schema != SCHEMA_REF {
        return Err(damaged(file, &format!("$schema must be \"{SCHEMA_REF}\"")));
    }
    let word = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_lowercase() || c.is_numeric());
    if let Some(bad) = raw.suffixes.iter().find(|s| !word(s)) {
        return Err(damaged(
            file,
            &format!("suffix {bad:?} is not one lower-case word"),
        ));
    }
    sorted_unique(file, "suffixes", &raw.suffixes)?;
    sorted_unique(file, "ignore", &raw.ignore)?;
    let mut ignore = Vec::with_capacity(raw.ignore.len());
    for entry in &raw.ignore {
        let (any_start, text) = match entry.strip_prefix('*') {
            Some(rest) => (true, rest),
            None => (false, entry.as_str()),
        };
        if text.is_empty() || !text.split(' ').all(word) {
            return Err(damaged(file, &format!("ignore {entry:?} is not a key")));
        }
        ignore.push((any_start, text.to_owned()));
    }
    if raw.patterns.is_empty() {
        return Err(damaged(file, "patterns is empty"));
    }
    let patterns = raw
        .patterns
        .iter()
        .map(|p| tokens(p).map_err(|why| damaged(file, &format!("pattern {p:?}: {why}"))))
        .collect::<FmResult<Vec<_>>>()?;
    for stop in raw.name_stops.iter().chain(&raw.notice_stops) {
        if stop.is_empty() {
            return Err(damaged(file, "a stop is empty"));
        }
    }
    if raw.foundries.is_empty() {
        return Err(damaged(file, "foundries is empty"));
    }
    if let Some(w) = raw
        .foundries
        .windows(2)
        .find(|w| w[0].name.to_lowercase() >= w[1].name.to_lowercase())
    {
        return Err(damaged(
            file,
            &format!("{}: name is out of order or listed twice", w[1].name),
        ));
    }
    let suffixes: BTreeSet<String> = raw.suffixes.into_iter().collect();
    let mut canonical: HashMap<String, String> = HashMap::new();
    for foundry in &raw.foundries {
        for text in std::iter::once(&foundry.name).chain(&foundry.aliases) {
            if text.trim().is_empty() || text.trim() != text {
                return Err(damaged(
                    file,
                    &format!(
                        "{}: {text:?} is empty or has white space at an end",
                        foundry.name
                    ),
                ));
            }
            let k = key_with(text, &suffixes);
            if k.is_empty() {
                return Err(damaged(file, &format!("{text:?} has no letters or digits")));
            }
            if let Some(other) = canonical.insert(k.clone(), foundry.name.clone())
                && other != foundry.name
            {
                return Err(damaged(
                    file,
                    &format!("{text:?} ({k}) belongs to {other} and {}", foundry.name),
                ));
            }
        }
    }
    let mut notice_stops = raw.name_stops.clone();
    notice_stops.extend(raw.notice_stops);
    Ok(Catalog {
        suffixes,
        ignore,
        patterns,
        name_stops: raw.name_stops,
        notice_stops,
        canonical,
    })
}

fn sorted_unique(file: &str, field: &str, list: &[String]) -> FmResult<()> {
    match list.windows(2).find(|w| w[0] >= w[1]) {
        Some(w) => Err(damaged(
            file,
            &format!("{field}: {:?} is out of order or listed twice", w[1]),
        )),
        None => Ok(()),
    }
}

/// The tokens of a notice pattern; exactly one `{name}`, at the end.
fn tokens(pattern: &str) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let mut rest = pattern;
    while let Some(c) = rest.chars().next() {
        if c == '{' {
            let end = rest.find('}').ok_or("a { is not closed")?;
            out.push(match &rest[1..end] {
                "years" => Token::Years,
                "*" => Token::Any,
                "name" => Token::Name,
                other => return Err(format!("{{{other}}} is not a placeholder")),
            });
            rest = &rest[end + 1..];
        } else if c == '}' {
            return Err("a } is not opened".into());
        } else {
            if c.is_whitespace() {
                if out.last() != Some(&Token::Space) {
                    out.push(Token::Space);
                }
            } else {
                out.extend(c.to_lowercase().map(Token::Char));
            }
            rest = &rest[c.len_utf8()..];
        }
    }
    let names = out.iter().filter(|t| **t == Token::Name).count();
    if names != 1 || out.last() != Some(&Token::Name) {
        return Err("it must end in its one {name}".into());
    }
    Ok(out)
}

/// Where `tokens` (from `t`) matches `text` (from `at`): the start of the name.
fn matches(tokens: &[Token], t: usize, text: &[char], at: usize) -> Option<usize> {
    match tokens.get(t)? {
        Token::Name => Some(at),
        Token::Char(c) => {
            let next = *text.get(at)?;
            let same = next == *c || next.to_lowercase().eq(std::iter::once(*c));
            if same {
                matches(tokens, t + 1, text, at + 1)
            } else {
                None
            }
        }
        Token::Space => {
            let mut at = at;
            while text.get(at).is_some_and(|c| c.is_whitespace()) {
                at += 1;
            }
            matches(tokens, t + 1, text, at)
        }
        Token::Years => matches(tokens, t + 1, text, years(text, at)?),
        Token::Any => (at..=text.len()).find_map(|from| matches(tokens, t + 1, text, from)),
    }
}

/// The end of the years at `at`: a four-digit year, then more years or
/// ranges joined by `-`, `–`, `,` or `/` (`2015-2022`, `2011, 2012`), and a
/// comma or full stop after them. `None` when no year starts at `at`.
fn years(text: &[char], at: usize) -> Option<usize> {
    let digits = |from: usize| {
        let n = text[from.min(text.len())..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        (from + n, n)
    };
    let (mut end, n) = digits(at);
    if n != 4 {
        return None;
    }
    loop {
        let mut next = end;
        while text.get(next).is_some_and(|c| *c == ' ') {
            next += 1;
        }
        if !text
            .get(next)
            .is_some_and(|c| matches!(c, '-' | '–' | '—' | ',' | '/'))
        {
            break;
        }
        next += 1;
        while text.get(next).is_some_and(|c| *c == ' ') {
            next += 1;
        }
        let (after, n) = digits(next);
        if !(2..=4).contains(&n) {
            break;
        }
        end = after;
    }
    if text.get(end).is_some_and(|c| matches!(c, ',' | '.')) {
        end += 1;
    }
    Some(end)
}

/// The key a name compares by: lower case, every run of characters that are
/// not letters or digits made one space, the legal suffixes at the end
/// dropped (never the whole name). Empty for a name without a letter or digit.
pub(crate) fn key(name: &str) -> String {
    key_with(name, &CATALOG.suffixes)
}

fn key_with(name: &str, suffixes: &BTreeSet<String>) -> String {
    let lower = name.to_lowercase();
    let mut words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    while words.len() > 1 && words.last().is_some_and(|w| suffixes.contains(*w)) {
        words.pop();
    }
    words.join(" ")
}

/// The key a name groups under: the key of its canonical name when the
/// alias table knows it, else its own key.
pub(crate) fn group_key(name: &str) -> String {
    let k = key(name);
    match CATALOG.canonical.get(&k) {
        Some(canonical) => key(canonical),
        None => k,
    }
}

/// The canonical name the alias table gives a key.
fn canonical_of(key: &str) -> Option<&'static str> {
    CATALOG.canonical.get(key).map(String::as_str)
}

/// The strings of a face's name table a foundry is read from.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FaceNames<'a> {
    /// Name ID 8.
    pub manufacturer: Option<&'a str>,
    /// Name ID 7.
    pub trademark: Option<&'a str>,
    /// Name ID 0.
    pub copyright: Option<&'a str>,
    /// Name ID 9.
    pub designer: Option<&'a str>,
}

/// A face's foundry as its fonts write it, and the key it groups under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Identified {
    /// The name as written, cut at its first stop (`Google LLC`).
    pub raw: String,
    /// [`group_key`] of it.
    pub key: String,
}

/// The foundry a face's names state; `None` when none of them does.
pub(crate) fn identify(names: &FaceNames<'_>) -> Option<Identified> {
    let catalog = &*CATALOG;
    let plain = |text: Option<&str>| text.and_then(|t| catalog.name(t, &catalog.name_stops));
    let notice = |text: Option<&str>| text.and_then(|t| catalog.name_in_notice(t));
    plain(names.manufacturer)
        .or_else(|| notice(names.trademark))
        .or_else(|| notice(names.copyright))
        .or_else(|| plain(names.designer))
        .map(|raw| Identified {
            key: group_key(&raw),
            raw,
        })
}

impl Catalog {
    /// The name a notice states, through the first pattern that matches it.
    fn name_in_notice(&self, text: &str) -> Option<String> {
        let chars: Vec<char> = text.trim().chars().collect();
        let start = self
            .patterns
            .iter()
            .find_map(|tokens| matches(tokens, 0, &chars, 0))?;
        let rest: String = chars[start..].iter().collect();
        self.name(&rest, &self.notice_stops)
    }

    /// `text` cut at its first stop and trimmed; `None` when that leaves no
    /// letter or digit, or a name the data file ignores.
    fn name(&self, text: &str, stops: &[String]) -> Option<String> {
        let lower = text.to_lowercase();
        // Lower-casing can change byte lengths; cut only where both agree.
        let cut = stops
            .iter()
            .filter_map(|stop| lower.find(stop.as_str()))
            .filter(|&at| text.is_char_boundary(at) && lower.len() == text.len())
            .min()
            .unwrap_or(text.len());
        let name = text[..cut]
            .trim_matches(|c: char| c.is_whitespace() || matches!(c, ',' | '.' | ';' | ':' | '-'))
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let k = key_with(&name, &self.suffixes);
        let ignored = self.ignore.iter().any(|(any_start, x)| {
            if *any_start {
                k.ends_with(x.as_str())
            } else {
                k == *x
            }
        });
        (!k.is_empty() && !ignored).then_some(name)
    }
}

/// The name [`group_key`] `k` lists under: the alias table's, or the
/// spelling most of its faces use (the first A to Z on a tie).
fn display(k: &str, spellings: Option<&BTreeMap<String, usize>>) -> Option<String> {
    if let Some(name) = canonical_of(k) {
        return Some(name.to_owned());
    }
    let spellings = spellings?;
    let most = spellings.values().max()?;
    spellings
        .iter()
        .find(|(_, n)| *n == most)
        .map(|(s, _)| s.clone())
}

/// A user's merge as the index stores it: the names as the user gave them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Merge {
    /// The name merged away.
    pub name: String,
    /// The name it became.
    pub into: String,
}

/// One foundry of a library, merges applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Slot {
    /// The canonical name.
    pub name: String,
    /// Its [`group_key`].
    pub key: String,
    /// The raw names that fold into it.
    pub raws: BTreeSet<String>,
    /// The names merged into it.
    pub merged: BTreeSet<String>,
}

/// The foundries of a library.
#[derive(Debug, Clone, Default)]
pub(crate) struct Table {
    /// A to Z by name, without case.
    pub slots: Vec<Slot>,
    /// The slot of every key a face or a merge names, merged keys included.
    lookup: HashMap<String, u32>,
}

impl Table {
    /// Groups the faces' foundries and applies the merges; also gives each
    /// face its slot (`None`: no foundry). A merge that would lead back to
    /// where it started (a damaged index) is not applied.
    pub fn build(faces: &[Option<Identified>], merges: &[Merge]) -> (Self, Vec<Option<u32>>) {
        let mut spellings: HashMap<&str, BTreeMap<String, usize>> = HashMap::new();
        for found in faces.iter().flatten() {
            *spellings
                .entry(found.key.as_str())
                .or_default()
                .entry(found.raw.clone())
                .or_default() += 1;
        }
        let into: HashMap<String, (String, String)> = merges
            .iter()
            .map(|m| (group_key(&m.name), (group_key(&m.into), m.into.clone())))
            .filter(|(from, (to, _))| !from.is_empty() && !to.is_empty() && from != to)
            .collect();
        let resolve = |start: &str| -> String {
            let mut at = start.to_owned();
            let mut seen = BTreeSet::from([at.clone()]);
            while let Some((next, _)) = into.get(&at) {
                if !seen.insert(next.clone()) {
                    return start.to_owned();
                }
                at.clone_from(next);
            }
            at
        };
        let mut by_key: BTreeMap<String, Slot> = BTreeMap::new();
        let slot_for = |k: &str, by_key: &mut BTreeMap<String, Slot>| -> String {
            let target = resolve(k);
            by_key.entry(target.clone()).or_insert_with(|| {
                // A merge target no face names any more keeps the name the
                // user gave it.
                let name = display(&target, spellings.get(target.as_str()))
                    .or_else(|| {
                        into.values()
                            .find(|(to, _)| *to == target)
                            .map(|(_, given)| given.clone())
                    })
                    .unwrap_or_else(|| target.clone());
                Slot {
                    name,
                    key: target.clone(),
                    raws: BTreeSet::new(),
                    merged: BTreeSet::new(),
                }
            });
            target
        };
        let keys: Vec<Option<String>> = faces
            .iter()
            .map(|found| {
                found.as_ref().map(|f| {
                    let target = slot_for(&f.key, &mut by_key);
                    if let Some(slot) = by_key.get_mut(&target) {
                        slot.raws.insert(f.raw.clone());
                    }
                    target
                })
            })
            .collect();
        for m in merges {
            let from = group_key(&m.name);
            if !into.contains_key(&from) {
                continue;
            }
            let target = resolve(&from);
            if target != from
                && let Some(slot) = by_key.get_mut(&target)
            {
                let name = display(&from, spellings.get(from.as_str())).unwrap_or(m.name.clone());
                slot.merged.insert(name);
            }
        }
        let mut slots: Vec<Slot> = by_key.into_values().collect();
        slots.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.name.cmp(&b.name))
        });
        let index: HashMap<&str, u32> = slots
            .iter()
            .enumerate()
            .map(|(i, s)| (s.key.as_str(), u32::try_from(i).unwrap_or(u32::MAX)))
            .collect();
        let of_face = keys
            .iter()
            .map(|k| k.as_deref().and_then(|k| index.get(k).copied()))
            .collect();
        let lookup = spellings
            .keys()
            .map(|k| (*k).to_owned())
            .chain(into.keys().cloned())
            .filter_map(|k| {
                let slot = index.get(resolve(&k).as_str()).copied()?;
                Some((k, slot))
            })
            .collect();
        (Self { slots, lookup }, of_face)
    }

    /// The slot a name lists under (compared as a [`group_key`]); a name
    /// merged away finds the foundry it became.
    pub fn find(&self, name: &str) -> Option<u32> {
        self.lookup.get(&group_key(name)).copied()
    }
}

#[cfg(test)]
mod tests;
