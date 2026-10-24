//! The Google Fonts catalog (ADR 0014).
//!
//! - **Index:** `GET {base}/metadata/fonts`, the JSON the fonts.google.com
//!   site reads: every family with its category, styles (`"400"`, `"700i"`),
//!   axes, subsets, designers and last-modified date, plus an axis registry
//!   that names the axes. It has no API key and no per-family license; the
//!   license comes with the files.
//! - **Files:** `GET {base}/download/list?family=<name>`: the font files
//!   with their `fonts.gstatic.com` URLs and the license file shipped with
//!   them (`OFL.txt`, `LICENSE.txt` for Apache 2.0, `UFL.txt`).
//!
//! Both answers may start with `)]}'`, Google's guard against JSON
//! hijacking; it is skipped. One family the parser cannot read is left out
//! with a warning; an index with no readable family is an error.

use std::collections::{BTreeMap, HashMap};

use fm_api::catalog::{CatalogFamily, CatalogId, CatalogStyle};
use fm_api::common::{Axis, Slant};
use fm_api::library::OpenLicense;
use serde::Deserialize;
use serde::de::IgnoredAny;

use super::{CatalogAdapter, CatalogFile, CatalogFiles, Fetcher, malformed};
use crate::error::FmResult;
use crate::inventory::open_license_label;

/// Where Google Fonts answers.
pub const BASE_URL: &str = "https://fonts.google.com";

/// The Google Fonts adapter.
#[derive(Debug, Clone)]
pub struct GoogleFonts {
    base_url: String,
}

impl Default for GoogleFonts {
    fn default() -> Self {
        Self::new()
    }
}

impl GoogleFonts {
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
        format!("{}/metadata/fonts", self.base_url)
    }

    /// The URL of one family's file list.
    pub fn files_url(&self, family: &str) -> String {
        format!(
            "{}/download/list?family={}",
            self.base_url,
            percent_encode(family)
        )
    }
}

impl CatalogAdapter for GoogleFonts {
    fn id(&self) -> CatalogId {
        CatalogId::GoogleFonts
    }

    fn homepage(&self) -> String {
        BASE_URL.to_owned()
    }

    fn fetch_index(&self, fetcher: &dyn Fetcher) -> FmResult<Vec<u8>> {
        fetcher.get(&self.index_url())
    }

    fn parse_index(&self, body: &[u8]) -> FmResult<Vec<CatalogFamily>> {
        parse_index(body)
    }

