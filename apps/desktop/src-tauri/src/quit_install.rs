//! "Install updates when I quit" (E55.9, ADR 0012) in the shell: the
//! background download after an automatic check, and the install after the
//! normal quit path.
//!
//! The decisions are made in `fm-core` (`fm_core::update::on_quit`): whether
//! a found update is downloaded ([`found_step`]) and what the quit does with
//! the kept package ([`exit_step`]); `fm-api` says which updates qualify and
//! what the next start tells the user. This file runs the steps.
//!
//! - **Only the user's quit installs.** Quit in the app menu or the tray, or
//!   the last window closed (with "keep in tray" off), marks the quit as the
//!   user's ([`OnQuit::mark_user_quit`]). A logout or a shutdown never does:
//!   on Windows the session end ends the process, on Linux the session sends
//!   `SIGTERM`, on macOS it asks the app to terminate without the menu item.
//!   A kept package then waits for the next quit, since an installer cut off
//!   by the session end could leave a half-replaced app.
//! - **Once, after the quit path.** `RunEvent::Exit` runs
//!   `AppState::run_quit_path` first, then [`at_exit`]; the app does not start
//!   again after this install (the Windows installer is told not to restart
//!   it, and macOS and Linux never restart by themselves).
//! - **The same checks as a confirmed install.** The folder probe
//!   (`updater::support`) runs before the download and before the install;
//!   the package is verified by the download and again before the install
//!   (signature, made for the announced version). A refused package is
//!   deleted, reported once, and nothing is installed.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use fm_api::update::{AvailableUpdate, UpdateNotice};
use fm_api::{ApiError, ApiResult, Event};
use fm_core::update::on_quit::{ExitStep, FoundStep, QuitInstall, exit_step, found_step};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::updater::{self, PendingUpdate};
use crate::{AppState, EVENT_CHANNEL};

/// "Install updates when I quit": managed state of the app.
#[derive(Debug, Default)]
pub struct OnQuit {
    /// The kept package and the records; `None` without app folders.
    store: Option<QuitInstall>,
    /// The quit that runs is the user's.
    user_quit: AtomicBool,
    /// A background download runs.
    downloading: AtomicBool,
    /// The version whose package was refused in this run: not downloaded again.
    refused: Mutex<Option<String>>,
}

impl OnQuit {
    /// Over the files of `store`; `None` without app folders (then nothing
    /// is downloaded or installed at quit).
    pub fn new(store: Option<QuitInstall>) -> Self {
        Self {
            store,
            ..Self::default()
        }
    }

    /// The files it keeps; `None` without app folders.
    pub fn store(&self) -> Option<&QuitInstall> {
        self.store.as_ref()
    }

    /// Marks the quit that starts as the user's: Quit in a menu or the last
    /// window closed. Only such a quit installs a kept package.
    pub fn mark_user_quit(&self) {
        self.user_quit.store(true, Ordering::SeqCst);
    }

    /// Whether the quit is the user's.
    pub fn is_user_quit(&self) -> bool {
        self.user_quit.load(Ordering::SeqCst)
    }

    fn refused(&self) -> std::sync::MutexGuard<'_, Option<String>> {
        self.refused.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Quit in the tray or the app menu: the user's quit, then the normal exit
/// (`RunEvent::Exit` runs [`on_exit`]).
pub fn quit<R: Runtime>(app: &AppHandle<R>) {
    if let Some(on_quit) = app.try_state::<OnQuit>() {
        on_quit.mark_user_quit();
    }
    app.exit(0);
}

/// `RunEvent::ExitRequested` with `code`: `None` means the last window
/// closed ("keep in tray" off, or no tray), which is the user's quit. An exit
/// the app asked for itself (`Some`: Quit, which marked itself, or the
/// restart after a confirmed install) marks nothing; a logout or a shutdown
/// sends no such event.
pub fn on_exit_requested<R: Runtime>(app: &AppHandle<R>, code: Option<i32>) {
    if code.is_none()
        && let Some(on_quit) = app.try_state::<OnQuit>()
    {
        on_quit.mark_user_quit();
    }
}

/// `RunEvent::Exit`, every quit: the normal quit path (session fonts are
/// deregistered and remembered for "reactivate on launch"), then `install`
/// ([`at_exit`] in the app), in that order and once.
pub fn on_exit(state: &AppState, install: impl FnOnce()) {
    state.run_quit_path();
    install();
}

/// Deletes the kept package of `version`, keeps the notice that it was not
/// installed for the UI to show once, and returns it.
fn refuse(store: &QuitInstall, version: &str, error: &ApiError) -> UpdateNotice {
    tracing::warn!(%version, error = %error, "the update package was refused");
    store.discard();
    let notice = UpdateNotice::NotInstalled {
        version: version.to_owned(),
        message: error.message.clone(),
    };
    store.record_notice(&notice);
    notice
}

/// After an automatic check found `found`: downloads its package in the
/// background and keeps it for the quit, when "Install updates when I quit"
/// applies ([`found_step`], with the folder probe made now). A major update
/// is only announced. A package the download refuses is deleted and
/// reported once; a download that fails otherwise (offline) is silent and
/// tried again after the next check.
pub fn on_found<R: Runtime>(app: &AppHandle<R>, found: &AvailableUpdate) {
    let (Some(on_quit), Some(state)) = (app.try_state::<OnQuit>(), app.try_state::<AppState>())
    else {
        return;
    };
    let Some(store) = on_quit.store().cloned() else {
        return;
    };
    let staged = store.staged_version();
    let step = found_step(
        &state.current_settings(),
        &updater::support(),
        found,
        staged.as_deref(),
    );
    if step != FoundStep::Download || on_quit.refused().as_deref() == Some(found.version.as_str()) {
        return;
    }
    let Some((shown, update)) = app.state::<PendingUpdate>().get() else {
        return;
    };
    if shown.version != found.version || on_quit.downloading.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let version = shown.version.clone();
        let downloaded = update.download(|_, _| {}, || {}).await;
        let Some(on_quit) = app.try_state::<OnQuit>() else {
            return;
        };
        match downloaded {
            Ok(bytes) => match store.stage(&version, &bytes) {
                Ok(()) => tracing::info!(%version, "update downloaded; it installs when you quit"),
                Err(e) => tracing::warn!(error = %e, "the downloaded update was not kept"),
            },
            Err(e) if updater::is_signature_error(&e) => {
                let notice = refuse(&store, &version, &updater::install_error(e));
                *on_quit.refused() = Some(version);
                if let Err(e) = app.emit(EVENT_CHANNEL, Event::UpdateNotice { notice }) {
                    tracing::warn!(error = %e, "could not tell the UI the update was refused");
                }
            }
            Err(e) => {
                tracing::info!(error = %e, "the update download failed; tried after the next check");
            }
        }
        on_quit.downloading.store(false, Ordering::SeqCst);
    });
}

