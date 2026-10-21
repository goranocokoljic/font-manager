// node --test scripts/release/build.test.mjs — the documented release build (E18.4):
// the doc and the script agree, the commands are locked, the fm-cli build is
// reproducible by construction, and SHA256SUMS is in `sha256sum -c` format.

import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  CLI_BINARY,
  DOC,
  SUMS,
  SYSTEM,
  cliSteps,
  docBlocks,
  fillDoc,
  installerSteps,
  renderStep,
  reproEnv,
  staleBlocks,
  sumsFor,
  toolchain,
} from './build.mjs';
import { BUNDLES } from './dev-build.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, '..', '..');
const SCRIPT = join(HERE, 'build.mjs');
const CTX = { root: '/src/fm', cargoHome: '/home/u/.cargo', epoch: '1700000000' };
const OSES = ['windows', 'macos', 'linux'];

const workflow = (name) =>
  readFileSync(join(ROOT, '.github', 'workflows', name), 'utf8').replace(/\r\n/g, '\n');

function tmp() {
  return mkdtempSync(join(tmpdir(), 'fm-build-'));
}

describe('docs/building.md', () => {
  test('its generated blocks match the script', () => {
    assert.deepEqual(staleBlocks(readFileSync(DOC, 'utf8'), docBlocks()), []);
  });

  test('shows the exact installer and fm-cli commands of every OS', () => {
    const doc = readFileSync(DOC, 'utf8');
    for (const os of OSES) {
      for (const step of installerSteps(os, { root: '$PWD', cargoHome: '$CARGO_HOME', epoch: '$(git log -1 --format=%ct)' })) {
        assert.ok(doc.includes(renderStep(step)), `${os}: ${renderStep(step)}`);
      }
    }
    assert.ok(doc.includes('node scripts/release/build.mjs cli'));
  });

  test('names the pinned toolchain and every Linux package', () => {
    const doc = readFileSync(DOC, 'utf8');
    const t = toolchain();
    for (const v of [t.rust, t.pnpm, t.tauri, t.node]) assert.ok(doc.includes(v), v);
    for (const p of SYSTEM.linux.packages) assert.ok(doc.includes(p), p);
  });

  test('names what is not reproducible and how to verify a download on each OS', () => {
    const doc = readFileSync(DOC, 'utf8');
    for (const s of [
      'Signed installers',
      'Bundler timestamps',
      'sha256sum -c',
      'shasum -a 256 -c',
      'Get-FileHash',
      'Get-AuthenticodeSignature',
      'codesign --verify',
      'spctl',
      'gpg --verify',
      SUMS,
    ]) {
      assert.ok(doc.includes(s), s);
    }
  });
});

describe('staleBlocks and fillDoc', () => {
  const blocks = { commands: 'cargo build --locked' };
  const fresh = '# t\n<!-- build.mjs:commands -->\ncargo build --locked\n<!-- /build.mjs:commands -->\n';

  test('a doc in step with the script has no stale block, also with CRLF', () => {
    assert.deepEqual(staleBlocks(fresh, blocks), []);
    assert.deepEqual(staleBlocks(fresh.replace(/\n/g, '\r\n'), blocks), []);
  });

  test('an edited command or a missing marker is drift', () => {
    assert.deepEqual(staleBlocks(fresh.replace('--locked', ''), blocks), ['commands']);
    assert.deepEqual(staleBlocks('# t\n', blocks), ['commands']);
  });

  test('fillDoc rewrites the block and keeps the rest', () => {
    const filled = fillDoc(fresh.replace('--locked', '--offline'), blocks);
    assert.equal(filled, fresh);
    assert.throws(() => fillDoc('# t\n', blocks), /no build\.mjs:commands block/);
  });
});

describe('installerSteps', () => {
  test('install with --frozen-lockfile, every cargo and tauri call with --locked', () => {
    for (const os of OSES) {
      const steps = installerSteps(os, CTX);
      assert.deepEqual(steps[0].cmd, ['pnpm', 'install', '--frozen-lockfile']);
      for (const { cmd } of steps.slice(1)) {
        assert.ok(cmd.includes('--locked'), `${os}: ${cmd.join(' ')}`);
      }
    }
  });

  test('builds the installer kinds each OS ships', () => {
    for (const os of OSES) {
      const tauri = installerSteps(os, CTX).at(-1).cmd;
      const bundles = tauri[tauri.indexOf('--bundles') + 1];
      assert.equal(bundles, BUNDLES[os].map((b) => b.dir).join(','));
    }
  });

  test('builds the app with the same reproducibility settings as fm-cli', () => {
    for (const os of OSES) {
      const tauri = installerSteps(os, CTX).at(-1);
      assert.equal(tauri.env.SOURCE_DATE_EPOCH, CTX.epoch);
      assert.deepEqual(tauri.env.RUSTFLAGS, reproEnv(CTX).RUSTFLAGS);
    }
  });

  test('rejects an unknown OS', () => {
    assert.throws(() => installerSteps('beos', CTX), /unknown os/);
  });
});

