//! The real library behind `fm-api` (E5.11): [`CoreApi`] implements every
//! service trait, so an adapter holds `Arc<dyn Api>` and serves it instead of
//! the mock.
//!
//! - **One index.** The index sits behind one mutex, shared with the
//!   [`ActivationManager`]; every method locks it for one read or one write.
//! - **Scans.** [`LibraryService::scan`] returns a [`JobId`] at once and
//!   scans on a thread named `fm-scan` (ADR 0006): the [`ScanJob`] events,
//!   then the write, then one [`Event::LibraryChanged`] naming the faces it
//!   added, removed and read again. Scans run one after another; the files
//!   read so far are loaded from the index on the first scan and kept.
//! - **Events.** Every subscriber of [`EventSource::subscribe`] gets every
//!   event sent after it subscribed: scans, activation changes and tag or
//!   metadata changes. The UI listens; it never polls.
//! - **Trash.** "Keep this one" moves copies through a [`DuplicateResolver`]
//!   over the OS trash ([`CoreApi::open`]) or the one a test passes
//!   ([`CoreApi::with_trash`]); without a trash it answers `Unsupported`
//!   and moves nothing.
//! - **Catalogs.** Online catalogs (E22.1) are off until the user turns
//!   one on; only [`CatalogService::refresh_catalog`], a preview,
//!   [`CatalogService::download_family`] and
//!   [`CatalogService::check_updates`] go to the network. A download
//!   records its provenance in the index and rescans the source.
//! - **Not built yet.** Export and workflows answer an
//!   `Unsupported` error with a message for the user, never a panic.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use fm_api::activation::{
    ActivateRequest, ActivationOutcome, ActivationService, ActiveFace, Clash, Conflict,
    DeactivateRequest, Target,
};
use fm_api::catalog::{
    Catalog, CatalogDownload, CatalogDownloadFile, CatalogFamily, CatalogFontRequest,
    CatalogFontSource, CatalogRef, CatalogService, CatalogUpdate, CheckUpdatesRequest,
    DownloadFamilyRequest, DownloadFamilyResponse, ListCatalogFamiliesRequest,
    SetCatalogEnabledRequest,
};
use fm_api::common::Paged;
use fm_api::comparison::{
    Comparison, ComparisonEntry, ComparisonRef, ComparisonService, RenameComparisonRequest,
    SaveComparisonRequest,
};
use fm_api::export::{
    CollectionSpecimenRequest, ExportService, ExportWebRequest, ExportWebResponse,
    FrameworkSnippet, FrameworkSnippetRequest, GlyphSvgRequest, LicenseInventory,
    LicenseInventoryRequest, SpecimenFormat, SpecimenRequest, SpecimenResponse,
};
use fm_api::layout::{
    DuplicateLayoutRequest, Layout, LayoutBounds, LayoutEntry, LayoutRef, LayoutService,
    LayoutVersion, RestoreLayoutRequest, RevertLayoutRequest, SaveLayoutRequest, SavedLayout,
};
use fm_api::library::{
    AddSourceRequest, AppInfo, DuplicateGroup, FaceDetail, Family, FamilySummary, FontMatches,
    GlyphDetail, GlyphPage, GlyphRequest, GlyphsRequest, ImportMode, KeepCopyRequest,
    LibraryService, ListFamiliesRequest, Platform, Problem, Resolution, ResolveFontRequest,
    ScanRequest, SetSourcePausedRequest, Source, SourceKind,
};
use fm_api::organization::{
    Collection, CollectionFacesRequest, CreateCollectionRequest, CreateSetRequest,
    FamilyNotesUpdate, ImportSetResponse, LicenseDeclaration, MoveCollectionRequest,
    OrganizationService, PinInstanceRequest, ProjectSet, QuickCollection, QuickCollectionRequest,
    ReceiptChange, ReceiptPlace, RenameCollectionRequest, SelectionTags, SelectionTagsRequest,
    SetExport, Tag, TagFacesRequest, UntagFacesRequest, UpdateCollectionRulesRequest,
    UserMetaUpdate, VirtualStyle, VirtualStylesRequest,
};
use fm_api::preview::{
    ComparePreviewRequest, CssValues, CssValuesRequest, FontSource, PreviewImage, PreviewRequest,
    PreviewService, SampleText, SampleTextRequest, SizeFactor, SizeFactorRequest,
};
use fm_api::search::{
    CoverageReport, CoverageRequest, FaceCoverage, LanguagePreset, PairingRequest,
    PairingSuggestion, SearchRequest, SearchResponse, SearchService, SimilarFace, SimilarRequest,
};
use fm_api::settings::{
    AppFolders, EndpointInfo, McpConfirmationAnswer, McpGrantUpdate, McpStatus, McpToolCall,
    McpUpdate, Settings, SettingsService, SettingsUpdate,
};
use fm_api::workflows::{
    BriefRequest, PrepareForWebRequest, PrepareForWebResponse, Shortlist, WorkflowService,
};
use fm_api::{
    API_VERSION, ApiError, ApiResult, CollectionId, ErrorCode, Event, EventSource, FaceId,
    FamilyId, JobId, ResolutionId, SetId, SourceId, VirtualStyleId,
};
use fm_platform::{AppPaths, FontActivator, Os, PdfPrinter, Trash, TraySupport};

use crate::activation::ActivationManager;
use crate::catalog::Catalogs;
use crate::endpoint::EndpointKeys;
use crate::error::{FmError, FmResult};
use crate::framework_snippet::{self, SnippetFace};
use crate::index::{DeclaredLicense, Index};
use crate::layout::store::LayoutStore;
use crate::preview::cache::{self, CacheKey, PreviewCache};
use crate::resolve::DuplicateResolver;
use crate::scanner::{FileHash, FileIndex, Rescan, ScanJob};
use crate::settings::SettingsStore;
use crate::{css, glyph_svg, glyphs, layout, pangrams, preview, specimen, subset, web_font};

/// Every subscriber of the event stream.
#[derive(Debug, Default)]
struct Broadcast {
    subscribers: Mutex<Vec<Sender<Event>>>,
}

impl Broadcast {
    fn subscribe(&self) -> Receiver<Event> {
        let (tx, rx) = mpsc::channel();
        lock(&self.subscribers).push(tx);
        rx
    }

    /// Sends `event` to every subscriber; one that dropped its receiver is
    /// forgotten.
    fn send(&self, event: &Event) {
        lock(&self.subscribers).retain(|tx| tx.send(event.clone()).is_ok());
    }
}

