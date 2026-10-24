# Building Font Manager from source

This page tells you how to build the release installers yourself from a release tag, and
how to check that a download from the releases page is the one we built.

The commands on this page come from one script, `scripts/release/build.mjs`. The blocks
between `build.mjs` markers are generated from it; do not edit them by hand. After a
change to the script, run `node scripts/release/build.mjs write-doc`. The `reproducible`
workflow (`.github/workflows/reproducible.yml`) runs the same script on Windows, macOS and
Linux from a clean checkout, and fails when this page and the script disagree.

## Pinned toolchain

<!-- build.mjs:toolchain -->
- Rust 1.99.0 with rustfmt and clippy — `rust-toolchain.toml`; rustup installs it on the first `cargo` call
- Node.js >=22 — `engines.node` in `package.json` (CI uses Node 22)
- pnpm 12.11.2 — `packageManager` in `package.json`; `corepack enable` picks it up
- Tauri CLI 2.12.3 — `@tauri-apps/cli` in `apps/desktop/package.json`, installed by `pnpm install`
<!-- /build.mjs:toolchain -->

Rust dependencies are pinned by `Cargo.lock` (every cargo call uses `--locked`). Node
dependencies are pinned by `pnpm-lock.yaml` (`pnpm install --frozen-lockfile`). A build
fails instead of resolving a different version.

## System packages

<!-- build.mjs:packages -->
**windows**

- Visual Studio 2022 Build Tools with the "Desktop development with C++" workload (MSVC, Windows SDK)
- WebView2 Runtime (preinstalled on Windows 10 21H2+ and Windows 11)
- The .msi bundler downloads WiX 3 and the .exe bundler downloads NSIS on first use

**macos**

- Xcode Command Line Tools (`xcode-select --install`)

**linux**

- Debian 12 / Ubuntu 22.04 or newer (WebKitGTK 4.1)

```sh
sudo apt-get install -y --no-install-recommends build-essential pkg-config file patchelf libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libfontconfig1-dev fontconfig libssl-dev
```
<!-- /build.mjs:packages -->

`node scripts/release/build.mjs packages --os linux` prints the Linux list. CI installs
exactly that list.

## Build the installers from a tag

Check out the tag, then run the script for your OS:

```sh
git clone https://github.com/goranocokoljic/font-manager.git
cd font-manager
git checkout v0.1.0            # the release tag
node scripts/release/build.mjs installer --os linux   # or windows, macos
```

The installers are in `target/release/bundle/` and `fm-cli` is in `target/release/`. The
script runs these commands. `$PWD` is the checkout, `$CARGO_HOME` is your cargo home
(`~/.cargo` by default), and the epoch is the commit time of the tag:

<!-- build.mjs:commands -->
**windows** (.msi, .exe): `node scripts/release/build.mjs installer --os windows` runs

```sh
pnpm install --frozen-lockfile
SOURCE_DATE_EPOCH="$(git log -1 --format=%ct)" CARGO_INCREMENTAL=0 RUSTFLAGS="--remap-path-prefix=$PWD=/build --remap-path-prefix=$CARGO_HOME=/cargo" cargo build --release --locked -p fm-cli
SOURCE_DATE_EPOCH="$(git log -1 --format=%ct)" CARGO_INCREMENTAL=0 RUSTFLAGS="--remap-path-prefix=$PWD=/build --remap-path-prefix=$CARGO_HOME=/cargo" pnpm --filter @fm/desktop exec tauri build --ci --bundles msi,nsis -- --locked
```

**macos** (.dmg): `node scripts/release/build.mjs installer --os macos` runs

```sh
pnpm install --frozen-lockfile
SOURCE_DATE_EPOCH="$(git log -1 --format=%ct)" CARGO_INCREMENTAL=0 RUSTFLAGS="--remap-path-prefix=$PWD=/build --remap-path-prefix=$CARGO_HOME=/cargo" cargo build --release --locked -p fm-cli
SOURCE_DATE_EPOCH="$(git log -1 --format=%ct)" CARGO_INCREMENTAL=0 RUSTFLAGS="--remap-path-prefix=$PWD=/build --remap-path-prefix=$CARGO_HOME=/cargo" pnpm --filter @fm/desktop exec tauri build --ci --bundles dmg -- --locked
```

**linux** (.AppImage): `node scripts/release/build.mjs installer --os linux` runs

