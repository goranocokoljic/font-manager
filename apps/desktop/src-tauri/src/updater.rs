//! In-app updates (E18.2, ADR 0012) through Tauri's updater. The decisions
//! are made elsewhere: whether this copy updates itself in `fm-platform` and
//! `fm-core` ([`support`]), how big a step an update is and whether an
//! install is confirmed in `fm-api` (`fm_api::update`). This file only asks
//! the release server, keeps the update the user was shown, and installs it
//! after the normal quit path.
//!
//! The app asks the server only when the user clicks "Check for updates" or
//! has turned on "Check automatically" (rule 7). A package installs only when
//! its minisign signature matches the public key in `tauri.conf.json` and was
//! made for the announced version (`requireSignedVersion`).

use std::sync::{Mutex, PoisonError};

use fm_api::update::{AvailableUpdate, InstallUpdateRequest, UpdateCheck, UpdateSupport};
use fm_api::{ApiError, ApiResult, ErrorCode};
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_updater::{Error as UpdaterError, Update, UpdaterExt};

use crate::AppState;

/// Whether this copy of the app updates itself.
pub fn support() -> UpdateSupport {
    fm_core::update::support(&fm_platform::updater::distribution())
}

/// The update the last check found, with what the user was shown of it.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<(AvailableUpdate, Update)>>);

impl std::fmt::Debug for PendingUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingUpdate").finish_non_exhaustive()
    }
}

impl PendingUpdate {
    fn set(&self, update: Option<(AvailableUpdate, Update)>) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = update;
    }

    fn get(&self) -> Option<(AvailableUpdate, Update)> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// What the user is shown of `update`; `None` when it is not newer.
pub fn available(update: &Update) -> ApiResult<Option<AvailableUpdate>> {
    let date = update
        .raw_json
        .get("pub_date")
        .and_then(|d| d.as_str())
        .map(str::to_owned);
    AvailableUpdate::new(
        &update.current_version,
        &update.version,
        update.body.clone(),
        date,
    )
}

/// Whether `e` says the package is not signed by the release key: a bad or
/// missing signature, another key, or a signature made for another version.
pub fn is_signature_error(e: &UpdaterError) -> bool {
    matches!(
        e,
        UpdaterError::Minisign(_)
            | UpdaterError::Base64(_)
            | UpdaterError::SignatureUtf8(_)
            | UpdaterError::MissingSignedVersion
            | UpdaterError::SignedVersionMismatch { .. }
    )
}

/// The error the user sees when a download or install fails. A package
/// whose signature does not match is refused as not coming from Font
/// Manager; nothing of it is installed.
pub fn install_error(e: UpdaterError) -> ApiError {
    if is_signature_error(&e) {
        ApiError::new(
            ErrorCode::PermissionDenied,
            "The update was not installed: it is not signed with Font Manager's release key. \
             Download Font Manager from its releases page instead.",
        )
        .with_detail(e.to_string())
    } else {
        ApiError::new(
            ErrorCode::Io,
            "The update could not be downloaded or installed. Try again later.",
        )
        .with_detail(e.to_string())
    }
}

/// Checks that `bytes` are the package `update` announced, signed with the
/// release key. The same check runs inside every download and install.
pub fn verify(update: &Update, bytes: &[u8]) -> ApiResult<()> {
    update.verify(bytes).map_err(install_error)
}

fn check_error(e: UpdaterError) -> ApiError {
    ApiError::new(
        ErrorCode::Io,
        "Font Manager could not reach its release server. Check your connection and try again.",
    )
    .with_detail(e.to_string())
}

fn not_offered(support: &UpdateSupport) -> ApiError {
    ApiError::new(
        ErrorCode::Unsupported,
        support
            .reason
            .clone()
            .unwrap_or_else(|| "This copy of Font Manager does not update itself.".to_owned()),
    )
}

/// Runs the normal quit path (session fonts are deregistered, ADR 0008),
/// then `install`. When the install fails the app keeps running as it was:
/// the session fonts are activated again, and the quit path runs again when
/// the app quits.
pub fn install_after_quit(
    state: &AppState,
    install: impl FnOnce() -> ApiResult<()>,
) -> ApiResult<()> {
    let session = state.session_faces();
    state.run_quit_path();
    let installed = install();
    if installed.is_err() {
        state.resume_after_quit(session);
    }
    installed
}

/// "Check for updates": asks the release server once and keeps what it
/// offers for [`install_update`].
#[tauri::command]
pub async fn check_for_update<R: Runtime>(
    app: AppHandle<R>,
    pending: State<'_, PendingUpdate>,
) -> ApiResult<UpdateCheck> {
    let support = support();
    if !support.offered {
        return Err(not_offered(&support));
    }
    let found = app
        .updater()
        .map_err(check_error)?
        .check()
        .await
        .map_err(check_error)?;
    let offer = match found {
        Some(update) => available(&update)?.map(|shown| (shown, update)),
        None => None,
    };
    pending.set(offer.clone());
    Ok(UpdateCheck {
        current_version: app.package_info().version.to_string(),
        update: offer.map(|(shown, _)| shown),
    })
}

/// Installs the update the last check found, after the user confirmed it
/// (twice for a major one): downloads and verifies it, quits as Quit does,
/// installs and starts the new version.
#[tauri::command]
pub async fn install_update<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    pending: State<'_, PendingUpdate>,
    req: InstallUpdateRequest,
) -> ApiResult<()> {
    let Some((shown, update)) = pending.get() else {
        return Err(ApiError::new(
            ErrorCode::NotFound,
            "No update is on offer. Check for updates first.",
        ));
    };
    req.check(&shown)?;
    // Asked again now: a copy moved since the check to a folder this user
    // cannot write would make the installer ask for admin rights (rule 4).
    let support = support();
    if !support.offered {
        return Err(not_offered(&support));
    }
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(install_error)?;
    install_after_quit(&state, || update.install(&bytes).map_err(install_error))?;
    // Windows: the installer has taken over and the app has exited.
    app.restart()
}

/// Whether this copy offers "Check for updates", and why not.
#[tauri::command]
pub fn update_support() -> UpdateSupport {
    support()
}
