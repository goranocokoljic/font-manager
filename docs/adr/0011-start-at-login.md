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
| Entry | `REG_SZ` value `org.fontmanager.app` under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` | launch agent `~/Library/LaunchAgents/org.fontmanager.app.autostart.plist` (`RunAtLoad`, GUI session only) | E17.3: `$XDG_CONFIG_HOME/autostart/<app-id>.desktop` |
| Data | `"<program>" --autostart` | `ProgramArguments` = program, `--autostart` | — |
| Ours when | the value carries `" --autostart` | the file has our label and `--autostart` | — |

- **The OS is the state.** `is_enabled` reads the entry on every request;
  `config.json` never stores it. The settings service reports it as
  `Settings.start_at_login` (`None`: not offered, the switch is hidden) and
  `SettingsUpdate.start_at_login` turns it on or off.
- **Only our entry.** `disable` removes the entry only when it is one this
  app wrote; a value or file of the same name by someone else stays, and on
  macOS `enable` refuses to overwrite it.
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
- **Parity.** The option stays hidden on every OS
  (`fm_core::settings::START_AT_LOGIN_OFFERED = false`) until E17.3 adds
  the Linux entry; E17.3 turns it on.

## Consequences

- A user who turns the entry off in Task Manager (Windows keeps that in
  `Explorer\StartupApproved\Run`) still sees the switch on: the `Run` value
  is there. The entry stays the app's; the user's choice there wins at login.
- Moving the app leaves an entry that points at the old path until the user
  turns the switch on again.
- The per-OS integration test (`crates/fm-platform/tests/autostart.rs`)
  writes a test entry in the real location, reads it back from a second
  process (`reg query`, `plutil`) and finds it gone after `disable`.