/// The library, search, previews, activation, organization and settings of
/// one app-data folder, behind `fm-api`.
pub struct CoreApi {
    index: Arc<Mutex<Index>>,
    activation: ActivationManager,
    settings: SettingsStore,
    previews: Option<PreviewCache>,
    /// "Keep this one"; `None` when there is no trash to move copies to.
    resolver: Option<DuplicateResolver>,
    /// The OS trash, for replaced receipts (E15.2); `None` when there is none.
    trash: Option<Arc<dyn Trash>>,
    /// Where receipts kept in the app's data folder go; `None` when the API
    /// has no data folder.
    receipts: Option<PathBuf>,
    /// The layout template files (E12.5); `None` when the API has no config folder.
    layouts: Option<LayoutStore>,
    /// The OS webview printer for PDF specimens (E21.2); `None` without the
    /// app (the CLI's direct link), when a PDF is `Unsupported`.
    pdf: Option<Arc<dyn PdfPrinter>>,
    /// The online catalogs (E22.1); `None` when the API has no data folder.
    catalogs: Option<Catalogs>,
    /// The local API token and discovery file (E25.1); `None` when the API
    /// has no data folder.
    endpoint: Option<EndpointKeys>,
    /// The MCP calls waiting for the user's Allow or Deny (E27.6).
    mcp_confirmations: crate::mcp::Confirmations,
    events: Arc<Broadcast>,
    /// The files read so far; loaded from the index by the first scan.
    files: Arc<Mutex<Option<FileIndex>>>,
    next_job: AtomicU32,
    /// What the shell found when it made the tray item; `None` before (and
    /// without a shell). Feeds the platform notes of [`AppInfo`].
    tray: Mutex<Option<TraySupport>>,
}

impl std::fmt::Debug for CoreApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoreApi").finish_non_exhaustive()
    }
}

impl CoreApi {
    /// The API over `index` in the app folders `paths`: activation through
    /// `activator` (the app passes [`fm_platform::activator::system`]), the
    /// settings in the config file and the preview cache in the cache
    /// folder, duplicates moved to the OS trash. A preview cache that cannot
    /// be opened is logged and left out; previews are then rendered every
    /// time. A trash that cannot be found is logged too; "Keep this one" then
    /// answers `Unsupported`.
    pub fn open(paths: &AppPaths, index: Index, activator: Arc<dyn FontActivator>) -> Self {
        let settings = SettingsStore::new(paths).with_system_autostart();
        let limit = settings
            .get_settings()
            .map(|s| cache::limit_from_mb(s.cache_limit_mb))
            .unwrap_or_else(|_| cache::limit_from_mb(Settings::default().cache_limit_mb));
        let previews = PreviewCache::open(&cache::cache_dir(paths), limit)
            .inspect_err(|e| tracing::warn!(error = %e, "preview cache not opened"))
            .ok();
        let api = Self::new(index, activator, settings, previews)
            .with_receipts_in(paths.data.join(crate::receipt::RECEIPTS_DIR))
            .with_layouts_in(paths.layouts_dir())
            .with_endpoint_in(paths.data.clone())
            .with_catalogs(
                Catalogs::open(paths.data.join(crate::catalog::CATALOGS_DIR))
                    .with_previews_in(paths.cache.join(crate::catalog::CATALOGS_DIR)),
            );
        match fm_platform::trash::system() {
            Ok(trash) => api.with_trash(trash),
            Err(e) => {
                tracing::warn!(error = %e, "no OS trash: duplicates cannot be moved");
                api
            }
        }
    }

    /// The API over these parts, with no trash; tests pass a mock activator
    /// and scratch folders.
    pub fn new(
        index: Index,
        activator: Arc<dyn FontActivator>,
        settings: SettingsStore,
        previews: Option<PreviewCache>,
    ) -> Self {
        let index = Arc::new(Mutex::new(index));
        let events = Arc::new(Broadcast::default());
        let sink = Arc::clone(&events);
        let activation =
            ActivationManager::new(Arc::clone(&index), activator, move |e| sink.send(&e));
        Self {
            index,
            activation,
            settings,
            previews,
            resolver: None,
            trash: None,
            receipts: None,
            layouts: None,
            pdf: None,
            catalogs: None,
            endpoint: None,
            mcp_confirmations: crate::mcp::Confirmations::default(),
            events,
            files: Arc::new(Mutex::new(None)),
            next_job: AtomicU32::new(1),
            tray: Mutex::new(None),
        }
    }

    /// Records whether the shell could make a tray item, for the platform
    /// notes (Settings → About).
    pub fn set_tray_support(&self, support: TraySupport) {
        *lock(&self.tray) = Some(support);
    }

    /// The same API, moving "Keep this one" copies through `trash`.
    #[must_use]
    pub fn with_trash(mut self, trash: Arc<dyn Trash>) -> Self {
        let sink = Arc::clone(&self.events);
        self.resolver = Some(DuplicateResolver::new(
            Arc::clone(&self.index),
            Arc::clone(&trash),
            move |e| sink.send(&e),
        ));
        self.trash = Some(trash);
        self
    }

    /// The same API, keeping receipts "in the app's data folder" in `dir`.
    #[must_use]
    pub fn with_receipts_in(mut self, dir: PathBuf) -> Self {
        self.receipts = Some(dir);
        self
    }

    /// The same API, keeping layout template files in `dir`.
    #[must_use]
    pub fn with_layouts_in(mut self, dir: PathBuf) -> Self {
        self.layouts = Some(LayoutStore::new(dir));
        self
    }

    /// The same API, keeping the local API token and discovery file in the
    /// data folder `dir` (ADR 0015).
    #[must_use]
    pub fn with_endpoint_in(mut self, dir: PathBuf) -> Self {
        self.endpoint = Some(EndpointKeys::new(dir));
        self
    }

