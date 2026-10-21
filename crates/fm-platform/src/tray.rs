//! The tray: the small icon that keeps the app reachable while its window is
//! closed, so session activations stay registered (E17.1, ADR 0010).
//!
//! | OS | Where the icon lives | Left click | Can be missing |
//! |----|----------------------|------------|----------------|
//! | Windows | notification area | opens the window | no |
//! | macOS | menu bar (template icon) | opens the menu | no |
//! | Linux | StatusNotifier item | opens the menu | yes: no StatusNotifier host |
//!
//! [`Tray`] answers the per-OS questions; the desktop shell builds the icon
//! and its menu with its windowing toolkit from these answers. When
//! [`Tray::support`] says [`TraySupport::Unavailable`], closing the window
//! quits as without a tray.

use std::fmt;
use std::sync::Arc;

/// Which kind of tray item the OS offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayKind {
    /// The Windows notification area.
    NotificationArea,
    /// The macOS menu bar.
    MenuBar,
    /// A freedesktop.org StatusNotifier item (KDE, GNOME with the AppIndicator
    /// extension, most other Linux desktops).
    StatusNotifier,
}

/// Whether the app can show a tray item on this desktop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraySupport {
    /// A tray item can be shown.
    Available,
    /// No tray item can be shown; the reason is for the log.
    Unavailable(String),
}

impl TraySupport {
    /// True for [`TraySupport::Available`].
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }
}

/// The tray of this OS.
pub trait Tray: Send + Sync + fmt::Debug {
    /// Which kind of item the OS offers.
    fn kind(&self) -> TrayKind;
    /// Whether a tray item can be shown now. On Linux this asks the session
    /// bus for a StatusNotifier host.
    fn support(&self) -> TraySupport;
    /// Whether the icon is drawn as a template image that follows the bar's
    /// appearance (the macOS menu bar).
    fn icon_as_template(&self) -> bool;
    /// Whether a left click opens the menu; else it opens the window and the
    /// menu is on the right click.
    fn menu_on_left_click(&self) -> bool;
}

/// The tray of the OS the app runs on. Any other OS gets a tray that is
/// always [`TraySupport::Unavailable`].
pub fn system() -> Arc<dyn Tray> {
    #[cfg(windows)]
    {
        Arc::new(crate::windows::WindowsTray)
    }
    #[cfg(target_os = "macos")]
    {
        Arc::new(crate::macos::MacosTray)
    }
    #[cfg(target_os = "linux")]
    {
        Arc::new(crate::linux::LinuxTray)
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        Arc::new(MockTray::new(
            TrayKind::NotificationArea,
            TraySupport::Unavailable("no tray on this OS".into()),
        ))
    }
}

/// A tray with fixed answers, for tests.
#[derive(Debug, Clone)]
pub struct MockTray {
    kind: TrayKind,
    support: TraySupport,
}

impl MockTray {
    /// A tray of `kind` that always answers `support`.
    pub fn new(kind: TrayKind, support: TraySupport) -> Self {
        Self { kind, support }
    }
}

impl Tray for MockTray {
    fn kind(&self) -> TrayKind {
        self.kind
    }
    fn support(&self) -> TraySupport {
        self.support.clone()
    }
    fn icon_as_template(&self) -> bool {
        self.kind == TrayKind::MenuBar
    }
    fn menu_on_left_click(&self) -> bool {
        self.kind != TrayKind::NotificationArea
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_system_tray_matches_the_os() {
        let tray = system();
        if cfg!(windows) {
            assert_eq!(tray.kind(), TrayKind::NotificationArea);
            assert!(tray.support().is_available());
            assert!(!tray.menu_on_left_click());
            assert!(!tray.icon_as_template());
        } else if cfg!(target_os = "macos") {
            assert_eq!(tray.kind(), TrayKind::MenuBar);
            assert!(tray.support().is_available());
            assert!(tray.menu_on_left_click());
            assert!(tray.icon_as_template());
        } else if cfg!(target_os = "linux") {
            assert_eq!(tray.kind(), TrayKind::StatusNotifier);
            assert!(tray.menu_on_left_click());
            assert!(!tray.icon_as_template());
            // A CI runner has no desktop: support depends on the session bus.
            let _ = tray.support();
        }
    }

    #[test]
    fn a_mock_tray_answers_what_it_was_given() {
        let tray = MockTray::new(
            TrayKind::StatusNotifier,
            TraySupport::Unavailable("x".into()),
        );
        assert!(!tray.support().is_available());
        assert_eq!(tray.kind(), TrayKind::StatusNotifier);
    }
}