    fn resolve_files(&self, fetcher: &dyn Fetcher, family: &str) -> FmResult<CatalogFiles> {
        parse_files(&fetcher.get(&self.files_url(family))?)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Index {
    #[serde(default)]
    axis_registry: Vec<RegistryAxis>,
    family_metadata_list: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistryAxis {
    tag: String,
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    family: String,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    subsets: Vec<String>,
    #[serde(default)]
    fonts: BTreeMap<String, IgnoredAny>,
    #[serde(default)]
    axes: Vec<EntryAxis>,
    #[serde(default)]
    designers: Vec<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    last_modified: Option<String>,
    #[serde(default)]
    is_open_source: Option<bool>,
    /// Not in the index today; the per-family metadata has it.
    #[serde(default)]
    license: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntryAxis {
    tag: String,
    min: f32,
    max: f32,
    default_value: f32,
}

/// The index as families, in the index's order.
pub fn parse_index(body: &[u8]) -> FmResult<Vec<CatalogFamily>> {
    let index: Index = serde_json::from_slice(strip_guard(body))
        .map_err(|e| malformed("an index", format!("Google Fonts index: {e}")))?;
    let names: HashMap<&str, &str> = index
        .axis_registry
        .iter()
        .filter_map(|a| Some((a.tag.as_str(), a.display_name.as_deref()?)))
        .collect();
    let mut families = Vec::with_capacity(index.family_metadata_list.len());
    for value in index.family_metadata_list {
        match serde_json::from_value::<Entry>(value) {
            Ok(entry) if !entry.family.trim().is_empty() => families.push(family(entry, &names)),
            Ok(_) => tracing::warn!("Google Fonts index: a family with no name left out"),
            Err(e) => tracing::warn!(error = %e, "Google Fonts index: a family left out"),
        }
    }
    if families.is_empty() {
        return Err(malformed(
            "an index",
            "Google Fonts index: no family could be read",
        ));
    }
    Ok(families)
}

fn family(entry: Entry, axis_names: &HashMap<&str, &str>) -> CatalogFamily {
    let mut styles: Vec<CatalogStyle> = entry.fonts.keys().filter_map(|k| style(k)).collect();
    styles.sort_by_key(|s| (s.slant != Slant::Upright, s.weight));
    styles.dedup();
    let license = entry.license.as_deref().and_then(license_id);
    CatalogFamily {
        name: entry.family.trim().to_owned(),
        category: entry.category.filter(|c| !c.is_empty()),
        styles,
        axes: entry
            .axes
            .into_iter()
            .map(|a| Axis {
                name: axis_names
                    .get(a.tag.as_str())
                    .map_or_else(|| a.tag.clone(), |n| (*n).to_owned()),
                tag: a.tag,
                min: a.min,
                default: a.default_value,
                max: a.max,
            })
            .collect(),
        // `menu` is Google's subset for drawing the family's own name, not a script.
        subsets: entry.subsets.into_iter().filter(|s| s != "menu").collect(),
        designers: entry.designers,
        version: entry.version.filter(|v| !v.is_empty()),
        last_modified: entry.last_modified.filter(|d| !d.is_empty()),
        open_source: entry.is_open_source.unwrap_or(false),
        license,
        license_name: license.map(|l| open_license_label(l).to_owned()),
    }
}

/// A style key of the index: a weight, `i` for italic (`"400"`, `"700i"`).
fn style(key: &str) -> Option<CatalogStyle> {
    let (weight, slant) = match key.strip_suffix('i') {
        Some(w) => (w, Slant::Italic),
        None => (key, Slant::Upright),
    };
    let weight: u32 = weight.parse().ok()?;
    (1..=1000)
        .contains(&weight)
        .then_some(CatalogStyle { weight, slant })
}

/// Google's license ids (`ofl`, `apache2`, `ufl`).
fn license_id(id: &str) -> Option<OpenLicense> {
    match id.to_ascii_lowercase().as_str() {
        "ofl" => Some(OpenLicense::Ofl),
        "apache" | "apache2" => Some(OpenLicense::Apache),
        "ufl" => Some(OpenLicense::Ufl),
        _ => None,
    }
}

#[derive(Deserialize)]
struct DownloadList {
    manifest: Manifest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    #[serde(default)]
    files: Vec<ManifestFile>,
    #[serde(default)]
    file_refs: Vec<FileRef>,
}

#[derive(Deserialize)]
struct ManifestFile {
    filename: String,
}

#[derive(Deserialize)]
struct FileRef {
    filename: String,
    url: String,
}

/// A family's file list: its font files and the license shipped with them.
pub fn parse_files(body: &[u8]) -> FmResult<CatalogFiles> {
    let list: DownloadList = serde_json::from_slice(strip_guard(body))
        .map_err(|e| malformed("a file list", format!("Google Fonts file list: {e}")))?;
    let files: Vec<CatalogFile> = list
        .manifest
        .file_refs
        .into_iter()
        .filter(|f| !f.filename.is_empty() && !f.url.is_empty())
        .map(|f| CatalogFile {
            name: f.filename,
            url: f.url,
        })
        .collect();
    if files.is_empty() {
        return Err(malformed(
            "a file list",
            "Google Fonts file list: no font file",
        ));
    }
    let license = list.manifest.files.iter().find_map(|f| {
        let name = f.filename.rsplit('/').next().unwrap_or(&f.filename);
        match name.to_ascii_uppercase().as_str() {
            "OFL.TXT" => Some(OpenLicense::Ofl),
            "LICENSE.TXT" => Some(OpenLicense::Apache),
            "UFL.TXT" => Some(OpenLicense::Ufl),
            _ => None,
        }
    });
    Ok(CatalogFiles { files, license })
}

/// The body without a byte-order mark, white space and Google's `)]}'` guard.
fn strip_guard(body: &[u8]) -> &[u8] {
    let body = body.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(body);
    let body = body.trim_ascii_start();
    body.strip_prefix(b")]}'").unwrap_or(body)
}

/// `text` for a URL query: unreserved characters as they are, every other
/// byte of its UTF-8 as `%XX`.
fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests;