    /// The same API, where an MCP call that needs the user's Allow waits up
    /// to `timeout` (60 s by default; tests wait less).
    #[must_use]
    pub fn with_mcp_confirm_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.mcp_confirmations = crate::mcp::Confirmations::new(timeout);
        self
    }

    /// The app's endpoint listens on `addr`: writes `endpoint.json`, and
    /// [`SettingsService::endpoint_info`] reports the address.
    pub fn publish_endpoint(&self, addr: std::net::SocketAddr) -> ApiResult<EndpointInfo> {
        Ok(self.endpoint_keys()?.publish(addr)?)
    }

    /// The app's endpoint stopped: removes `endpoint.json`. Nothing to do
    /// without a data folder.
    pub fn unpublish_endpoint(&self) -> ApiResult<()> {
        match &self.endpoint {
            Some(keys) => Ok(keys.unpublish()?),
            None => Ok(()),
        }
    }

    fn endpoint_keys(&self) -> FmResult<&EndpointKeys> {
        self.endpoint
            .as_ref()
            .ok_or_else(|| FmError::unsupported("The local API is not available here."))
    }

    /// The same API, printing PDF specimens through `printer`; the app
    /// passes [`fm_platform::pdf::system`].
    #[must_use]
    pub fn with_pdf_printer(mut self, printer: Arc<dyn PdfPrinter>) -> Self {
        self.pdf = Some(printer);
        self
    }

    /// The same API, with these online catalogs.
    #[must_use]
    pub fn with_catalogs(mut self, catalogs: Catalogs) -> Self {
        self.catalogs = Some(catalogs);
        self
    }

    /// The catalogs, or the answer when there is no catalogs folder.
    fn catalogs(&self) -> ApiResult<&Catalogs> {
        self.catalogs
            .as_ref()
            .ok_or_else(|| not_available("Online catalogs"))
    }

    /// The layout files, or the answer when there is no layouts folder.
    fn layouts(&self) -> ApiResult<&LayoutStore> {
        self.layouts
            .as_ref()
            .ok_or_else(|| not_available("Saving layouts"))
    }

    /// Moves the receipt copy `old` to the trash, when it is one the app
    /// made and it is there; a path that is not the app's is only forgotten.
    fn trash_receipt(&self, old: &Path) -> ApiResult<()> {
        if !crate::receipt::is_app_copy(old, self.receipts.as_deref()) || !old.is_file() {
            return Ok(());
        }
        let trash = self
            .trash
            .as_ref()
            .ok_or_else(|| not_available("Moving files to the trash"))?;
        trash.trash(old).map(|_| ()).map_err(|e| {
            ApiError::from(
                FmError::new(
                    ErrorCode::Io,
                    format!(
                        "The old receipt could not be moved to the trash: {}",
                        e.message()
                    ),
                )
                .with_detail(e.detail),
            )
        })
    }

    /// The resolver, or the answer when there is no trash.
    fn resolver(&self) -> ApiResult<&DuplicateResolver> {
        self.resolver
            .as_ref()
            .ok_or_else(|| not_available("Moving files to the trash"))
    }

    /// What the app does at start (ADR 0008): cleans up what a crashed run
    /// left registered and, with "reactivate on launch" on, activates the
    /// last session's fonts again.
    pub fn start(&self) -> FmResult<Vec<ActivationOutcome>> {
        let settings = self.settings.get_settings()?;
        self.activation
            .start(settings.reactivate_on_launch, settings.on_conflict)
    }

    /// What the app does at quit: deregisters the session fonts and
    /// remembers them for "reactivate on launch".
    pub fn shutdown(&self) -> FmResult<Vec<ActivationOutcome>> {
        self.activation.shutdown()
    }

    fn lock(&self) -> MutexGuard<'_, Index> {
        lock(&self.index)
    }

    fn send(&self, event: Event) {
        self.events.send(&event);
    }

    /// Faces whose tags or metadata changed.
    fn updated(&self, faces: &[FaceId]) {
        if !faces.is_empty() {
            self.send(Event::LibraryChanged {
                source: None,
                added: Vec::new(),
                removed: Vec::new(),
                updated: faces.to_vec(),
            });
        }
    }

    /// Scans `sources` on the scan thread, one job each, and returns the
    /// first job (a job with no source when there is nothing to scan).
    fn start_scan(&self, sources: Vec<Source>) -> ApiResult<JobId> {
        let jobs: Vec<JobId> = sources
            .iter()
            .map(|_| JobId(self.next_job.fetch_add(1, Ordering::Relaxed)))
            .collect();
        let Some(&first) = jobs.first() else {
            let job = JobId(self.next_job.fetch_add(1, Ordering::Relaxed));
            self.send(Event::ScanStarted { job, source: None });
            self.send(Event::ScanFinished {
                job,
                added: 0,
                changed: 0,
                removed: 0,
                problems: 0,
            });
            return Ok(job);
        };
        let scan = Scan {
            index: Arc::clone(&self.index),
            files: Arc::clone(&self.files),
            events: Arc::clone(&self.events),
        };
        std::thread::Builder::new()
            .name("fm-scan".into())
            .spawn(move || {
                for (job, source) in jobs.into_iter().zip(sources) {
                    scan.run(job, &source);
                }
            })
            .map_err(|e| {
                ApiError::new(ErrorCode::Internal, "The scan could not be started.")
                    .with_detail(e.to_string())
            })?;
        Ok(first)
    }

    /// Forgets the files read so far; the next scan loads them again.
    fn forget_files(&self) {
        *lock(&self.files) = None;
    }
}

/// What the scan thread needs.
struct Scan {
    index: Arc<Mutex<Index>>,
    files: Arc<Mutex<Option<FileIndex>>>,
    events: Arc<Broadcast>,
}

impl Scan {
    /// Scans one source as `job`: walks and reads it, writes what changed,
    /// then sends what the library gained and lost. Every failure is a
    /// [`Event::JobFailed`].
    fn run(&self, job: JobId, source: &Source) {
        if let Err(e) = self.try_run(job, source) {
            tracing::warn!(%job, source = %source.path, error = %e, "scan failed");
        }
    }

    fn try_run(&self, job: JobId, source: &Source) -> FmResult<()> {
        let fail = |e: FmError| {
            self.events.send(&Event::JobFailed {
                job,
                error: e.clone().into(),
            });
            e
        };
        let mut files = lock(&self.files);
        if files.is_none() {
            *files = Some(lock(&self.index).load_files().map_err(fail)?);
        }
        let Some(files) = files.as_mut() else {
            return Err(fail(FmError::internal("the file index is not loaded")));
        };
        let before = lock(&self.index).faces_of_source(source.id).map_err(fail)?;
        // `run` reports its own failure as `JobFailed`. Its `ScanFinished`
        // waits until the index holds the scan: a reader that sees it (the
        // `fm scan` command, which exits on it) finds the faces written.
        let mut finished = None;
        let rescan = ScanJob::new(job, Some(source.id), &source.path).run(files, &mut |e| {
            if matches!(e, Event::ScanFinished { .. }) {
                finished = Some(e);
            } else {
                self.events.send(&e);
            }
        })?;
        let after = {
            let mut index = lock(&self.index);
            index
                .apply_rescan(source.id, files, &rescan)
                .map_err(fail)?;
            index.faces_of_source(source.id).map_err(fail)?
        };
        if let Some(event) = finished {
            self.events.send(&event);
        }
        if let Some(event) = changes(source.id, &before, &after, &rescan) {
            self.events.send(&event);
        }
        Ok(())
    }
}

/// What a scan of `source` changed, from its faces before and after and the
/// files it read again or no longer found; `None` when nothing changed.
fn changes(
    source: SourceId,
    before: &HashMap<FaceId, std::path::PathBuf>,
    after: &HashMap<FaceId, std::path::PathBuf>,
    rescan: &Rescan,
) -> Option<Event> {
    let changed: HashSet<&Path> = rescan.changed.iter().map(|p| p.as_path()).collect();
    let gone: HashSet<&Path> = rescan.gone.iter().map(|p| p.as_path()).collect();
    let mut added: Vec<FaceId> = after
        .keys()
        .filter(|f| !before.contains_key(f))
        .copied()
        .collect();
    let mut removed: Vec<FaceId> = before
        .iter()
        .filter(|(f, path)| !after.contains_key(f) || gone.contains(path.as_path()))
        .map(|(f, _)| *f)
        .collect();
    let mut updated: Vec<FaceId> = after
        .iter()
        .filter(|(f, path)| before.contains_key(f) && changed.contains(path.as_path()))
        .map(|(f, _)| *f)
        .collect();
    if added.is_empty() && removed.is_empty() && updated.is_empty() {
        return None;
    }
    added.sort();
    removed.sort();
    updated.sort();
    Some(Event::LibraryChanged {
        source: Some(source),
        added,
        removed,
        updated,
    })
}

