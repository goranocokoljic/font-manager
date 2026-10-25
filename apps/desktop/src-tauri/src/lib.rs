//! The Font Manager desktop shell: Tauri commands are thin adapters over
//! `fm-api`. Each command takes the shared [`Api`] from Tauri state, calls one
//! service method off the async runtime (ADR 0006) and returns its result as is.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use fm_api::activation::{
    ActivateRequest, ActivationOutcome, ActiveFace, Clash, Conflict, DeactivateRequest, Target,
};
use fm_api::catalog::{
    Catalog, CatalogDownload, CatalogFamily, CatalogFontRequest, CatalogId, CatalogRef,
    CatalogService, CatalogUpdate, CheckUpdatesRequest, DownloadFamilyRequest,
    DownloadFamilyResponse, ListCatalogFamiliesRequest, SetCatalogEnabledRequest,
};
use fm_api::common::Paged;
use fm_api::comparison::{
    Comparison, ComparisonEntry, ComparisonRef, RenameComparisonRequest, SaveComparisonRequest,
};
use fm_api::export::{ExportService, LicenseInventoryRequest};
use fm_api::layout::{
    DuplicateLayoutRequest, Layout, LayoutBounds, LayoutEntry, LayoutRef, LayoutVersion,
    RestoreLayoutRequest, RevertLayoutRequest, SaveLayoutRequest, SavedLayout,
};
use fm_api::library::{
    AddSourceRequest, AppInfo, DuplicateGroup, FaceDetail, Family, FamilySummary, GlyphDetail,
    GlyphPage, GlyphRequest, GlyphsRequest, KeepCopyRequest, LibraryService, ListFamiliesRequest,
    Problem, Resolution, ScanRequest, SetSourcePausedRequest, Source,
};
use fm_api::mock::MockApi;
use fm_api::organization::{
    Collection, CollectionFacesRequest, CreateCollectionRequest, FamilyNotesUpdate,
    LicenseDeclaration, MoveCollectionRequest, PinInstanceRequest, ProjectSet, QuickCollection,
    QuickCollectionRequest, RenameCollectionRequest, SelectionTags, SelectionTagsRequest, Tag,
    TagFacesRequest, UntagFacesRequest, UpdateCollectionRulesRequest, UserMetaUpdate, VirtualStyle,
    VirtualStylesRequest,
};
use fm_api::preview::{
    CssValues, CssValuesRequest, PreviewImage, PreviewRequest, PreviewService, SampleText,
    SampleTextRequest, SizeFactor, SizeFactorRequest,
};
use fm_api::search::{
    CoverageReport, CoverageRequest, FaceCoverage, LanguagePreset, PairingRequest,
    PairingSuggestion, SearchRequest, SearchResponse, SimilarFace, SimilarRequest,
};
use fm_api::settings::{AppFolders, EndpointInfo, Settings, SettingsUpdate};
use fm_api::{
    Api, ApiError, ApiResult, CollectionId, ErrorCode, FaceId, FamilyId, JobId, ResolutionId,
    SettingsService, SourceId, VirtualStyleId,
};
use fm_core::index::Opened;
use fm_core::lifecycle::{self, LaunchWindow};
use fm_core::{CoreApi, SettingsStore};
use fm_platform::{AppPaths, FontActivator, MockActivator, Tray, TraySupport};
use tauri::http::{Response, StatusCode, header};
use tauri::{AppHandle, Emitter, Manager, RunEvent, Runtime, State, Webview, WindowEvent};
use tauri_plugin_dialog::DialogExt;

pub mod help;
pub mod shell_text;
pub mod tray;
pub mod updater;

/// The Tauri event every `fm_api::Event` is forwarded on.
pub const EVENT_CHANNEL: &str = "fm://event";

/// The URI scheme the webview loads font files from, by face id:
/// `fmfont://localhost/<face>` (`http://fmfont.localhost/<face>` on Windows).
/// Only files of faces the API knows are served, so the webview needs no file
/// system scope over the user's folders.
pub const FONT_SCHEME: &str = "fmfont";

/// Which implementation of [`Api`] the commands talk to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// `fm-core`'s [`CoreApi`]: the real library.
    Core,
    /// Any other implementation: [`MockApi`] when the app folders or the
    /// index could not be opened, and in tests.
    Other,
}

/// The API every command talks to: `fm-core`'s [`CoreApi`] when startup
/// found the app folders and opened the index ([`serve`]), else [`MockApi`]
/// with the settings from `fm-core`'s [`SettingsStore`] when the folders were
/// found.
pub struct AppState {
    api: Arc<dyn Api>,
    settings: Option<Arc<dyn SettingsService + Send + Sync>>,
    backend: Backend,
    core: Option<Arc<CoreApi>>,
    tray: Option<Arc<dyn Tray>>,
    /// The quit path ran ([`AppState::run_quit_path`]).
    quit_ran: AtomicBool,
    /// The local API endpoint (ADR 0015); shared with the thread that starts it.
    endpoint: Arc<Mutex<EndpointSlot>>,
}

/// The local API endpoint while it runs, and whether the quit path stopped it.
#[derive(Debug, Default)]
struct EndpointSlot {
    running: Option<fm_endpoint::Endpoint>,
    stopped: bool,
}

impl AppState {
    /// Wraps an API implementation for Tauri's managed state.
    pub fn new(api: Arc<dyn Api>) -> Self {
        Self {
            api,
            settings: None,
            backend: Backend::Other,
            core: None,
            tray: None,
            quit_ran: AtomicBool::new(false),
            endpoint: Arc::default(),
        }
    }

    /// Serves the real library.
    pub fn core(api: Arc<CoreApi>) -> Self {
        Self {
            api: Arc::clone(&api) as Arc<dyn Api>,
            settings: None,
            backend: Backend::Core,
            core: Some(api),
            tray: None,
            quit_ran: AtomicBool::new(false),
            endpoint: Arc::default(),
        }
    }

    /// Shows a tray item from `tray` when the app starts ([`tray::install`]).
    /// Without one (tests), closing the window quits.
    #[must_use]
    pub fn with_tray(mut self, tray: Arc<dyn Tray>) -> Self {
        self.tray = Some(tray);
        self
    }

