# 0010 — The tray per OS

Status: Accepted (2026-10-20)

## Context

Session activations end when the app quits (ADR 0008). Users close the window
long before they are done with the fonts, so story E17.1 keeps the app running
in the tray when the window closes. Every OS offers a different kind of tray
item, and one Linux desktop in common use (plain GNOME) offers none.

## Decision

`fm-platform` has one trait, `Tray`, with one implementation per OS under
`src/{windows,macos,linux}/tray.rs`. It answers what differs per OS; the desktop
shell builds the item and its menu with Tauri's tray API from those answers.

| | Windows | macOS | Linux |
|-|---------|-------|-------|
| Item | notification-area icon | menu-bar item | StatusNotifier item (AppIndicator) |
| Icon | app icon | app icon as a template image (follows the bar) | app icon |
| Left click | opens the window; menu on right click | opens the menu | opens the menu |
| Available | always | always | only when a StatusNotifier host runs |

- **Linux availability.** The app asks the session bus for
  `org.kde.StatusNotifierWatcher.IsStatusNotifierHostRegistered` with `gdbus`
  (GLib, which the GTK stack Tauri uses on Linux needs anyway). No host, no bus,
  no `gdbus`, or a tray toolkit that fails (the AppIndicator library missing)
  → no tray item, closing the window quits as before, and Settings → About →
  Platform notes says so.
- **The menu.** Open Font Manager; the active-font count, updated from
  `Event::ActivationChanged` (never polled); Deactivate session fonts (permanent
  ones stay); Quit Font Manager. Native menus, so the OS gives keyboard access.
- **Closing.** "Keep running in the tray when the window is closed" (Settings →
  Activation, on by default) and an available tray hide the main window; any
  other case closes it and the app quits.
- **Quitting** from the tray, the app menu, or a close without a tray all end
  in the same exit handler, which runs `ActivationManager::shutdown`: session
  fonts are deregistered, permanent ones stay (ADR 0008).
- The menu labels, the count line, what closing does and the platform notes
  are `fm-core` (`lifecycle`); the shell only wires them.
- Tauri's tray code lives in the shell, not in `fm-platform`: the CLI and the
  MCP server link `fm-platform` and must not link the windowing toolkit.

## Consequences

- With the default setting, closing the window no longer ends session
  activations; Quit does.
- Linux desktops without a StatusNotifier host keep the old behaviour, and the
  user is told why.
- The per-OS integration test (`apps/desktop/src-tauri/tests/tray_lifecycle.rs`)
  proves on each runner that a second process still sees a session font after
  the close handler, and not after Quit, while a permanent one stays.