/// A poisoned lock still guards consistent data: every write is a
/// transaction, so a panic elsewhere must not make the library unusable.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The answer of a service that is not built yet.
/// The answer to a PDF specimen without a printer (the CLI's direct link).
pub const PDF_NEEDS_APP: &str = "PDF export needs the Font Manager app running.";

fn not_available(what: &str) -> ApiError {
    ApiError::new(
        ErrorCode::Unsupported,
        format!("{what} is not available in this version yet."),
    )
}

impl LibraryService for CoreApi {
    fn app_info(&self) -> ApiResult<AppInfo> {
        let mut platform_notes = crate::lifecycle::platform_notes(
            Os::current(),
            lock(&self.tray).as_ref(),
            fm_platform::autostart::sandboxed(),
        );
        platform_notes.extend(crate::update::support(&fm_platform::updater::distribution()).reason);
        Ok(AppInfo {
            name: "Font Manager".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            api_version: API_VERSION,
            platform: match Os::current() {
                Os::Windows => Platform::Windows,
                Os::MacOs => Platform::Macos,
                Os::Linux => Platform::Linux,
            },
            platform_notes,
        })
    }

    fn list_sources(&self) -> ApiResult<Vec<Source>> {
        Ok(self.lock().sources()?)
    }

    /// Adds a folder the user chose, indexed in place (ADR 0004); it is
    /// scanned when the caller asks. Copying into a folder is not built.
    fn add_source(&self, req: AddSourceRequest) -> ApiResult<Source> {
        if let ImportMode::CopyTo { .. } = req.mode {
            return Err(not_available("Copying fonts into a folder"));
        }
        let path = Path::new(&req.path);
        if !req.path.trim().is_empty() && !path.is_dir() {
            return Err(FmError::not_found(format!("The folder {}", req.path)).into());
        }
        Ok(self.lock().add_source(&req.path, SourceKind::Watched)?)
    }

    fn remove_source(&self, source: SourceId) -> ApiResult<()> {
        let removed = {
            let mut index = self.lock();
            let faces = index.faces_of_source(source)?;
            index.remove_source(source)?;
            faces
        };
        self.forget_files();
        let mut removed: Vec<FaceId> = removed.into_keys().collect();
        removed.sort();
        if !removed.is_empty() {
            self.send(Event::LibraryChanged {
                source: Some(source),
                added: Vec::new(),
                removed,
                updated: Vec::new(),
            });
        }
        Ok(())
    }

    fn set_source_paused(&self, req: SetSourcePausedRequest) -> ApiResult<Source> {
        let (source, faces) = {
            let mut index = self.lock();
            let source = index.set_source_paused(req.source, req.paused)?;
            (source, index.faces_of_source(req.source)?)
        };
        // A paused source's faces leave the library view; resumed, they come back.
        let mut faces: Vec<FaceId> = faces.into_keys().collect();
        faces.sort();
        if !faces.is_empty() {
            let (added, removed) = if req.paused {
                (Vec::new(), faces)
            } else {
                (faces, Vec::new())
            };
            self.send(Event::LibraryChanged {
                source: Some(req.source),
                added,
                removed,
                updated: Vec::new(),
            });
        }
        Ok(source)
    }

    /// Scans one source (paused or not, the user asked), or every source
    /// that is not paused.
    fn scan(&self, req: ScanRequest) -> ApiResult<JobId> {
        let sources = self.lock().sources()?;
        let sources = match req.source {
            Some(id) => vec![
                sources
                    .into_iter()
                    .find(|s| s.id == id)
                    .ok_or_else(|| ApiError::not_found(format!("Source {id}")))?,
            ],
            None => sources.into_iter().filter(|s| !s.paused).collect(),
        };
        self.start_scan(sources)
    }

    fn rebuild_index(&self) -> ApiResult<JobId> {
        let sources = self.lock().rebuild()?;
        self.forget_files();
        self.start_scan(sources.into_iter().filter(|s| !s.paused).collect())
    }

    fn list_families(&self, req: ListFamiliesRequest) -> ApiResult<Paged<FamilySummary>> {
        Ok(self.lock().list_families(&req)?)
    }

    fn get_family(&self, family: FamilyId) -> ApiResult<Family> {
        Ok(self.lock().family(family)?)
    }

    fn get_face(&self, face: FaceId) -> ApiResult<FaceDetail> {
        Ok(self.lock().face_detail(face)?)
    }

    /// The index's problems, then the activation conflicts the OS reports.
    fn list_problems(&self) -> ApiResult<Vec<Problem>> {
        let mut problems = self.lock().problems()?;
        problems.extend(self.activation.problems()?);
        Ok(problems)
    }

    fn list_duplicates(&self) -> ApiResult<Vec<DuplicateGroup>> {
        Ok(self.lock().duplicates()?)
    }

    fn keep_copy(&self, req: KeepCopyRequest) -> ApiResult<Resolution> {
        let dry_run = req.dry_run;
        let resolution = self.resolver()?.keep_copy(req)?;
        if !dry_run {
            // The next scan reads the moved files' state from the index again.
            self.forget_files();
        }
        Ok(resolution)
    }

    fn list_resolutions(&self) -> ApiResult<Vec<Resolution>> {
        Ok(self.lock().resolutions()?)
    }

    fn undo_resolution(&self, resolution: ResolutionId) -> ApiResult<Resolution> {
        let resolution = self.resolver()?.undo_resolution(resolution)?;
        self.forget_files();
        Ok(resolution)
    }

    /// Reads the face's file outside the index lock: a CJK font is large.
    fn list_glyphs(&self, req: GlyphsRequest) -> ApiResult<GlyphPage> {
        let file = self.lock().face_file(req.face)?;
        Ok(glyphs::glyph_page_file(
            &file.path,
            file.collection_index,
            &req,
        )?)
    }

    /// Reads the face's file outside the index lock, as `list_glyphs`.
    fn get_glyph(&self, req: GlyphRequest) -> ApiResult<GlyphDetail> {
        let file = self.lock().face_file(req.face)?;
        Ok(glyphs::glyph_detail_file(
            &file.path,
            file.collection_index,
            req.code_point,
        )?)
    }

    /// Reads the names of the visible faces, then matches outside the lock.
    fn resolve_font(&self, req: ResolveFontRequest) -> ApiResult<FontMatches> {
        let names = self.lock().font_names()?;
        req.resolve(names)
    }
}

impl SearchService for CoreApi {
    fn search(&self, req: SearchRequest) -> ApiResult<SearchResponse> {
        Ok(self.lock().search(&req)?)
    }

    fn check_coverage(&self, req: CoverageRequest) -> ApiResult<CoverageReport> {
        Ok(self.lock().check_coverage(&req)?)
    }

    fn face_coverage(&self, face: FaceId) -> ApiResult<FaceCoverage> {
        Ok(self.lock().face_coverage(face)?)
    }

