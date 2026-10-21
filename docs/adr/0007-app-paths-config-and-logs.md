# 0007 — App folders, config and logs per OS

Status: Accepted (2026-10-16; layouts folder added 2026-10-21, E12.5)

## Context

The app keeps its own files: the index database, caches, a settings file and logs. Each
OS has a convention for where per-user app files go. The spec also offers "Copy to
library folder" on import, which needs a suggested folder. Rule 5 asks that platform
differences live in `fm-platform` and are written down here.

## Decision

`fm_platform::AppPaths` resolves five per-user folders. The rules for every OS are pure
functions over the environment (`src/{windows,macos,linux}/paths.rs`), compiled and
tested on every host; only `AppPaths::current()` picks the host's rules.

| Folder  | Windows | macOS | Linux |
|---------|---------|-------|-------|
| config  | `%APPDATA%\FontManager` | `~/Library/Application Support/org.fontmanager.app` | `$XDG_CONFIG_HOME/font-manager` |
| data    | `%LOCALAPPDATA%\FontManager\data` | `~/Library/Application Support/org.fontmanager.app` | `$XDG_DATA_HOME/font-manager` |
| cache   | `%LOCALAPPDATA%\FontManager\cache` | `~/Library/Caches/org.fontmanager.app` | `$XDG_CACHE_HOME/font-manager` |
| logs    | `%LOCALAPPDATA%\FontManager\logs` | `~/Library/Logs/org.fontmanager.app` | `$XDG_STATE_HOME/font-manager/logs` |
| library | `%USERPROFILE%\Fonts` | `~/Fonts` | `~/Fonts` |

- Unset, empty or relative variables fall back to the documented defaults (XDG
  defaults on Linux; `%USERPROFILE%\AppData\{Roaming,Local}` on Windows). A missing home
  variable is an error, never a panic.
- The settings are `config.json` in the config folder: JSON through `serde_json`
  (already in the library table), unknown keys ignored, missing keys defaulted, a
  malformed file reported and never overwritten. Telemetry defaults to off.
- Body-text layouts (E12.5) are template files in `layouts/` inside the config folder
  (`AppPaths::layouts_dir`), one `<id>.json` per layout: a `Layout` as pretty JSON through
  `serde_json`, so a user can edit it by hand. The folder roams with the settings on
  Windows. The six presets are written on first use and recorded in `layouts/.presets`;
  a preset file is never written again, so an edit or a deletion stays. Every call reads
  the folder again; a file that is not a layout is listed as broken with the reason. A
  save keeps the content it replaces in `layouts/.history/<id>/<number>-<unix ms>.json`,
  the newest 20 kept; these are app-owned copies, so the oldest are removed, not trashed.
- Logs use `tracing` with `tracing-appender`: one file per day, `fm.YYYY-MM-DD.log`,
  the newest 7 kept, written from a background thread.
- A panic writes a crash log next to them, synchronously from the panic hook:
  `crash-<unix ms>-<pid>.log`, the newest 10 kept (E9.4). The next start reports it once
  and renames it to `*.reported.log`. It is a local file only; sending a crash report
  is the separate, off-by-default telemetry flag.
- The library folder is only a suggestion. It is never created by the app, and it is
  never an OS font folder (`%LOCALAPPDATA%\Microsoft\Windows\Fonts`, `~/Library/Fonts`,
  `~/.local/share/fonts`), because a file placed there would be activated by the OS
  without the app tracking it (ADR 0004, rule 3).

## Consequences

- No `dirs`/`directories` crate: the env-var rules above are the whole implementation.
- Windows keeps the config roaming with the profile while the database and caches
  stay local to the machine.
