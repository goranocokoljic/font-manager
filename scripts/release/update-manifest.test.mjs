// node --test scripts/release/update-manifest.test.mjs — the update manifest
// (latest.json) the release workflow publishes next to the installers (E18.2).

import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  MANIFEST,
  UPDATER_ARTIFACTS,
  artifactOf,
  buildManifest,
  checkManifest,
  findReleaseFiles,
  signedAssets,
  tagVersion,
} from './update-manifest.mjs';
import { BUNDLES } from './dev-build.mjs';
import { UPDATER_CONFIG, installerSteps } from './build.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, '..', '..');
const SCRIPT = join(HERE, 'update-manifest.mjs');
// A real minisign signature (the fm-app updater test fixture), base64 as Tauri writes it.
const SIG = readFileSync(
  join(ROOT, 'apps/desktop/src-tauri/tests/fixtures/updater/package.bin.sig'),
  'utf8',
).trim();
const BASE = 'https://github.com/goranocokoljic/font-manager/releases/download/v0.2.0';
const V = '0.2.0';

function tmp() {
  return mkdtempSync(join(tmpdir(), 'fm-update-manifest-'));
}

/** A fake Tauri bundle folder with the given `dir/file` entries. */
function bundle(files) {
  const root = tmp();
  for (const f of files) {
    mkdirSync(join(root, dirname(f)), { recursive: true });
    writeFileSync(join(root, f), f.endsWith('.sig') ? SIG : f);
  }
  return root;
}

const ALL_ASSETS = [
  `font-manager_${V}_windows_x64.exe`,
  `font-manager_${V}_macos_arm64.app.tar.gz`,
  `font-manager_${V}_linux_x64.AppImage`,
].map((name) => ({ name, signature: SIG }));

const manifest = () =>
  buildManifest({
    version: V,
    notes: '- Faster scans.',
    pubDate: '2026-11-01T10:00:00Z',
    baseUrl: BASE,
    assets: ALL_ASSETS,
  });

describe('buildManifest', () => {
  test('lists every platform with its URL and signature, and a default per OS and CPU', () => {
    const m = manifest();
    assert.equal(m.version, V);
    assert.equal(m.notes, '- Faster scans.');
    assert.equal(m.pub_date, '2026-11-01T10:00:00Z');
    assert.deepEqual(Object.keys(m.platforms).sort(), [
      'darwin-aarch64',
      'darwin-aarch64-app',
      'linux-x86_64',
      'linux-x86_64-appimage',
      'windows-x86_64',
      'windows-x86_64-nsis',
    ]);
    // Windows updates from the per-user .exe (NSIS) installer.
    assert.deepEqual(m.platforms['windows-x86_64'], m.platforms['windows-x86_64-nsis']);
    assert.equal(m.platforms['windows-x86_64'].url, `${BASE}/font-manager_${V}_windows_x64.exe`);
    assert.equal(
      m.platforms['darwin-aarch64'].url,
      `${BASE}/font-manager_${V}_macos_arm64.app.tar.gz`,
    );
    for (const p of Object.values(m.platforms)) assert.equal(p.signature, SIG);
  });

  test('refuses a file that is not an updater artifact of this version', () => {
    for (const name of [
      `font-manager_${V}_macos_arm64.dmg`,
      // The per-machine .msi never updates in place (rule 4).
      `font-manager_${V}_windows_x64.msi`,
      `font-manager_0.1.0_linux_x64.AppImage`,
      'latest.json',
    ]) {
      assert.throws(
        () => buildManifest({ version: V, pubDate: '2026-11-01T10:00:00Z', baseUrl: BASE, assets: [{ name, signature: SIG }] }),
        /not an updater artifact/,
      );
    }
  });
});

