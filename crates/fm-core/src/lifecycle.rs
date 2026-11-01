//! The app's life with its window closed (E17.1, ADR 0010): what closing the
//! window does, what the tray menu says and does, and the platform notes
//! Settings → About shows. The desktop shell only wires these to the window
//! and the tray item the OS offers (`fm_platform::Tray`).
//!
//! Quitting, from the tray or the app menu, is the normal quit:
//! [`crate::CoreApi::shutdown`] deregisters the session fonts (ADR 0008).

use fm_api::activation::{
    ActivateRequest, ActivationMode, ActivationOutcome, ActivationService, ActiveFace,
    ConflictPolicy, DeactivateRequest, Target,
};
use fm_api::settings::Settings;
use fm_api::{ApiResult, Event, FaceId};
use fm_platform::{AUTOSTART_ARG, Os, TraySupport};

/// The tray menu item that shows the main window.
pub const MENU_OPEN: &str = "Open Font Manager";
/// The tray menu item that deactivates every session font.
pub const MENU_DEACTIVATE_SESSION: &str = "Deactivate session fonts";
/// The tray menu item that quits the app.
pub const MENU_QUIT: &str = "Quit Font Manager";

/// What closing the main window does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseAction {
    /// Hide the window; the app keeps running in the tray and session fonts
    /// stay registered.
    HideToTray,
    /// Quit as an app without a tray does: session fonts are deregistered.
    Quit,
}

/// Closing the window hides it to the tray when the user wants that ("Keep
/// running in the tray", on by default) and the desktop can show a tray item;
/// else the app quits.
pub fn close_action(settings: &Settings, tray: &TraySupport) -> CloseAction {
    if settings.keep_in_tray && tray.is_available() {
        CloseAction::HideToTray
    } else {
        CloseAction::Quit
    }
}

/// What the main window does as the app starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchWindow {
    /// Show the window, as any launch by the user.
    Show,
    /// Keep the window hidden; the app waits in the tray (a start at login).
    StayInTray,
}

/// A launch with [`AUTOSTART_ARG`] (the login entry, ADR 0011) stays in the
/// tray; any other launch shows the window, and so does a start at login
/// when the desktop has no tray item, so the app is never running unseen.
/// `args` are the program's arguments without the program itself.
pub fn launch_window<I, S>(args: I, tray: &TraySupport) -> LaunchWindow
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let autostart = args.into_iter().any(|a| a.as_ref() == AUTOSTART_ARG);
    if autostart && tray.is_available() {
        LaunchWindow::StayInTray
    } else {
        LaunchWindow::Show
    }
}

/// The tray menu's count line: "No fonts active", "1 font active", "12 fonts active".
pub fn active_label(count: usize) -> String {
    match count {
        0 => "No fonts active".to_owned(),
        1 => "1 font active".to_owned(),
        n => format!("{n} fonts active"),
    }
}

/// The faces of `active` that are active for this session only.
pub fn session_faces(active: &[ActiveFace]) -> Vec<FaceId> {
    active
        .iter()
        .filter(|a| a.mode == ActivationMode::Session)
        .map(|a| a.face)
        .collect()
}

/// The session faces of `active` the user activated: the ones
/// [`restore_session`] brings back after a quit. A face only documents
/// activated (E29.3) is left out; the plugin asks for it again.
pub fn resumable_faces(active: &[ActiveFace]) -> Vec<FaceId> {
    active
        .iter()
        .filter(|a| a.mode == ActivationMode::Session && a.document.is_none())
        .map(|a| a.face)
        .collect()
}

/// "Deactivate session fonts": deactivates every session-active face and
/// leaves permanent ones. Nothing is asked of `api` when no session font is
/// active (a request without a target would deactivate everything).
pub fn deactivate_session(api: &dyn ActivationService) -> ApiResult<Vec<ActivationOutcome>> {
    let faces = session_faces(&api.list_active()?);
    if faces.is_empty() {
        return Ok(Vec::new());
    }
    api.deactivate(DeactivateRequest {
        target: Some(Target::Faces { faces }),
        except: None,
    })
}