    fn list_languages(&self) -> ApiResult<Vec<LanguagePreset>> {
        Ok(pangrams::presets()?)
    }

    fn find_similar(&self, req: SimilarRequest) -> ApiResult<Vec<SimilarFace>> {
        Ok(self.lock().find_similar(&req)?)
    }

    fn suggest_pairings(&self, req: PairingRequest) -> ApiResult<Vec<PairingSuggestion>> {
        Ok(self.lock().suggest_pairings(&req)?)
    }
}

impl PreviewService for CoreApi {
    /// Renders from the face's file, through the preview cache when the
    /// file has been hashed.
    fn render_preview(&self, req: PreviewRequest) -> ApiResult<PreviewImage> {
        let file = self.lock().face_file(req.face)?;
        let render = || preview::render_preview_file(&file.path, file.collection_index, &req);
        let image = match (&self.previews, &file.hash) {
            (Some(previews), Some(hash)) => {
                let key = CacheKey::new(hash, file.collection_index, &req);
                previews.get_or_render(&key, render)?
            }
            _ => render()?,
        };
        Ok(image)
    }

    /// Checks the request, resolves every face to its file, and renders
    /// through the preview cache when every file has been hashed.
    fn compare_preview(&self, req: ComparePreviewRequest) -> ApiResult<PreviewImage> {
        preview::check_compare(&req)?;
        let files = {
            let index = self.lock();
            req.faces
                .iter()
                .map(|face| index.face_file(*face))
                .collect::<FmResult<Vec<_>>>()?
        };
        let paths: Vec<(&Path, u32)> = files
            .iter()
            .map(|f| (f.path.as_path(), f.collection_index))
            .collect();
        let render = || preview::compare_preview_files(&paths, &req);
        let hashes: Option<Vec<(&FileHash, u32)>> = files
            .iter()
            .map(|f| f.hash.as_ref().map(|h| (h, f.collection_index)))
            .collect();
        let image = match (&self.previews, hashes, req.faces.first()) {
            (Some(previews), Some(hashes), Some(first)) => {
                let key = CacheKey::compare(&hashes, &req.row(*first));
                previews.get_or_render(&key, render)?
            }
            _ => render()?,
        };
        Ok(image)
    }

    fn font_source(&self, face: FaceId) -> ApiResult<FontSource> {
        Ok(self.lock().font_source(face)?)
    }

    fn sample_texts(&self, req: SampleTextRequest) -> ApiResult<Vec<SampleText>> {
        Ok(self.lock().sample_texts(&req)?)
    }

    fn size_factors(&self, req: SizeFactorRequest) -> ApiResult<Vec<SizeFactor>> {
        Ok(self.lock().size_factors(&req)?)
    }

    /// From the face's axes and features as the inspector shows them.
    fn css_values(&self, req: CssValuesRequest) -> ApiResult<CssValues> {
        let detail = self.lock().face_detail(req.face)?;
        Ok(css::css_values(
            &detail.axes,
            &detail.features,
            &req.variations,
            &req.features,
        ))
    }
}

impl ActivationService for CoreApi {
    fn activate(&self, req: ActivateRequest) -> ApiResult<Vec<ActivationOutcome>> {
        self.activation.activate(req)
    }

    fn deactivate(&self, req: DeactivateRequest) -> ApiResult<Vec<ActivationOutcome>> {
        self.activation.deactivate(req)
    }

    fn list_active(&self) -> ApiResult<Vec<ActiveFace>> {
        self.activation.list_active()
    }

    fn conflicts(&self, target: Target) -> ApiResult<Vec<Conflict>> {
        self.activation.conflicts(target)
    }

    fn list_conflicts(&self) -> ApiResult<Vec<Clash>> {
        self.activation.list_conflicts()
    }
}

impl OrganizationService for CoreApi {
    fn list_collections(&self) -> ApiResult<Vec<Collection>> {
        Ok(self.lock().collections()?)
    }

    fn create_collection(&self, req: CreateCollectionRequest) -> ApiResult<Collection> {
        Ok(self.lock().create_collection(&req)?)
    }

    fn rename_collection(&self, req: RenameCollectionRequest) -> ApiResult<Collection> {
        Ok(self.lock().rename_collection(&req)?)
    }

    fn move_collection(&self, req: MoveCollectionRequest) -> ApiResult<Collection> {
        Ok(self.lock().move_collection(&req)?)
    }

    fn update_collection_rules(&self, req: UpdateCollectionRulesRequest) -> ApiResult<Collection> {
        Ok(self.lock().update_collection_rules(&req)?)
    }

    fn delete_collection(&self, collection: CollectionId) -> ApiResult<()> {
        Ok(self.lock().delete_collection(collection)?)
    }

    fn add_to_collection(&self, req: CollectionFacesRequest) -> ApiResult<Collection> {
        Ok(self.lock().add_to_collection(&req)?)
    }

    fn remove_from_collection(&self, req: CollectionFacesRequest) -> ApiResult<Collection> {
        Ok(self.lock().remove_from_collection(&req)?)
    }

    fn list_tags(&self) -> ApiResult<Vec<Tag>> {
        Ok(self.lock().tags()?)
    }

    fn tag_faces(&self, req: TagFacesRequest) -> ApiResult<Vec<Tag>> {
        let (tags, faces) = {
            let mut index = self.lock();
            let tags = index.tag_faces(&req)?;
            (tags, index.faces_of(&req.faces, &req.families)?)
        };
        self.updated(&faces);
        Ok(tags)
    }

    fn untag_faces(&self, req: UntagFacesRequest) -> ApiResult<()> {
        self.lock().untag_faces(&req)?;
        self.updated(&req.faces);
        Ok(())
    }

    fn selection_tags(&self, req: SelectionTagsRequest) -> ApiResult<SelectionTags> {
        Ok(self.lock().selection_tags(&req.faces, &req.families)?)
    }

    fn update_user_meta(&self, req: UserMetaUpdate) -> ApiResult<()> {
        self.lock().update_user_meta(&req)?;
        self.updated(&req.faces);
        Ok(())
    }

    fn list_sets(&self) -> ApiResult<Vec<ProjectSet>> {
        Ok(self.lock().sets()?)
    }

    fn create_set(&self, req: CreateSetRequest) -> ApiResult<ProjectSet> {
        Ok(self.lock().create_set(&req)?)
    }

    fn delete_set(&self, set: SetId) -> ApiResult<()> {
        Ok(self.lock().delete_set(set)?)
    }

    fn export_set(&self, set: SetId) -> ApiResult<SetExport> {
        Ok(self.lock().export_set(set)?)
    }

    fn import_set(&self, export: SetExport) -> ApiResult<ImportSetResponse> {
        Ok(self.lock().import_set(&export)?)
    }

    fn pin_instance(&self, req: PinInstanceRequest) -> ApiResult<VirtualStyle> {
        Ok(self.lock().pin_instance(&req)?)
    }