describe('checkManifest', () => {
  test('accepts what buildManifest writes', () => {
    checkManifest(manifest());
  });

  const broken = {
    'a version with a leading v': (m) => (m.version = 'v0.2.0'),
    'a date that is not RFC 3339': (m) => (m.pub_date = '1 Nov 2026'),
    'notes that are not text': (m) => (m.notes = null),
    'no platforms': (m) => (m.platforms = {}),
    'an unknown platform key': (m) => (m.platforms['beos-x86_64'] = m.platforms['linux-x86_64']),
    'an http URL': (m) => (m.platforms['linux-x86_64'].url = 'http://example.com/a.AppImage'),
    'a missing signature': (m) => delete m.platforms['windows-x86_64-nsis'].signature,
    'a signature that is not minisign': (m) =>
      (m.platforms['windows-x86_64-nsis'].signature = Buffer.from('hello').toString('base64')),
    'a signature not bound to a version': (m) =>
      (m.platforms['windows-x86_64-nsis'].signature = Buffer.from(
        'untrusted comment: x\nRUQ\ntrusted comment: timestamp:1\tfile:a\nabc',
      ).toString('base64')),
    'an installer key without its default': (m) => delete m.platforms['darwin-aarch64'],
  };
  for (const [what, breakIt] of Object.entries(broken)) {
    test(`refuses ${what}`, () => {
      const m = structuredClone(manifest());
      breakIt(m);
      assert.throws(() => checkManifest(m));
    });
  }
});

describe('artifactOf', () => {
  test('names the OS, CPU and installer kind of each updater artifact', () => {
    assert.deepEqual(artifactOf(`font-manager_${V}_windows_x64.exe`, V), {
      os: 'windows',
      arch: 'x64',
      installer: 'nsis',
      isDefault: true,
    });
    assert.equal(artifactOf(`font-manager_${V}_windows_x64.msi`, V), null);
    assert.equal(artifactOf(`font-manager_${V}_linux_arm64.AppImage`, V).arch, 'arm64');
    assert.equal(artifactOf(`font-manager_${V}_macos_arm64.dmg`, V), null);
  });
});

describe('findReleaseFiles', () => {
  test('gathers the installers and the signed updater artifacts of each OS', () => {
    const root = bundle([
      'nsis/Font Manager_0.2.0_x64-setup.exe',
      'nsis/Font Manager_0.2.0_x64-setup.exe.sig',
      'msi/Font Manager_0.2.0_x64_en-US.msi',
      'dmg/Font Manager_0.2.0_aarch64.dmg',
      'macos/Font Manager.app.tar.gz',
      'macos/Font Manager.app.tar.gz.sig',
      'appimage/font-manager_0.2.0_amd64.AppImage',
      'appimage/font-manager_0.2.0_amd64.AppImage.sig',
    ]);
    const names = (os, arch) => findReleaseFiles(root, os, arch, V).map((f) => f.name).sort();
    assert.deepEqual(names('windows', 'x64'), [
      `font-manager_${V}_windows_x64.exe`,
      `font-manager_${V}_windows_x64.exe.sig`,
      `font-manager_${V}_windows_x64.msi`,
    ]);
    assert.deepEqual(names('macos', 'arm64'), [
      `font-manager_${V}_macos_arm64.app.tar.gz`,
      `font-manager_${V}_macos_arm64.app.tar.gz.sig`,
      `font-manager_${V}_macos_arm64.dmg`,
    ]);
    assert.deepEqual(names('linux', 'x64'), [
      `font-manager_${V}_linux_x64.AppImage`,
      `font-manager_${V}_linux_x64.AppImage.sig`,
    ]);
  });

  test('fails when an updater artifact is not signed', () => {
    const root = bundle(['appimage/font-manager_0.2.0_amd64.AppImage']);
    assert.throws(() => findReleaseFiles(root, 'linux', 'x64', V), /no \.sig/);
  });

  test('every OS updates from a kind it builds', () => {
    assert.deepEqual(Object.keys(UPDATER_ARTIFACTS), Object.keys(BUNDLES));
  });
});

describe('tagVersion', () => {
  test('a release tag names the Cargo manifest version', () => {
    assert.equal(tagVersion('v0.2.0', '0.2.0'), '0.2.0');
    assert.throws(() => tagVersion('v0.3.0', '0.2.0'), /does not match/);
    assert.throws(() => tagVersion('0.2.0', '0.2.0'), /not a release tag/);
    assert.throws(() => tagVersion('nightly-0.2.0-1a2b3c4', '0.2.0'), /not a release tag/);
  });
});

