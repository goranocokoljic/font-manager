import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { describe, test } from 'node:test';
import { fileURLToPath } from 'node:url';

import { installerSteps } from './build.mjs';
import { BINARIES, SHIM, hostTriple, plan } from './sidecar.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const read = (...p) => readFileSync(join(ROOT, ...p), 'utf8').replace(/\r\n/g, '\n');
const LINUX = 'x86_64-unknown-linux-gnu';
const WINDOWS = 'x86_64-pc-windows-msvc';

describe('plan', () => {
  test('a debug build for the host by default, copied under the target triple', () => {
    const p = plan({ host: LINUX, root: '/r' });
    assert.deepEqual(p.cargo, ['cargo', 'build', '-p', 'fm-mcp', '--bin', 'fm-mcp-stdio']);
    assert.equal(p.source, join('/r', 'target', 'debug', 'fm-mcp-stdio'));
    assert.equal(p.dest, join('/r', BINARIES, `fm-mcp-stdio-${LINUX}`));
  });

  test('Windows gets the .exe suffix on both ends', () => {
    const p = plan({ host: WINDOWS, root: '/r' });
    assert.ok(p.source.endsWith('fm-mcp-stdio.exe'));
    assert.ok(p.dest.endsWith(`fm-mcp-stdio-${WINDOWS}.exe`));
  });

  test('--release builds release and locked', () => {
    const p = plan({ argv: ['--release'], host: LINUX, root: '/r' });
    assert.deepEqual(p.cargo.slice(0, 4), ['cargo', 'build', '--release', '--locked']);
    assert.equal(p.source, join('/r', 'target', 'release', 'fm-mcp-stdio'));
  });

  test("follows Tauri's profile and target triple in before*Command", () => {
    const env = { TAURI_ENV_DEBUG: 'false', TAURI_ENV_TARGET_TRIPLE: 'aarch64-apple-darwin' };
    const p = plan({ env, host: 'x86_64-apple-darwin', root: '/r' });
    assert.ok(p.cargo.includes('--release'));
    assert.deepEqual(p.cargo.slice(-2), ['--target', 'aarch64-apple-darwin']);
    assert.equal(
      p.source,
      join('/r', 'target', 'aarch64-apple-darwin', 'release', 'fm-mcp-stdio'),
    );
    assert.ok(p.dest.endsWith('fm-mcp-stdio-aarch64-apple-darwin'));
    const dev = plan({ env: { TAURI_ENV_DEBUG: 'true' }, host: LINUX, root: '/r' });
    assert.ok(!dev.cargo.includes('--release'));
  });

  test('honors CARGO_TARGET_DIR', () => {
    const p = plan({ env: { CARGO_TARGET_DIR: 'out' }, host: LINUX, root: '/r' });
    assert.equal(p.source, join('/r', 'out', 'debug', 'fm-mcp-stdio'));
  });

  test('rejects an unknown argument', () => {
    assert.throws(() => plan({ argv: ['--fast'], host: LINUX }), /unexpected argument/);
  });

  test('reads the host triple from rustc -vV', () => {
    assert.equal(hostTriple(`rustc 1.99.0\nbinary: rustc\nhost: ${WINDOWS}\n`), WINDOWS);
    assert.throws(() => hostTriple('rustc 1.99.0\n'), /no host triple/);
  });
});

describe('wiring', () => {
  test('the shim is the [[bin]] of fm-mcp and the externalBin of the bundle', () => {
    assert.match(read('crates', 'fm-mcp', 'Cargo.toml'), new RegExp(`name = "${SHIM}"`));
    const conf = JSON.parse(read('apps', 'desktop', 'src-tauri', 'tauri.conf.json'));
    assert.deepEqual(conf.bundle.externalBin, [`binaries/${SHIM}`]);
  });

  test("Tauri's beforeDevCommand and beforeBuildCommand build the sidecar first", () => {
    const conf = JSON.parse(read('apps', 'desktop', 'src-tauri', 'tauri.conf.json'));
    const desktop = JSON.parse(read('apps', 'desktop', 'package.json'));
    assert.match(desktop.scripts.sidecar, /scripts\/release\/sidecar\.mjs/);
    assert.match(conf.build.beforeDevCommand, /^pnpm sidecar && /);
    assert.match(conf.build.beforeBuildCommand, /^pnpm sidecar && /);
  });

  test('ci.yml builds the sidecar before Lint', () => {
    const ci = read('.github', 'workflows', 'ci.yml');
    const sidecar = ci.indexOf('node scripts/release/sidecar.mjs');
    assert.ok(sidecar > 0, 'a sidecar step');
    assert.ok(sidecar < ci.indexOf('run: pnpm lint'), 'before Lint');
  });

  test('pnpm lint builds the sidecar before clippy', () => {
    const pkg = JSON.parse(read('package.json'));
    assert.match(pkg.scripts['lint:rust'], /^node scripts\/release\/sidecar\.mjs && /);
  });

  test('the release build builds it locked and in release', () => {
    for (const os of ['windows', 'macos', 'linux']) {
      const steps = installerSteps(os, { root: '/r', cargoHome: '/c', epoch: '1' });
      const step = steps.find((s) => s.cmd.join(' ').includes('scripts/release/sidecar.mjs'));
      assert.ok(step, os);
      assert.ok(step.cmd.includes('--release') && step.cmd.includes('--locked'), os);
      const tauri = steps.findIndex((s) => s.cmd.includes('tauri'));
      assert.ok(steps.indexOf(step) < tauri, `${os}: before tauri build`);
    }
  });

  test('docs/building.md says so, and git ignores the copies', () => {
    const doc = read('docs', 'building.md');
    assert.ok(doc.includes('scripts/release/sidecar.mjs'));
    assert.ok(doc.includes(`${SHIM}-<target-triple>`));
    assert.match(read('.gitignore'), /^apps\/desktop\/src-tauri\/binaries\/$/m);
  });
});
