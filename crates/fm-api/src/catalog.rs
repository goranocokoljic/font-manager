//! Online catalogs (E22.1, ADR 0014): font services such as Google Fonts,
//! browsed from a metadata cache in the app's data folder.
//!
//! Every catalog is off until the user turns it on. Refreshing a catalog is
//! the only call that goes to the network; listing its families answers from
//! the cache, offline and after a restart.
//!
//! Downloading a family (E22.3) is the other call that goes to the network:
//! it writes the family's files into a folder of a watched source, records
//! where they came from, and rescans the source.

use serde::{Deserialize, Serialize};

use crate::common::{Axis, Page, Paged, Slant};
use crate::library::OpenLicense;
use crate::{ApiError, ApiResult, JobId, SourceId};

/// A catalog the app knows how to read.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum CatalogId {
    /// Google Fonts (fonts.google.com).
    GoogleFonts,
    /// Fontshare (fontshare.com), Indian Type Foundry's free fonts.
    Fontshare,
}

impl CatalogId {
    /// Every catalog, in the order the app lists them.
    pub const ALL: [CatalogId; 2] = [CatalogId::GoogleFonts, CatalogId::Fontshare];

    /// The catalog's name as the user sees it.
    pub fn name(self) -> &'static str {
        match self {
            Self::GoogleFonts => "Google Fonts",
            Self::Fontshare => "Fontshare",
        }
    }
}

impl std::fmt::Display for CatalogId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// A catalog and the state of its cache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    /// Id.
    pub id: CatalogId,
    /// The name the user sees ("Google Fonts").
    pub name: String,
    /// The catalog's web site.
    pub homepage: String,
    /// The user turned it on. Off by default; an off catalog is never fetched.
    pub enabled: bool,
    /// When the cache was last refreshed (RFC 3339, UTC); `None` before the
    /// first refresh.
    #[serde(default)]
    pub refreshed_at: Option<String>,
    /// Families in the cache.
    pub family_count: u32,
}

/// One style a catalog family offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CatalogStyle {
    /// Weight class (100–1000).
    pub weight: u32,
    /// Upright or italic.
    pub slant: Slant,
}

/// A family as a catalog describes it, before anything is downloaded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CatalogFamily {
    /// The family name ("Roboto Flex").
    pub name: String,
    /// The catalog's category, as the catalog names it ("Sans Serif").
    #[serde(default)]
    pub category: Option<String>,
    /// The styles it offers, upright first, then by weight.
    pub styles: Vec<CatalogStyle>,
    /// Its variation axes; empty for a static family.
    #[serde(default)]
    pub axes: Vec<Axis>,
    /// The character subsets it covers ("latin", "cyrillic-ext").
    #[serde(default)]
    pub subsets: Vec<String>,
    /// Its designers.
    #[serde(default)]
    pub designers: Vec<String>,
    /// Its version, when the catalog gives one.
    #[serde(default)]
    pub version: Option<String>,
    /// When the catalog last changed it, as the catalog gives it (Google
    /// Fonts: a date, `2025-09-04`).
    #[serde(default)]
    pub last_modified: Option<String>,
    /// The catalog says it is open source.
    #[serde(default)]
    pub open_source: bool,
    /// Its license, when the catalog's index names it. Google Fonts' index
    /// does not; its license comes with the files.
    #[serde(default)]
    pub license: Option<OpenLicense>,
    /// The license's name as the catalog gives it, also for a license that
    /// is not an open license (Fontshare's "ITF Free Font License"); `None`
    /// when the index names no license.
    #[serde(default)]
    pub license_name: Option<String>,
}

/// Turns a catalog on or off. Turning it off keeps its cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SetCatalogEnabledRequest {
    /// The catalog.
    pub id: CatalogId,
    /// On or off.
    pub enabled: bool,
}

/// Names one catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CatalogRef {
    /// The catalog.
    pub id: CatalogId,
}

