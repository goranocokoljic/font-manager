//! The Fontshare catalog (E22.5, ADR 0014).
//!
//! - **Index:** `GET {base}/v2/fonts`, Fontshare's public JSON (no key, no
//!   account): every family with its category, script, designers, version,
//!   license type (`itf_ffl`, `sil_ofl`), axes and styles. Each style names
//!   its weight, whether it is italic or variable, and its file on
//!   `cdn.fontshare.com` (a scheme-relative URL without an extension; the
//!   CDN serves the TrueType file at that URL plus `.ttf`).
//! - **Files:** Fontshare has no per-family file list by name, so
//!   [`Fontshare::resolve_files`] reads the index again and takes the
//!   family's style files from it.
//!
//! One family the parser cannot read is left out with a warning; an index
//! with no readable family, or one that says it holds more families than it
//! sent (`has_more`, `count_total`), is an error, so a partial answer never
//! replaces the cache.

use std::collections::BTreeSet;

use fm_api::catalog::{CatalogFamily, CatalogId, CatalogStyle};
use fm_api::common::{Axis, Slant};
use fm_api::library::OpenLicense;
use serde::Deserialize;

use super::{CatalogAdapter, CatalogFile, CatalogFiles, Fetcher, malformed};
use crate::error::{FmError, FmResult};
use crate::inventory::open_license_label;

/// Where Fontshare's API answers.
pub const BASE_URL: &str = "https://api.fontshare.com";
/// Fontshare's web site.
pub const HOMEPAGE: &str = "https://www.fontshare.com";

/// The Fontshare adapter.
#[derive(Debug, Clone)]
pub struct Fontshare {
    base_url: String,
}

impl Default for Fontshare {
    fn default() -> Self {
        Self::new()
    }
}

impl Fontshare {
    /// The adapter at [`BASE_URL`].
    pub fn new() -> Self {
        Self::with_base_url(BASE_URL)
    }

    /// The adapter at another address; tests pass a local server
    /// (`http://127.0.0.1:<port>`).
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let base_url: String = base_url.into();
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
        }
    }

    /// The URL of the index.
    pub fn index_url(&self) -> String {
        format!("{}/v2/fonts", self.base_url)
    }
}

impl CatalogAdapter for Fontshare {
    fn id(&self) -> CatalogId {
        CatalogId::Fontshare
    }

    fn homepage(&self) -> String {
        HOMEPAGE.to_owned()
    }

    fn fetch_index(&self, fetcher: &dyn Fetcher) -> FmResult<Vec<u8>> {
        fetcher.get(&self.index_url())
    }

    fn parse_index(&self, body: &[u8]) -> FmResult<Vec<CatalogFamily>> {
        parse_index(body)
    }

    fn resolve_files(&self, fetcher: &dyn Fetcher, family: &str) -> FmResult<CatalogFiles> {
        parse_files(&fetcher.get(&self.index_url())?, family)
    }
}

#[derive(Deserialize)]
struct Index {
    fonts: Vec<serde_json::Value>,
    #[serde(default)]
    has_more: bool,
    #[serde(default)]
    count_total: Option<u64>,
}

#[derive(Deserialize)]
struct Entry {
    name: String,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    script: Option<String>,
    #[serde(default)]
    designers: Vec<Designer>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    license_type: Option<String>,
    #[serde(default)]
    axes: Vec<EntryAxis>,
    #[serde(default)]
    styles: Vec<Style>,
}

#[derive(Deserialize)]
struct Designer {
    name: String,
}

#[derive(Deserialize)]
struct EntryAxis {
    #[serde(default)]
    name: Option<String>,
    property: String,
    range_left: f32,
    range_right: f32,
    range_default: f32,
}

#[derive(Deserialize)]
struct Style {
    #[serde(default)]
    file: Option<String>,
    weight: Weight,
    #[serde(default)]
    is_variable: bool,
    #[serde(default)]
    is_italic: bool,
}

#[derive(Deserialize)]
struct Weight {
    /// The style's name without spaces ("SemiboldItalic", "Variable").
    #[serde(default)]
    name: Option<String>,
    /// The weight class; 0 for a variable style.
    weight: u32,
}

/// The index as its entries, after checking it is the whole index.
fn read_index(body: &[u8]) -> FmResult<Vec<serde_json::Value>> {
    let body = body.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(body);
    let index: Index = serde_json::from_slice(body)
        .map_err(|e| malformed("an index", format!("Fontshare index: {e}")))?;
    let sent = index.fonts.len() as u64;
    if index.has_more || index.count_total.is_some_and(|total| total > sent) {
        return Err(malformed(
            "an incomplete index",
            format!(
                "Fontshare index: {sent} of {} families, has_more {}",
                index
                    .count_total
                    .map_or_else(|| "?".to_owned(), |t| t.to_string()),
                index.has_more
            ),
        ));
    }
    Ok(index.fonts)
}

/// The entries that can be read, with a warning for each one that cannot.
fn entries(body: &[u8]) -> FmResult<Vec<Entry>> {
    let mut entries = Vec::new();
    for value in read_index(body)? {
        match serde_json::from_value::<Entry>(value) {
            Ok(entry) if !entry.name.trim().is_empty() => entries.push(entry),
            Ok(_) => tracing::warn!("Fontshare index: a family with no name left out"),
            Err(e) => tracing::warn!(error = %e, "Fontshare index: a family left out"),
        }
    }
    if entries.is_empty() {
        return Err(malformed(
            "an index",
            "Fontshare index: no family could be read",
        ));
    }
    Ok(entries)
}