    /// The settings now, read synchronously (the close handler); the
    /// defaults when they cannot be read.
    pub fn current_settings(&self) -> Settings {
        let read = match &self.settings {
            Some(store) => store.get_settings(),
            None => self.api.get_settings(),
        };
        read.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "settings not read; using the defaults");
            Settings::default()
        })
    }

    /// Which implementation the commands talk to.
    pub fn backend(&self) -> Backend {
        self.backend
    }

    /// The normal quit path, [`on_exit`], once: every quit ends here, and so
    /// does an update before it installs (E18.2). A second run would record
    /// an empty last session over the real one.
    pub fn run_quit_path(&self) {
        if !self.quit_ran.swap(true, Ordering::SeqCst) {
            self.stop_endpoint();
            on_exit(self.core.as_deref());
        }
    }

    /// Starts the local API endpoint (ADR 0015) on a thread of its own, so
    /// the window never waits for it; it then writes `endpoint.json`. Only
    /// the real library is served: without it there is no data folder.
    /// The handle is for tests, which wait for the start.
    pub fn start_endpoint(&self) -> Option<std::thread::JoinHandle<()>> {
        let core = Arc::clone(self.core.as_ref()?);
        let slot = Arc::clone(&self.endpoint);
        lock_slot(&slot).stopped = false;
        let started = std::thread::Builder::new()
            .name("fm-endpoint-start".into())
            .spawn(move || {
                let endpoint = match fm_endpoint::Endpoint::start(Arc::clone(&core) as Arc<dyn Api>)
                {
                    Ok(endpoint) => endpoint,
                    Err(e) => {
                        tracing::warn!(error = %e, "the local API could not start");
                        return;
                    }
                };
                let mut slot = lock_slot(&slot);
                // The app began to quit meanwhile: no discovery file.
                if slot.stopped || slot.running.is_some() {
                    return;
                }
                if let Err(e) = core.publish_endpoint(endpoint.addr()) {
                    tracing::warn!(error = %e, "endpoint.json not written");
                }
                slot.running = Some(endpoint);
            });
        started
            .inspect_err(|e| tracing::warn!(error = %e, "the local API could not start"))
            .ok()
    }

    /// Stops the local API endpoint and removes `endpoint.json`.
    pub fn stop_endpoint(&self) {
        let mut slot = lock_slot(&self.endpoint);
        slot.stopped = true;
        if let Some(endpoint) = slot.running.take() {
            endpoint.stop();
        }
        if let Some(core) = &self.core
            && let Err(e) = core.unpublish_endpoint()
        {
            tracing::warn!(error = %e, "endpoint.json not removed");
        }
    }

    /// Where the local API endpoint listens; `None` while it does not run.
    pub fn endpoint_addr(&self) -> Option<std::net::SocketAddr> {
        lock_slot(&self.endpoint)
            .running
            .as_ref()
            .map(fm_endpoint::Endpoint::addr)
    }

    /// The faces active for this session now: the ones the quit path ends.
    /// Empty without the real library.
    pub fn session_faces(&self) -> Vec<FaceId> {
        let Some(core) = &self.core else {
            return Vec::new();
        };
        match fm_api::ActivationService::list_active(core.as_ref()) {
            Ok(active) => lifecycle::session_faces(&active),
            Err(e) => {
                tracing::warn!(error = %e, "session fonts not read");
                Vec::new()
            }
        }
    }

    /// Undoes the quit path when the app goes on running after all (an
    /// update that did not install): `faces`, the session fonts it ended,
    /// are activated again, and the quit path runs again when the app quits,
    /// so the last session it records is the real one.
    pub fn resume_after_quit(&self, faces: Vec<FaceId>) {
        if let Some(core) = &self.core {
            let on_conflict = self.current_settings().on_conflict;
            match lifecycle::restore_session(core.as_ref(), faces, on_conflict) {
                Ok(outcomes) => {
                    let missed = outcomes
                        .iter()
                        .filter(|o| o.outcome != fm_api::activation::OutcomeStatus::Done)
                        .count();
                    if missed > 0 {
                        tracing::warn!(missed, "some session fonts were not activated again");
                    }
                }
                Err(e) => tracing::warn!(error = %e, "session fonts not activated again"),
            }
        }
        self.quit_ran.store(false, Ordering::SeqCst);
        // The quit path stopped the local API; it serves again.
        self.start_endpoint();
    }

    /// Serves the settings from `settings` instead of the API.
    pub fn with_settings(mut self, settings: Arc<dyn SettingsService + Send + Sync>) -> Self {
        self.settings = Some(settings);
        self
    }
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState").finish_non_exhaustive()
    }
}

fn lock_slot(slot: &Mutex<EndpointSlot>) -> std::sync::MutexGuard<'_, EndpointSlot> {
    slot.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Runs a synchronous API call on the blocking pool.
async fn call<T, F>(state: &AppState, f: F) -> ApiResult<T>
where
    T: Send + 'static,
    F: FnOnce(&dyn Api) -> ApiResult<T> + Send + 'static,
{
    let api = Arc::clone(&state.api);
    tauri::async_runtime::spawn_blocking(move || f(api.as_ref()))
        .await
        .map_err(|e| {
            ApiError::new(ErrorCode::Internal, "The app could not finish the request.")
                .with_detail(e.to_string())
        })?
}

/// Runs a synchronous settings call on the blocking pool, on the settings
/// store when there is one, else on the API.
async fn call_settings<T, F>(state: &AppState, f: F) -> ApiResult<T>
where
    T: Send + 'static,
    F: FnOnce(&dyn SettingsService) -> ApiResult<T> + Send + 'static,
{
    match &state.settings {
        Some(store) => {
            let store = Arc::clone(store);
            tauri::async_runtime::spawn_blocking(move || f(store.as_ref()))
                .await
                .map_err(|e| {
                    ApiError::new(ErrorCode::Internal, "The app could not finish the request.")
                        .with_detail(e.to_string())
                })?
        }
        None => call(state, move |api| f(api)).await,
    }
}

