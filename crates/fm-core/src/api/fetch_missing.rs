//! Fetching a font a document needs from a catalog the user turned on
//! (E29.5, ADR 0014 "Fetch on demand").
//!
//! Only `fonts_needed` with `activate` fetches, and only while
//! `Settings::fetch_missing_fonts` is on: the switch is the user's action
//! (rule 7). A font the library does not have is looked up in the cached
//! lists of the catalogs that are on, so the lookup makes no request; a hit
//! runs the E22.7 install path without its activation (download into the
//! download folder, rescan), and `fonts_needed` then matches and activates
//! the new faces like any other.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use fm_api::activation::{FetchedFont, FontsNeededRequest};
use fm_api::catalog::{CatalogFileKind, CatalogId, InstallFamilyRequest};
use fm_api::{ActivationService, ApiResult, FaceId, SettingsService};

use super::CoreApi;

/// How long the catalog downloads of one `fonts_needed` may take in all.
/// A family not downloaded by then stays unmatched.
pub const FETCH_BUDGET: Duration = Duration::from_secs(60);

impl CoreApi {
    /// Downloads the families of the fonts of `req` the library does not
    /// have, when the user turned the fetch on; the fonts whose family was
    /// downloaded, in request order. Each family is downloaded once; a
    /// download that fails or does not finish within the budget is logged
    /// and its fonts stay unmatched, the others go on. A family the library
    /// already has is not downloaded again and not reported.
    pub(super) fn fetch_missing(&self, req: &FontsNeededRequest) -> ApiResult<Vec<FetchedFont>> {
        let settings = self.settings.get_settings()?;
        let has_folder = settings
            .download_folder
            .as_deref()
            .is_some_and(|f| !f.trim().is_empty());
        let Some(catalogs) = self.catalogs.as_ref() else {
            return Ok(Vec::new());
        };
        if !settings.fetch_missing_fonts || !has_folder || req.fonts.is_empty() {
            return Ok(Vec::new());
        }
        let found = self.activation.fonts_needed(FontsNeededRequest {
            fonts: req.fonts.clone(),
            document: None,
            activate: false,
            on_conflict: req.on_conflict,
        })?;
        let deadline = Instant::now() + self.fetch_budget;
        let mut families: HashMap<(CatalogId, String), Option<Vec<FaceId>>> = HashMap::new();
        let mut fetched = Vec::new();
        for font in found.unmatched {
            let Some((catalog, family)) =
                catalogs.find_cached_family(font.family.as_deref(), font.full_name.as_deref())
            else {
                continue;
            };
            let faces = families
                .entry((catalog, family.clone()))
                .or_insert_with(|| self.fetch_family(catalog, &family, &settings, deadline))
                .clone();
            if let Some(faces) = faces {
                fetched.push(FetchedFont {
                    font,
                    catalog,
                    family,
                    faces,
                });
            }
        }
        Ok(fetched)
    }

    /// The faces the download of `family` brought, or `None` when nothing
    /// was downloaded: the deadline passed, the download failed (logged), or
    /// the library had the family already.
    fn fetch_family(
        &self,
        catalog: CatalogId,
        family: &str,
        settings: &fm_api::settings::Settings,
        deadline: Instant,
    ) -> Option<Vec<FaceId>> {
        if Instant::now() >= deadline {
            tracing::warn!(%catalog, family, "document font not fetched: out of time");
            return None;
        }
        // Static files: the matcher finds a document's font by its face's
        // names, and a static file is one face per style.
        let req = InstallFamilyRequest {
            id: catalog,
            family: family.to_owned(),
            kind: CatalogFileKind::Static,
        };
        match self.place_family(&req, settings, Some(deadline)) {
            Ok(placed) if !placed.already_installed && !placed.faces.is_empty() => {
                if let Some(e) = &placed.error {
                    tracing::warn!(%catalog, family, error = %e.message, "document font partly fetched");
                }
                Some(placed.faces)
            }
            Ok(_) => None,
            Err(e) => {
                tracing::warn!(%catalog, family, error = %e.message, "document font not fetched");
                None
            }
        }
    }
}
