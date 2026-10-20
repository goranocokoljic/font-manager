//! Sidecar metadata (E7.3): the user's metadata for the faces of one family
//! in one source, as JSON in `<source>/.fontlibrary/<family-hash>.json`.
//!
//! The sidecar travels with the fonts when the folder is synced to another
//! machine (ADR 0005). It holds what the user put on each face: favorite,
//! rating, notes, tags and the manual collections the face is in (each as the
//! path of names from the top level). A face is named by its file's path
//! relative to the source, with `/` separators, and its index in the file,
//! so the same face matches on every machine that syncs the folder.
//!
//! Two machines can change the same face before they sync. [`merge`] is the
//! merge rule: a three-way merge of this machine's metadata and the sidecar
//! on disk against their common ancestor ([`ancestor`]). Each entry carries
//! its history: an id for each of its last versions, newest first. When the
//! entry on disk descends from the version this machine last wrote or read,
//! that version is the ancestor. When it does not, both machines changed the
//! entry independently (a sync client kept one of two files) and nothing is
//! assumed common: every value that differs is kept or reported. The file
//! has a history too: an entry missing from the file on disk was removed
//! only when the file descends from this machine's version; otherwise the
//! other machine never had it and this machine's entry is kept.
//!
//! - Tags and collections merge as sets: what one side added is added, what
//!   one side removed (and the other left alone) is removed. They never
//!   conflict.
//! - Favorite, rating and notes take the side that changed. When both
//!   changed to different values, this machine's value stays and the other
//!   one is returned as a [`Conflict`], so it is never silently lost.
//!
//! - Virtual styles (E10.2, `pins`) merge as sets too, each known by its
//!   holder and name; when both sides changed one's coordinates, this
//!   machine's stay.
//!
//! Without a last-known sidecar (a new machine, a rebuilt index) the base is
//! empty: whatever either side has set is kept, and two different values are
//! a conflict.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::grouping::family_key;

/// The folder in a source that holds its sidecars.
pub const SIDECAR_DIR: &str = ".fontlibrary";

/// The highest format version this build reads and writes. A sidecar with
/// a higher version was written by a newer Font Manager and is only read,
/// never rewritten.
///
/// Version 2 adds `pins` (virtual styles, E10.2). A file with pins is
/// written as version 2, so an older build, which ignores fields it does not
/// know, leaves it alone instead of rewriting it without them; a file
/// without pins stays [`PLAIN_VERSION`], so older builds still merge it.
pub const SIDECAR_VERSION: u32 = 2;

/// The format version of a sidecar without pins.
pub const PLAIN_VERSION: u32 = 1;

/// How many hex digits of the blake3 hash name a family's sidecar, and
/// make one version id of an entry.
const HASH_DIGITS: usize = 16;

/// How many version ids an entry's history keeps.
const HISTORY: usize = 16;

/// The file name of a family's sidecar: `<family-hash>.json`, where the hash
/// is taken over the family name as the library groups it (case and runs of
/// white space do not matter).
pub fn file_name(family: &str) -> String {
    let hash = blake3::hash(family_key(family).as_bytes()).to_hex();
    format!("{}.json", &hash[..HASH_DIGITS])
}

/// One sidecar file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sidecar {
    /// Format version ([`SIDECAR_VERSION`]).
    pub version: u32,
    /// The family name, for a person reading the file.
    #[serde(default)]
    pub family: String,
    /// The faces with metadata, by file and index.
    #[serde(default)]
    pub faces: Vec<FaceEntry>,
    /// Ids of the file's last versions, newest first ([`next_file_history`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<String>,
}

impl Sidecar {
    /// An empty sidecar for `family`.
    pub fn new(family: impl Into<String>) -> Self {
        Self {
            version: PLAIN_VERSION,
            family: family.into(),
            faces: Vec::new(),
            history: Vec::new(),
        }
    }

    /// The metadata by face.
    pub fn by_face(&self) -> BTreeMap<FaceKey, &Meta> {
        self.faces.iter().map(|f| (f.key(), &f.meta)).collect()
    }

    /// The entries by face.
    pub fn entries(&self) -> BTreeMap<FaceKey, &FaceEntry> {
        self.faces.iter().map(|f| (f.key(), f)).collect()
    }