/// Answers a request on [`FONT_SCHEME`] whose URI path is `/<face id>`: the
/// face's font file with its media type, or a status with one plain sentence.
pub fn font_response(api: &dyn PreviewService, uri_path: &str) -> Response<Vec<u8>> {
    let Some(face) = uri_path
        .strip_prefix('/')
        .and_then(|id| id.parse::<u32>().ok())
        .map(FaceId)
    else {
        return error_response(StatusCode::BAD_REQUEST, "The font address is not valid.");
    };
    let file = api.font_source(face).and_then(|source| {
        fm_core::font_file::read_font_file(Path::new(&source.path)).map_err(ApiError::from)
    });
    match file {
        Ok(file) => font_ok(file.mime, file.bytes),
        Err(e) => {
            tracing::debug!(face = face.0, code = ?e.code, detail = ?e.detail, "font not served");
            let status = match e.code {
                ErrorCode::NotFound => StatusCode::NOT_FOUND,
                ErrorCode::PermissionDenied => StatusCode::FORBIDDEN,
                ErrorCode::Unsupported => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            error_response(status, &e.message)
        }
    }
}

/// The URI path of a catalog family's preview on [`FONT_SCHEME`]:
/// `/catalog-<catalog id>-<the family name's UTF-8 as hex>`. Hex, because the
/// webview's address helper percent-encodes everything else.
const CATALOG_FONT_PREFIX: &str = "/catalog-";

/// The catalog and family a [`CATALOG_FONT_PREFIX`] path names; `None` for
/// any other path, `Some(Err)` for a catalog path that is not valid.
fn catalog_font_request(uri_path: &str) -> Option<Result<CatalogFontRequest, ()>> {
    let rest = uri_path.strip_prefix(CATALOG_FONT_PREFIX)?;
    let parse = || {
        let (id, hex) = rest.split_once('-').ok_or(())?;
        let id: CatalogId =
            serde_json::from_value(serde_json::Value::String(id.to_owned())).map_err(|_| ())?;
        if hex.is_empty() || hex.len() % 2 != 0 {
            return Err(());
        }
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(hex.get(i..i + 2).ok_or(())?, 16).map_err(|_| ()))
            .collect::<Result<Vec<u8>, ()>>()?;
        let family = String::from_utf8(bytes).map_err(|_| ())?;
        Ok(CatalogFontRequest { id, family })
    };
    Some(parse())
}