describe('cliSteps and reproEnv', () => {
  test('fm-cli builds locked, at the commit time, with the build paths remapped', () => {
    const [step] = cliSteps(CTX);
    assert.deepEqual(step.cmd, ['cargo', 'build', '--release', '--locked', '-p', 'fm-cli']);
    assert.equal(step.env.SOURCE_DATE_EPOCH, '1700000000');
    assert.equal(step.env.CARGO_INCREMENTAL, '0');
    assert.deepEqual(step.env.RUSTFLAGS, [
      '--remap-path-prefix=/src/fm=/build',
      '--remap-path-prefix=/home/u/.cargo=/cargo',
    ]);
  });

  test('looks for the fm-cli binary under the [[bin]] name of crates/fm-cli', () => {
    const toml = readFileSync(join(ROOT, 'crates', 'fm-cli', 'Cargo.toml'), 'utf8');
    const bin = /\[\[bin\]\]\s*\r?\nname\s*=\s*"([^"]+)"/.exec(toml)[1];
    assert.deepEqual(CLI_BINARY, {
      windows: `target/release/${bin}.exe`,
      macos: `target/release/${bin}`,
      linux: `target/release/${bin}`,
    });
  });

  test('renders quoted values for the doc', () => {
    assert.equal(
      renderStep({ env: { A: '1', B: ['x y', 'z'] }, cmd: ['cargo', 'build'] }),
      'A=1 B="x y z" cargo build',
    );
  });
});

describe('toolchain', () => {
  test('reads the pins of this repository', () => {
    const t = toolchain();
    assert.match(t.rust, /^\d+\.\d+\.\d+$/);
    assert.match(t.pnpm, /^\d+\.\d+\.\d+$/);
    assert.match(t.tauri, /^\d+\.\d+\.\d+$/);
  });

  test('fails when pnpm is not pinned', () => {
    const root = tmp();
    mkdirSync(join(root, 'apps', 'desktop'), { recursive: true });
    writeFileSync(join(root, 'rust-toolchain.toml'), '[toolchain]\nchannel = "1.99.0"\n');
    writeFileSync(join(root, 'package.json'), JSON.stringify({ engines: { node: '>=22' } }));
    writeFileSync(
      join(root, 'apps', 'desktop', 'package.json'),
      JSON.stringify({ devDependencies: { '@tauri-apps/cli': '2.0.0' } }),
    );
    assert.throws(() => toolchain(root), /not pinned/);
  });
});

describe('SHA256SUMS', () => {
  test('one `<hash>  <name>` line per file, sorted, without itself or folders', () => {
    const dir = tmp();
    writeFileSync(join(dir, 'b.msi'), 'abc');
    writeFileSync(join(dir, 'a.dmg'), '');
    writeFileSync(join(dir, SUMS), 'old');
    mkdirSync(join(dir, 'sub'));
    assert.equal(
      sumsFor(dir),
      'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  a.dmg\n' +
        'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  b.msi\n',
    );
  });

  test('an empty folder is an error, not an empty checksum file', () => {
    assert.throws(() => sumsFor(tmp()), /no files/);
  });

  test('the sums command writes the file', () => {
    const dir = tmp();
    writeFileSync(join(dir, 'x.AppImage'), 'abc');
    execFileSync(process.execPath, [SCRIPT, 'sums', '--dir', dir]);
    assert.match(readFileSync(join(dir, SUMS), 'utf8'), /^ba7816bf[0-9a-f]{56} {2}x\.AppImage\n$/);
  });
});

describe('command line', () => {
  test('packages prints the Linux list CI installs', () => {
    const out = execFileSync(process.execPath, [SCRIPT, 'packages', '--os', 'linux'], { encoding: 'utf8' });
    assert.equal(out.trim(), SYSTEM.linux.packages.join(' '));
  });

  test('check-doc passes on this checkout', () => {
    const r = spawnSync(process.execPath, [SCRIPT, 'check-doc'], { encoding: 'utf8' });
    assert.equal(r.status, 0, r.stderr);
  });

  test('an unknown command or OS exits non-zero', () => {
    assert.notEqual(spawnSync(process.execPath, [SCRIPT, 'nope']).status, 0);
    assert.notEqual(spawnSync(process.execPath, [SCRIPT, 'packages', '--os', 'beos']).status, 0);
  });
});

describe('workflows', () => {
  test('reproducible.yml checks the doc and builds every OS with the script, uncached', () => {
    const yml = workflow('reproducible.yml');
    assert.match(yml, /run: node scripts\/release\/build\.mjs check-doc/);
    const oses = [...yml.matchAll(/- name: (\w+)\n\s+os: \S+/g)].map((m) => m[1]);
    assert.deepEqual(oses, OSES);
    assert.match(yml, /run: node scripts\/release\/build\.mjs installer --os \$\{\{ matrix\.name \}\}/);
    assert.match(yml, /build\.mjs packages --os linux/);
    assert.doesNotMatch(yml, /rust-cache|cache: pnpm/);
  });

  test('reproducible.yml builds fm-cli twice from two folders and compares', () => {
    const yml = workflow('reproducible.yml');
    assert.match(yml, /node first\/scripts\/release\/build\.mjs cli/);
    assert.match(yml, /node second-checkout\/scripts\/release\/build\.mjs cli/);
    assert.match(yml, /if \[ "\$\{a%% \*\}" != "\$\{b%% \*\}" \]/);
  });

  test('nightly.yml builds with the script and publishes SHA256SUMS', () => {
    const yml = workflow('nightly.yml');
    assert.match(yml, /node scripts\/release\/build\.mjs installer --os \$\{\{ matrix\.name \}\}/);
    assert.match(yml, /node scripts\/release\/build\.mjs sums --dir nightly-assets/);
  });
});