    /// Sorts the entries by face, so equal content is equal JSON, and sets
    /// the version the content needs ([`SIDECAR_VERSION`]).
    pub fn normalize(&mut self) {
        for face in &mut self.faces {
            face.meta.normalize();
        }
        self.faces.sort_by_key(FaceEntry::key);
        self.faces.dedup_by_key(|f| f.key());
        self.version = if self.faces.iter().any(|f| !f.meta.pins.is_empty()) {
            SIDECAR_VERSION
        } else {
            PLAIN_VERSION
        };
    }

    /// The file's text: pretty JSON with a final newline.
    pub fn to_json(&self) -> String {
        // Serializing plain structs of strings and numbers cannot fail.
        serde_json::to_string_pretty(self).unwrap_or_default() + "\n"
    }
}

/// What reading a sidecar's text found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parsed {
    /// A sidecar this build can merge and rewrite.
    Sidecar(Sidecar),
    /// A sidecar from a newer Font Manager: its version.
    Newer(u32),
    /// Not a sidecar: why.
    Unreadable(String),
}

/// Reads a sidecar's text. Never panics on malformed input.
pub fn parse(text: &str) -> Parsed {
    #[derive(Deserialize)]
    struct Version {
        version: u32,
    }
    match serde_json::from_str::<Version>(text) {
        Ok(v) if v.version > SIDECAR_VERSION => return Parsed::Newer(v.version),
        Ok(_) => {}
        Err(e) => return Parsed::Unreadable(e.to_string()),
    }
    match serde_json::from_str::<Sidecar>(text) {
        Ok(mut sidecar) => {
            sidecar.normalize();
            Parsed::Sidecar(sidecar)
        }
        Err(e) => Parsed::Unreadable(e.to_string()),
    }
}

/// A face, portable across machines: its file relative to the source, with
/// `/` separators, and its index in the file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FaceKey {
    /// The file, relative to the source.
    pub file: String,
    /// The face's index in the file.
    pub index: u32,
}

/// One face of a sidecar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceEntry {
    /// The file, relative to the source, with `/` separators.
    pub file: String,
    /// The face's index in the file.
    #[serde(default)]
    pub index: u32,
    /// The face's PostScript name, for a person reading the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postscript_name: Option<String>,
    /// The metadata.
    #[serde(flatten)]
    pub meta: Meta,
    /// Ids of this entry's last versions, newest first ([`next_history`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<String>,
}

impl FaceEntry {
    /// The face it is about.
    pub fn key(&self) -> FaceKey {
        FaceKey {
            file: self.file.clone(),
            index: self.index,
        }
    }
}

/// The user's metadata on one face.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    /// Favorite.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub favorite: bool,
    /// 1–5 stars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<u8>,
    /// Notes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Tag names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Manual collections, each as the names from the top level down.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collections: Vec<Vec<String>>,
    /// Virtual styles of this face (E10.2).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pins: Vec<Pin>,
}

/// A virtual style (E10.2): an instance of the face under a name of its
/// own, held by a manual collection (its names from the top level down) or
/// a project set (its name).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pin {
    /// Style name.
    pub name: String,
    /// The collection that holds it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection: Option<Vec<String>>,
    /// The set that holds it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set: Option<String>,
    /// The coordinate on every axis, by tag.
    #[serde(default)]
    pub coordinates: BTreeMap<String, f32>,
}

// Equality is total: JSON has no NaN, and `Meta::normalize` drops pins with
// a coordinate that is not a finite number.
impl Eq for Pin {}

/// What a merge knows a pin by: its holder and its name, the set's name and
/// the style name in any case (as the index compares them).
pub type PinKey = (Option<Vec<String>>, Option<String>, String);

impl Pin {
    /// Its key in a merge.
    pub fn key(&self) -> PinKey {
        (
            self.collection.clone(),
            self.set.as_deref().map(tag_key),
            tag_key(&self.name),
        )
    }

    /// Trimmed; `false` when it cannot be a pin (no name, no holder or two,
    /// an empty name on the way, a coordinate that is not a number).
    fn normalize(&mut self) -> bool {
        self.name = self.name.trim().to_owned();
        self.set = self.set.take().map(|s| s.trim().to_owned());
        if let Some(path) = &mut self.collection {
            for name in path.iter_mut() {
                *name = name.trim().to_owned();
            }
        }
        let holder_ok = match (&self.collection, &self.set) {
            (Some(path), None) => !path.is_empty() && path.iter().all(|n| !n.is_empty()),
            (None, Some(set)) => !set.is_empty(),
            _ => false,
        };
        holder_ok && !self.name.is_empty() && self.coordinates.values().all(|v| v.is_finite())
    }
}

