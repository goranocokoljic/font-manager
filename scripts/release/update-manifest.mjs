#!/usr/bin/env node
// The update manifest of a release (E18.2, ADR 0012). The release workflow
// (.github/workflows/release.yml) calls this to gather the signed updater artifacts
// Tauri wrote and to write `latest.json`, which the in-app updater reads from
// https://github.com/<repo>/releases/latest/download/latest.json.
//
//   node scripts/release/update-manifest.mjs version --tag v0.2.0
//       prints `version=0.2.0`; fails unless the tag names the Cargo manifest version
//   node scripts/release/update-manifest.mjs collect --os <windows|macos|linux>
//       --arch <x64|arm64> --version <x.y.z> [--bundle target/release/bundle] [--out release-assets]
//       copies the installers and the updater artifacts, each with its `.sig`, to <out>/
//   node scripts/release/update-manifest.mjs manifest --dir release-assets --version <x.y.z>
//       --repo <owner/name> [--notes-file <file>] [--date <RFC 3339>]
//       writes <dir>/latest.json from the `.sig` files in <dir> and checks its shape
//   node scripts/release/update-manifest.mjs check --file latest.json
//       fails unless the file has the shape the updater reads

import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { BUNDLES, assetName, manifestVersion } from './dev-build.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');

/** The file name of the update manifest. */
export const MANIFEST = 'latest.json';

/**
 * The files of one OS the updater installs, as `bundle/<dir>/*<ext>` with a `.sig`
 * next to each, and the installer kind Tauri's updater names it by. The first one is
 * also the default for its `<os>-<arch>` key. Windows and Linux update from the
 * installers themselves; macOS from the app bundle as `.app.tar.gz` (the .dmg is for
 * a first install only). The .msi is not here: it installs per machine, so updating
 * it would run msiexec with admin rights (rule 4); an .msi copy shows a platform note
 * instead (ADR 0012).
 */
export const UPDATER_ARTIFACTS = {
  windows: [{ dir: 'nsis', ext: '-setup.exe', as: '.exe', installer: 'nsis' }],
  macos: [{ dir: 'macos', ext: '.app.tar.gz', installer: 'app' }],
  linux: [{ dir: 'appimage', ext: '.AppImage', installer: 'appimage' }],
};

/** The OS and CPU names Tauri's updater looks up (`{os}-{arch}[-{installer}]`). */
export const TARGET_OS = { windows: 'windows', macos: 'darwin', linux: 'linux' };
export const TARGET_ARCH = { x64: 'x86_64', arm64: 'aarch64' };

const SEMVER = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/;
const RFC3339 = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/;
const KEY = /^(windows|darwin|linux)-(x86_64|aarch64)(-(nsis|msi|app|appimage))?$/;

/** The version a release tag names (`v0.2.0` → `0.2.0`); it must be the manifest's. */
export function tagVersion(tag, manifest) {
  const m = /^v(\d+\.\d+\.\d+)$/.exec(String(tag ?? '').trim());
  if (!m) throw new Error(`not a release tag (vX.Y.Z): ${tag}`);
  if (m[1] !== manifest) throw new Error(`tag ${tag} does not match the Cargo version ${manifest}`);
  return m[1];
}

/**
 * The release files of one OS in `bundleDir`: every installer (BUNDLES) and every
 * updater artifact with its signature, as `{ from, name }`. Throws when a kind is
 * missing, ambiguous, or unsigned: a release without a signed update is a failure.
 */
export function findReleaseFiles(bundleDir, os, arch, version) {
  if (!UPDATER_ARTIFACTS[os]) throw new Error(`unknown os: ${os}`);
  const build = { version };
  const one = (dir, ext) => {
    const d = join(bundleDir, dir);
    const hits = existsSync(d) ? readdirSync(d).filter((f) => f.endsWith(ext)) : [];
    if (hits.length !== 1) throw new Error(`expected one *${ext} in ${d}, found ${hits.length}`);
    return join(d, hits[0]);
  };
  const files = new Map();
  for (const { dir, ext, as } of BUNDLES[os]) {
    files.set(assetName(build, os, arch, as ?? ext), one(dir, ext));
  }
  for (const { dir, ext, as } of UPDATER_ARTIFACTS[os]) {
    const from = one(dir, ext);
    const name = assetName(build, os, arch, as ?? ext);
    if (!existsSync(`${from}.sig`)) throw new Error(`${from} has no .sig: the updater key was not set`);
    files.set(name, from);
    files.set(`${name}.sig`, `${from}.sig`);
  }
  return [...files].map(([name, from]) => ({ from, name }));
}

/** Which updater artifact a release asset name is: `{ os, arch, installer, isDefault }`. */
export function artifactOf(name, version) {
  const prefix = `font-manager_${version}_`;
  if (!name.startsWith(prefix)) return null;
  const m = /^(windows|macos|linux)_(x64|arm64)(.+)$/.exec(name.slice(prefix.length));
  if (!m) return null;
  const [, os, arch, ext] = m;
  const kinds = UPDATER_ARTIFACTS[os];
  const i = kinds.findIndex((k) => (k.as ?? k.ext) === ext);
  if (i < 0) return null;
  return { os, arch, installer: kinds[i].installer, isDefault: i === 0 };
}