/// The index as families, in the index's order.
pub fn parse_index(body: &[u8]) -> FmResult<Vec<CatalogFamily>> {
    Ok(entries(body)?.into_iter().map(family).collect())
}

fn family(entry: Entry) -> CatalogFamily {
    let mut styles: Vec<CatalogStyle> = entry
        .styles
        .iter()
        .filter(|s| !s.is_variable && (1..=1000).contains(&s.weight.weight))
        .map(|s| CatalogStyle {
            weight: s.weight.weight,
            slant: if s.is_italic {
                Slant::Italic
            } else {
                Slant::Upright
            },
        })
        .collect();
    styles.sort_by_key(|s| (s.slant != Slant::Upright, s.weight));
    styles.dedup();
    // A family with an upright and an italic variable font lists its axes
    // once per font; one entry per tag is kept.
    let mut tags = BTreeSet::new();
    let axes = entry
        .axes
        .into_iter()
        .filter(|a| !a.property.trim().is_empty() && tags.insert(a.property.clone()))
        .map(|a| Axis {
            name: axis_name(&a),
            tag: a.property,
            min: a.range_left,
            default: a.range_default,
            max: a.range_right,
        })
        .collect();
    let (license, license_name) = license(entry.license_type.as_deref());
    CatalogFamily {
        name: entry.name.trim().to_owned(),
        category: entry
            .category
            .map(|c| c.trim().to_owned())
            .filter(|c| !c.is_empty()),
        styles,
        axes,
        subsets: entry
            .script
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .into_iter()
            .collect(),
        designers: entry
            .designers
            .into_iter()
            .map(|d| d.name.trim().to_owned())
            .filter(|d| !d.is_empty())
            .collect(),
        version: entry.version.filter(|v| !v.is_empty()),
        // `inserted_at` is when the family was published, not changed.
        last_modified: None,
        open_source: license.is_some(),
        license,
        license_name,
    }
}

/// The axis's name: Fontshare's own when it gives one, else the registered
/// name of the tag, else the tag. Fontshare names some axes by their tag
/// (`"name": "wght"`).
fn axis_name(axis: &EntryAxis) -> String {
    let given = axis.name.as_deref().map(str::trim).unwrap_or("");
    if !given.is_empty() && given != axis.property {
        return given.to_owned();
    }
    match axis.property.as_str() {
        "wght" => "Weight",
        "wdth" => "Width",
        "opsz" => "Optical size",
        "ital" => "Italic",
        "slnt" => "Slant",
        other => other,
    }
    .to_owned()
}

/// Fontshare's license type as an open license and the license's name.
/// The ITF Free Font License is free to use but not an open license.
fn license(license_type: Option<&str>) -> (Option<OpenLicense>, Option<String>) {
    match license_type.map(str::trim) {
        None | Some("") => (None, None),
        Some("sil_ofl") => (
            Some(OpenLicense::Ofl),
            Some(open_license_label(OpenLicense::Ofl).to_owned()),
        ),
        Some("itf_ffl") => (None, Some("ITF Free Font License".to_owned())),
        Some(other) => (None, Some(other.to_owned())),
    }
}

/// The style files of `family` in an index: one TrueType file per style,
/// named like Fontshare's own download (`GeneralSans-BoldItalic.ttf`).
pub fn parse_files(body: &[u8], family: &str) -> FmResult<CatalogFiles> {
    let wanted = family.trim();
    let mut entries = entries(body)?;
    let at = entries
        .iter()
        .position(|e| e.name.trim() == wanted)
        .or_else(|| {
            entries
                .iter()
                .position(|e| e.name.trim().eq_ignore_ascii_case(wanted))
        })
        .ok_or_else(|| FmError::not_found(format!("The Fontshare family \"{wanted}\"")))?;
    let entry = entries.swap_remove(at);
    let stem = match file_part(&entry.name) {
        stem if stem.is_empty() => "Font".to_owned(),
        stem => stem,
    };
    let mut names = BTreeSet::new();
    let files: Vec<CatalogFile> = entry
        .styles
        .iter()
        .filter_map(|s| {
            let url = file_url(s.file.as_deref()?)?;
            let style = s.weight.name.as_deref().map(file_part).unwrap_or_default();
            let style = if style.is_empty() {
                s.weight.weight.to_string()
            } else {
                style
            };
            let name = format!("{stem}-{style}.ttf");
            names.insert(name.clone()).then_some(CatalogFile {
                name,
                url,
                variable: s.is_variable,
            })
        })
        .collect();
    if files.is_empty() {
        return Err(malformed(
            "a file list",
            format!("Fontshare index: no font file for {wanted}"),
        ));
    }
    let (license, _) = license(entry.license_type.as_deref());
    let family_id = entry
        .slug
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(entry.name.trim())
        .to_owned();
    Ok(CatalogFiles {
        family_id,
        files,
        license,
    })
}

/// A style's TrueType URL: a scheme-relative CDN path gets `https:`, and
/// the file gets its `.ttf`. Anything that is not an absolute URL is left
/// out.
fn file_url(file: &str) -> Option<String> {
    let file = file.trim();
    let url = if let Some(rest) = file.strip_prefix("//") {
        format!("https://{rest}")
    } else if file.starts_with("https://") || file.starts_with("http://") {
        file.to_owned()
    } else {
        return None;
    };
    Some(if url.to_ascii_lowercase().ends_with(".ttf") {
        url
    } else {
        format!("{url}.ttf")
    })
}

/// `text` with only ASCII letters, digits, `-` and `_`, for a file name.
fn file_part(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .collect()
}

#[cfg(test)]
mod tests;
