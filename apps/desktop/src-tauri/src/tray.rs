//! The tray item and the close button (E17.1, ADR 0010). Wiring only: the
//! per-OS answers come from `fm_platform::Tray`, what closing does and what
//! the menu does from `fm_core::lifecycle`, what it says from the UI's message
//! catalog in the UI's language (`shell_text`, E19.2).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, PoisonError};

use fm_api::{Api, Event};
use fm_core::lifecycle::{self, CloseAction};
use fm_platform::{Tray, TraySupport};
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime, Window};

use crate::AppState;
use crate::shell_text;

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

/// The tray menu's items and the API that counts the active fonts; managed
/// state, so the labels can follow the language.
struct TrayItems<R: Runtime> {
    open: MenuItem<R>,
    count: MenuItem<R>,
    deactivate: MenuItem<R>,
    quit: MenuItem<R>,
    api: Arc<dyn Api>,
}

impl<R: Runtime> TrayItems<R> {
    fn label(&self, locale: &str) -> tauri::Result<()> {
        self.open
            .set_text(shell_text::text(locale, "shell.tray.open"))?;
        self.deactivate
            .set_text(shell_text::text(locale, "shell.tray.deactivateSession"))?;
        self.quit
            .set_text(shell_text::text(locale, "shell.tray.quit"))?;
        self.count_label(locale)
    }

    fn count_label(&self, locale: &str) -> tauri::Result<()> {
        let n = active_count(self.api.as_ref());
        self.count
            .set_text(shell_text::count(locale, "shell.tray.active", n))
    }
}

fn build<R: Runtime>(app: &AppHandle<R>, tray: &dyn Tray, api: Arc<dyn Api>) -> tauri::Result<()> {
    let locale = shell_text::current(app);
    let open = MenuItem::with_id(
        app,
        ID_OPEN,
        shell_text::text(&locale, "shell.tray.open"),
        true,
        None::<&str>,
    )?;
    let count = MenuItem::with_id(
        app,
        ID_COUNT,
        shell_text::count(&locale, "shell.tray.active", active_count(api.as_ref())),
        false,
        None::<&str>,
    )?;
    let deactivate = MenuItem::with_id(
        app,
        ID_DEACTIVATE,
        shell_text::text(&locale, "shell.tray.deactivateSession"),
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(
        app,
        ID_QUIT,
        shell_text::text(&locale, "shell.tray.quit"),
        true,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &count, &separator, &deactivate, &quit])?;

    let menu_api = Arc::clone(&api);
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(shell_text::text(&locale, "shell.tray.tooltip"))
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

    let events = api.subscribe();
    app.manage(TrayItems {
        open,
        count,
        deactivate,
        quit,
        api,
    });

    // The count follows activation events; nothing polls.
    let handle = app.clone();
    std::thread::Builder::new()
        .name("fm-tray".into())
        .spawn(move || {
            for event in events {
                if let Event::ActivationChanged { .. } = event
                    && let Some(items) = handle.try_state::<TrayItems<R>>()
                    && let Err(e) = items.count_label(&shell_text::current(&handle))
                {
                    tracing::warn!(error = %e, "tray count not updated");
                }
            }
        })?;
    Ok(())
}

/// Shows the tray's tooltip and menu in `locale`; nothing without a tray item.
pub fn relabel<R: Runtime>(app: &AppHandle<R>, locale: &str) {
    if let Some(items) = app.try_state::<TrayItems<R>>()
        && let Err(e) = items.label(locale)
    {
        tracing::warn!(error = %e, "tray menu not relabelled");
    }
    if let Some(icon) = app.tray_by_id(TRAY_ID)
        && let Err(e) = icon.set_tooltip(Some(shell_text::text(locale, "shell.tray.tooltip")))
    {
        tracing::warn!(error = %e, "tray tooltip not relabelled");
    }
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

/// Handles a close request for `window`: the main window hides to the tray
/// when "Keep running in the tray" is on and the app has a tray item, and
/// `prevent_close` stops the close; every other case closes as usual.
/// Returns the action taken.
pub fn on_close_requested<R: Runtime>(
    window: &Window<R>,
    prevent_close: impl FnOnce(),
) -> CloseAction {
    let action = close_request_action(window);
    if action == CloseAction::HideToTray {
        prevent_close();
    }
    action
}

fn close_request_action<R: Runtime>(window: &Window<R>) -> CloseAction {
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
