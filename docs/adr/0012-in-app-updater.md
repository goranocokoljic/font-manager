# 0012 — In-app updates from signed releases

Status: Accepted (2026-10-21)

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
  user clicks "Check for updates" (Settings → About) or, once at start, when
  the user turned on "Check automatically" (`Settings.check_updates_automatically`,
  off by default). Nothing installs until the user confirms; a major update
  (`fm_api::update::update_kind` with `semver`; before 1.0 a new minor
  version counts as major) asks a second time, and fm-api refuses an install
  request without `confirm_major`.
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
| Folders that must be writable | the program's folder | the folder holding `Font Manager.app`, and the bundle itself | the AppImage's folder |
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

## Consequences

- `createUpdaterArtifacts` is not in `tauri.conf.json`, so nightly,
  reproducible and local builds need no signing key.
- Losing the private key means the installed copies can no longer be
  updated in place; a new key ships with a release users install by hand.
- E18.3 (winget, Homebrew cask) uses the release installers, which update
  themselves; a cask can declare `auto_updates true`.
