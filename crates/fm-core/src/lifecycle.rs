//! The app's life with its window closed (E17.1, ADR 0010): what closing the
//! window does, what the tray menu says and does, and the platform notes
//! Settings → About shows. The desktop shell only wires these to the window
//! and the tray item the OS offers (`fm_platform::Tray`).
//!
//! Quitting, from the tray or the app menu, is the normal quit:
//! [`crate::CoreApi::shutdown`] deregisters the session fonts (ADR 0008).

use fm_api::activation::{
    ActivationMode, ActivationOutcome, ActivationService, ActiveFace, DeactivateRequest, Target,
};
use fm_api::settings::Settings;
use fm_api::{ApiResult, FaceId};
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

/// What works differently on `os`, one sentence each, for Settings → About →
/// Platform notes. `tray` is what the shell found when it made the tray item;
/// `None` before it tried (or without a shell, as in the CLI).
pub fn platform_notes(os: Os, tray: Option<&TraySupport>) -> Vec<String> {
    let mut notes = Vec::new();
    if let Some(TraySupport::Unavailable(_)) = tray {
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
    if os == Os::Linux {
        notes.push(
            "Apps that are already running can take up to 30 seconds to see a font you \
             activate; some see it only after a restart."
                .to_owned(),
        );
    }
    notes
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
        let linux = platform_notes(Os::Linux, Some(&none));
        assert!(linux[0].contains("no StatusNotifier host"), "{linux:?}");
        assert!(linux[0].contains("closing the window quits"), "{linux:?}");
        // With a tray, Linux keeps only the running-apps note (ADR 0008).
        let with_tray = platform_notes(Os::Linux, Some(&TraySupport::Available));
        assert_eq!(with_tray.len(), 1);
        assert!(with_tray[0].contains("30 seconds"));
        assert!(platform_notes(Os::Windows, Some(&TraySupport::Available)).is_empty());
        assert!(platform_notes(Os::MacOs, None).is_empty());
        assert!(platform_notes(Os::Windows, Some(&none))[0].contains("closing the window quits"));
    }
}
