//! The Font Manager desktop shell: Tauri commands are thin adapters over
//! `fm-api`. Each command takes the shared [`Api`] from Tauri state, calls one
//! service method off the async runtime (ADR 0006) and returns its result as is.

use std::sync::Arc;

use fm_api::library::AppInfo;
use fm_api::mock::MockApi;
use fm_api::{Api, ApiError, ApiResult, ErrorCode};
use tauri::{Runtime, State};

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

/// Registers the shared state and every command on a builder.
pub fn setup<R: Runtime>(builder: tauri::Builder<R>, api: Arc<dyn Api>) -> tauri::Builder<R> {
    builder
        .manage(AppState::new(api))
        .invoke_handler(tauri::generate_handler![get_app_info])
}

/// Starts the desktop app.
pub fn run() {
    let api: Arc<dyn Api> = Arc::new(MockApi::new());
    if let Err(e) = setup(tauri::Builder::default(), api).run(tauri::generate_context!()) {
        eprintln!("font manager failed to start: {e}");
        std::process::exit(1);
    }
}