/// Answers a request on [`FONT_SCHEME`] whose path is a catalog family's
/// preview (E22.2): the core fetches it into the cache folder the first
/// time, only while the catalog is on. `None` when the path is not one.
pub fn catalog_font_response(
    api: &dyn CatalogService,
    uri_path: &str,
) -> Option<Response<Vec<u8>>> {
    let Ok(req) = catalog_font_request(uri_path)? else {
        return Some(error_response(
            StatusCode::BAD_REQUEST,
            "The font address is not valid.",
        ));
    };
    let file = api.catalog_font_source(req).and_then(|source| {
        fm_core::font_file::read_font_file(Path::new(&source.path)).map_err(ApiError::from)
    });
    Some(match file {
        Ok(file) => font_ok(file.mime, file.bytes),
        Err(e) => {
            tracing::debug!(code = ?e.code, detail = ?e.detail, "catalog preview not served");
            let status = match e.code {
                ErrorCode::NotFound => StatusCode::NOT_FOUND,
                ErrorCode::Conflict | ErrorCode::PermissionDenied => StatusCode::FORBIDDEN,
                ErrorCode::Unsupported => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                ErrorCode::Io => StatusCode::BAD_GATEWAY,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            error_response(status, &e.message)
        }
    })
}

/// Font loads are CORS requests and the app page has another origin than
/// [`FONT_SCHEME`], so every answer allows any origin.
fn font_ok(mime: &str, bytes: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(bytes)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn error_response(status: StatusCode, message: &str) -> Response<Vec<u8>> {
    let mut response = Response::new(message.as_bytes().to_vec());
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        header::HeaderValue::from_static("*"),
    );
    response
}

/// What the running app is: name, version, API contract and platform.
#[tauri::command]
async fn get_app_info(state: State<'_, AppState>) -> ApiResult<AppInfo> {
    call(&state, |api| api.app_info()).await
}

/// Full detail of one face, for the inspector.
#[tauri::command]
async fn get_face(state: State<'_, AppState>, face: FaceId) -> ApiResult<FaceDetail> {
    call(&state, move |api| api.get_face(face)).await
}

/// One page of a face's glyph map, for the inspector's Glyphs tab.
#[tauri::command]
async fn list_glyphs(state: State<'_, AppState>, req: GlyphsRequest) -> ApiResult<GlyphPage> {
    call(&state, move |api| api.list_glyphs(req)).await
}

/// One character of a face in detail, for the Glyphs tab's detail panel.
#[tauri::command]
async fn get_glyph(state: State<'_, AppState>, req: GlyphRequest) -> ApiResult<GlyphDetail> {
    call(&state, move |api| api.get_glyph(req)).await
}

/// One page of families.
#[tauri::command]
async fn list_families(
    state: State<'_, AppState>,
    req: ListFamiliesRequest,
) -> ApiResult<Paged<FamilySummary>> {
    call(&state, move |api| api.list_families(req)).await
}

/// A grid thumbnail: text rasterized in one face (cached by the API).
#[tauri::command]
async fn render_preview(
    state: State<'_, AppState>,
    req: PreviewRequest,
) -> ApiResult<PreviewImage> {
    call(&state, move |api| api.render_preview(req)).await
}

/// One family with its faces.
#[tauri::command]
async fn sample_texts(
    state: State<'_, AppState>,
    req: SampleTextRequest,
) -> ApiResult<Vec<SampleText>> {
    call(&state, move |api| api.sample_texts(req)).await
}

/// The auto size factor of each face, for the faces on screen (E16.2).
#[tauri::command]
async fn size_factors(
    state: State<'_, AppState>,
    req: SizeFactorRequest,
) -> ApiResult<Vec<SizeFactor>> {
    call(&state, move |api| api.size_factors(req)).await
}

/// The CSS for a face's moved axes and switched features ("Copy CSS").
#[tauri::command]
async fn css_values(state: State<'_, AppState>, req: CssValuesRequest) -> ApiResult<CssValues> {
    call(&state, move |api| api.css_values(req)).await
}

/// The six body-text layout presets.
#[tauri::command]
async fn layout_presets(state: State<'_, AppState>) -> ApiResult<Vec<Layout>> {
    call(&state, move |api| api.layout_presets()).await
}

/// A layout with every block value clamped into its range.
#[tauri::command]
async fn clamp_layout(state: State<'_, AppState>, req: Layout) -> ApiResult<Layout> {
    call(&state, move |api| api.clamp_layout(req)).await
}

/// The ranges block values are clamped into, with their steps.
#[tauri::command]
async fn layout_bounds(state: State<'_, AppState>) -> ApiResult<LayoutBounds> {
    call(&state, move |api| api.layout_bounds()).await
}

/// Every layout template file in the layouts folder, broken ones with the reason.
#[tauri::command]
async fn list_layouts(state: State<'_, AppState>) -> ApiResult<Vec<LayoutEntry>> {
    call(&state, move |api| api.list_layouts()).await
}

/// One saved layout.
#[tauri::command]
async fn get_layout(state: State<'_, AppState>, req: LayoutRef) -> ApiResult<SavedLayout> {
    call(&state, move |api| api.get_layout(req)).await
}

/// Saves a layout; the content it replaces becomes a version.
#[tauri::command]
async fn save_layout(state: State<'_, AppState>, req: SaveLayoutRequest) -> ApiResult<SavedLayout> {
    call(&state, move |api| api.save_layout(req)).await
}

/// Copies a layout into a new template file.
#[tauri::command]
async fn duplicate_layout(
    state: State<'_, AppState>,
    req: DuplicateLayoutRequest,
) -> ApiResult<SavedLayout> {
    call(&state, move |api| api.duplicate_layout(req)).await
}

/// The saved file again, or a preset's shipped original.
#[tauri::command]
async fn revert_layout(
    state: State<'_, AppState>,
    req: RevertLayoutRequest,
) -> ApiResult<SavedLayout> {
    call(&state, move |api| api.revert_layout(req)).await
}

/// The kept versions of a layout, newest first.
#[tauri::command]
async fn layout_history(
    state: State<'_, AppState>,
    req: LayoutRef,
) -> ApiResult<Vec<LayoutVersion>> {
    call(&state, move |api| api.layout_history(req)).await
}

/// Makes a kept version current.
#[tauri::command]
async fn restore_layout(
    state: State<'_, AppState>,
    req: RestoreLayoutRequest,
) -> ApiResult<SavedLayout> {
    call(&state, move |api| api.restore_layout(req)).await
}

/// The width the layout's text is set at; none when it follows the window.
#[tauri::command]
async fn layout_width(state: State<'_, AppState>, req: Layout) -> ApiResult<Option<f32>> {
    call(&state, move |api| api.layout_width(req)).await
}

/// Every online catalog with the state of its cache (E22.1).
#[tauri::command]
async fn list_catalogs(state: State<'_, AppState>) -> ApiResult<Vec<Catalog>> {
    call(&state, move |api| api.list_catalogs()).await
}

/// Turns a catalog on or off; makes no request.
#[tauri::command]
async fn set_catalog_enabled(
    state: State<'_, AppState>,
    req: SetCatalogEnabledRequest,
) -> ApiResult<Catalog> {
    call(&state, move |api| api.set_catalog_enabled(req)).await
}

/// Fetches a catalog's index (a request of a few seconds, off the async runtime).
#[tauri::command]
async fn refresh_catalog(state: State<'_, AppState>, req: CatalogRef) -> ApiResult<Catalog> {
    call(&state, move |api| api.refresh_catalog(req)).await
}

/// A page of a catalog's cached families; makes no request.
#[tauri::command]
async fn list_catalog_families(
    state: State<'_, AppState>,
    req: ListCatalogFamiliesRequest,
) -> ApiResult<Paged<CatalogFamily>> {
    call(&state, move |api| api.list_catalog_families(req)).await
}

/// Downloads a family into a watched source and starts its rescan (E22.3);
/// requests of a few seconds, off the async runtime.
#[tauri::command]
async fn download_family(
    state: State<'_, AppState>,
    req: DownloadFamilyRequest,
) -> ApiResult<DownloadFamilyResponse> {
    call(&state, move |api| api.download_family(req)).await
}

/// Every family downloaded from a catalog, with where its files are now.
#[tauri::command]
async fn list_catalog_downloads(state: State<'_, AppState>) -> ApiResult<Vec<CatalogDownload>> {
    call(&state, move |api| api.list_catalog_downloads()).await
}

/// The downloaded families with a newer version in their catalog, or no
/// longer in it (E22.4); refreshes the catalogs first unless `cachedOnly`.
#[tauri::command]
async fn check_updates(
    state: State<'_, AppState>,
    req: CheckUpdatesRequest,
) -> ApiResult<Vec<CatalogUpdate>> {
    call(&state, move |api| api.check_updates(req)).await
}

/// Every saved comparison, by name (E13.2).
#[tauri::command]
async fn list_comparisons(state: State<'_, AppState>) -> ApiResult<Vec<ComparisonEntry>> {
    call(&state, move |api| api.list_comparisons()).await
}

/// One saved comparison: its faces left and the missing ones.
#[tauri::command]
async fn get_comparison(state: State<'_, AppState>, req: ComparisonRef) -> ApiResult<Comparison> {
    call(&state, move |api| api.get_comparison(req)).await
}

/// Saves a comparison, new or over an existing one.
#[tauri::command]
async fn save_comparison(
    state: State<'_, AppState>,
    req: SaveComparisonRequest,
) -> ApiResult<Comparison> {
    call(&state, move |api| api.save_comparison(req)).await
}

/// Renames a saved comparison.
#[tauri::command]
async fn rename_comparison(
    state: State<'_, AppState>,
    req: RenameComparisonRequest,
) -> ApiResult<ComparisonEntry> {
    call(&state, move |api| api.rename_comparison(req)).await
}

/// Deletes a saved comparison; the fonts are not touched.
#[tauri::command]
async fn delete_comparison(state: State<'_, AppState>, req: ComparisonRef) -> ApiResult<()> {
    call(&state, move |api| api.delete_comparison(req)).await
}

#[tauri::command]
async fn get_family(state: State<'_, AppState>, family: FamilyId) -> ApiResult<Family> {
    call(&state, move |api| api.get_family(family)).await
}

/// Activates faces; one outcome per face.
#[tauri::command]
async fn activate(
    state: State<'_, AppState>,
    req: ActivateRequest,
) -> ApiResult<Vec<ActivationOutcome>> {
    call(&state, move |api| api.activate(req)).await
}

/// Deactivates faces the app registered; one outcome per face.
#[tauri::command]
async fn deactivate(
    state: State<'_, AppState>,
    req: DeactivateRequest,
) -> ApiResult<Vec<ActivationOutcome>> {
    call(&state, move |api| api.deactivate(req)).await
}

/// Every face the app has active (the status bar count).
#[tauri::command]
async fn list_active(state: State<'_, AppState>) -> ApiResult<Vec<ActiveFace>> {
    call(&state, |api| api.list_active()).await
}

/// The clashes activating `target` would cause, without activating anything.
#[tauri::command]
async fn conflicts(state: State<'_, AppState>, target: Target) -> ApiResult<Vec<Conflict>> {
    call(&state, move |api| api.conflicts(target)).await
}

/// Every clash of an active face, with its winner (the Conflicts view).
#[tauri::command]
async fn list_conflicts(state: State<'_, AppState>) -> ApiResult<Vec<Clash>> {
    call(&state, |api| api.list_conflicts()).await
}

/// Everything wrong with the library: duplicates, broken and legacy files,
/// sidecar and activation conflicts (the Problems view).
#[tauri::command]
async fn list_problems(state: State<'_, AppState>) -> ApiResult<Vec<Problem>> {
    call(&state, |api| api.list_problems()).await
}

/// Duplicate groups, each with its recommended keeper first (Problems view).
#[tauri::command]
async fn list_duplicates(state: State<'_, AppState>) -> ApiResult<Vec<DuplicateGroup>> {
    call(&state, |api| api.list_duplicates()).await
}

/// "Keep this one": moves the other copies to the OS trash, or with
/// `dryRun` only says what would move.
#[tauri::command]
async fn keep_copy(state: State<'_, AppState>, req: KeepCopyRequest) -> ApiResult<Resolution> {
    call(&state, move |api| api.keep_copy(req)).await
}

/// The "Keep this one" actions, newest first (Problems history).
#[tauri::command]
async fn list_resolutions(state: State<'_, AppState>) -> ApiResult<Vec<Resolution>> {
    call(&state, |api| api.list_resolutions()).await
}

/// Puts the copies of a "Keep this one" action back from the trash.
#[tauri::command]
async fn undo_resolution(
    state: State<'_, AppState>,
    resolution: ResolutionId,
) -> ApiResult<Resolution> {
    call(&state, move |api| api.undo_resolution(resolution)).await
}

/// Searches the library: free text and filters, grouped by family, with filter counts.
#[tauri::command]
async fn search(state: State<'_, AppState>, req: SearchRequest) -> ApiResult<SearchResponse> {
    call(&state, move |api| api.search(req)).await
}

/// Faces that look like one face, ranked, with reasons ("Find similar", E23.3).
#[tauri::command]
async fn find_similar(
    state: State<'_, AppState>,
    req: SimilarRequest,
) -> ApiResult<Vec<SimilarFace>> {
    call(&state, move |api| api.find_similar(req)).await
}

/// Faces of other families that pair with one face by rule, with reasons
/// (the Compare sheet's Pairings panel, E23.4).
#[tauri::command]
async fn suggest_pairings(
    state: State<'_, AppState>,
    req: PairingRequest,
) -> ApiResult<Vec<PairingSuggestion>> {
    call(&state, move |api| api.suggest_pairings(req)).await
}

/// How well one face covers some languages or text, for the glyph compare overlay.
#[tauri::command]
async fn check_coverage(
    state: State<'_, AppState>,
    req: CoverageRequest,
) -> ApiResult<CoverageReport> {
    call(&state, move |api| api.check_coverage(req)).await
}

/// One face's coverage per Unicode block and language, for the Glyphs tab.
#[tauri::command]
async fn face_coverage(state: State<'_, AppState>, face: FaceId) -> ApiResult<FaceCoverage> {
    call(&state, move |api| api.face_coverage(face)).await
}

/// The language presets a `supports` filter can name.
#[tauri::command]
async fn list_languages(state: State<'_, AppState>) -> ApiResult<Vec<LanguagePreset>> {
    call(&state, |api| api.list_languages()).await
}

/// Creates a collection; with rules, a smart one.
#[tauri::command]
async fn create_collection(
    state: State<'_, AppState>,
    req: CreateCollectionRequest,
) -> ApiResult<Collection> {
    call(&state, move |api| api.create_collection(req)).await
}

/// Every collection, parents before children, siblings in the user's order.
#[tauri::command]
async fn list_collections(state: State<'_, AppState>) -> ApiResult<Vec<Collection>> {
    call(&state, |api| api.list_collections()).await
}

/// Every project set.
#[tauri::command]
async fn list_sets(state: State<'_, AppState>) -> ApiResult<Vec<ProjectSet>> {
    call(&state, |api| api.list_sets()).await
}

/// Pins an instance of a variable face as a virtual style of a manual
/// collection or a project set.
#[tauri::command]
async fn pin_instance(
    state: State<'_, AppState>,
    req: PinInstanceRequest,
) -> ApiResult<VirtualStyle> {
    call(&state, move |api| api.pin_instance(req)).await
}

/// The virtual styles of one manual collection or one project set.
#[tauri::command]
async fn list_virtual_styles(
    state: State<'_, AppState>,
    req: VirtualStylesRequest,
) -> ApiResult<Vec<VirtualStyle>> {
    call(&state, move |api| api.list_virtual_styles(req)).await
}

/// Deletes a virtual style; its face stays.
#[tauri::command]
async fn delete_virtual_style(state: State<'_, AppState>, style: VirtualStyleId) -> ApiResult<()> {
    call(&state, move |api| api.delete_virtual_style(style)).await
}

/// Replaces the rule of a smart collection (E14.1).
#[tauri::command]
async fn update_collection_rules(
    state: State<'_, AppState>,
    req: UpdateCollectionRulesRequest,
) -> ApiResult<Collection> {
    call(&state, move |api| api.update_collection_rules(req)).await
}

/// Renames a collection.
#[tauri::command]
async fn rename_collection(
    state: State<'_, AppState>,
    req: RenameCollectionRequest,
) -> ApiResult<Collection> {
    call(&state, move |api| api.rename_collection(req)).await
}

/// Nests a collection under another one, or reorders it among its siblings.
#[tauri::command]
async fn move_collection(
    state: State<'_, AppState>,
    req: MoveCollectionRequest,
) -> ApiResult<Collection> {
    call(&state, move |api| api.move_collection(req)).await
}

/// Deletes a collection and its children; the faces are not touched.
#[tauri::command]
async fn delete_collection(state: State<'_, AppState>, collection: CollectionId) -> ApiResult<()> {
    call(&state, move |api| api.delete_collection(collection)).await
}

/// Adds faces or whole families to a manual collection.
#[tauri::command]
async fn add_to_collection(
    state: State<'_, AppState>,
    req: CollectionFacesRequest,
) -> ApiResult<Collection> {
    call(&state, move |api| api.add_to_collection(req)).await
}

/// Removes faces or whole families from a manual collection.
#[tauri::command]
async fn remove_from_collection(
    state: State<'_, AppState>,
    req: CollectionFacesRequest,
) -> ApiResult<Collection> {
    call(&state, move |api| api.remove_from_collection(req)).await
}

/// Every tag, A to Z, with its face count (the Info tab completes from it).
#[tauri::command]
async fn list_tags(state: State<'_, AppState>) -> ApiResult<Vec<Tag>> {
    call(&state, |api| api.list_tags()).await
}

/// Puts tags on faces, creating new ones; returns the tags named.
#[tauri::command]
async fn tag_faces(state: State<'_, AppState>, req: TagFacesRequest) -> ApiResult<Vec<Tag>> {
    call(&state, move |api| api.tag_faces(req)).await
}

/// Takes tags off faces.
#[tauri::command]
async fn untag_faces(state: State<'_, AppState>, req: UntagFacesRequest) -> ApiResult<()> {
    call(&state, move |api| api.untag_faces(req)).await
}

/// Sets favorite, rating or notes on faces.
#[tauri::command]
async fn update_user_meta(state: State<'_, AppState>, req: UserMetaUpdate) -> ApiResult<()> {
    call(&state, move |api| api.update_user_meta(req)).await
}

/// Sets the notes of a family; an empty string clears them.
#[tauri::command]
async fn update_family_notes(state: State<'_, AppState>, req: FamilyNotesUpdate) -> ApiResult<()> {
    call(&state, move |api| api.update_family_notes(req)).await
}

/// The Quick Collection (E13.3), first added first.
#[tauri::command]
async fn quick_collection(state: State<'_, AppState>) -> ApiResult<QuickCollection> {
    call(&state, move |api| api.quick_collection()).await
}

/// Adds faces to the end of the Quick Collection.
#[tauri::command]
async fn add_to_quick_collection(
    state: State<'_, AppState>,
    req: QuickCollectionRequest,
) -> ApiResult<QuickCollection> {
    call(&state, move |api| api.add_to_quick_collection(req)).await
}

/// Removes faces from the Quick Collection.
#[tauri::command]
async fn remove_from_quick_collection(
    state: State<'_, AppState>,
    req: QuickCollectionRequest,
) -> ApiResult<QuickCollection> {
    call(&state, move |api| api.remove_from_quick_collection(req)).await
}

/// Empties the Quick Collection; the fonts are not touched.
#[tauri::command]
async fn clear_quick_collection(state: State<'_, AppState>) -> ApiResult<()> {
    call(&state, move |api| api.clear_quick_collection()).await
}

/// The tags the faces of a selection carry, with how many carry each (E13.4).
#[tauri::command]
async fn selection_tags(
    state: State<'_, AppState>,
    req: SelectionTagsRequest,
) -> ApiResult<SelectionTags> {
    call(&state, move |api| api.selection_tags(req)).await
}

/// Declares the license of a family (E15.2).
#[tauri::command]
async fn declare_license(state: State<'_, AppState>, req: LicenseDeclaration) -> ApiResult<()> {
    call(&state, move |api| api.declare_license(req)).await
}

/// Asks where to save the license inventory (E15.4) with the native save
/// dialog, then writes it there. The path comes from the dialog, never from
/// the webview. Resolves to the number of families written, or `None` when
/// the user cancels.
#[tauri::command]
async fn save_license_inventory<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    req: LicenseInventoryRequest,
    file_name: String,
) -> ApiResult<Option<u32>> {
    let dialog = app
        .dialog()
        .file()
        .set_title("Export license inventory")
        .set_file_name(file_name)
        .add_filter("CSV", &["csv"]);
    let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_save_file())
        .await
        .map_err(|e| {
            ApiError::new(ErrorCode::Internal, "The app could not finish the request.")
                .with_detail(e.to_string())
        })?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| {
        ApiError::new(ErrorCode::InvalidRequest, "Choose a file on this computer.")
            .with_detail(e.to_string())
    })?;
    call(&state, move |api| write_license_inventory(api, req, &path))
        .await
        .map(Some)
}

