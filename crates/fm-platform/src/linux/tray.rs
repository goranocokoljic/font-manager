//! The Linux tray: a freedesktop.org StatusNotifier item (ADR 0010). It shows
//! only when a StatusNotifier host runs (KDE Plasma, GNOME with the
//! AppIndicator extension, most other panels); plain GNOME has none. The
//! host is found through the `org.kde.StatusNotifierWatcher` service on the
//! session bus, asked with `gdbus` (part of GLib, which the app's GTK stack
//! needs anyway). No host, no bus or no `gdbus` → unavailable, and closing
//! the window quits.
//!
//! The parsing builds on every OS, so its tests run on every CI runner.

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::process::{Command, Stdio};

use crate::tray::{Tray, TrayKind, TraySupport};

/// The StatusNotifier tray.
#[derive(Debug, Clone, Copy, Default)]
pub struct LinuxTray;

impl Tray for LinuxTray {
    fn kind(&self) -> TrayKind {
        TrayKind::StatusNotifier
    }
    fn support(&self) -> TraySupport {
        let out = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                "org.kde.StatusNotifierWatcher",
                "--object-path",
                "/StatusNotifierWatcher",
                "--method",
                "org.freedesktop.DBus.Properties.Get",
                "org.kde.StatusNotifierWatcher",
                "IsStatusNotifierHostRegistered",
            ])
            .stdin(Stdio::null())
            .output();
        match out {
            Ok(out) if out.status.success() => {
                host_registered(&String::from_utf8_lossy(&out.stdout))
            }
            Ok(out) => TraySupport::Unavailable(format!(
                "no StatusNotifier watcher on the session bus: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )),
            Err(e) => TraySupport::Unavailable(format!("gdbus could not run: {e}")),
        }
    }
    fn icon_as_template(&self) -> bool {
        false
    }
    fn menu_on_left_click(&self) -> bool {
        true
    }
}

/// The answer to `IsStatusNotifierHostRegistered` as `gdbus` prints it:
/// `(<true>,)` when a host shows StatusNotifier items.
pub(crate) fn host_registered(gdbus_stdout: &str) -> TraySupport {
    let value: String = gdbus_stdout
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect();
    if value == "true" {
        TraySupport::Available
    } else {
        TraySupport::Unavailable(format!(
            "a StatusNotifier watcher runs but no host shows items: {}",
            gdbus_stdout.trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_registered_host_makes_the_tray_available() {
        assert_eq!(host_registered("(<true>,)\n"), TraySupport::Available);
    }

    #[test]
    fn no_host_or_an_odd_answer_makes_it_unavailable() {
        for out in ["(<false>,)\n", "", "(<'yes'>,)", "Error: something"] {
            assert!(!host_registered(out).is_available(), "{out:?}");
        }
    }
}
