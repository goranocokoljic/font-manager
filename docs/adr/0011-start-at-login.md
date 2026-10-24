# 0011 — Start at login per OS

Status: Accepted (2026-10-21)

## Context

Session fonts live as long as the app (ADR 0008), and the app can wait in
the tray (ADR 0010). Users who keep fonts active every day want it to be
there after they log in, without opening its window. Every OS keeps its
login entries in a different place. Rule 4 forbids admin rights; rule 2
says the app touches only what it wrote.

## Decision

`fm-platform` has one trait, `Autostart` (`enable`, `disable`,
`is_enabled`), with one implementation per OS. The entry launches the
running program with `--autostart` (`fm_platform::AUTOSTART_ARG`).

| | Windows | macOS | Linux |
|-|---------|-------|-------|
| Entry | `REG_SZ` value `org.fontmanager.app` under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` | launch agent `~/Library/LaunchAgents/org.fontmanager.app.autostart.plist` (`RunAtLoad`, GUI session only) | XDG autostart entry `$XDG_CONFIG_HOME/autostart/org.fontmanager.app.desktop` (default `~/.config`) |
| Data | `"<program>" --autostart` | `ProgramArguments` = program, `--autostart` | `Exec="<program>" --autostart` (inside an AppImage the program is `$APPIMAGE`) |
| Ours when | the value carries `" --autostart` | the file has our label and `--autostart` | `[Desktop Entry]` has `X-FontManager-Autostart=true` |

- **The OS is the state.** `is_enabled` reads the entry on every request;
  `config.json` never stores it. The settings service reports it as
  `Settings.start_at_login` (`None`: not offered, the switch is hidden) and
  `SettingsUpdate.start_at_login` turns it on or off.
- **Only our entry.** `disable` removes the entry only when it is one this
  app wrote; a value or file of the same name by someone else stays, and on
  macOS and Linux `enable` refuses to overwrite it. On Linux an entry of
  ours that already launches this program is not rewritten, so keys the user
  added stay; `Hidden=true` or `X-GNOME-Autostart-enabled=false` reads as off.
- **An entry of another copy** (the app moved) is not "on" for this copy;
  turning the switch on rewrites it, off removes it.
- **The launch.** `fm_core::lifecycle::launch_window`: with `--autostart`
  and a tray item the window stays hidden; without a tray item it shows, so
  the app never runs unseen. The main window is created hidden and shown by
  the shell for every other launch, so a start at login never flashes it.
- **macOS: a launch agent, not `SMAppService`.** `SMAppService` needs a
  signed app bundle (E18.1) and Objective-C bindings the library table does
  not list. macOS 13+ lists a launch agent under Login Items, where the user
  can turn it off.
- **Parity.** E17.2 kept the option hidden on every OS until E17.3 added the
  Linux entry; since E17.3 Windows, macOS and Linux offer it together.
- **Flatpak: hidden, not the Background portal.** Inside a Flatpak
  (`FLATPAK_ID` or `/.flatpak-info`, `fm_platform::autostart::sandboxed`) the
  sandbox's config folder is not the host session's, so a file there never
  starts the app. `org.freedesktop.portal.Background` would, but it needs a
  D-Bus client the library table does not list. The option is hidden there
  (`Unsupported`) and Settings → About → Platform notes says why; the portal
  can come with the Flatpak build (E18.1).

## Consequences

- A user who turns the entry off in Task Manager (Windows keeps that in
  `Explorer\StartupApproved\Run`) still sees the switch on: the `Run` value
  is there. The entry stays the app's; the user's choice there wins at login.
- Moving the app leaves an entry that points at the old path until the user
  turns the switch on again.
- The per-OS integration test (`crates/fm-platform/tests/autostart.rs`)
  writes a test entry in the real location, reads it back from a second
  process (`reg query`, `plutil`) and finds it gone after `disable`. On Linux
  a child process writes the entry under a temporary `XDG_CONFIG_HOME`, and CI
  checks it with `desktop-file-validate` (`desktop-file-utils`).