/// Writes the license inventory of `req` to `path`, the file the user
/// picked; resolves to the number of families written.
pub fn write_license_inventory(
    api: &dyn ExportService,
    req: LicenseInventoryRequest,
    path: &Path,
) -> ApiResult<u32> {
    let inventory = api.license_inventory(req)?;
    fm_core::inventory::save(path, &inventory.csv)?;
    Ok(inventory.rows)
}

/// Every source: watched folders, the library folder and the system fonts.
#[tauri::command]
async fn list_sources(state: State<'_, AppState>) -> ApiResult<Vec<Source>> {
    call(&state, |api| api.list_sources()).await
}

/// Adds a folder the user chose as a source; it is indexed in place.
#[tauri::command]
async fn add_source(state: State<'_, AppState>, req: AddSourceRequest) -> ApiResult<Source> {
    call(&state, move |api| api.add_source(req)).await
}

/// Removes a source from the library; its files are not touched.
#[tauri::command]
async fn remove_source(state: State<'_, AppState>, source: SourceId) -> ApiResult<()> {
    call(&state, move |api| api.remove_source(source)).await
}

/// Pauses or resumes a source.
#[tauri::command]
async fn set_source_paused(
    state: State<'_, AppState>,
    req: SetSourcePausedRequest,
) -> ApiResult<Source> {
    call(&state, move |api| api.set_source_paused(req)).await
}