```sh
pnpm install --frozen-lockfile
SOURCE_DATE_EPOCH="$(git log -1 --format=%ct)" CARGO_INCREMENTAL=0 RUSTFLAGS="--remap-path-prefix=$PWD=/build --remap-path-prefix=$CARGO_HOME=/cargo" cargo build --release --locked -p fm-cli
SOURCE_DATE_EPOCH="$(git log -1 --format=%ct)" CARGO_INCREMENTAL=0 RUSTFLAGS="--remap-path-prefix=$PWD=/build --remap-path-prefix=$CARGO_HOME=/cargo" APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=true pnpm --filter @fm/desktop exec tauri build --ci --bundles appimage -- --locked
```

**fm-cli alone**: `node scripts/release/build.mjs cli` runs

```sh
SOURCE_DATE_EPOCH="$(git log -1 --format=%ct)" CARGO_INCREMENTAL=0 RUSTFLAGS="--remap-path-prefix=$PWD=/build --remap-path-prefix=$CARGO_HOME=/cargo" cargo build --release --locked -p fm-cli
```
<!-- /build.mjs:commands -->

The script passes `RUSTFLAGS` as `CARGO_ENCODED_RUSTFLAGS`, so a checkout path with
spaces works.

The release workflow (`.github/workflows/release.yml`) runs the same script with
`--updater`. That also writes the updater artifacts and signs them with the updater key
(the secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; the
public half is in `tauri.conf.json`). Your own build does not need the key. A copy you
build yourself still checks for updates; a package manager that builds Font Manager sets
`FM_PACKAGED_BY=<its name>` at build time to turn that off (ADR 0012).

## What is reproducible, and what is not

**`fm-cli` on Linux is bit-for-bit reproducible.** Two builds of the same commit give the
same SHA256, also from two different folders. The `reproducible / fm-cli-linux` CI job
proves it on every run. Two settings make this possible:

- `SOURCE_DATE_EPOCH` is the commit time, so no build step writes the current time.
- `--remap-path-prefix` replaces the checkout folder and the cargo home in panic messages
  and debug info, so the folder you build in does not change the binary.

To check a release yourself, run `node scripts/release/build.mjs cli` on Linux at the
release tag. The last line is the SHA256 of `target/release/fm`. Compare it with the
`fm` binary inside the release.

**These outputs are not bit-for-bit reproducible, and why:**

- **Signed installers** (.msi, .exe, .dmg, the signed .app inside). A signature contains
  the signing time from a timestamp server and our private certificate. Nobody else can
  make the same bytes. Remove the signature, or compare the unsigned files inside.
- **Bundler timestamps and IDs.** The .msi database stores its creation time and a random
  package code. NSIS writes the build time into the .exe. A .dmg is a disk image whose
  file system records creation and change times. An AppImage packs a squashfs image with
  file times, and `linuxdeploy` copies shared libraries from the build machine, so a
  different distribution gives different bytes.
- **Windows and macOS binaries.** The MSVC linker writes a time stamp into each PE file,
  and macOS binaries carry an ad-hoc or Developer ID signature. CI checks reproducibility
  on Linux only.

The app code inside an installer is built by the same `cargo build --locked` with the same
settings as `fm-cli`. You can compare the binaries inside, not the installer files.

## Verify a download

Every release has a `SHA256SUMS` file. It has one line per file: the SHA256 hash, two
spaces, the file name. Download it into the folder that holds the installer.

**Linux**

```sh
sha256sum -c --ignore-missing SHA256SUMS
```

The line for your file must say `OK`. When the release has `SHA256SUMS.asc`, check the
signature of the checksum file with the project's public key (published with the first
signed release):

```sh
gpg --verify SHA256SUMS.asc SHA256SUMS
```

AppImages have no OS-level signature. The checksum file and its signature are what you
check.

**macOS**

```sh
shasum -a 256 -c --ignore-missing SHA256SUMS
```

Then check the signature and the notarization. Open the .dmg, then:

```sh
codesign --verify --deep --strict --verbose=2 "/Volumes/Font Manager/Font Manager.app"
spctl --assess --type execute --verbose "/Volumes/Font Manager/Font Manager.app"
xcrun stapler validate "/Volumes/Font Manager/Font Manager.app"
```

`spctl` must say `accepted` and `source=Notarized Developer ID`.

**Windows (PowerShell)**

```powershell
$file = 'font-manager_0.1.0_windows_x64.msi'
$hash = (Get-FileHash $file -Algorithm SHA256).Hash.ToLower()
Select-String -Path SHA256SUMS -SimpleMatch "$hash  $file"
```

The command must print the line for your file. No output means the file is not the one
we built. Then check the Authenticode signature:

```powershell
Get-AuthenticodeSignature $file | Format-List Status, SignerCertificate
```

`Status` must be `Valid`, and the signer must be the project.

**Nightly builds** are not signed (see the nightly workflow). Their `SHA256SUMS` lets
you check the download, but there is no signature to check. Signing comes with
E18.1.