/// Activates `faces` for the session again: the session fonts a quit ended
/// when the app goes on running after all (an update that did not install,
/// E18.2), as [`resumable_faces`] lists them before the quit. Nothing is
/// asked of `api` when `faces` is empty.
pub fn restore_session(
    api: &dyn ActivationService,
    faces: Vec<FaceId>,
    on_conflict: ConflictPolicy,
) -> ApiResult<Vec<ActivationOutcome>> {
    if faces.is_empty() {
        return Ok(Vec::new());
    }
    api.activate(ActivateRequest {
        target: Target::Faces { faces },
        mode: ActivationMode::Session,
        on_conflict,
    })
}

/// The platform note for an app whose window quits it when closed: the
/// design-app plugins (E29) reach the app over its IPC endpoint, which ends
/// with the app.
pub const PLUGINS_NEED_THE_WINDOW: &str = "Plugins in design apps can reach Font Manager only \
                                           while its window is open, because closing the window \
                                           quits it.";

/// What works differently on `os`, one sentence each, for Settings → About →
/// Platform notes. `tray` is what the shell found when it made the tray item;
/// `None` before it tried (or without a shell, as in the CLI). `keep_in_tray`
/// is the "Keep running in the tray" setting. `sandboxed`: the app runs in a
/// Flatpak, where it offers no start at login.
pub fn platform_notes(
    os: Os,
    tray: Option<&TraySupport>,
    keep_in_tray: bool,
    sandboxed: bool,
) -> Vec<String> {
    let mut notes = Vec::new();
    let no_tray = matches!(tray, Some(TraySupport::Unavailable(_)));
    if no_tray {
        notes.push(match os {
            Os::Linux => {
                "This desktop shows no tray items (no StatusNotifier host), so closing the \
                          window quits Font Manager and ends session activations. KDE Plasma and \
                          GNOME with the AppIndicator extension show the tray item."
                    .to_owned()
            }
            Os::Windows | Os::MacOs => "The tray item could not be made, so closing the window \
                                        quits Font Manager and ends session activations."
                .to_owned(),
        });
    }
    // Closing the window quits in both cases (`close_action`).
    if no_tray || !keep_in_tray {
        notes.push(PLUGINS_NEED_THE_WINDOW.to_owned());
    }
    if os == Os::Linux {
        notes.push(
            "Apps that are already running can take up to 30 seconds to see a font you \
             activate; some see it only after a restart."
                .to_owned(),
        );
    }
    if sandboxed {
        notes.push(
            "Inside the Flatpak sandbox Font Manager cannot add itself to the apps that \
             start when you log in, so Start at login is not shown. Add Font Manager in \
             your desktop's startup settings instead."
                .to_owned(),
        );
    }
    notes
}

/// What an event asks of the main window (E27.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowCall {
    /// An AI assistant waits for the user's Allow or Deny: bring the window
    /// to the front, so the toast is on screen while the call waits.
    Raise,
    /// A confirmation was answered or timed out: stop asking for attention.
    ClearAttention,
}

/// What `event` asks of the main window; `None` for every event that leaves
/// it alone. `queued` are the events that already wait behind `event`: a
/// confirmation that is answered or timed out there is over before the
/// window could show it, so it does not raise the window.
pub fn window_call(event: &Event, queued: &[Event]) -> Option<WindowCall> {
    match event {
        Event::McpConfirmationRequested { id, .. } => {
            let closed = queued.iter().any(
                |later| matches!(later, Event::McpConfirmationClosed { id: done, .. } if done == id),
            );
            (!closed).then_some(WindowCall::Raise)
        }
        Event::McpConfirmationClosed { .. } => Some(WindowCall::ClearAttention),
        _ => None,
    }
}

/// How the main window is now, as the shell reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowView {
    /// Shown (not hidden to the tray).
    pub visible: bool,
    /// Minimized to the taskbar or the Dock.
    pub minimized: bool,
    /// The window has the keyboard focus: it is in front.
    pub focused: bool,
}

/// The window calls that bring the main window to the front, in this order:
/// unminimize, show, focus, then ask for attention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RaiseSteps {
    /// Restore a minimized window.
    pub unminimize: bool,
    /// Show a hidden window.
    pub show: bool,
    /// Ask the OS to give the window the focus.
    pub focus: bool,
    /// Flash the taskbar entry, bounce the Dock icon or mark the window
    /// urgent. Windows and GNOME refuse focus to an app in the background;
    /// then this is what the user sees. The OS stops it once the window is in
    /// front.
    pub request_attention: bool,
}

