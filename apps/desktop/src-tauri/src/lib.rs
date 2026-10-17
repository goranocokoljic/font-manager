//! The Font Manager desktop shell: Tauri commands are thin adapters over
//! `fm-api`. Each command takes the shared [`Api`] from Tauri state, calls one
//! service method off the async runtime (ADR 0006) and returns its result as is.

use std::sync::Arc;

use fm_api::activation::{ActivateRequest, ActivationOutcome, DeactivateRequest};
use fm_api::library::{AddSourceRequest, AppInfo, FaceDetail, SetSourcePausedRequest, Source};
use fm_api::mock::MockApi;
use fm_api::settings::{AppFolders, Settings, SettingsUpdate};
use fm_api::{Api, ApiError, ApiResult, ErrorCode, FaceId, SettingsService, SourceId};
use fm_core::SettingsStore;
use tauri::{Runtime, State};

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

/// Every source, for the Settings window.
#[tauri::command]
async fn list_sources(state: State<'_, AppState>) -> ApiResult<Vec<Source>> {
    call(&state, |api| api.list_sources()).await
}

/// Adds a folder as a source; does not scan it.
#[tauri::command]
async fn add_source(state: State<'_, AppState>, req: AddSourceRequest) -> ApiResult<Source> {
    call(&state, move |api| api.add_source(req)).await
}

/// Forgets a source and its faces; never touches the files.
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

/// Registers the shared state and every command on a builder.
pub fn setup<R: Runtime>(builder: tauri::Builder<R>, api: Arc<dyn Api>) -> tauri::Builder<R> {
    setup_state(builder, AppState::new(api))
}

/// Registers this state and every command on a builder.
pub fn setup_state<R: Runtime>(builder: tauri::Builder<R>, state: AppState) -> tauri::Builder<R> {
    builder
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            get_face,
            activate,
            deactivate,
            list_sources,
            add_source,
            remove_source,
            set_source_paused,
            get_settings,
            update_settings,
            get_app_folders
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
    if let Err(e) = setup_state(tauri::Builder::default(), state).run(tauri::generate_context!()) {
        tracing::error!(error = %e, "font manager failed to start");
        eprintln!("font manager failed to start: {e}");
        drop(startup); // flush the log before exit skips destructors
        std::process::exit(1);
    }
}
