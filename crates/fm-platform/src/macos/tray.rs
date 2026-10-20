//! The macOS tray: a menu-bar item (ADR 0010). The menu bar is always there,
//! so it is always available. The icon is a template image, so it follows the
//! bar's light or dark appearance, and a click opens the menu, as menu-bar
//! items do.

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use crate::tray::{Tray, TrayKind, TraySupport};

/// The menu-bar tray.
#[derive(Debug, Clone, Copy, Default)]
pub struct MacosTray;

impl Tray for MacosTray {
    fn kind(&self) -> TrayKind {
        TrayKind::MenuBar
    }
    fn support(&self) -> TraySupport {
        TraySupport::Available
    }
    fn icon_as_template(&self) -> bool {
        true
    }
    fn menu_on_left_click(&self) -> bool {
        true
    }
}
