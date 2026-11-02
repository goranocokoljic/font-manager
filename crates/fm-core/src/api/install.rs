//! Installing a catalog family with one click (E22.7, ADR 0014): download it
//! into the download folder of the settings, wait for the rescan, activate
//! the faces it brought.
//!
//! The download folder is the one folder the app writes into without a
//! choice per download; the user chose it once (rule 2). It is never a system
//! font folder (rule 4). A family already in the library is not downloaded
//! again: what is there is activated.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use fm_api::activation::{ActivateRequest, ActivationOutcome, ActivationService, Target};
use fm_api::catalog::{
    CatalogDownload, CatalogDownloadFile, InstallFamilyRequest, InstallFamilyResponse,
};
use fm_api::library::{Source, SourceKind};
use fm_api::settings::Settings;
use fm_api::{ApiError, ApiResult, ErrorCode, FaceId, SettingsService};
use fm_platform::Os;
use fm_platform::paths::{is_within, resolve_links, system_font_folders};

use super::CoreApi;

/// A family [`CoreApi::place_family`] put into the library, or found there.
pub(super) struct Placed {
    /// Its provenance: recorded now, or before.
    pub download: CatalogDownload,
    /// The live faces of its files.
    pub faces: Vec<FaceId>,
    /// The library had it; nothing was downloaded.
    pub already_installed: bool,
    /// A download that stopped after some files were written.
    pub error: Option<ApiError>,
}

impl Placed {
    /// What [`CoreApi::installed_already`] found.
    fn found((download, faces): (CatalogDownload, Vec<FaceId>)) -> Self {
        Self {
            download,
            faces,
            already_installed: true,
            error: None,
        }
    }
}
use crate::catalog::{family_folder, now_rfc3339};

/// The answer to an install while no download folder is set.
pub const NO_DOWNLOAD_FOLDER: &str =
    "Choose the folder for installed fonts first, under Settings → Online catalogs.";

/// The answer to an install while the download folder has no full path for
/// the host (on Windows: no drive or network share).
pub const RELATIVE_DOWNLOAD_FOLDER: &str =
    "Choose the folder for installed fonts with its full path, under Settings → Online catalogs.";

/// The answer to an install into a system font folder (rule 4).
pub const SYSTEM_DOWNLOAD_FOLDER: &str = "Fonts cannot be installed into a system font folder. \
     Choose another folder under Settings → Online catalogs.";

impl CoreApi {
    /// [`fm_api::CatalogService::install_family`].
    pub(super) fn install(&self, req: InstallFamilyRequest) -> ApiResult<InstallFamilyResponse> {
        let settings = self.settings.get_settings()?;
        let placed = self.place_family(&req, &settings, None)?;
        let outcomes = self.activate_inactive(&placed.faces, &settings)?;
        Ok(InstallFamilyResponse {
            download: placed.download,
            faces: placed.faces,
            outcomes,
            already_installed: placed.already_installed,
            error: placed.error,
        })
    }

    /// An install without the activation: checks the download folder,
    /// downloads the family into it unless the library has it already,
    /// rescans and answers the family's faces. `fonts_needed` (E29.5)
    /// activates them itself; with a `deadline`, every request ends by then.
    pub(super) fn place_family(
        &self,
        req: &InstallFamilyRequest,
        settings: &Settings,
        deadline: Option<Instant>,
    ) -> ApiResult<Placed> {
        let folder = settings
            .download_folder
            .as_deref()
            .map(str::trim)
            .filter(|f| !f.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| ApiError::new(ErrorCode::Conflict, NO_DOWNLOAD_FOLDER))?;
        let catalogs = self.catalogs()?;
        catalogs.ensure_enabled(req.id)?;
        let os = Os::current();
        if !folder.is_absolute() {
            return Err(ApiError::new(ErrorCode::Conflict, RELATIVE_DOWNLOAD_FOLDER));
        }
        let sources = self.lock().sources()?;
        let system = system_font_folders().into_iter().chain(
            sources
                .iter()
                .filter(|s| s.kind == SourceKind::System)
                .map(|s| PathBuf::from(&s.path)),
        );
        let real = resolve_links(&folder);
        let inside =
            |dir: &Path| is_within(os, &folder, dir) || is_within(os, &real, &resolve_links(dir));
        if system.into_iter().any(|dir| inside(&dir)) {
            return Err(ApiError::new(ErrorCode::Conflict, SYSTEM_DOWNLOAD_FOLDER));
        }

        if let Some(found) = self.installed_already(os, req, &folder)? {
            return Ok(Placed::found(found));
        }

        let source = self.download_source(os, &folder, &sources)?;
        let family_dir = family_folder(&folder, &req.family);
        // Files already on disk in the family folder but not in the library
        // (a removed source, a folder chosen with an earlier download): index
        // them first, so they are activated, not fetched again.
        if std::fs::read_dir(&family_dir).is_ok_and(|mut entries| entries.next().is_some()) {
            self.scan_and_wait(&source)?;
            if let Some(found) = self.installed_already(os, req, &folder)? {
                return Ok(Placed::found(found));
            }
        }
        let saved =
            self.save_download(req.id, &req.family, req.kind, &source, &folder, deadline)?;
        self.scan_and_wait(&source)?;
        // Every face in the family folder: the files written and the ones the
        // download skipped because they were already there.
        let faces = self.faces_within(os, &family_dir)?;
        Ok(Placed {
            download: saved.download,
            faces,
            already_installed: false,
            error: saved.error,
        })
    }