    fn list_virtual_styles(&self, req: VirtualStylesRequest) -> ApiResult<Vec<VirtualStyle>> {
        Ok(self.lock().virtual_styles(&req)?)
    }

    fn delete_virtual_style(&self, style: VirtualStyleId) -> ApiResult<()> {
        Ok(self.lock().delete_virtual_style(style)?)
    }

    /// The list shows the family again through `LibraryChanged`.
    fn update_family_notes(&self, req: FamilyNotesUpdate) -> ApiResult<()> {
        let faces = self.lock().update_family_notes(&req)?;
        self.updated(&faces);
        Ok(())
    }

    /// The receipt is copied first; the copy it replaces goes to the trash
    /// before the declaration is stored, so a failed move changes nothing
    /// (the new copy goes to the trash too). The list shows the family again
    /// through `LibraryChanged`.
    fn declare_license(&self, req: LicenseDeclaration) -> ApiResult<()> {
        let seats = req.seats.map(crate::receipt::check_seats).transpose()?;
        let purchase_date = req
            .purchase_date
            .map(|d| d.trim().to_owned())
            .filter(|d| !d.is_empty());
        if let Some(date) = &purchase_date {
            crate::receipt::check_date(date)?;
        }
        if let ReceiptChange::Attach { path, .. } = &req.receipt {
            crate::receipt::check_receipt(Path::new(path))?;
        }
        let target = self.lock().license_target(req.family)?;
        let old = target.current.and_then(|c| c.receipt);
        let (receipt, attached) = match req.receipt {
            ReceiptChange::Keep => (old.clone(), false),
            ReceiptChange::Clear => (None, false),
            ReceiptChange::Attach { path, place } => {
                let dir = match place {
                    ReceiptPlace::NextToFont => target
                        .sidecar_dir
                        .ok_or_else(|| {
                            FmError::invalid(
                                "The receipt cannot be kept next to this font: its folder is a system folder, paused or not there. Keep it in the app's data folder instead.",
                            )
                        })?
                        .join(crate::receipt::RECEIPTS_DIR),
                    ReceiptPlace::AppData => self
                        .receipts
                        .clone()
                        .ok_or_else(|| not_available("Keeping receipts in the app's data folder"))?,
                };
                (
                    Some(crate::receipt::copy_into(Path::new(&path), &dir)?),
                    true,
                )
            }
        };
        if let Some(old) = old.as_deref().filter(|o| receipt.as_deref() != Some(*o))
            && let Err(e) = self.trash_receipt(old)
        {
            if let Some(new) = receipt.as_deref().filter(|_| attached) {
                // The copy just made is the app's: it goes to the trash too.
                if let Err(undo) = self.trash_receipt(new) {
                    tracing::warn!(error = %undo, "new receipt copy left in place");
                }
            }
            return Err(e);
        }
        let license = DeclaredLicense {
            license_type: req.license_type,
            seats,
            purchase_date,
            receipt,
        };
        let faces = self.lock().declare_license(req.family, &license)?;
        self.updated(&faces);
        Ok(())
    }

    fn quick_collection(&self) -> ApiResult<QuickCollection> {
        Ok(self.lock().quick_collection()?)
    }

    fn add_to_quick_collection(&self, req: QuickCollectionRequest) -> ApiResult<QuickCollection> {
        Ok(self.lock().add_to_quick_collection(&req.faces)?)
    }

    fn remove_from_quick_collection(
        &self,
        req: QuickCollectionRequest,
    ) -> ApiResult<QuickCollection> {
        Ok(self.lock().remove_from_quick_collection(&req.faces)?)
    }

    fn clear_quick_collection(&self) -> ApiResult<()> {
        Ok(self.lock().clear_quick_collection()?)
    }
}

impl ExportService for CoreApi {
    /// The faces are looked up under the index lock; their files are read,
    /// subset and written outside it. The subset is checked before any face
    /// is looked up. With `css`, `fonts.css` is written beside the fonts
    /// ([`web_font::export_with_css`]) and its text returned; with
    /// `overwrite`, files of the same name are replaced ([`web_font::write`]).
    fn export_web(&self, req: ExportWebRequest) -> ApiResult<ExportWebResponse> {
        if req.faces.is_empty() || req.formats.is_empty() {
            return Err(ApiError::invalid("Pick at least one font and one format."));
        }
        if req.out_dir.trim().is_empty() {
            return Err(ApiError::invalid("Choose an output folder."));
        }
        let keep = req.subset.as_ref().map(subset::code_points).transpose()?;
        let mut ids = HashSet::new();
        let mut formats = Vec::new();
        for f in req.formats {
            if !formats.contains(&f) {
                formats.push(f);
            }
        }
        let found = {
            let index = self.lock();
            req.faces
                .into_iter()
                .filter(|id| ids.insert(*id))
                .map(|id| Ok((id, index.web_export_face(id)?)))
                .collect::<FmResult<Vec<_>>>()?
        };
        // The license guard (E20.6): one blocked face refuses the whole
        // request before anything is written, and the refusal names them all.
        let blocked: Vec<_> = found
            .iter()
            .filter_map(|(id, f)| f.blocked.map(|why| (*id, &f.face.label, why)))
            .collect();
        if !blocked.is_empty() {
            let reasons: Vec<_> = blocked
                .iter()
                .map(|(_, label, why)| format!("{label}: {}", why.text()))
                .collect();
            let ids: Vec<_> = blocked.iter().map(|(id, ..)| id.0.to_string()).collect();
            return Err(FmError::new(
                ErrorCode::PermissionDenied,
                format!(
                    "{}. Declare a web license for the family to export it.",
                    reasons.join("; ")
                ),
            )
            .with_detail(format!("blocked faces: {}", ids.join(", ")))
            .into());
        }
        let (faces, styles): (Vec<_>, Vec<_>) =
            found.into_iter().map(|(_, f)| (f.face, f.style)).unzip();
        let existing = if req.overwrite {
            web_font::Existing::Replace
        } else {
            web_font::Existing::Refuse
        };
        let styles = req.css.then_some(styles.as_slice());
        let out = web_font::write(
            &faces,
            styles,
            &formats,
            keep.as_ref(),
            Path::new(&req.out_dir),
            existing,
        )?;
        Ok(ExportWebResponse {
            files: out.files.iter().map(|p| p.display().to_string()).collect(),
            css: out.css,
        })
    }

    /// Built from the index alone ([`framework_snippet::snippet`]); no font
    /// file is read and nothing is written, so a face whose license keeps it
    /// off the web still gets a snippet. A face listed twice counts once.
    fn framework_snippet(&self, req: FrameworkSnippetRequest) -> ApiResult<FrameworkSnippet> {
        if req.faces.is_empty() {
            return Err(ApiError::invalid("Pick at least one font."));
        }
        let mut ids = HashSet::new();
        let faces = {
            let index = self.lock();
            req.faces
                .into_iter()
                .filter(|id| ids.insert(*id))
                .map(|id| {
                    let f = index.web_export_face(id)?;
                    Ok(SnippetFace {
                        style: f.style,
                        file_stem: f.face.file_stem,
                        classification: f.classification,
                    })
                })
                .collect::<FmResult<Vec<_>>>()?
        };
        Ok(FrameworkSnippet {
            text: framework_snippet::snippet(req.framework, &faces, req.fonts_path.as_deref()),
        })
    }