/**
 * The update manifest for `version`: one entry per updater artifact
 * (`<os>-<arch>-<installer>`) and one per OS and CPU (`<os>-<arch>`, the first kind),
 * each with its download URL under `baseUrl` and the text of its `.sig` file.
 * `assets` are `{ name, signature }` of the artifacts (not the .sig files).
 */
export function buildManifest({ version, notes, pubDate, baseUrl, assets }) {
  const platforms = {};
  for (const { name, signature } of assets) {
    const a = artifactOf(name, version);
    if (!a) throw new Error(`not an updater artifact of ${version}: ${name}`);
    const entry = { signature: signature.trim(), url: `${baseUrl}/${encodeURIComponent(name)}` };
    const key = `${TARGET_OS[a.os]}-${TARGET_ARCH[a.arch]}`;
    platforms[`${key}-${a.installer}`] = entry;
    if (a.isDefault) platforms[key] = entry;
  }
  const manifest = { version, notes: notes ?? '', pub_date: pubDate, platforms };
  checkManifest(manifest);
  return manifest;
}

/**
 * Throws unless `m` has the shape Tauri's updater reads: a plain semver version, an
 * RFC 3339 date, notes as text, and for every platform an https URL and a minisign
 * signature (base64 of the `.sig` text); every `<os>-<arch>-<installer>` has its
 * `<os>-<arch>` default.
 */
export function checkManifest(m) {
  if (!m || typeof m !== 'object') throw new Error('the manifest is not an object');
  if (typeof m.version !== 'string' || !SEMVER.test(m.version)) {
    throw new Error(`version is not semver without a leading v: ${m.version}`);
  }
  if (typeof m.pub_date !== 'string' || !RFC3339.test(m.pub_date)) {
    throw new Error(`pub_date is not RFC 3339: ${m.pub_date}`);
  }
  if (typeof m.notes !== 'string') throw new Error('notes is not text');
  const keys = Object.keys(m.platforms ?? {});
  if (!keys.length) throw new Error('the manifest lists no platforms');
  for (const key of keys) {
    if (!KEY.test(key)) throw new Error(`unknown platform key: ${key}`);
    const { url, signature } = m.platforms[key] ?? {};
    let parsed;
    try {
      parsed = new URL(url);
    } catch {
      throw new Error(`${key}: url is not a URL: ${url}`);
    }
    if (parsed.protocol !== 'https:') throw new Error(`${key}: url is not https: ${url}`);
    const text = typeof signature === 'string' ? Buffer.from(signature, 'base64').toString('utf8') : '';
    if (!/^untrusted comment: /.test(text) || !/\ntrusted comment: [^\n]*version:/.test(text)) {
      throw new Error(`${key}: signature is not a minisign signature bound to a version`);
    }
    const base = key.split('-').slice(0, 2).join('-');
    if (!m.platforms[base]) throw new Error(`${key} has no ${base} default`);
  }
}

/** The assets of `dir` that have a `.sig` next to them, with the signature text. */
export function signedAssets(dir) {
  const names = readdirSync(dir);
  return names
    .filter((n) => n.endsWith('.sig') && names.includes(n.slice(0, -4)))
    .sort()
    .map((sig) => ({ name: sig.slice(0, -4), signature: readFileSync(join(dir, sig), 'utf8') }));
}

/** `--name value` pairs. */
function args(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    if (!argv[i].startsWith('--')) throw new Error(`unexpected argument: ${argv[i]}`);
    out[argv[i].slice(2)] = argv[++i];
  }
  return out;
}

function need(a, ...names) {
  for (const n of names) if (!a[n]) throw new Error(`missing --${n}`);
}

function main([cmd, ...rest]) {
  const a = args(rest);
  if (cmd === 'version') {
    need(a, 'tag');
    const manifest = manifestVersion(readFileSync(join(ROOT, 'Cargo.toml'), 'utf8'));
    console.log(`version=${tagVersion(a.tag, manifest)}`);
  } else if (cmd === 'collect') {
    need(a, 'os', 'arch', 'version');
    const bundle = a.bundle ?? join(ROOT, 'target', 'release', 'bundle');
    const out = a.out ?? join(ROOT, 'release-assets');
    const files = findReleaseFiles(bundle, a.os, a.arch, a.version);
    mkdirSync(out, { recursive: true });
    for (const { from, name } of files) {
      copyFileSync(from, join(out, name));
      console.log(join(out, name));
    }
  } else if (cmd === 'manifest') {
    need(a, 'dir', 'version', 'repo');
    const notes = a['notes-file'] ? readFileSync(a['notes-file'], 'utf8').trim() : '';
    const manifest = buildManifest({
      version: a.version,
      notes,
      pubDate: a.date ?? new Date().toISOString().replace(/\.\d+Z$/, 'Z'),
      baseUrl: `https://github.com/${a.repo}/releases/download/v${a.version}`,
      assets: signedAssets(a.dir),
    });
    writeFileSync(join(a.dir, MANIFEST), `${JSON.stringify(manifest, null, 2)}\n`);
    console.log(join(a.dir, MANIFEST));
  } else if (cmd === 'check') {
    need(a, 'file');
    checkManifest(JSON.parse(readFileSync(a.file, 'utf8')));
    console.log(`${a.file} has the shape the updater reads`);
  } else {
    throw new Error('usage: update-manifest.mjs version|collect|manifest|check [...]');
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    main(process.argv.slice(2));
  } catch (e) {
    console.error(`update-manifest: ${e.message}`);
    process.exit(1);
  }
}
