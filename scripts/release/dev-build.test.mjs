// node --test scripts/release/dev-build.test.mjs — naming and gathering of the nightly
// dev builds (E9.3).

import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { BUNDLES, assetName, devBuild, findInstallers, manifestVersion, shortSha } from './dev-build.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const SCRIPT = join(HERE, 'dev-build.mjs');
const SHA = '1a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d';

function tmp() {
  return mkdtempSync(join(tmpdir(), 'fm-dev-build-'));
}

/** A fake Tauri bundle folder with the given `dir/file` entries. */
function bundle(files) {
  const root = tmp();
  for (const f of files) {
    mkdirSync(join(root, dirname(f)), { recursive: true });
    writeFileSync(join(root, f), f);
  }
  return root;
}

describe('BUNDLES', () => {
  test('ships .msi and .exe on Windows, .dmg on macOS, AppImage on Linux', () => {
    const exts = Object.fromEntries(
      Object.entries(BUNDLES).map(([os, kinds]) => [os, kinds.map((k) => k.as ?? k.ext)]),
    );
    assert.deepEqual(exts, { windows: ['.msi', '.exe'], macos: ['.dmg'], linux: ['.AppImage'] });
  });

  test('matches the bundles the nightly workflow builds per OS', () => {
    const yml = readFileSync(join(HERE, '..', '..', '.github', 'workflows', 'nightly.yml'), 'utf8');
    const matrix = [...yml.matchAll(/- name: (\w+)\r?\n\s+os: \S+\r?\n\s+bundles: (\S+)/g)];
    const built = Object.fromEntries(matrix.map(([, os, b]) => [os, b.split(',')]));
    const wanted = Object.fromEntries(
      Object.entries(BUNDLES).map(([os, kinds]) => [os, kinds.map((k) => k.dir)]),
    );
    assert.deepEqual(built, wanted);
  });
});

describe('manifestVersion', () => {
  test('reads [workspace.package] version, not a dependency or package version', () => {
    const toml = [
      '[workspace]',
      'members = ["crates/*"]',
      'version = "9.9.9"',
      '',
      '[workspace.package]  # shared',
      'version = "0.4.2" # bumped by release.sh',
      'edition = "2024"',
      '',
      '[workspace.dependencies]',
      'serde = { version = "1" }',
    ].join('\r\n');
    assert.equal(manifestVersion(toml), '0.4.2');
  });

  test('matches the root Cargo.toml', () => {
    const toml = readFileSync(join(HERE, '..', '..', 'Cargo.toml'), 'utf8');
    assert.match(manifestVersion(toml), /^\d+\.\d+\.\d+$/);
  });

  test('fails without a workspace version or with a non-numeric one', () => {
    assert.throws(() => manifestVersion('[package]\nversion = "0.1.0"\n'), /no version/);
    assert.throws(
      () => manifestVersion('[workspace.package]\nversion = "0.1.0-beta"\n'),
      /plain x\.y\.z/,
    );
  });
});

describe('devBuild', () => {
  test('is the manifest version plus the short commit hash', () => {
    assert.deepEqual(devBuild('0.1.0', SHA), {
      version: '0.1.0-1a2b3c4',
      label: '0.1.0+1a2b3c4',
      tag: 'nightly-0.1.0-1a2b3c4',
    });
  });

  test('keeps a hash with leading zeros and lower-cases it', () => {
    assert.equal(shortSha('0123ABCDEF'), '0123abc');
    assert.equal(devBuild('1.2.3', '0000000').label, '1.2.3+0000000');
  });

  test('rejects a missing or malformed hash', () => {
    for (const bad of [undefined, '', 'abc', 'zzzzzzz', '1a2b3c4 ; rm -rf /']) {
      assert.throws(() => devBuild('0.1.0', bad), /not a commit hash/);
    }
  });
});