describe('the release build', () => {
  test('--updater adds the updater config and the macOS app bundle; other builds do not', () => {
    const ctx = { root: '/r', cargoHome: '/c', epoch: '1' };
    for (const os of Object.keys(BUNDLES)) {
      const plain = installerSteps(os, ctx).at(-1).cmd;
      assert.ok(!plain.includes('--config'), `${os}: ${plain.join(' ')}`);
      const signed = installerSteps(os, ctx, { updater: true }).at(-1).cmd;
      assert.equal(signed[signed.indexOf('--config') + 1], UPDATER_CONFIG);
      const bundles = signed[signed.indexOf('--bundles') + 1].split(',');
      for (const { dir } of UPDATER_ARTIFACTS[os]) {
        assert.ok(bundles.includes(dir === 'macos' ? 'app' : dir), `${os}: ${bundles}`);
      }
    }
    const config = JSON.parse(readFileSync(join(ROOT, 'apps/desktop', UPDATER_CONFIG), 'utf8'));
    assert.equal(config.bundle.createUpdaterArtifacts, true);
  });

  test('the release workflow signs, collects, writes and checks latest.json, then publishes it', () => {
    const yml = readFileSync(join(ROOT, '.github/workflows/release.yml'), 'utf8');
    assert.match(yml, /tags: \['v\*'\]/);
    assert.match(yml, /TAURI_SIGNING_PRIVATE_KEY: \$\{\{ secrets\.TAURI_SIGNING_PRIVATE_KEY \}\}/);
    assert.match(yml, /build\.mjs installer --os \$\{\{ matrix\.name \}\} --updater/);
    assert.match(yml, /update-manifest\.mjs collect /);
    assert.match(yml, /update-manifest\.mjs manifest /);
    assert.match(yml, /update-manifest\.mjs check --file release-assets\/latest\.json/);
    assert.match(yml, /gh release create "\$TAG" release-assets\/\*/);
    // E31.2: the Sketch plugin zip is built once and attached with the installers.
    assert.match(yml, /pnpm --filter @fm\/sketch-plugin build/);
    assert.match(yml, /release-assets\/font-manager_\$\{\{ needs\.plan\.outputs\.version \}\}_sketch-plugin\.zip/);
    assert.match(yml, /needs: \[plan, installer, sketch-plugin\]/);
  });

  test('the endpoint in tauri.conf.json is the latest.json the workflow publishes', () => {
    const conf = JSON.parse(readFileSync(join(ROOT, 'apps/desktop/src-tauri/tauri.conf.json'), 'utf8'));
    const [endpoint] = conf.plugins.updater.endpoints;
    assert.match(endpoint, new RegExp(`/releases/latest/download/${MANIFEST.replace('.', '\\.')}$`));
  });
});

describe('the CLI', () => {
  test('manifest writes latest.json from the signed assets of a folder, and check accepts it', () => {
    const dir = tmp();
    for (const { name } of ALL_ASSETS) {
      writeFileSync(join(dir, name), name);
      writeFileSync(join(dir, `${name}.sig`), `${SIG}\n`);
    }
    writeFileSync(join(dir, `font-manager_${V}_macos_arm64.dmg`), 'dmg');
    // The Sketch plugin zip (E31.2) is attached unsigned: it is not an updater artifact.
    writeFileSync(join(dir, `font-manager_${V}_sketch-plugin.zip`), 'zip');
    writeFileSync(join(dir, 'SHA256SUMS'), 'sums');
    const notes = join(tmp(), 'notes.md');
    writeFileSync(notes, '- Faster scans.\n');
    assert.equal(signedAssets(dir).length, ALL_ASSETS.length);
    execFileSync(process.execPath, [
      SCRIPT, 'manifest', '--dir', dir, '--version', V, '--repo', 'goranocokoljic/font-manager',
      '--notes-file', notes, '--date', '2026-11-01T10:00:00Z',
    ]);
    assert.ok(readdirSync(dir).includes(MANIFEST));
    const written = JSON.parse(readFileSync(join(dir, MANIFEST), 'utf8'));
    assert.deepEqual(written, manifest());
    const out = execFileSync(process.execPath, [SCRIPT, 'check', '--file', join(dir, MANIFEST)], {
      encoding: 'utf8',
    });
    assert.match(out, /has the shape the updater reads/);
  });

  test('check fails on a manifest without signatures', () => {
    const file = join(tmp(), MANIFEST);
    const m = manifest();
    for (const p of Object.values(m.platforms)) p.signature = '';
    writeFileSync(file, JSON.stringify(m));
    assert.throws(() =>
      execFileSync(process.execPath, [SCRIPT, 'check', '--file', file], { stdio: 'pipe' }),
    );
  });
});