/// A page of a catalog's cached families.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ListCatalogFamiliesRequest {
    /// The catalog.
    pub id: CatalogId,
    /// Families whose name contains this text, ignoring case; empty lists
    /// every family.
    #[serde(default)]
    pub query: String,
    /// The window; `limit` is capped at 500.
    #[serde(default)]
    pub page: Page,
}

/// The preview file of one catalog family (E22.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CatalogFontRequest {
    /// The catalog.
    pub id: CatalogId,
    /// The family's name, as the catalog lists it.
    pub family: String,
}

/// Where a catalog family's preview file is on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CatalogFontSource {
    /// The catalog.
    pub id: CatalogId,
    /// The family.
    pub family: String,
    /// Absolute path of the file in the app's cache folder, served to the
    /// webview by the app's font scheme.
    pub path: String,
}

/// Which files of a family to download: its static styles or its variable
/// fonts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CatalogFileKind {
    /// One file per style (`Lato-Bold.ttf`).
    Static,
    /// The variable fonts (`Inter[opsz,wght].ttf`).
    Variable,
}

/// Downloads a family of a catalog into a watched source (E22.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DownloadFamilyRequest {
    /// The catalog.
    pub id: CatalogId,
    /// The family's name, as the catalog lists it.
    pub family: String,
    /// Static or variable files. A family that has only the other kind gets
    /// that kind; [`CatalogDownload::kind`] says which.
    pub kind: CatalogFileKind,
    /// The watched source to download into; the files go into a subfolder
    /// named after the family.
    pub source: SourceId,
}

/// One downloaded file and where it is now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDownloadFile {
    /// Its file name as downloaded (`Lato-Bold.ttf`).
    pub name: String,
    /// The blake3 hash of its bytes, as lowercase hex.
    pub hash: String,
    /// Where the library has a file with these bytes now (the user may have
    /// moved it); `None` when the library has none.
    #[serde(default)]
    pub path: Option<String>,
}

/// Where a downloaded family came from (its provenance).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDownload {
    /// The catalog.
    pub catalog: CatalogId,
    /// The family's name, as the catalog lists it.
    pub family: String,
    /// The catalog's id of the family: the name for Google Fonts, the slug
    /// for Fontshare.
    pub catalog_family_id: String,
    /// The family's version as the catalog gives it; for a catalog that
    /// gives none (Google Fonts), the date it last changed.
    #[serde(default)]
    pub version: Option<String>,
    /// Static or variable files.
    pub kind: CatalogFileKind,
    /// When it was downloaded (RFC 3339, UTC).
    pub downloaded_at: String,
    /// The source it was downloaded into.
    pub source: SourceId,
    /// The folder it was downloaded into.
    pub folder: String,
    /// The files written, by name.
    pub files: Vec<CatalogDownloadFile>,
}

/// What a download wrote and the rescan it started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DownloadFamilyResponse {
    /// The provenance recorded for the files written.
    pub download: CatalogDownload,
    /// Files not written because a file of that name was already in the
    /// folder; it is never replaced.
    #[serde(default)]
    pub skipped: Vec<String>,
    /// The rescan of the source; its progress and finish arrive as
    /// `Event::ScanProgress` and `Event::ScanFinished`.
    pub scan: JobId,
    /// Why the download stopped after the files in `download` were written
    /// (a file that could not be saved, or the provenance that could not be
    /// recorded); `None` when it finished. The files written stay and are
    /// rescanned. A failure before any file was written is an error of the
    /// call instead.
    #[serde(default)]
    pub error: Option<ApiError>,
}