    /// The CSV is returned, nothing is written: the shell saves it to a
    /// file the user picks.
    fn license_inventory(&self, req: LicenseInventoryRequest) -> ApiResult<LicenseInventory> {
        let rows = self.lock().license_inventory(req.collection)?;
        Ok(crate::inventory::to_csv(&rows))
    }

    /// Reads the face's file outside the index lock, as `list_glyphs`; the
    /// SVG is returned, nothing is written.
    fn export_glyph_svg(&self, req: GlyphSvgRequest) -> ApiResult<String> {
        let file = self.lock().face_file(req.face)?;
        Ok(glyph_svg::glyph_svg_file(
            &file.path,
            file.collection_index,
            &req,
        )?)
    }

    /// The family is looked up under the index lock; its files are read
    /// and the page written outside it ([`specimen::write_html`]). A PDF is
    /// that page printed by the injected printer ([`specimen::write_pdf`]),
    /// on the caller's thread, which is never the UI thread.
    fn export_specimen(&self, req: SpecimenRequest) -> ApiResult<SpecimenResponse> {
        if req.out_path.trim().is_empty() {
            return Err(ApiError::invalid("Choose a file to save the specimen to."));
        }
        let printer = self.specimen_printer(req.format)?;
        let spec = self.lock().specimen(req.family)?;
        let out = Path::new(&req.out_path);
        let path = match printer {
            Some(printer) => specimen::write_pdf(&spec, out, printer)?,
            None => specimen::write_html(&spec, out)?,
        };
        Ok(SpecimenResponse {
            path: path.display().to_string(),
        })
    }

    /// As [`Self::export_specimen`]: the collection's families are looked
    /// up under the index lock ([`Index::collection_specimen`]), the files
    /// read and the document written or printed outside it, dated today.
    fn export_collection_specimen(
        &self,
        req: CollectionSpecimenRequest,
    ) -> ApiResult<SpecimenResponse> {
        if req.out_path.trim().is_empty() {
            return Err(ApiError::invalid("Choose a file to save the specimen to."));
        }
        let printer = self.specimen_printer(req.format)?;
        let spec = self.lock().collection_specimen(req.collection)?;
        let out = Path::new(&req.out_path);
        let date = specimen::today();
        let path = match printer {
            Some(printer) => specimen::write_collection_pdf(&spec, &date, out, printer)?,
            None => specimen::write_collection_html(&spec, &date, out)?,
        };
        Ok(SpecimenResponse {
            path: path.display().to_string(),
        })
    }
}

impl CoreApi {
    /// The printer a specimen in `format` needs: none for HTML, the injected
    /// one for a PDF (`Unsupported` without it, the CLI's direct link).
    fn specimen_printer(&self, format: SpecimenFormat) -> ApiResult<Option<&dyn PdfPrinter>> {
        match format {
            SpecimenFormat::Html => Ok(None),
            SpecimenFormat::Pdf => self
                .pdf
                .as_deref()
                .map(Some)
                .ok_or_else(|| ApiError::new(ErrorCode::Unsupported, PDF_NEEDS_APP)),
        }
    }
}

impl WorkflowService for CoreApi {
    fn pick_fonts_for_brief(&self, _req: BriefRequest) -> ApiResult<Shortlist> {
        Err(not_available("Picking fonts for a brief"))
    }

    fn prepare_for_web(&self, _req: PrepareForWebRequest) -> ApiResult<PrepareForWebResponse> {
        Err(not_available("Preparing fonts for the web"))
    }
}

impl LayoutService for CoreApi {
    fn layout_presets(&self) -> ApiResult<Vec<Layout>> {
        Ok(layout::presets())
    }

    fn clamp_layout(&self, req: Layout) -> ApiResult<Layout> {
        Ok(layout::clamp_layout(req))
    }

    fn layout_bounds(&self) -> ApiResult<LayoutBounds> {
        Ok(layout::bounds())
    }

    fn list_layouts(&self) -> ApiResult<Vec<LayoutEntry>> {
        self.layouts()?.list()
    }

    fn get_layout(&self, req: LayoutRef) -> ApiResult<SavedLayout> {
        self.layouts()?.get(&req.id)
    }

    fn save_layout(&self, req: SaveLayoutRequest) -> ApiResult<SavedLayout> {
        self.layouts()?.save(&req.id, req.layout)
    }

    fn duplicate_layout(&self, req: DuplicateLayoutRequest) -> ApiResult<SavedLayout> {
        self.layouts()?.duplicate(&req.id, req.name.as_deref())
    }

    fn revert_layout(&self, req: RevertLayoutRequest) -> ApiResult<SavedLayout> {
        self.layouts()?.revert(&req.id, req.to_original)
    }

    fn layout_history(&self, req: LayoutRef) -> ApiResult<Vec<LayoutVersion>> {
        self.layouts()?.history(&req.id)
    }

    fn restore_layout(&self, req: RestoreLayoutRequest) -> ApiResult<SavedLayout> {
        self.layouts()?.restore(&req.id, req.number)
    }

    fn layout_width(&self, layout: Layout) -> ApiResult<Option<f32>> {
        Ok(layout::layout_width(&layout))
    }
}

impl ComparisonService for CoreApi {
    fn list_comparisons(&self) -> ApiResult<Vec<ComparisonEntry>> {
        Ok(self.lock().list_comparisons()?)
    }

    fn get_comparison(&self, req: ComparisonRef) -> ApiResult<Comparison> {
        Ok(self.lock().comparison(req.id)?)
    }

    fn save_comparison(&self, req: SaveComparisonRequest) -> ApiResult<Comparison> {
        Ok(self.lock().save_comparison(&req)?)
    }

    fn rename_comparison(&self, req: RenameComparisonRequest) -> ApiResult<ComparisonEntry> {
        Ok(self.lock().rename_comparison(req.id, &req.name)?)
    }

    fn delete_comparison(&self, req: ComparisonRef) -> ApiResult<()> {
        Ok(self.lock().delete_comparison(req.id)?)
    }
}

impl CatalogService for CoreApi {
    fn list_catalogs(&self) -> ApiResult<Vec<Catalog>> {
        Ok(self.catalogs()?.list()?)
    }

    fn set_catalog_enabled(&self, req: SetCatalogEnabledRequest) -> ApiResult<Catalog> {
        Ok(self.catalogs()?.set_enabled(req.id, req.enabled)?)
    }

    fn refresh_catalog(&self, req: CatalogRef) -> ApiResult<Catalog> {
        Ok(self.catalogs()?.refresh(req.id)?)
    }

