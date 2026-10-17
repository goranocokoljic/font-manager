# 0008 — Font activation per OS

Status: Accepted (2026-10-18)

## Context

Activation makes a font usable in other apps without copying or installing it (ADR
0004, rules 2–4). The spec planned these mechanisms:

| Mode | Windows | macOS | Linux |
|------|---------|-------|-------|
| Session | `AddFontResourceEx` (no `FR_PRIVATE`) + `WM_FONTCHANGE` | `CTFontManagerRegisterFontsForURL`, session scope | symlink in `~/.local/share/fonts/<app>/session/` + fontconfig refresh |
| Permanent | per-user value under `HKCU\…\Windows NT\CurrentVersion\Fonts` | same call, user scope | symlink in `~/.local/share/fonts/<app>/` |

Story E6.1 tested each one with throwaway programs (`spikes/e6.1-activation/`), run by
the `spike-activation` workflow on GitHub runners and once on a Windows 11 PC. Every
register, look and remove runs in its own process, so the results show what a
**second app** sees. "Running app" = a process started before the change that keeps
asking (GDI/DirectWrite on Windows, CoreText on macOS, `FcInitBringUptoDate` on Linux).

Test fonts: Diplomata (session) and Emblema One (permanent) from `fixtures/fonts/`;
no runner image ships them, so "before" is a real negative.

## Results

**Windows** — Server 2022 build 20348 (the Windows 10 21H2 code base), Server 2025
build 26100 (Windows 11 24H2 code base) and Windows 11 build 26300 gave the same
results. GDI = classic Win32 apps; DirectWrite = WebView2, browsers, Office, most
current apps.

| Step | GDI | DirectWrite |
|------|-----|-------------|
| `AddFontResourceEx`, new process | yes | **no** |
| `AddFontResourceEx`, running app | yes, ~1.1 s | **no** |
| `AddFontResourceEx`, after the registering process exits | yes | no |
| `RemoveFontResourceEx` from another process | gone | gone |
| HKCU value only (no other call), new process | yes | yes |
| HKCU value only, running app | yes, ~1.1 s | yes, ~1.1 s |
| HKCU value, then `RemoveFontResourceEx` | gone | still there |
| HKCU value deleted | gone | gone |

**macOS** 14.8, 15.7 and 26.6 gave the same results.

| Step | Result |
|------|--------|
| process scope, other process (control) | not seen |
| session scope, new process | yes |
| session scope, running app | yes, 0.3–0.6 s |
| session scope, after the registering process exits | **still there** |
| session scope, after unregister | gone |
| user scope, new process / running app | yes / 0.3–0.7 s |
| user scope, after the registering process exits | yes |
| user scope, after unregister | gone |

**Linux** — Ubuntu 22.04 (fontconfig 2.13.1) and 24.04 (fontconfig 2.15.0) gave the
same results.

| Step | Result |
|------|--------|
| symlink, no `fc-cache`, new process | yes |
| symlink + `fc-cache`, new process | yes |
| symlink, running app | yes, ~30 s (fontconfig's rescan interval) |
| link removed (with or without `fc-cache`), new process | gone |
| link target deleted + `fc-cache` (dir unchanged) | **still listed** |
| dangling link + `fc-cache -f` | gone |

## Decision

`fm-platform` implements `FontActivator` per OS as follows.

- **Windows: one mechanism, the per-user registry, for both modes.**
  `AddFontResourceEx` is not used: DirectWrite apps never see it, so it fails the
  "every app can use it" goal. Activation writes a value under
  `HKCU\Software\Microsoft\Windows NT\CurrentVersion\Fonts` whose data is the file's
  full path in place, then broadcasts `WM_FONTCHANGE`. Deactivation deletes exactly the
  value the app wrote and broadcasts again. Value names are recorded in the database; a
  name already taken by a value the app did not write gets a numeric suffix, so the app
  never overwrites or deletes a foreign value (rule 3). No admin rights.
  - Session mode = the same value, marked `session` in the database and deleted on
    quit. A crash leaves it in place (the registry persists), so startup deletes every
    session value left from an earlier run before doing anything else.
  - Supported from Windows 10 1809 (build 17763), where per-user fonts arrived; the
    runner results on build 20348 cover the Windows 10 code base. Earlier builds are
    out of Microsoft support and out of scope.
- **macOS: CoreText as planned.** `CTFontManagerRegisterFontsForURL` with
  `.session` for session mode and `.persistent` (= user scope) for permanent mode;
  `CTFontManagerUnregisterFontsForURL` with the same scope to deactivate. A session
  registration outlives the app until logout, so quit unregisters it explicitly, and
  startup unregisters session fonts the database still lists (a failure there because
  the user logged out since is expected and ignored).
- **Linux: symlinks as planned.** A symlink to the file in place under
  `$XDG_DATA_HOME/fonts/font-manager/` (permanent) or `…/font-manager/session/`
  (session), then `fc-cache` on that folder so other apps start fast. Quit and the next
  start after a crash remove the session links. The app creates both folders at first
  start, before any activation, because a running fontconfig app only notices changes
  in folders that existed when it loaded.
- **All OS: the path is the identity.** Every mechanism points at the file where it
  is. When the watcher sees an active file move, the activator deactivates the old path
  and activates the new one; when the file is deleted, it deactivates. On Linux a
  removed target also needs `fc-cache -f` on the app folder, because the cache is keyed
  on folder times and keeps listing a dangling link otherwise.

## Consequences

- The spec's Windows session row changes from `AddFontResourceEx` to the HKCU value
  plus cleanup on quit and at startup.
- A Windows or macOS session font is visible to new and running apps within about a
  second. On Linux new processes see it at once; a running app sees it after up to 30 s
  if it re-checks fontconfig (not tested per toolkit), and apps that never re-check
  need a restart. The UI says this on Linux.
- Session cleanup at startup is mandatory on every OS; the activation state table
  (rule 3) is the list of what to clean.
- The platform integration tests of E6.2+ reuse the spike's approach: activate, then
  check from a separate process (GDI and DirectWrite on Windows).
- Not covered by the spike, to check in E6.2+: sandboxed apps (Flatpak/Snap on Linux
  see `~/.local/share/fonts` but may not reach a symlink target outside it; Mac App
  Store apps), fonts on network or removable drives, and Windows 10 desktop editions
  older than build 20348 (no runner exists).