/// Starts a scan; progress arrives on [`EVENT_CHANNEL`].
#[tauri::command]
async fn scan(state: State<'_, AppState>, req: ScanRequest) -> ApiResult<JobId> {
    call(&state, move |api| api.scan(req)).await
}

/// The user's settings.
#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> ApiResult<Settings> {
    call_settings(&state, |s| s.get_settings()).await
}

/// Changes and saves some settings; returns all of them.
#[tauri::command]
async fn update_settings(state: State<'_, AppState>, req: SettingsUpdate) -> ApiResult<Settings> {
    call_settings(&state, move |s| s.update_settings(req)).await
}

/// Where the app keeps its config, data, cache and logs.
#[tauri::command]
async fn get_app_folders(state: State<'_, AppState>) -> ApiResult<AppFolders> {
    call_settings(&state, |s| s.app_folders()).await
}

/// The local API endpoint's address and token (Settings → Local API).
#[tauri::command]
async fn endpoint_info(state: State<'_, AppState>) -> ApiResult<EndpointInfo> {
    call(&state, |api| api.endpoint_info()).await
}

/// Makes a new local API token; the old one fails at once.
#[tauri::command]
async fn regenerate_endpoint_token(state: State<'_, AppState>) -> ApiResult<EndpointInfo> {
    call(&state, |api| api.regenerate_endpoint_token()).await
}