impl Meta {
    /// Nothing set.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Sorted, without duplicates, ratings out of range and blank values, as
    /// the index would store them: tags differing only in ASCII case are one
    /// tag, a rating of 0 or above 5 is none, empty notes are none.
    pub fn normalize(&mut self) {
        self.rating = self.rating.filter(|r| (1..=5).contains(r));
        self.notes = self.notes.take().filter(|n| !n.is_empty());
        for tag in &mut self.tags {
            *tag = tag.trim().to_owned();
        }
        self.tags.retain(|t| !t.is_empty());
        self.tags
            .sort_by(|a, b| tag_key(a).cmp(&tag_key(b)).then_with(|| a.cmp(b)));
        self.tags.dedup_by(|a, b| tag_key(a) == tag_key(b));
        for path in &mut self.collections {
            for name in path.iter_mut() {
                *name = name.trim().to_owned();
            }
        }
        self.collections
            .retain(|p| !p.is_empty() && p.iter().all(|n| !n.is_empty()));
        self.collections.sort();
        self.collections.dedup();
        self.pins.retain_mut(Pin::normalize);
        self.pins.sort_by_key(Pin::key);
        self.pins.dedup_by_key(|p| p.key());
    }
}

/// Tags compare without regard to ASCII case, as `tag.name` does.
fn tag_key(name: &str) -> String {
    name.to_ascii_lowercase()
}

/// A field both sides set to different values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    /// Favorite.
    Favorite,
    /// Rating.
    Rating,
    /// Notes.
    Notes,
}

impl Field {
    /// The name stored with a conflict.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Favorite => "favorite",
            Self::Rating => "rating",
            Self::Notes => "notes",
        }
    }

    /// The field named `name`.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "favorite" => Some(Self::Favorite),
            "rating" => Some(Self::Rating),
            "notes" => Some(Self::Notes),
            _ => None,
        }
    }
}

/// A field on which this machine and the sidecar disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// Which field.
    pub field: Field,
    /// This machine's value, as kept (JSON).
    pub local: String,
    /// The sidecar's value, set aside (JSON).
    pub disk: String,
}

/// The result of [`merge`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merged {
    /// The metadata to keep.
    pub meta: Meta,
    /// Fields both sides changed to different values; `meta` holds the local
    /// value of each.
    pub conflicts: Vec<Conflict>,
}

/// The common ancestor of this machine's metadata and the sidecar entry
/// `disk`, given `base`, the entry as this machine last wrote or read it,
/// and whether the file on disk descends from this machine's version of it
/// (`file_descends`, see [`descends`]).
///
/// - No base (a new machine, a rebuilt index): nothing in common.
/// - An entry equal to the base, or one whose history holds the base's
///   version: the base.
/// - No entry on disk: the base when the file descends from this machine's
///   version (the other machine removed the entry), else nothing in common
///   (the other machine never had it).
/// - Else the two changed independently: nothing in common, so every value
///   either side has set is kept, and different values are a conflict.
pub fn ancestor(base: Option<&FaceEntry>, disk: Option<&FaceEntry>, file_descends: bool) -> Meta {
    let Some(base) = base else {
        return Meta::default();
    };
    let descends = match disk {
        None => file_descends,
        Some(disk) => {
            disk.meta == base.meta
                || base
                    .history
                    .first()
                    .is_some_and(|rev| disk.history.contains(rev))
        }
    };
    if descends {
        base.meta.clone()
    } else {
        Meta::default()
    }
}

/// The history of an entry with `meta` written over `prior`, the entry it
/// replaces: unchanged when the metadata is, else a new version id first.
pub fn next_history(meta: &Meta, prior: Option<&FaceEntry>) -> Vec<String> {
    if let Some(prior) = prior.filter(|p| p.meta == *meta && !p.history.is_empty()) {
        return prior.history.clone();
    }
    extend(
        meta,
        prior.map(|p| p.history.as_slice()).unwrap_or_default(),
    )
}