    fn list_catalog_families(
        &self,
        req: ListCatalogFamiliesRequest,
    ) -> ApiResult<Paged<CatalogFamily>> {
        Ok(self.catalogs()?.families(req.id, &req.query, req.page)?)
    }

    fn catalog_font_source(&self, req: CatalogFontRequest) -> ApiResult<CatalogFontSource> {
        let path = self.catalogs()?.font_source(req.id, &req.family)?;
        Ok(CatalogFontSource {
            id: req.id,
            family: req.family,
            path: path.to_string_lossy().into_owned(),
        })
    }

    /// Downloads into a watched source, records the provenance of the files
    /// written (none recorded when every file was already there) and starts
    /// a rescan of the source, whose events bring the new faces. A download
    /// that stopped after some files were written answers them with its
    /// `error`, recorded and rescanned like a finished one.
    fn download_family(&self, req: DownloadFamilyRequest) -> ApiResult<DownloadFamilyResponse> {
        let catalogs = self.catalogs()?;
        let source = self
            .lock()
            .sources()?
            .into_iter()
            .find(|s| s.id == req.source)
            .ok_or_else(|| ApiError::not_found(format!("Source {}", req.source)))?;
        if source.kind != SourceKind::Watched {
            return Err(ApiError::invalid(
                "Fonts can be downloaded only into a folder you added.",
            ));
        }
        let got = catalogs.download(req.id, &req.family, req.kind, Path::new(&source.path))?;
        let at = crate::catalog::now_rfc3339();
        let mut error = got.stopped.clone().map(ApiError::from);
        if !got.written.is_empty() {
            // The files are on disk either way: a provenance that cannot be
            // recorded is reported with them, and they are still rescanned.
            if let Err(e) =
                self.lock()
                    .record_catalog_download(req.id, &req.family, source.id, &at, &got)
            {
                tracing::warn!(error = %e, "catalog download provenance not recorded");
                error.get_or_insert_with(|| e.into());
            }
        }
        let mut files: Vec<CatalogDownloadFile> = got
            .written
            .iter()
            .map(|f| CatalogDownloadFile {
                name: f.name.clone(),
                hash: blake3::Hash::from_bytes(f.hash).to_hex().to_string(),
                path: Some(f.path.to_string_lossy().into_owned()),
            })
            .collect();
        // By name, as `list_catalog_downloads` lists them.
        files.sort_by(|a, b| a.name.cmp(&b.name));
        let download = CatalogDownload {
            catalog: req.id,
            family: req.family,
            catalog_family_id: got.family_id,
            version: got.version,
            kind: got.kind,
            downloaded_at: at,
            source: source.id,
            folder: got.folder.to_string_lossy().into_owned(),
            files,
        };
        let scan = self.start_scan(vec![source])?;
        Ok(DownloadFamilyResponse {
            download,
            skipped: got.skipped,
            scan,
            error,
        })
    }

    fn list_catalog_downloads(&self) -> ApiResult<Vec<CatalogDownload>> {
        Ok(self.lock().catalog_downloads()?)
    }

    fn check_updates(&self, req: CheckUpdatesRequest) -> ApiResult<Vec<CatalogUpdate>> {
        let catalogs = self.catalogs()?;
        let downloads = self.lock().catalog_downloads()?;
        Ok(catalogs.check_updates(&downloads, !req.cached_only)?)
    }
}

impl SettingsService for CoreApi {
    fn get_settings(&self) -> ApiResult<Settings> {
        self.settings.get_settings()
    }

    /// Saves the settings; a new cache limit applies to the preview cache at once.
    fn update_settings(&self, req: SettingsUpdate) -> ApiResult<Settings> {
        let settings = self.settings.update_settings(req)?;
        if let Some(previews) = &self.previews {
            previews.set_limit(cache::limit_from_mb(settings.cache_limit_mb));
        }
        Ok(settings)
    }

    fn app_folders(&self) -> ApiResult<AppFolders> {
        self.settings.app_folders()
    }

    fn endpoint_info(&self) -> ApiResult<EndpointInfo> {
        Ok(self.endpoint_keys()?.info()?)
    }

    fn regenerate_endpoint_token(&self) -> ApiResult<EndpointInfo> {
        Ok(self.endpoint_keys()?.regenerate()?)
    }

    /// The switch and grants from `config.json`, the URL and snippets from
    /// the endpoint.
    fn mcp_status(&self) -> ApiResult<McpStatus> {
        let endpoint = self.endpoint_keys()?.info()?;
        Ok(crate::mcp::status(
            self.settings.mcp_enabled()?,
            &endpoint,
            &self.settings.mcp_grants()?,
        ))
    }

    /// Saves the switch; the endpoint reads it on the next `/mcp` request.
    fn set_mcp_enabled(&self, req: McpUpdate) -> ApiResult<McpStatus> {
        let endpoint = self.endpoint_keys()?.info()?;
        self.settings.set_mcp_enabled(req.enabled)?;
        Ok(crate::mcp::status(
            req.enabled,
            &endpoint,
            &self.settings.mcp_grants()?,
        ))
    }

    /// Saves the grant; the next tool call reads it from `config.json`.
    fn set_mcp_grant(&self, req: McpGrantUpdate) -> ApiResult<McpStatus> {
        if !crate::mcp::needs_grant(&req.tool) {
            return Err(ApiError::invalid(format!(
                "{} needs no grant: only the tools that change something do.",
                req.tool
            )));
        }
        self.settings.set_mcp_grant(&req.tool, req.granted)?;
        self.mcp_status()
    }

    /// Read tools run; a mutating tool needs its grant, and a destructive
    /// one the user's Allow too. Nothing is changed here either way.
    fn authorize_mcp_tool(&self, req: McpToolCall) -> ApiResult<()> {
        if !crate::mcp::needs_grant(&req.tool) {
            return Ok(());
        }
        if !self.settings.mcp_grants()?.contains(&req.tool) {
            return Err(crate::mcp::not_granted(&req.tool));
        }
        if !crate::mcp::needs_confirmation(&req.tool) {
            return Ok(());
        }
        // An unknown target is refused before the user is asked. No target
        // on deactivate_fonts means every face the app activated.
        let count = |n: usize| Some(u32::try_from(n).unwrap_or(u32::MAX));
        let faces = match &req.target {
            Some(target) => count(self.lock().target_faces(target)?.len()),
            None if req.tool == "deactivate_fonts" => count(self.list_active()?.len()),
            None => None,
        };
        let emit = |e: &Event| self.events.send(e);
        if self.mcp_confirmations.ask(&req.tool, faces, &emit) {
            Ok(())
        } else {
            Err(crate::mcp::not_confirmed(&req.tool))
        }
    }

    fn answer_mcp_confirmation(&self, req: McpConfirmationAnswer) -> ApiResult<()> {
        let emit = |e: &Event| self.events.send(e);
        self.mcp_confirmations.answer(req.id, req.allow, &emit)
    }
}

impl EventSource for CoreApi {
    fn subscribe(&self) -> Receiver<Event> {
        self.events.subscribe()
    }
}

#[cfg(test)]
mod tests;
