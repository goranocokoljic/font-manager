//! The tray item and the close button (E17.1, ADR 0010). Wiring only: the
//! per-OS answers come from `fm_platform::Tray`, what closing does and what
//! the menu says and does from `fm_core::lifecycle`.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, PoisonError};

use fm_api::{Api, Event};
use fm_core::lifecycle::{self, CloseAction};
use fm_platform::{Tray, TraySupport};
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime, Window};

use crate::AppState;

/// The id of the tray item.
pub const TRAY_ID: &str = "fm-tray";
/// The label of the main window.
pub const MAIN_WINDOW: &str = "main";

const ID_OPEN: &str = "tray:open";
const ID_COUNT: &str = "tray:count";
const ID_DEACTIVATE: &str = "tray:deactivate-session";
const ID_QUIT: &str = "tray:quit";

/// Whether the app has a tray item; managed state, read by the close handler.
#[derive(Debug)]
pub struct TrayState(Mutex<TraySupport>);

impl TrayState {
    /// The state for `support`.
    pub fn new(support: TraySupport) -> Self {
        Self(Mutex::new(support))
    }

    /// What the app found when it made the tray item.
    pub fn support(&self) -> TraySupport {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Records what the app found when it made the tray item.
    pub fn set(&self, support: TraySupport) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = support;
    }
}

/// Makes the tray item when `tray` says the desktop can show one, and
/// returns what was found. A tray that cannot be made (a Linux desktop
/// without the AppIndicator library) is unavailable too; closing the window
/// then quits.
pub fn install<R: Runtime>(app: &AppHandle<R>, tray: &dyn Tray, api: Arc<dyn Api>) -> TraySupport {
    let support = match tray.support() {
        TraySupport::Available => {
            // The Linux toolkit panics when the AppIndicator library is missing.
            match catch_unwind(AssertUnwindSafe(|| build(app, tray, Arc::clone(&api)))) {
                Ok(Ok(())) => TraySupport::Available,
                Ok(Err(e)) => TraySupport::Unavailable(format!("the tray item failed: {e}")),
                Err(_) => TraySupport::Unavailable("the tray toolkit panicked".into()),
            }
        }
        unavailable => unavailable,
    };
    if let TraySupport::Unavailable(reason) = &support {
        tracing::info!(reason, "no tray item: closing the window quits");
    }
    support
}

fn build<R: Runtime>(app: &AppHandle<R>, tray: &dyn Tray, api: Arc<dyn Api>) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, ID_OPEN, lifecycle::MENU_OPEN, true, None::<&str>)?;
    let count = MenuItem::with_id(
        app,
        ID_COUNT,
        lifecycle::active_label(active_count(api.as_ref())),
        false,
        None::<&str>,
    )?;
    let deactivate = MenuItem::with_id(
        app,
        ID_DEACTIVATE,
        lifecycle::MENU_DEACTIVATE_SESSION,
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, ID_QUIT, lifecycle::MENU_QUIT, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &count, &separator, &deactivate, &quit])?;

    let menu_api = Arc::clone(&api);
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Font Manager")
        .menu(&menu)
        .icon_as_template(tray.icon_as_template())
        .show_menu_on_left_click(tray.menu_on_left_click())
        .on_menu_event(move |app, event| on_menu_event(app, &event, &menu_api));
    if !tray.menu_on_left_click() {
        builder = builder.on_tray_icon_event(on_icon_event);
    }
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;

    // The count follows activation events; nothing polls.
    let events = api.subscribe();
    std::thread::Builder::new()
        .name("fm-tray".into())
        .spawn(move || {
            for event in events {
                if let Event::ActivationChanged { .. } = event
                    && let Err(e) =
                        count.set_text(lifecycle::active_label(active_count(api.as_ref())))
                {
                    tracing::warn!(error = %e, "tray count not updated");
                }
            }
        })?;
    Ok(())
}

fn active_count(api: &dyn Api) -> usize {
    api.list_active().map(|a| a.len()).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "active fonts not counted");
        0
    })
}

fn on_menu_event<R: Runtime>(app: &AppHandle<R>, event: &MenuEvent, api: &Arc<dyn Api>) {
    match event.id().as_ref() {
        ID_OPEN => show_main(app),
        ID_DEACTIVATE => {
            let api = Arc::clone(api);
            // An OS call per face: keep it off the event loop.
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(e) = lifecycle::deactivate_session(api.as_ref()) {
                    tracing::warn!(error = %e, "session fonts not deactivated from the tray");
                }
            });
        }
        // The normal quit: `RunEvent::Exit` deregisters the session fonts.
        ID_QUIT => app.exit(0),
        _ => {}
    }
}

/// Windows: a left click on the icon opens the window.
fn on_icon_event<R: Runtime>(tray: &TrayIcon<R>, event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        show_main(tray.app_handle());
    }
}

/// Shows, restores and focuses the main window.
pub fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// What closing `window` does: the main window hides to the tray when
/// "Keep running in the tray" is on and the app has a tray item; every other
/// case closes as usual. Returns the action; the caller prevents the close
/// for [`CloseAction::HideToTray`].
pub fn on_close_requested<R: Runtime>(window: &Window<R>) -> CloseAction {
    if window.label() != MAIN_WINDOW {
        return CloseAction::Quit;
    }
    let app = window.app_handle();
    let support = app
        .try_state::<TrayState>()
        .map(|t| t.support())
        .unwrap_or_else(|| TraySupport::Unavailable("no tray item".into()));
    let settings = app
        .try_state::<AppState>()
        .map(|s| s.current_settings())
        .unwrap_or_default();
    let action = lifecycle::close_action(&settings, &support);
    if action == CloseAction::HideToTray
        && let Err(e) = window.hide()
    {
        tracing::warn!(error = %e, "the window could not be hidden");
        return CloseAction::Quit;
    }
    action
}