/// Opens a page of the user docs in the help window (the palette's Help actions).
#[tauri::command]
async fn open_help<R: Runtime>(app: AppHandle<R>, page: String) -> ApiResult<()> {
    help::open(&app, &page)
}

/// Opens an `https://` address in the default browser (the "Open Adobe Fonts"
/// link); `fm_platform::open_url` refuses every other scheme.
#[tauri::command]
async fn open_url(url: String) -> ApiResult<()> {
    fm_platform::open_url(&url).map_err(|e| match e {
        fm_platform::OpenUrlError::NotHttps(_) => {
            ApiError::invalid("Only https addresses can be opened.").with_detail(e.to_string())
        }
        fm_platform::OpenUrlError::Launch(_) => {
            ApiError::new(ErrorCode::Io, "The browser could not be opened.")
                .with_detail(e.to_string())
        }
    })
}

/// Shows the native shell's labels (Help menu, tray) in the UI's locale; the
/// UI calls it at start and when the Language setting changes.
#[tauri::command]
async fn set_shell_locale<R: Runtime>(app: AppHandle<R>, locale: String) -> ApiResult<()> {
    shell_text::apply(&app, &locale)
}

/// Opens the OS print dialog for the calling webview's page (Pairing Studio's
/// "Print...", E24.4). The page's print stylesheet decides what prints; the
/// dialog's own "Save as PDF" / "Print to File" makes the PDF on every OS.
#[tauri::command]
async fn print_page<R: Runtime>(webview: Webview<R>) -> ApiResult<()> {
    webview.print().map_err(|e| {
        ApiError::new(ErrorCode::Io, "The print dialog could not be opened.")
            .with_detail(e.to_string())
    })
}

/// Registers the shared state and every command on a builder.
pub fn setup<R: Runtime>(builder: tauri::Builder<R>, api: Arc<dyn Api>) -> tauri::Builder<R> {
    setup_state(builder, AppState::new(api))
}

/// Registers this state and every command on a builder.
pub fn setup_state<R: Runtime>(builder: tauri::Builder<R>, state: AppState) -> tauri::Builder<R> {
    // Subscribe before the first command can run, so no event is missed; a
    // thread forwards the stream to the webview until the API drops it.
    let events = state.api.subscribe();
    let tray_api = Arc::clone(&state.api);
    let tray = state.tray.clone();
    let core = state.core.clone();
    let locale = shell_text::initial_locale(state.current_settings().language);
    builder
        .manage(shell_text::ShellLocale::new(locale))
        .manage(state)
        .manage(updater::PendingUpdate::default())
        .manage(tray::TrayState::new(TraySupport::Unavailable(
            "no tray item yet".into(),
        )))
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                tray::on_close_requested(window, || api.prevent_close());
            }
        })
        .register_asynchronous_uri_scheme_protocol(FONT_SCHEME, |ctx, request, responder| {
            let api = Arc::clone(&ctx.app_handle().state::<AppState>().api);
            let path = request.uri().path().to_owned();
            // Reading a file blocks; keep it off the webview's thread.
            tauri::async_runtime::spawn_blocking(move || {
                let response = catalog_font_response(api.as_ref(), &path)
                    .unwrap_or_else(|| font_response(api.as_ref(), &path));
                responder.respond(response);
            });
        })
        .setup(move |app| {
            // The tray is made once the event loop runs (macOS needs that).
            let support = match tray {
                Some(tray) => {
                    let support = tray::install(app.handle(), tray.as_ref(), tray_api);
                    if let Some(core) = &core {
                        core.set_tray_support(support.clone());
                    }
                    app.state::<tray::TrayState>().set(support.clone());
                    support
                }
                None => TraySupport::Unavailable("no tray item".into()),
            };
            // The window starts hidden (tauri.conf.json); a start at login
            // (ADR 0011) leaves it so, every other launch shows it.
            if lifecycle::launch_window(std::env::args_os().skip(1), &support) == LaunchWindow::Show
            {
                tray::show_main(app.handle());
            }
            // After the window: the local API never delays it (E25.1).
            app.state::<AppState>().start_endpoint();
            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("fm-events".into())
                .spawn(move || {
                    for event in events {
                        if let Err(e) = handle.emit(EVENT_CHANNEL, &event) {
                            tracing::warn!(error = %e, "could not forward an event to the UI");
                        }
                    }
                })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            get_face,
            list_glyphs,
            get_glyph,
            list_families,
            get_family,
            search,
            find_similar,
            suggest_pairings,
            check_coverage,
            face_coverage,
            list_languages,
            list_collections,
            create_collection,
            rename_collection,
            update_collection_rules,
            move_collection,
            delete_collection,
            add_to_collection,
            remove_from_collection,
            list_tags,
            tag_faces,
            untag_faces,
            update_user_meta,
            selection_tags,
            update_family_notes,
            declare_license,
            quick_collection,
            add_to_quick_collection,
            remove_from_quick_collection,
            clear_quick_collection,
            save_license_inventory,
            list_sets,
            pin_instance,
            list_virtual_styles,
            delete_virtual_style,
            sample_texts,
            size_factors,
            css_values,
            layout_presets,
            clamp_layout,
            layout_bounds,
            list_layouts,
            get_layout,
            save_layout,
            duplicate_layout,
            revert_layout,
            layout_history,
            restore_layout,
            layout_width,
            list_comparisons,
            get_comparison,
            save_comparison,
            rename_comparison,
            delete_comparison,
            list_catalogs,
            set_catalog_enabled,
            refresh_catalog,
            list_catalog_families,
            download_family,
            list_catalog_downloads,
            check_updates,
            render_preview,
            activate,
            deactivate,
            list_active,
            conflicts,
            list_conflicts,
            list_problems,
            list_duplicates,
            keep_copy,
            list_resolutions,
            undo_resolution,
            list_sources,
            add_source,
            remove_source,
            set_source_paused,
            scan,
            get_settings,
            update_settings,
            get_app_folders,
            endpoint_info,
            regenerate_endpoint_token,
            open_help,
            open_url,
            set_shell_locale,
            print_page,
            updater::update_support,
            updater::check_for_update,
            updater::install_update
        ])
}