describe('assetName', () => {
  const build = devBuild('0.1.0', SHA);

  test('names every installer kind with version, OS and arch', () => {
    assert.equal(
      assetName(build, 'windows', 'x64', '.msi'),
      'font-manager_0.1.0-1a2b3c4_windows_x64.msi',
    );
    assert.equal(
      assetName(build, 'macos', 'arm64', '.dmg'),
      'font-manager_0.1.0-1a2b3c4_macos_arm64.dmg',
    );
    assert.equal(
      assetName(build, 'linux', 'x64', '.AppImage'),
      'font-manager_0.1.0-1a2b3c4_linux_x64.AppImage',
    );
  });

  test('rejects an unknown OS or arch', () => {
    assert.throws(() => assetName(build, 'freebsd', 'x64', '.pkg'), /unknown os/);
    assert.throws(() => assetName(build, 'linux', 'X64', '.AppImage'), /unknown arch/);
  });
});

describe('findInstallers', () => {
  const build = devBuild('0.1.0', SHA);

  test('finds the .msi and the NSIS .exe on Windows', () => {
    const root = bundle([
      'msi/Font Manager_0.1.0_x64_en-US.msi',
      'nsis/Font Manager_0.1.0_x64-setup.exe',
      'nsis/unrelated.nsi',
    ]);
    try {
      const found = findInstallers(root, 'windows', 'x64', build);
      assert.deepEqual(
        found.map((f) => f.name),
        ['font-manager_0.1.0-1a2b3c4_windows_x64.msi', 'font-manager_0.1.0-1a2b3c4_windows_x64.exe'],
      );
      assert.ok(found[1].from.endsWith('Font Manager_0.1.0_x64-setup.exe'));
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test('finds the .dmg on macOS and the AppImage on Linux', () => {
    const root = bundle(['dmg/Font Manager_0.1.0_aarch64.dmg', 'appimage/font-manager_0.1.0_amd64.AppImage']);
    try {
      assert.equal(findInstallers(root, 'macos', 'arm64', build)[0].name, 'font-manager_0.1.0-1a2b3c4_macos_arm64.dmg');
      assert.equal(findInstallers(root, 'linux', 'x64', build)[0].name, 'font-manager_0.1.0-1a2b3c4_linux_x64.AppImage');
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test('fails when an installer is missing', () => {
    const root = bundle(['msi/Font Manager_0.1.0_x64_en-US.msi']);
    try {
      assert.throws(() => findInstallers(root, 'windows', 'x64', build), /expected one \*-setup\.exe .* found 0/);
      assert.throws(() => findInstallers(join(root, 'nope'), 'linux', 'x64', build), /found 0/);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test('fails when a stale second installer would make the pick ambiguous', () => {
    const root = bundle(['dmg/Font Manager_0.0.9_aarch64.dmg', 'dmg/Font Manager_0.1.0_aarch64.dmg']);
    try {
      assert.throws(() => findInstallers(root, 'macos', 'arm64', build), /found 2/);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});

describe('CLI', () => {
  test('version prints GITHUB_OUTPUT lines', () => {
    const out = execFileSync(process.execPath, [SCRIPT, 'version', '--sha', SHA], { encoding: 'utf8' });
    assert.match(out, /^version=\d+\.\d+\.\d+-1a2b3c4\nlabel=\d+\.\d+\.\d+\+1a2b3c4\ntag=nightly-\d+\.\d+\.\d+-1a2b3c4\n$/);
  });

  test('collect copies the installers under their release names', () => {
    const root = bundle(['appimage/font-manager_0.1.0_amd64.AppImage']);
    const out = join(root, 'out');
    try {
      execFileSync(process.execPath, [
        SCRIPT, 'collect', '--os', 'linux', '--arch', 'x64', '--sha', SHA, '--bundle', root, '--out', out,
      ]);
      const [name] = readdirSync(out);
      assert.match(name, /^font-manager_\d+\.\d+\.\d+-1a2b3c4_linux_x64\.AppImage$/);
      assert.equal(readFileSync(join(out, name), 'utf8'), 'appimage/font-manager_0.1.0_amd64.AppImage');
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test('exits non-zero on a bad hash', () => {
    assert.throws(
      () => execFileSync(process.execPath, [SCRIPT, 'version', '--sha', 'nope'], { stdio: 'pipe' }),
      (e) => e.status === 1 && /not a commit hash/.test(String(e.stderr)),
    );
  });
});
