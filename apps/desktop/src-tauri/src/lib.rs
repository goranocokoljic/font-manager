//! The Font Manager desktop shell: Tauri commands are thin adapters over
//! `fm-api`. Each command takes the shared [`Api`] from Tauri state, calls one
//! service method off the async runtime (ADR 0006) and returns its result as is.

use std::sync::Arc;

use fm_api::activation::{ActivateRequest, ActivationOutcome, DeactivateRequest};
use fm_api::library::{
    AddSourceRequest, AppInfo, FaceDetail, ScanRequest, SetSourcePausedRequest, Source,
};
use fm_api::mock::MockApi;
use fm_api::{Api, ApiError, ApiResult, ErrorCode, FaceId, JobId, SourceId};
use tauri::{Emitter, Runtime, State};

/// The Tauri event every `fm_api::Event` is forwarded on.
pub const EVENT_CHANNEL: &str = "fm://event";

/// The API every command talks to. `fm-core` does not implement [`Api`] yet,
/// so the shell serves [`MockApi`] until it does.
pub struct AppState {
    api: Arc<dyn Api>,
}

impl AppState {
    /// Wraps an API implementation for Tauri's managed state.
    pub fn new(api: Arc<dyn Api>) -> Self {
        Self { api }
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

/// Registers the shared state and every command on a builder.
pub fn setup<R: Runtime>(builder: tauri::Builder<R>, api: Arc<dyn Api>) -> tauri::Builder<R> {
    // Subscribe before the first command can run, so no event is missed; a
    // thread forwards the stream to the webview until the API drops it.
    let events = api.subscribe();
    builder
        .manage(AppState::new(api))
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
            activate,
            deactivate,
            list_sources,
            add_source,
            remove_source,
            set_source_paused,
            scan
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
    let builder = tauri::Builder::default().plugin(tauri_plugin_dialog::init());
    if let Err(e) = setup(builder, api).run(tauri::generate_context!()) {
        tracing::error!(error = %e, "font manager failed to start");
        eprintln!("font manager failed to start: {e}");
        drop(startup); // flush the log before exit skips destructors
        std::process::exit(1);
    }
}
