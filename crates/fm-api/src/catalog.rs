//! Online catalogs (E22.1, ADR 0014): font services such as Google Fonts,
//! browsed from a metadata cache in the app's data folder.
//!
//! Every catalog is off until the user turns it on. Refreshing a catalog is
//! the only call that goes to the network; listing its families answers from
//! the cache, offline and after a restart.

use serde::{Deserialize, Serialize};

use crate::ApiResult;
use crate::common::{Axis, Page, Paged, Slant};
use crate::library::OpenLicense;

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
    }
}