    /// The family's provenance and faces when the library has it: the newest
    /// download recorded for it whose files are still in the library, else
    /// the faces of the library in its folder inside `folder` (files put
    /// there before provenance was recorded, or by hand). `None` when the
    /// library has none of its files.
    fn installed_already(
        &self,
        os: Os,
        req: &InstallFamilyRequest,
        folder: &Path,
    ) -> ApiResult<Option<(CatalogDownload, Vec<FaceId>)>> {
        let recorded = self
            .lock()
            .catalog_downloads()?
            .into_iter()
            .find(|d| d.catalog == req.id && d.family == req.family);
        if let Some(download) = recorded {
            let paths: Vec<PathBuf> = download
                .files
                .iter()
                .filter_map(|f| f.path.as_ref().map(PathBuf::from))
                .collect();
            let faces = self.faces_at(os, &paths)?;
            if !faces.is_empty() {
                return Ok(Some((download, faces)));
            }
        }
        let family_dir = family_folder(folder, &req.family);
        let found = self.faces_with_files_within(os, &family_dir)?;
        if found.is_empty() {
            return Ok(None);
        }
        let mut files: Vec<CatalogDownloadFile> = Vec::new();
        let mut seen = HashSet::new();
        for (_, path) in &found {
            if !seen.insert(path.clone()) {
                continue;
            }
            files.push(CatalogDownloadFile {
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                hash: std::fs::read(path)
                    .map(|bytes| blake3::hash(&bytes).to_hex().to_string())
                    .unwrap_or_default(),
                path: Some(path.to_string_lossy().into_owned()),
            });
        }
        files.sort_by(|a, b| a.name.cmp(&b.name));
        let source = self
            .lock()
            .sources()?
            .into_iter()
            .find(|s| is_within(os, &family_dir, Path::new(&s.path)))
            .map_or(fm_api::SourceId(0), |s| s.id);
        let download = CatalogDownload {
            catalog: req.id,
            family: req.family.clone(),
            catalog_family_id: req.family.clone(),
            version: None,
            kind: req.kind,
            downloaded_at: now_rfc3339(),
            source,
            folder: family_dir.to_string_lossy().into_owned(),
            files,
        };
        let mut faces: Vec<FaceId> = found.into_iter().map(|(f, _)| f).collect();
        faces.sort();
        faces.dedup();
        Ok(Some((download, faces)))
    }

    /// The watched source the download folder is in: the source of that
    /// folder, else one of a folder it is inside (a nested source would
    /// index the same files twice). Else it is added as a watched source,
    /// indexed in place. Either way the folder is made when it is not there:
    /// the user chose it.
    fn download_source(&self, os: Os, folder: &Path, sources: &[Source]) -> ApiResult<Source> {
        std::fs::create_dir_all(folder).map_err(|e| {
            ApiError::from(crate::error::FmError::io(
                "The folder for installed fonts could not be made.",
                folder,
                &e,
            ))
        })?;
        let watched = || sources.iter().filter(|s| s.kind == SourceKind::Watched);
        let exact = watched().find(|s| {
            let path = Path::new(&s.path);
            is_within(os, folder, path) && is_within(os, path, folder)
        });
        if let Some(source) =
            exact.or_else(|| watched().find(|s| is_within(os, folder, Path::new(&s.path))))
        {
            return Ok(source.clone());
        }
        Ok(self
            .lock()
            .add_source(&folder.to_string_lossy(), SourceKind::Watched)?)
    }

    /// The live faces with their file, for files in `dir`.
    fn faces_with_files_within(&self, os: Os, dir: &Path) -> ApiResult<Vec<(FaceId, PathBuf)>> {
        Ok(self
            .lock()
            .live_faces_with_files()?
            .into_iter()
            .filter(|(_, path)| is_within(os, path, dir))
            .collect())
    }

    /// The live faces of the files in `dir`, by id, once each.
    fn faces_within(&self, os: Os, dir: &Path) -> ApiResult<Vec<FaceId>> {
        let mut faces: Vec<FaceId> = self
            .faces_with_files_within(os, dir)?
            .into_iter()
            .map(|(face, _)| face)
            .collect();
        faces.sort();
        faces.dedup();
        Ok(faces)
    }

    /// The live faces whose file is one of `paths`, by id.
    fn faces_at(&self, os: Os, paths: &[PathBuf]) -> ApiResult<Vec<FaceId>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        let same = |a: &Path, b: &Path| is_within(os, a, b) && is_within(os, b, a);
        Ok(self
            .lock()
            .live_faces_with_files()?
            .into_iter()
            .filter(|(_, path)| paths.iter().any(|p| same(path, p)))
            .map(|(face, _)| face)
            .collect())
    }

    /// Activates the faces of `faces` that are not active yet, in the mode
    /// and with the conflict policy of `settings`. An active face is left as
    /// it is: sent again, a permanent face would turn into a session one.
    fn activate_inactive(
        &self,
        faces: &[FaceId],
        settings: &Settings,
    ) -> ApiResult<Vec<ActivationOutcome>> {
        let active: HashSet<FaceId> = self
            .activation
            .list_active()?
            .into_iter()
            .map(|a| a.face)
            .collect();
        let inactive: Vec<FaceId> = faces
            .iter()
            .copied()
            .filter(|f| !active.contains(f))
            .collect();
        if inactive.is_empty() {
            return Ok(Vec::new());
        }
        self.activation.activate(ActivateRequest {
            target: Target::Faces { faces: inactive },
            mode: settings.activation_mode,
            on_conflict: settings.on_conflict,
        })
    }
}