/// The history of the file `next` written over `prior`, the file it
/// replaces: unchanged when its entries are, else a new version id first.
pub fn next_file_history(next: &Sidecar, prior: Option<&Sidecar>) -> Vec<String> {
    if let Some(prior) = prior.filter(|p| p.faces == next.faces && !p.history.is_empty()) {
        return prior.history.clone();
    }
    extend(
        &next.faces,
        prior.map(|p| p.history.as_slice()).unwrap_or_default(),
    )
}

/// Whether the file `disk` is `base` or descends from it.
pub fn descends(base: &Sidecar, disk: &Sidecar) -> bool {
    disk.faces == base.faces
        || base
            .history
            .first()
            .is_some_and(|rev| disk.history.contains(rev))
}

/// `history` with the id of a new version holding `content` first.
fn extend<T: Serialize + ?Sized>(content: &T, history: &[String]) -> Vec<String> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(
        serde_json::to_string(content)
            .unwrap_or_default()
            .as_bytes(),
    );
    hasher.update(b"\n");
    hasher.update(history.first().map_or("", String::as_str).as_bytes());
    let rev = hasher.finalize().to_hex()[..HASH_DIGITS].to_owned();
    std::iter::once(rev)
        .chain(history.iter().cloned())
        .take(HISTORY)
        .collect()
}

/// Merges this machine's metadata (`local`) with the sidecar's (`disk`),
/// both changed from `base`, their common ancestor ([`ancestor`]). All three
/// are normalized first.
pub fn merge(base: &Meta, local: &Meta, disk: &Meta) -> Merged {
    let norm = |m: &Meta| {
        let mut m = m.clone();
        m.normalize();
        m
    };
    let (base, local, disk) = (norm(base), norm(local), norm(disk));
    let mut conflicts = Vec::new();
    let meta = Meta {
        favorite: scalar(
            Field::Favorite,
            base.favorite,
            local.favorite,
            disk.favorite,
            &mut conflicts,
        ),
        rating: scalar(
            Field::Rating,
            base.rating,
            local.rating,
            disk.rating,
            &mut conflicts,
        ),
        notes: scalar(
            Field::Notes,
            base.notes,
            local.notes,
            disk.notes,
            &mut conflicts,
        ),
        tags: set(&base.tags, &local.tags, &disk.tags, |t| tag_key(t)),
        collections: set(
            &base.collections,
            &local.collections,
            &disk.collections,
            |p| p.clone(),
        ),
        pins: pins(&base.pins, &local.pins, &disk.pins),
    };
    Merged {
        meta: norm(&meta),
        conflicts,
    }
}

/// The side that changed; on two different changes the local value, with a
/// conflict.
fn scalar<T: PartialEq + Serialize>(
    field: Field,
    base: T,
    local: T,
    disk: T,
    conflicts: &mut Vec<Conflict>,
) -> T {
    if disk == local || disk == base {
        local
    } else if local == base {
        disk
    } else {
        conflicts.push(Conflict {
            field,
            local: serde_json::to_string(&local).unwrap_or_default(),
            disk: serde_json::to_string(&disk).unwrap_or_default(),
        });
        local
    }
}

/// [`set`] for pins, by [`Pin::key`]; a pin on both sides is the one that
/// changed, this machine's when both did.
fn pins(base: &[Pin], local: &[Pin], disk: &[Pin]) -> Vec<Pin> {
    let find = |items: &[Pin], k: &PinKey| items.iter().find(|p| p.key() == *k).cloned();
    set(base, local, disk, Pin::key)
        .into_iter()
        .map(|kept| {
            let k = kept.key();
            match (find(local, &k), find(disk, &k)) {
                (Some(l), Some(d)) if find(base, &k).as_ref() == Some(&l) => d,
                (Some(l), _) => l,
                (None, _) => kept,
            }
        })
        .collect()
}

/// Items both sides have, plus those one side added; an item of `base` one
/// side removed is removed.
fn set<T: Clone, K: Ord>(base: &[T], local: &[T], disk: &[T], key: impl Fn(&T) -> K) -> Vec<T> {
    let has = |items: &[T], k: &K| items.iter().any(|i| key(i) == *k);
    let mut out: BTreeMap<K, T> = BTreeMap::new();
    for item in local.iter().chain(disk) {
        let k = key(item);
        let keep = (has(local, &k) && has(disk, &k)) || !has(base, &k);
        if keep {
            out.entry(k).or_insert_with(|| item.clone());
        }
    }
    out.into_values().collect()
}

#[cfg(test)]
mod tests;
