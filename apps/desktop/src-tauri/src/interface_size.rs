//! The interface size (E58.7, ADR 0029): the webview's own zoom.
//!
//! Every webview of the app is zoomed to `InterfaceSize::factor()` of the
//! settings: once the windows exist, before the main window is shown (so the
//! first paint is at the right size), after each page load, and at once on
//! `Event::InterfaceSizeChanged`. The zoom keeps the window's size and
//! position; only what the webview draws grows. The surfaces that show a font
//! take the zoom back in CSS (`fm-true-size`, DESIGN.md).

use std::sync::Mutex;

use fm_api::event::Event;
use tauri::webview::PageLoadPayload;
use tauri::{AppHandle, Manager, Runtime, Webview};

/// The zoom factor every webview of the app has now.
#[derive(Debug)]
pub struct InterfaceZoom(Mutex<f64>);

impl InterfaceZoom {
    /// The zoom at start: the factor of the size in the settings.
    pub fn new(factor: f64) -> Self {
        Self(Mutex::new(factor))
    }

    /// The factor applied now.
    pub fn factor(&self) -> f64 {
        *self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn set(&self, factor: f64) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = factor;
    }
}

/// Registers the zoom at start and the page-load hook that keeps every
/// webview at it.
pub fn wire<R: Runtime>(builder: tauri::Builder<R>, factor: f64) -> tauri::Builder<R> {
    builder
        .manage(InterfaceZoom::new(factor))
        .on_page_load(on_page_load)
}

/// Zooms every webview of the app to the factor applied now (the setup
/// hook, before the main window is shown).
pub fn apply_current<R: Runtime>(app: &AppHandle<R>) {
    let factor = app.state::<InterfaceZoom>().factor();
    for (label, window) in app.webview_windows() {
        if let Err(e) = window.set_zoom(factor) {
            warn(&e, factor, &label);
        }
    }
}

/// Applies a new interface size at once, in every webview.
pub fn on_app_event<R: Runtime>(app: &AppHandle<R>, event: &Event) {
    if let Event::InterfaceSizeChanged { interface_size } = event {
        app.state::<InterfaceZoom>().set(interface_size.factor);
        apply_current(app);
    }
}

/// A page that starts or finishes loading (a reload, the help window's next
/// page) gets the zoom again: as it starts, and once it has loaded, in case
/// the engine reset it on the navigation.
fn on_page_load<R: Runtime>(webview: &Webview<R>, _: &PageLoadPayload<'_>) {
    let factor = webview.app_handle().state::<InterfaceZoom>().factor();
    zoom(webview, factor);
}

fn zoom<R: Runtime>(webview: &Webview<R>, factor: f64) {
    if let Err(e) = webview.set_zoom(factor) {
        warn(&e, factor, webview.label());
    }
}

fn warn(e: &tauri::Error, factor: f64, label: &str) {
    tracing::warn!(error = %e, factor, webview = label, "the interface size was not applied");
}