/// The steps that bring the window in `view` to the front: none when it is
/// already shown, restored and focused.
pub fn raise_steps(view: WindowView) -> RaiseSteps {
    if view.visible && !view.minimized && view.focused {
        return RaiseSteps::default();
    }
    RaiseSteps {
        unminimize: view.minimized,
        show: !view.visible,
        focus: true,
        request_attention: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fm_api::activation::{ActivationService, ActiveFace, Clash, Conflict, OutcomeStatus};
    use fm_api::common::Timestamp;
    use std::sync::Mutex;

    fn settings(keep_in_tray: bool) -> Settings {
        Settings {
            keep_in_tray,
            ..Settings::default()
        }
    }

    #[test]
    fn closing_hides_to_the_tray_only_when_wanted_and_possible() {
        let none = TraySupport::Unavailable("no host".into());
        assert_eq!(
            close_action(&settings(true), &TraySupport::Available),
            CloseAction::HideToTray
        );
        assert_eq!(close_action(&settings(true), &none), CloseAction::Quit);
        assert_eq!(
            close_action(&settings(false), &TraySupport::Available),
            CloseAction::Quit
        );
        assert_eq!(close_action(&settings(false), &none), CloseAction::Quit);
        // The default keeps the app in the tray.
        assert_eq!(
            close_action(&Settings::default(), &TraySupport::Available),
            CloseAction::HideToTray
        );
    }

    #[test]
    fn a_start_at_login_stays_in_the_tray_and_every_other_launch_shows_the_window() {
        let tray = TraySupport::Available;
        let none: [&str; 0] = [];
        assert_eq!(launch_window(none, &tray), LaunchWindow::Show);
        assert_eq!(
            launch_window(["--autostart"], &tray),
            LaunchWindow::StayInTray
        );
        assert_eq!(
            launch_window(["--verbose", "--autostart"], &tray),
            LaunchWindow::StayInTray
        );
        // Only the exact argument: a file or another flag opens the window.
        for other in ["--autostart=1", "--AUTOSTART", "autostart", "-autostart"] {
            assert_eq!(launch_window([other], &tray), LaunchWindow::Show, "{other}");
        }
        // No tray item: the app must not run unseen.
        let no_tray = TraySupport::Unavailable("no StatusNotifier host".into());
        assert_eq!(launch_window(["--autostart"], &no_tray), LaunchWindow::Show);
    }

    #[test]
    fn the_launch_argument_is_the_one_the_login_entry_writes() {
        assert_eq!(AUTOSTART_ARG, "--autostart");
        // Arguments that are not UTF-8 (possible on Linux) are compared as they are.
        #[cfg(unix)]
        {
            use std::ffi::OsStr;
            use std::os::unix::ffi::OsStrExt;
            let odd = OsStr::from_bytes(b"\xff--autostart");
            assert_eq!(
                launch_window([odd], &TraySupport::Available),
                LaunchWindow::Show
            );
        }
    }

    #[test]
    fn the_count_line_reads_naturally() {
        assert_eq!(active_label(0), "No fonts active");
        assert_eq!(active_label(1), "1 font active");
        assert_eq!(active_label(12), "12 fonts active");
    }

    fn active(face: u32, mode: ActivationMode) -> ActiveFace {
        ActiveFace {
            face: FaceId(face),
            mode,
            since: Timestamp::default(),
            document: None,
        }
    }

    /// Lists fixed faces and records every deactivation request.
    struct Recorder {
        active: Vec<ActiveFace>,
        asked: Mutex<Vec<DeactivateRequest>>,
    }

    impl ActivationService for Recorder {
        fn activate(
            &self,
            _: fm_api::activation::ActivateRequest,
        ) -> ApiResult<Vec<ActivationOutcome>> {
            Ok(Vec::new())
        }
        fn deactivate(&self, req: DeactivateRequest) -> ApiResult<Vec<ActivationOutcome>> {
            let faces = match &req.target {
                Some(Target::Faces { faces }) => faces.clone(),
                _ => Vec::new(),
            };
            self.asked.lock().expect("lock").push(req);
            Ok(faces
                .into_iter()
                .map(|face| ActivationOutcome {
                    face,
                    outcome: OutcomeStatus::Done,
                })
                .collect())
        }
        fn list_active(&self) -> ApiResult<Vec<ActiveFace>> {
            Ok(self.active.clone())
        }
        fn conflicts(&self, _: Target) -> ApiResult<Vec<Conflict>> {
            Ok(Vec::new())
        }
        fn list_conflicts(&self) -> ApiResult<Vec<Clash>> {
            Ok(Vec::new())
        }
        fn fonts_needed(
            &self,
            _: fm_api::activation::FontsNeededRequest,
        ) -> ApiResult<fm_api::activation::FontsNeededResponse> {
            Ok(fm_api::activation::FontsNeededResponse {
                matched: Vec::new(),
                unmatched: Vec::new(),
                missing: 0,
                outcomes: Vec::new(),
            })
        }
        fn document_closed(
            &self,
            _: fm_api::activation::DocumentClosedRequest,
        ) -> ApiResult<fm_api::activation::DocumentClosedResponse> {
            Ok(fm_api::activation::DocumentClosedResponse { offer: Vec::new() })
        }
    }

    /// E29.3 criterion 4: the tray's "Deactivate session fonts" ends the
    /// document-scoped faces too; the resume after a quit leaves them out.
    #[test]
    fn document_scoped_faces_are_session_faces_but_not_resumed() {
        let mut from_doc = active(4, ActivationMode::Session);
        from_doc.document = Some("poster.indd".into());
        let list = vec![
            active(1, ActivationMode::Session),
            active(2, ActivationMode::Permanent),
            from_doc,
        ];
        assert_eq!(session_faces(&list), [FaceId(1), FaceId(4)]);
        assert_eq!(resumable_faces(&list), [FaceId(1)]);
    }

    #[test]
    fn deactivate_session_leaves_permanent_fonts() {
        let api = Recorder {
            active: vec![
                active(1, ActivationMode::Session),
                active(2, ActivationMode::Permanent),
                active(3, ActivationMode::Session),
            ],
            asked: Mutex::new(Vec::new()),
        };
        let out = deactivate_session(&api).expect("deactivates");
        assert_eq!(out.len(), 2);
        let asked = api.asked.lock().expect("lock");
        assert_eq!(asked.len(), 1);
        assert_eq!(
            asked[0].target,
            Some(Target::Faces {
                faces: vec![FaceId(1), FaceId(3)]
            })
        );
    }

    #[test]
    fn deactivate_session_without_session_fonts_asks_nothing() {
        // A request without a target would deactivate the permanent font too.
        let api = Recorder {
            active: vec![active(2, ActivationMode::Permanent)],
            asked: Mutex::new(Vec::new()),
        };
        assert!(deactivate_session(&api).expect("ok").is_empty());
        assert!(api.asked.lock().expect("lock").is_empty());
    }

    #[test]
    fn platform_notes_say_when_closing_quits() {
        let none = TraySupport::Unavailable("no host".into());
        let linux = platform_notes(Os::Linux, Some(&none), true, false);
        assert!(linux[0].contains("no StatusNotifier host"), "{linux:?}");
        assert!(linux[0].contains("closing the window quits"), "{linux:?}");
        // With a tray, Linux keeps only the running-apps note (ADR 0008).
        let with_tray = platform_notes(Os::Linux, Some(&TraySupport::Available), true, false);
        assert_eq!(with_tray.len(), 1);
        assert!(with_tray[0].contains("30 seconds"));
        assert!(platform_notes(Os::Windows, Some(&TraySupport::Available), true, false).is_empty());
        assert!(platform_notes(Os::MacOs, None, true, false).is_empty());
        assert!(
            platform_notes(Os::Windows, Some(&none), true, false)[0]
                .contains("closing the window quits")
        );
    }

    /// E29.4 criterion 4: where closing the window quits, plugins need it open.
    #[test]
    fn platform_notes_say_plugins_need_the_window_where_closing_quits() {
        let plugins = |notes: &[String]| notes.iter().any(|n| n == PLUGINS_NEED_THE_WINDOW);
        let none = TraySupport::Unavailable("no host".into());
        for os in [Os::Windows, Os::MacOs, Os::Linux] {
            // A tray and the setting on: the app keeps serving with the window closed.
            let kept = platform_notes(os, Some(&TraySupport::Available), true, false);
            assert!(!plugins(&kept), "{os:?}: {kept:?}");
            // No tray item.
            let no_tray = platform_notes(os, Some(&none), true, false);
            assert!(plugins(&no_tray), "{os:?}: {no_tray:?}");
            // "Keep running in the tray" off, with or without a tray answer yet.
            for tray in [Some(&TraySupport::Available), None] {
                let off = platform_notes(os, tray, false, false);
                assert!(plugins(&off), "{os:?}: {off:?}");
                assert!(
                    !off.iter()
                        .any(|n| n.contains("StatusNotifier") || n.contains("tray item")),
                    "{os:?}: the setting is the reason, not the tray: {off:?}"
                );
            }
            // Said once, however many reasons there are.
            let both = platform_notes(os, Some(&none), false, false);
            assert_eq!(
                both.iter()
                    .filter(|n| *n == PLUGINS_NEED_THE_WINDOW)
                    .count(),
                1,
                "{os:?}: {both:?}"
            );
            // Every sentence reads as one line: no gaps from the source's line breaks.
            let all = platform_notes(os, Some(&none), false, true);
            assert!(all.iter().all(|n| !n.contains("  ")), "{os:?}: {all:?}");
        }
        assert!(PLUGINS_NEED_THE_WINDOW.contains("window is open"));
    }

    #[test]
    fn platform_notes_say_why_start_at_login_is_missing_in_a_flatpak() {
        let notes = platform_notes(Os::Linux, Some(&TraySupport::Available), true, true);
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert!(notes[1].contains("Flatpak sandbox"), "{notes:?}");
        assert!(!notes[1].contains("  "), "stray spaces: {notes:?}");
        assert!(
            notes[1].contains("Start at login is not shown"),
            "{notes:?}"
        );
        let plain = platform_notes(Os::Linux, Some(&TraySupport::Available), true, false);
        assert!(
            !plain.iter().any(|n| n.contains("Start at login")),
            "{plain:?}"
        );
    }

    fn requested(id: u32) -> Event {
        Event::McpConfirmationRequested {
            id,
            tool: "deactivate_fonts".into(),
            faces: Some(1),
            timeout_secs: 60,
        }
    }

    fn closed(id: u32) -> Event {
        Event::McpConfirmationClosed { id, allowed: false }
    }

    #[test]
    fn only_a_waiting_confirmation_raises_the_window() {
        assert_eq!(window_call(&requested(4), &[]), Some(WindowCall::Raise));
        // Another confirmation closing behind it does not count.
        assert_eq!(
            window_call(&requested(4), &[closed(3)]),
            Some(WindowCall::Raise)
        );
        // Answered or timed out before the window could show it.
        assert_eq!(window_call(&requested(4), &[closed(4)]), None);
        assert_eq!(window_call(&requested(4), &[requested(5), closed(4)]), None);
        assert_eq!(
            window_call(&closed(4), &[]),
            Some(WindowCall::ClearAttention)
        );
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
            assert_eq!(window_call(other, &[]), None, "{other:?}");
        }
    }

    #[test]
    fn raising_restores_shows_and_focuses_only_what_is_needed() {
        let front = WindowView {
            visible: true,
            minimized: false,
            focused: true,
        };
        assert_eq!(raise_steps(front), RaiseSteps::default());
        let hidden = WindowView {
            visible: false,
            minimized: false,
            focused: false,
        };
        assert_eq!(
            raise_steps(hidden),
            RaiseSteps {
                unminimize: false,
                show: true,
                focus: true,
                request_attention: true,
            }
        );
        let minimized = WindowView {
            visible: true,
            minimized: true,
            focused: false,
        };
        assert_eq!(
            raise_steps(minimized),
            RaiseSteps {
                unminimize: true,
                show: false,
                focus: true,
                request_attention: true,
            }
        );
        // Shown but behind another app's window: focus, and ask for
        // attention where the OS refuses the focus.
        let behind = WindowView {
            visible: true,
            minimized: false,
            focused: false,
        };
        assert_eq!(
            raise_steps(behind),
            RaiseSteps {
                unminimize: false,
                show: false,
                focus: true,
                request_attention: true,
            }
        );
    }
}
