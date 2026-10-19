//! The Font Manager desktop shell: Tauri commands are thin adapters over
//! `fm-api`. Each command takes the shared [`Api`] from Tauri state, calls one
//! service method off the async runtime (ADR 0006) and returns its result as is.

use std::path::Path;
use std::sync::Arc;

use fm_api::activation::{ActivateRequest, ActivationOutcome, DeactivateRequest};
use fm_api::common::Paged;
use fm_api::library::{
    AddSourceRequest, AppInfo, FaceDetail, Family, FamilySummary, ListFamiliesRequest, ScanRequest,
    SetSourcePausedRequest, Source,
};
use fm_api::mock::MockApi;
use fm_api::organization::{
    Collection, CollectionFacesRequest, CreateCollectionRequest, MoveCollectionRequest,
    RenameCollectionRequest, Tag, TagFacesRequest, UntagFacesRequest, UserMetaUpdate,
};
use fm_api::preview::{
    PreviewImage, PreviewRequest, PreviewService, SampleText, SampleTextRequest,
};
use fm_api::search::{LanguagePreset, SearchRequest, SearchResponse};
use fm_api::settings::{AppFolders, Settings, SettingsUpdate};
use fm_api::{
    Api, ApiError, ApiResult, CollectionId, ErrorCode, FaceId, FamilyId, JobId, SettingsService,
    SourceId,
};
use fm_core::SettingsStore;
use tauri::http::{Response, StatusCode, header};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

pub mod help;

/// The Tauri event every `fm_api::Event` is forwarded on.
pub const EVENT_CHANNEL: &str = "fm://event";

/// The URI scheme the webview loads font files from, by face id:
/// `fmfont://localhost/<face>` (`http://fmfont.localhost/<face>` on Windows).
/// Only files of faces the API knows are served, so the webview needs no file
/// system scope over the user's folders.
pub const FONT_SCHEME: &str = "fmfont";

/// The API every command talks to. `fm-core` does not implement [`Api`] yet,
/// so the shell serves [`MockApi`] until it does; the settings already come
/// from `fm-core`'s [`SettingsStore`] when the app folders were found.
pub struct AppState {
    api: Arc<dyn Api>,
    settings: Option<Arc<dyn SettingsService + Send + Sync>>,
}

impl AppState {
    /// Wraps an API implementation for Tauri's managed state.
    pub fn new(api: Arc<dyn Api>) -> Self {
        Self {
            api,
            settings: None,
        }
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

/// Searches the library: free text and filters, grouped by family, with filter counts.
#[tauri::command]
async fn search(state: State<'_, AppState>, req: SearchRequest) -> ApiResult<SearchResponse> {
    call(&state, move |api| api.search(req)).await
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

/// Opens a page of the user docs in the help window (the palette's Help actions).
#[tauri::command]
async fn open_help<R: Runtime>(app: AppHandle<R>, page: String) -> ApiResult<()> {
    help::open(&app, &page)
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
    builder
        .manage(state)
        .register_asynchronous_uri_scheme_protocol(FONT_SCHEME, |ctx, request, responder| {
            let api = Arc::clone(&ctx.app_handle().state::<AppState>().api);
            let path = request.uri().path().to_owned();
            // Reading a file blocks; keep it off the webview's thread.
            tauri::async_runtime::spawn_blocking(move || {
                responder.respond(font_response(api.as_ref(), &path));
            });
        })
        .setup(move |app| {
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
            list_families,
            get_family,
            search,
            list_languages,
            list_collections,
            create_collection,
            rename_collection,
            move_collection,
            delete_collection,
            add_to_collection,
            remove_from_collection,
            list_tags,
            tag_faces,
            untag_faces,
            update_user_meta,
            sample_texts,
            render_preview,
            activate,
            deactivate,
            list_sources,
            add_source,
            remove_source,
            set_source_paused,
            scan,
            get_settings,
            update_settings,
            get_app_folders,
            open_help
        ])
}

/// Starts the desktop app.
pub fn run() {
    // App folders, settings and the rotating log file. The guard lives until
    // `run` returns so buffered log lines are flushed on exit. A failure here
    // is reported but does not stop the app: it runs on default settings.
    let startup = match fm_core::startup::init() {
        Ok(startup) => Some(startup),
        Err(e) => {
            eprintln!("font manager: {e}");
            None
        }
    };
    let api: Arc<dyn Api> = Arc::new(MockApi::new());
    let mut state = AppState::new(api);
    if let Some(startup) = &startup {
        state = state.with_settings(Arc::new(SettingsStore::new(&startup.paths)));
    }
    let builder =
        help::with_help_menu(tauri::Builder::default().plugin(tauri_plugin_dialog::init()));
    if let Err(e) = setup_state(builder, state).run(tauri::generate_context!()) {
        tracing::error!(error = %e, "font manager failed to start");
        eprintln!("font manager failed to start: {e}");
        drop(startup); // flush the log before exit skips destructors
        std::process::exit(1);
    }
}
