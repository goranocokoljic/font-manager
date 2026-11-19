# 0012 — In-app updates from signed releases

Status: Accepted (2026-10-21); amended 2026-11-19 (E55.7: the daily check, the release address;
E55.9: install when the user quits)

## Context

Users of the release installers need a way to get fixes without going back
to the download page. Rule 7 forbids a network call without a user action.
An update replaces the program, so it must come only from the project's own
releases. Session fonts end with the app (ADR 0008), and an install restarts
it. Flatpak and package managers already install updates for their copies.

## Decision

- **Tauri's updater** (`tauri-plugin-updater`, in the spec's library table
  for `fm-app`), with `default-features = false` and `native-tls`: the
  default `rustls` features bring `webpki-root-certs` (CDLA-Permissive-2.0,
  not allowed by `deny.toml`), and the `zip` feature brings `zip` 4.x with
  open advisories. `zip` is only for v1-style zipped Windows updates; the
  v2 artifacts below do not need it. Linux builds need `libssl-dev`.
- **Signed releases only.** The minisign public key ships as
  `plugins.updater.pubkey` in `tauri.conf.json`; the private key and its
  password are the GitHub secrets `TAURI_SIGNING_PRIVATE_KEY` and
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` of the release workflow, never in the
  repository. `requireSignedVersion` is on: a package installs only when its
  signature matches the key and was made for the version the manifest
  announces, so a tampered manifest cannot pair a new version number with an
  older package. A refused package gives a user-facing error and nothing is
  installed.
- **Only after a user action.** The app asks the release server when the
  user clicks "Check for updates" (Settings → About) or, at start and once a
  day, when the user turned on "Check automatically"
  (`Settings.check_updates_automatically`, off by default; Goran,
  18 Nov 2026). Nothing installs until the user confirms, with one
  exception the user turns on ("Install updates when I quit", below); a major update
  (`fm_api::update::update_kind` with `semver`; before 1.0 a new minor
  version counts as major) asks a second time, and fm-api refuses an install
  request without `confirm_major`.
- **The automatic check** (E55.7) runs in the shell, not in the UI, so it
  keeps running while the app waits in the tray. `fm_api::update::checks_automatically`
  decides whether it runs (the setting is on and this copy updates itself);
  `fm_api::update::automatic_check_wait` decides when the next one is due
  (24 hours after the last one started). The timer (`fm-app`
  `auto_update`) is one thread parked between checks; it runs only while
  automatic checks are on and ends when they are turned off, so then no
  timer runs and nothing is asked. The time of the last check is kept for
  the running app only: every start checks. A check that fails (offline, the
  server down) is silent and counts as a check: the next one is at the next
  start or 24 hours later, never sooner. A found update is announced with
  `Event::UpdateAvailable` and the tray item "Update to <version>…", which
  opens the window at the update dialog; an automatic check never opens,
  raises or focuses a window. `UpdateSupport.release_host` names the host of
  the configured endpoint, so no UI text hard-codes it.
- **Install when the user quits** (E55.9; Goran, 18 Nov 2026): the one
  exception to "nothing installs until the user confirms", and the user
  turns it on. `Settings.install_updates_on_quit`, off by default
  (`#[serde(default)]`), has an effect only while automatic checks run
  (`fm_api::update::installs_on_quit`), and fm-api refuses to turn it on
  where this copy does not update itself (`check_settings_update`; without
  a shell, as in the CLI's direct link, it may only be turned off). The
  rules:
  - Only a **patch or minor** update an **automatic** check found
    (`AvailableUpdate::installs_on_quit`). A major one (before 1.0 a minor
    one too, as `update_kind` says) is only announced and asks twice.
  - It is **downloaded in the background** into the app's cache folder
    (`<cache>/updates`, `fm_core::update::on_quit::QuitInstall`) and verified
    exactly as a confirmed install is: the download checks the signature and
    the signed version, and the install checks them again on the kept bytes.
    A refused package is deleted, reported once (`Event::UpdateNotice`, and
    the next start), and nothing is installed; that version is not
    downloaded again in this run. The folder probe of `update_folders` runs
    before the download and again before the install; admin rights are never
    asked for (rule 4).
  - It **installs only when the user quits**: Quit in the app menu (the
    shell's own item, which replaces Tauri's predefined one) or in the tray,
    or the last window closed (`RunEvent::ExitRequested` without a code:
    "keep in tray" off). `RunEvent::Exit` runs the quit path first, then the
    install, once (`fm_core::update::on_quit::exit_step`). The app does not
    start again after this install: the Windows installer gets no `/R`
    (`restart_after_install(false)`), and macOS and Linux never restart by
    themselves. Nothing installs while the app runs.
  - **A logout or a shutdown never installs**; the package waits for the
    next quit, since an installer cut off by the session end could leave a
    half-replaced app. Windows: the session end (`WM_ENDSESSION`) ends the
    process; no window closes and no menu item runs, so the quit is not
    marked as the user's. macOS: the logout asks the app to terminate
    (`applicationWillTerminate`), not through the menu item, so it is not
    the user's quit either; the Dock's Quit counts the same way. Linux: the
    session sends `SIGTERM`; the app ends without `RunEvent::Exit`.
  - **The next start says once** what happened: before the installer runs
    the app records the versions (`<data>/update-install.json`); the next
    start compares them with the version that started and keeps a notice
    (`fm_api::update::UpdateNotice`) the UI reads once
    (`take_update_notice`): "Updated to <version>" with a link to the
    release page of that version, or that the update could not be installed,
    with a button that checks again and opens the update dialog. The quit
    path ran once before the install, so "reactivate on launch" keeps the
    real session in both cases.
- **The normal quit first.** Before the installer runs, the shell runs the
  quit path every quit uses (`AppState::run_quit_path`: session fonts are
  deregistered and remembered for "reactivate on launch"). It runs once: the
  exit that follows the install does not record an empty session over the
  real one. When the install fails the app goes on running as before: the
  session fonts the quit path ended are activated again and the quit path
  runs again at the next quit, so "reactivate on launch" keeps the real
  session.
- **Where the app updates itself** (`fm_platform::updater::distribution`):

| | Windows | macOS | Linux |
|-|---------|-------|-------|
| Updates itself | the .exe (NSIS) installer, per user | the app (from the .dmg) | the AppImage (`APPIMAGE` set) |
| Update artifact | the .exe, run in passive mode | `.app.tar.gz` | the AppImage |
| Folders that must be writable | the program's folder | the folder holding `Font Fu.app`, and the bundle itself | the AppImage's folder |
| Hidden, with a platform note | the .msi (per machine, Program Files); any copy in a folder the user cannot write; a build with `FM_PACKAGED_BY` | the app where the account cannot change its folder or the bundle (a standard account and `/Applications`; a bundle owned by root or another account); a build with `FM_PACKAGED_BY` | an AppImage in a read-only folder; Flatpak; a system package (.deb, .rpm); a build with `FM_PACKAGED_BY` |

  **Never admin rights (rule 4).** Tauri's updater would elevate in two
  places: `msiexec` for the per-machine .msi, and an AppleScript
  "with administrator privileges" prompt when the app bundle cannot be
  swapped. So the app offers updates only when this user can create a file in
  every folder the update writes into (`fm_platform::updater::update_folders`,
  `writable_dir`: a probe file created and removed at once; no FFI crate for
  `access(2)` is in the library table), and asks again before every install.
  On macOS that is the bundle too: swapping it moves it to another folder,
  which needs write access to the bundle itself. The
  .msi has no entry in `latest.json`. A package manager that builds the app
  sets `FM_PACKAGED_BY=<its name>` at build time. A system package is not
  updated in place either.
- **The manifest.** The release workflow (`.github/workflows/release.yml`,
  on a `v*` tag) builds with `scripts/release/build.mjs installer --updater`
  (adds `tauri.updater.conf.json`: `createUpdaterArtifacts`), gathers the
  artifacts and their `.sig` files, and writes `latest.json`
  (`scripts/release/update-manifest.mjs`: `<os>-<arch>-<installer>` and
  `<os>-<arch>` entries with URL and signature), published next to the
  installers. The app reads
  `https://github.com/goranocokoljic/font-manager/releases/latest/download/latest.json`;
  nightly pre-releases are never "latest".
- **The release address is GitHub releases for now** (Goran,
  18 Nov 2026); it may move to the project's website later, or stay.
  Moving it takes three things. Copies already installed keep asking the
  address they were built with (`plugins.updater.endpoints` is compiled in),
  so that address must keep answering, with a `latest.json` that leads them
  to a version that knows the new one. The new address goes in front of the
  endpoint list with GitHub kept after it: the updater asks the next
  address only when one gives a non-2xx answer, so GitHub stays the fallback.
  The signing key does not change: the packages are checked against the
  same public key wherever they come from.

## Consequences

- `createUpdaterArtifacts` is not in `tauri.conf.json`, so nightly,
  reproducible and local builds need no signing key.
- Losing the private key means the installed copies can no longer be
  updated in place; a new key ships with a release users install by hand.
- E18.3 (winget, Homebrew cask) uses the release installers, which update
  themselves; a cask can declare `auto_updates true`.