/// The state the app serves from the app folders `paths` and the index
/// startup opened: [`CoreApi`] over the index, activating through
/// `activator` and printing PDF specimens through the OS webview, when the
/// index opened; it has cleaned up the last session
/// (ADR 0008) and, when the index was rebuilt, started a scan of every source.
/// When the index could not be opened, [`MockApi`] with the real settings.
/// The [`CoreApi`] is returned too, for [`CoreApi::shutdown`] at exit.
pub fn serve(
    paths: &AppPaths,
    index: Result<Opened, ApiError>,
    activator: Arc<dyn FontActivator>,
) -> (AppState, Option<Arc<CoreApi>>) {
    match index {
        Ok(opened) => {
            let core = CoreApi::open(paths, opened.index, activator);
            // PDF specimens print through an offscreen webview on this
            // app's UI thread (ADR 0013); the CLI's direct link has none.
            let core = match fm_platform::pdf::system(paths) {
                Ok(printer) => core.with_pdf_printer(printer),
                Err(e) => {
                    tracing::warn!(error = %e, "no PDF printer: PDF specimens are unavailable");
                    core
                }
            };
            let core = Arc::new(core);
            if let Err(e) = core.start() {
                tracing::warn!(error = %e, "activation start-up failed");
            }
            if opened.rebuilt.is_some()
                && let Err(e) = core.scan(ScanRequest::default())
            {
                tracing::warn!(error = %e, "the rebuilt index could not be scanned");
            }
            (AppState::core(Arc::clone(&core)), Some(core))
        }
        Err(e) => {
            tracing::error!(error = %e, "serving sample data: the library index is not available");
            let state = AppState::new(Arc::new(MockApi::new()))
                .with_settings(Arc::new(SettingsStore::new(paths).with_system_autostart()));
            (state, None)
        }
    }
}

/// Starts the desktop app.
pub fn run() {
    // App folders, settings and the rotating log file. The guard lives until
    // `run` returns so buffered log lines are flushed on exit. A failure here
    // is reported but does not stop the app: it runs on default settings.
    let mut startup = match fm_core::startup::init() {
        Ok(startup) => Some(startup),
        Err(e) => {
            eprintln!("font manager: {e}");
            None
        }
    };
    let state = match startup.as_mut() {
        Some(startup) => {
            let index = std::mem::replace(
                &mut startup.index,
                Err(ApiError::new(ErrorCode::Internal, "The index is served.")),
            );
            match fm_platform::activator::system(&startup.paths) {
                Ok(activator) => serve(&startup.paths, index, activator).0,
                Err(e) => {
                    let e = ApiError::new(
                        ErrorCode::Unsupported,
                        "Font activation is not available on this system.",
                    )
                    .with_detail(e.to_string());
                    serve(&startup.paths, Err(e), Arc::new(MockActivator::new())).0
                }
            }
        }
        None => AppState::new(Arc::new(MockApi::new())),
    };
    let builder = help::with_help_menu(
        tauri::Builder::default()
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_updater::Builder::new().build()),
    );
    let state = state.with_tray(fm_platform::tray::system());
    let app = match setup_state(builder, state).build(tauri::generate_context!()) {
        Ok(app) => app,
        Err(e) => {
            tracing::error!(error = %e, "font manager failed to start");
            eprintln!("font manager failed to start: {e}");
            drop(startup); // flush the log before exit skips destructors
            std::process::exit(1);
        }
    };
    // Every quit ends here: the tray's Quit, the app menu's Quit, closing the
    // window without a tray item, the restart after an update.
    app.run(move |handle, event| {
        if let RunEvent::Exit = event {
            handle.state::<AppState>().run_quit_path();
            // `run` may exit the process without returning: flush the log now.
            drop(startup.take());
        }
    });
}

/// What the app does as it quits: session fonts end with the app (ADR 0008);
/// permanent ones stay.
pub fn on_exit(core: Option<&CoreApi>) {
    if let Some(core) = core
        && let Err(e) = core.shutdown()
    {
        tracing::warn!(error = %e, "session fonts not deactivated at exit");
    }
}
