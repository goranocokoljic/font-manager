//! The Windows tray: an icon in the notification area (ADR 0010). Every
//! Windows desktop has one, so it is always available. A left click opens
//! the window, the menu is on the right click, as Windows apps do.

#![cfg_attr(not(windows), allow(dead_code))]

use crate::tray::{Tray, TrayKind, TraySupport};

/// The notification-area tray.
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsTray;

impl Tray for WindowsTray {
    fn kind(&self) -> TrayKind {
        TrayKind::NotificationArea
    }
    fn support(&self) -> TraySupport {
        TraySupport::Available
    }
    fn icon_as_template(&self) -> bool {
        false
    }
    fn menu_on_left_click(&self) -> bool {
        false
    }
}
