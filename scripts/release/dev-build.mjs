#!/usr/bin/env node
// Dev builds (E9.3): the nightly workflow (.github/workflows/nightly.yml) calls this to
// name a build and to gather the unsigned installers Tauri wrote.
//
//   node scripts/release/dev-build.mjs version --sha <commit>
//       prints `version=…`, `label=…`, `tag=…` lines (append them to $GITHUB_OUTPUT)
//   node scripts/release/dev-build.mjs collect --os <windows|macos|linux> --arch <x64|arm64>
//       --sha <commit> [--bundle target/release/bundle] [--out nightly-assets]
//       copies the installers to <out>/ under their release names, prints the paths
//
// The version is the `[workspace.package] version` of the root Cargo.toml plus the short
// commit hash. The app itself keeps the plain manifest version: the WiX (.msi) bundler
// accepts only numeric versions.

import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');

/** The installers each OS ships, as `bundle/<dir>/*<ext>` (Tauri's output layout). */
export const BUNDLES = {
  windows: [
    { dir: 'msi', ext: '.msi' },
    { dir: 'nsis', ext: '-setup.exe', as: '.exe' },
  ],
  macos: [{ dir: 'dmg', ext: '.dmg' }],
  linux: [{ dir: 'appimage', ext: '.AppImage' }],
};

/** The `version` of `[workspace.package]` in a Cargo.toml text. */
export function manifestVersion(cargoToml) {
  let section = '';
  for (const raw of cargoToml.split(/\r?\n/)) {
    const line = raw.replace(/#.*$/, '').trim();
    const header = line.match(/^\[([^\]]+)\]$/);
    if (header) {
      section = header[1].trim();
      continue;
    }
    const m = line.match(/^version\s*=\s*"([^"]+)"$/);
    if (m && section === 'workspace.package') {
      if (!/^\d+\.\d+\.\d+$/.test(m[1])) throw new Error(`not a plain x.y.z version: ${m[1]}`);
      return m[1];
    }
  }
  throw new Error('no version in [workspace.package] of Cargo.toml');
}

/** The 7-character short hash of a full or short commit id. */
export function shortSha(sha) {
  const s = String(sha ?? '')
    .trim()
    .toLowerCase();
  if (!/^[0-9a-f]{7,40}$/.test(s)) throw new Error(`not a commit hash: ${sha}`);
  return s.slice(0, 7);
}

/**
 * The names of one dev build: `label` is semver with the hash as build metadata
 * (0.1.0+1a2b3c4), `version` is the same with a dash for file names (GitHub rewrites `+`
 * in asset names), `tag` is the git tag of its nightly release.
 */
export function devBuild(version, sha) {
  const h = shortSha(sha);
  return { version: `${version}-${h}`, label: `${version}+${h}`, tag: `nightly-${version}-${h}` };
}

/** The release name of one installer: font-manager_0.1.0-1a2b3c4_windows_x64.msi. */
export function assetName(build, os, arch, ext) {
  if (!BUNDLES[os]) throw new Error(`unknown os: ${os}`);
  if (!/^(x64|arm64)$/.test(arch)) throw new Error(`unknown arch: ${arch}`);
  return `font-manager_${build.version}_${os}_${arch}${ext}`;
}

/**
 * The installers for `os` under `bundleDir`, as `{ from, name }`. Throws when one
 * expected kind is missing or ambiguous — a nightly without its installer is a failure.
 */
export function findInstallers(bundleDir, os, arch, build) {
  const kinds = BUNDLES[os];
  if (!kinds) throw new Error(`unknown os: ${os}`);
  return kinds.map(({ dir, ext, as }) => {
    const d = join(bundleDir, dir);
    const hits = existsSync(d) ? readdirSync(d).filter((f) => f.endsWith(ext)) : [];
    if (hits.length !== 1) {
      throw new Error(`expected one *${ext} in ${d}, found ${hits.length}`);
    }
    return { from: join(d, hits[0]), name: assetName(build, os, arch, as ?? ext) };
  });
}

function args(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    if (!argv[i].startsWith('--')) throw new Error(`unexpected argument: ${argv[i]}`);
    out[argv[i].slice(2)] = argv[++i];
  }
  return out;
}

function main([cmd, ...rest]) {
  const a = args(rest);
  const version = manifestVersion(readFileSync(join(ROOT, 'Cargo.toml'), 'utf8'));
  const build = devBuild(version, a.sha);
  if (cmd === 'version') {
    console.log(`version=${build.version}\nlabel=${build.label}\ntag=${build.tag}`);
  } else if (cmd === 'collect') {
    const bundle = a.bundle ?? join(ROOT, 'target', 'release', 'bundle');
    const out = a.out ?? join(ROOT, 'nightly-assets');
    const found = findInstallers(bundle, a.os, a.arch, build);
    mkdirSync(out, { recursive: true });
    for (const { from, name } of found) {
      copyFileSync(from, join(out, name));
      console.log(join(out, name));
    }
  } else {
    throw new Error('usage: dev-build.mjs version|collect --sha <commit> [...]');
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    main(process.argv.slice(2));
  } catch (e) {
    console.error(`dev-build: ${e.message}`);
    process.exit(1);
  }
}