/// Installs the kept package of `version` at quit, after the quit path ran:
/// reads it, `verify`s it (signature, made for the announced version), records
/// the install for the next start (`current` → `version`, with the
/// `notes_url` "Updated to" links to), `flush`es the log and runs `install`.
/// A refused package is deleted, its notice kept, and nothing is installed.
/// When `install` fails the record stays: the next start says the update
/// could not be installed.
pub fn install_kept(
    store: &QuitInstall,
    current: &str,
    version: &str,
    notes_url: Option<String>,
    verify: impl FnOnce(&[u8]) -> ApiResult<()>,
    flush: impl FnOnce(),
    install: impl FnOnce(&[u8]) -> ApiResult<()>,
) -> ApiResult<()> {
    let Some(bytes) = store.staged(version) else {
        return Err(ApiError::new(
            fm_api::ErrorCode::NotFound,
            "The downloaded update is gone.",
        ));
    };
    if let Err(e) = verify(&bytes) {
        refuse(store, version, &e);
        return Err(e);
    }
    store
        .begin_install(current, version, notes_url)
        .map_err(ApiError::from)?;
    tracing::info!(from = %current, %version, "installing the update as the app quits");
    flush();
    install(&bytes)
}

/// At `RunEvent::Exit`, after the quit path: installs the kept package when
/// [`exit_step`] says so (the user's quit, the setting still on, the folder
/// probe made now passes, the package is the update on offer), deletes it
/// when it no longer applies, keeps it for the next quit otherwise. `flush`
/// writes the log out before the installer may end the process (Windows).
pub fn at_exit<R: Runtime>(app: &AppHandle<R>, flush: impl FnOnce()) {
    let (Some(on_quit), Some(state)) = (app.try_state::<OnQuit>(), app.try_state::<AppState>())
    else {
        return;
    };
    let Some(store) = on_quit.store() else {
        return;
    };
    let Some(staged) = store.staged_version() else {
        return;
    };
    let pending = app.try_state::<PendingUpdate>().and_then(|p| p.get());
    let step = exit_step(
        on_quit.is_user_quit(),
        &state.current_settings(),
        &updater::support(),
        pending.as_ref().map(|(shown, _)| shown),
        Some(&staged),
    );
    let version = match step {
        ExitStep::Keep => return,
        ExitStep::Discard => return store.discard(),
        ExitStep::Install(version) => version,
    };
    let Some((_, update)) = pending else {
        return;
    };
    let notes_url = fm_core::update::release_notes_url(
        updater::endpoints().iter().map(String::as_str),
        &version,
    );
    let current = app.package_info().version.to_string();
    // The app does not start again after this install.
    let update = update.restart_after_install(false);
    let installed = install_kept(
        store,
        &current,
        &version,
        notes_url,
        |bytes| updater::verify(&update, bytes),
        flush,
        |bytes| update.install(bytes).map_err(updater::install_error),
    );
    if let Err(e) = installed {
        tracing::warn!(error = %e, "the update did not install as the app quit");
    }
}

/// At start, once: what the last quit's install did, kept for the UI
/// ([`take_update_notice`]). Deletes a package that is no newer than this
/// version.
pub fn resolve_at_start<R: Runtime>(app: &AppHandle<R>) {
    if let Some(store) = app.try_state::<OnQuit>().and_then(|o| o.store().cloned()) {
        store.resolve_at_start(&app.package_info().version.to_string());
    }
}

/// The notice about an update installed at quit, once: "Updated to
/// <version>", or that it could not be installed. `None` when there is none.
#[tauri::command]
pub fn take_update_notice(on_quit: State<'_, OnQuit>) -> Option<UpdateNotice> {
    on_quit.store().and_then(QuitInstall::take_notice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quit_is_the_users_only_once_marked() {
        let on_quit = OnQuit::default();
        assert!(!on_quit.is_user_quit());
        on_quit.mark_user_quit();
        assert!(on_quit.is_user_quit());
        assert!(on_quit.store().is_none());
    }
}
