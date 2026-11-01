//! Brings the main window to the front when an AI assistant asks for a
//! confirmation (E27.9), so the Allow / Deny toast is on screen while the call
//! waits. Wiring only: which events raise the window and which window calls
//! that takes come from `fm_core::lifecycle` (`window_call`, `raise_steps`);
//! Tauri makes the calls the same way on every OS.

use fm_api::Event;
use fm_core::lifecycle::{self, RaiseSteps, WindowCall, WindowView};
use tauri::{AppHandle, Manager, Runtime, UserAttentionType, WebviewWindow};

use crate::tray::MAIN_WINDOW;

/// The window calls this module makes; the main window in the app, a
/// recording double in the tests.
pub trait RaiseTarget {
    /// How the window is now.
    fn view(&self) -> WindowView;
    /// Restores a minimized window.
    fn unminimize(&self);
    /// Shows a hidden window.
    fn show(&self);
    /// Asks the OS for the focus.
    fn set_focus(&self);
    /// Flashes the taskbar entry or bounces the Dock icon (`true`), or stops
    /// it (`false`).
    fn request_attention(&self, on: bool);
}

/// Does what `event` asks of `window`; `queued` are the events already
/// waiting behind it. Returns the call taken, `None` when the event leaves the
/// window alone.
pub fn on_event(window: &dyn RaiseTarget, event: &Event, queued: &[Event]) -> Option<WindowCall> {
    let call = lifecycle::window_call(event, queued)?;
    match call {
        WindowCall::Raise => raise(window, lifecycle::raise_steps(window.view())),
        WindowCall::ClearAttention => window.request_attention(false),
    }
    Some(call)
}

fn raise(window: &dyn RaiseTarget, steps: RaiseSteps) {
    if steps.unminimize {
        window.unminimize();
    }
    if steps.show {
        window.show();
    }
    if steps.focus {
        window.set_focus();
    }
    if steps.request_attention {
        window.request_attention(true);
    }
}

/// Does what `event` asks of the app's main window; nothing without one.
pub fn on_app_event<R: Runtime>(app: &AppHandle<R>, event: &Event, queued: &[Event]) {
    // Most events leave the window alone: do not look it up for them.
    if lifecycle::window_call(event, queued).is_none() {
        return;
    }
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        on_event(&window, event, queued);
    }
}

impl<R: Runtime> RaiseTarget for WebviewWindow<R> {
    fn view(&self) -> WindowView {
        WindowView {
            visible: self.is_visible().unwrap_or(false),
            minimized: self.is_minimized().unwrap_or(false),
            focused: self.is_focused().unwrap_or(false),
        }
    }

    fn unminimize(&self) {
        if let Err(e) = WebviewWindow::unminimize(self) {
            tracing::warn!(error = %e, "main window not restored for a confirmation");
        }
    }

    fn show(&self) {
        if let Err(e) = WebviewWindow::show(self) {
            tracing::warn!(error = %e, "main window not shown for a confirmation");
        }
    }

    fn set_focus(&self) {
        if let Err(e) = WebviewWindow::set_focus(self) {
            tracing::warn!(error = %e, "main window not focused for a confirmation");
        }
    }

    fn request_attention(&self, on: bool) {
        let kind = on.then_some(UserAttentionType::Critical);
        if let Err(e) = self.request_user_attention(kind) {
            tracing::warn!(error = %e, "attention request for a confirmation failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fm_api::FaceId;
    use std::cell::RefCell;

    /// A window that records the calls made on it.
    struct Recorder {
        view: WindowView,
        calls: RefCell<Vec<&'static str>>,
    }

    impl Recorder {
        fn new(visible: bool, minimized: bool, focused: bool) -> Self {
            Self {
                view: WindowView {
                    visible,
                    minimized,
                    focused,
                },
                calls: RefCell::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<&'static str> {
            self.calls.borrow().clone()
        }
    }

    impl RaiseTarget for Recorder {
        fn view(&self) -> WindowView {
            self.view
        }
        fn unminimize(&self) {
            self.calls.borrow_mut().push("unminimize");
        }
        fn show(&self) {
            self.calls.borrow_mut().push("show");
        }
        fn set_focus(&self) {
            self.calls.borrow_mut().push("set_focus");
        }
        fn request_attention(&self, on: bool) {
            self.calls
                .borrow_mut()
                .push(if on { "attention" } else { "attention_off" });
        }
    }

    fn requested(id: u32) -> Event {
        Event::McpConfirmationRequested {
            id,
            tool: "deactivate_fonts".into(),
            faces: Some(2),
            timeout_secs: 60,
        }
    }

    #[test]
    fn a_confirmation_shows_a_hidden_window() {
        let window = Recorder::new(false, false, false);
        assert_eq!(
            on_event(&window, &requested(1), &[]),
            Some(WindowCall::Raise)
        );
        assert_eq!(window.calls(), ["show", "set_focus", "attention"]);
    }

    #[test]
    fn a_confirmation_restores_a_minimized_window() {
        let window = Recorder::new(true, true, false);
        on_event(&window, &requested(1), &[]);
        assert_eq!(window.calls(), ["unminimize", "set_focus", "attention"]);
    }

    #[test]
    fn a_confirmation_brings_a_window_behind_others_to_the_front() {
        let window = Recorder::new(true, false, false);
        on_event(&window, &requested(1), &[]);
        assert_eq!(window.calls(), ["set_focus", "attention"]);
        // Already in front: nothing to do.
        let front = Recorder::new(true, false, true);
        on_event(&front, &requested(2), &[]);
        assert!(front.calls().is_empty(), "{:?}", front.calls());
    }

    #[test]
    fn other_events_leave_the_window_alone() {
        let window = Recorder::new(false, true, false);
        let others = [
            Event::ActivationChanged {
                activated: vec![FaceId(1)],
                deactivated: vec![],
            },
            Event::ScanProgress {
                job: fm_api::JobId(1),
                done: 1,
                total: 2,
            },
        ];
        for other in &others {
            assert_eq!(on_event(&window, other, &[]), None, "{other:?}");
        }
        assert!(window.calls().is_empty(), "{:?}", window.calls());
    }

    #[test]
    fn a_confirmation_already_over_leaves_the_window_alone() {
        let window = Recorder::new(false, true, false);
        let over = [Event::McpConfirmationClosed {
            id: 7,
            allowed: false,
        }];
        assert_eq!(on_event(&window, &requested(7), &over), None);
        assert!(window.calls().is_empty(), "{:?}", window.calls());
        // Once it closes, the attention request stops.
        assert_eq!(
            on_event(&window, &over[0], &[]),
            Some(WindowCall::ClearAttention)
        );
        assert_eq!(window.calls(), ["attention_off"]);
    }
}