/// Online catalogs over a metadata cache.
pub trait CatalogService {
    /// Every catalog the app knows, on or off, with the state of its cache.
    fn list_catalogs(&self) -> ApiResult<Vec<Catalog>>;
    /// Turns a catalog on or off; makes no request.
    fn set_catalog_enabled(&self, req: SetCatalogEnabledRequest) -> ApiResult<Catalog>;
    /// Fetches the catalog's index and replaces its cache: the only call that
    /// goes to the network. An off catalog is `Conflict` and nothing is
    /// fetched; a failed or malformed answer is an error and the previous
    /// cache stays as it was.
    fn refresh_catalog(&self, req: CatalogRef) -> ApiResult<Catalog>;
    /// A page of the cached families by name, ignoring case; makes no
    /// request. An off catalog, or one never refreshed, has none.
    fn list_catalog_families(
        &self,
        req: ListCatalogFamiliesRequest,
    ) -> ApiResult<Paged<CatalogFamily>>;
    /// The preview file of a family in the cache: fetched once (a request)
    /// into the app's cache folder, then read from there. An off catalog is
    /// `Conflict` and nothing is fetched; a family not in the cached list is
    /// `NotFound`.
    fn catalog_font_source(&self, req: CatalogFontRequest) -> ApiResult<CatalogFontSource>;
    /// Downloads a family's files into a subfolder named after it inside a
    /// watched source (requests; only on this call), records their
    /// provenance and starts a rescan of the source. A file already there is
    /// never replaced; a download that fails before writing a file writes
    /// none, and one that stops after some files were written answers them
    /// with [`DownloadFamilyResponse::error`]. An off catalog is
    /// `Conflict` and a family not in the cached list `NotFound`, both before
    /// any request; a source that is not a watched folder is `InvalidRequest`.
    fn download_family(&self, req: DownloadFamilyRequest) -> ApiResult<DownloadFamilyResponse>;
    /// Every family downloaded from a catalog, newest first, with where its
    /// files are now (matched by hash, so a moved file is followed).
    fn list_catalog_downloads(&self) -> ApiResult<Vec<CatalogDownload>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_format_is_camel_case() {
        let json = serde_json::to_value(CatalogId::GoogleFonts).expect("json");
        assert_eq!(json, "googleFonts");
        let req: ListCatalogFamiliesRequest =
            serde_json::from_str(r#"{"id":"googleFonts"}"#).expect("query and page default");
        assert_eq!(req.query, "");
        assert_eq!(req.page, Page::default());
        let family = CatalogFamily {
            name: "Lato".into(),
            category: Some("Sans Serif".into()),
            styles: vec![CatalogStyle {
                weight: 400,
                slant: Slant::Italic,
            }],
            axes: Vec::new(),
            subsets: vec!["latin".into()],
            designers: Vec::new(),
            version: None,
            last_modified: Some("2025-09-16".into()),
            open_source: true,
            license: Some(OpenLicense::Ofl),
            license_name: Some("SIL Open Font License".into()),
        };
        let json = serde_json::to_value(&family).expect("json");
        assert_eq!(json["lastModified"], "2025-09-16");
        assert_eq!(json["openSource"], true);
        assert_eq!(json["licenseName"], "SIL Open Font License");
        assert_eq!(json["styles"][0]["slant"], "italic");
        let back: CatalogFamily = serde_json::from_value(json).expect("round trip");
        assert_eq!(back, family);
        assert_eq!(
            serde_json::to_value(CatalogId::Fontshare).expect("json"),
            "fontshare"
        );
        let old: CatalogFamily = serde_json::from_str(r#"{"name":"Lato","styles":[]}"#)
            .expect("a cache without licenseName");
        assert_eq!(old.license_name, None);

        let req: DownloadFamilyRequest = serde_json::from_str(
            r#"{"id":"fontshare","family":"General Sans","kind":"variable","source":3}"#,
        )
        .expect("download request");
        assert_eq!(req.kind, CatalogFileKind::Variable);
        assert_eq!(req.source, SourceId(3));
        let answer: DownloadFamilyResponse = serde_json::from_str(
            r#"{"download":{"catalog":"googleFonts","family":"Lato","catalogFamilyId":"Lato",
                "kind":"static","downloadedAt":"2026-10-24T08:00:00Z","source":1,
                "folder":"/f/Lato","files":[{"name":"Lato-Bold.ttf","hash":"ab"}]},"scan":7}"#,
        )
        .expect("skipped, version and path default");
        assert!(answer.skipped.is_empty());
        assert_eq!(answer.download.version, None);
        assert_eq!(answer.download.files[0].path, None);
        assert_eq!(answer.scan, JobId(7));
        assert_eq!(answer.error, None, "a finished download has no error");
    }
}
