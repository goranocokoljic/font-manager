#!/usr/bin/env node
// Release builds from a tag (E18.4). This script is the one place that holds the build
// commands: docs/building.md shows them in generated blocks, and the reproducible
// workflow (.github/workflows/reproducible.yml) runs them. When the two drift,
// `check-doc` and build.test.mjs fail.
//
//   node scripts/release/build.mjs installer --os <windows|macos|linux>
//       pnpm install --frozen-lockfile, then fm-cli and the installers with --locked
//   node scripts/release/build.mjs cli
//       fm-cli only, with SOURCE_DATE_EPOCH and --remap-path-prefix; prints its SHA256
//   node scripts/release/build.mjs same-hash --first <cli line> --second <cli line>
//       prints the hash when both lines carry the same SHA256; fails otherwise
//   node scripts/release/build.mjs packages --os <os>
//       the system packages the build needs (apt names on Linux), one line
//   node scripts/release/build.mjs sums --dir <dir>
//       writes <dir>/SHA256SUMS for every file in <dir> (`sha256sum -c` reads it)
//   node scripts/release/build.mjs check-doc | write-doc
//       compares (or rewrites) the generated blocks of docs/building.md

import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { BUNDLES } from './dev-build.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');

/** The build doc whose generated blocks this script owns. */
export const DOC = join(ROOT, 'docs', 'building.md');

/** The name of the checksum file attached to every release. */
export const SUMS = 'SHA256SUMS';

/** The fm-cli binary `cargo build --release` writes, per OS. */
export const CLI_BINARY = {
  windows: 'target/release/fm.exe',
  macos: 'target/release/fm',
  linux: 'target/release/fm',
};

/**
 * What each OS needs before the build. Linux lists apt packages (Debian/Ubuntu names;
 * CI installs exactly these); Windows and macOS need the vendor toolchains only.
 */
export const SYSTEM = {
  windows: {
    packages: [],
    notes: [
      'Visual Studio 2022 Build Tools with the "Desktop development with C++" workload (MSVC, Windows SDK)',
      'WebView2 Runtime (preinstalled on Windows 10 21H2+ and Windows 11)',
      'The .msi bundler downloads WiX 3 and the .exe bundler downloads NSIS on first use',
    ],
  },
  macos: {
    packages: [],
    notes: ['Xcode Command Line Tools (`xcode-select --install`)'],
  },
  linux: {
    packages: [
      'build-essential',
      'pkg-config',
      'file',
      'patchelf',
      'libwebkit2gtk-4.1-dev',
      'libgtk-3-dev',
      'libsoup-3.0-dev',
      'libjavascriptcoregtk-4.1-dev',
      'libayatana-appindicator3-dev',
      'librsvg2-dev',
      'libfontconfig1-dev',
      'fontconfig',
    ],
    notes: ['Debian 12 / Ubuntu 22.04 or newer (WebKitGTK 4.1)'],
  },
};

function checkOs(os) {
  if (!BUNDLES[os]) throw new Error(`unknown os: ${os} (windows, macos or linux)`);
}

/**
 * The environment that makes cargo output independent of when and where it is built:
 * the commit time as SOURCE_DATE_EPOCH, and the checkout and cargo home paths remapped
 * out of panic messages and debug info. `rustflags` is a list; `run` passes it as
 * CARGO_ENCODED_RUSTFLAGS so paths with spaces survive.
 */
export function reproEnv({ root, cargoHome, epoch }) {
  return {
    SOURCE_DATE_EPOCH: epoch,
    CARGO_INCREMENTAL: '0',
    RUSTFLAGS: [`--remap-path-prefix=${root}=/build`, `--remap-path-prefix=${cargoHome}=/cargo`],
  };
}

/** The steps of `installer --os <os>`, as `{ env, cmd }` (cmd is an argv list). */
export function installerSteps(os, ctx) {
  checkOs(os);
  const env = reproEnv(ctx);
  const bundles = BUNDLES[os].map((b) => b.dir).join(',');
  // linuxdeploy is itself an AppImage: without FUSE it must extract itself, and its
  // strip breaks on the .relr.dyn sections of current toolchains.
  const bundleEnv = os === 'linux' ? { APPIMAGE_EXTRACT_AND_RUN: '1', NO_STRIP: 'true' } : {};
  return [
    { env: {}, cmd: ['pnpm', 'install', '--frozen-lockfile'] },
    { env, cmd: ['cargo', 'build', '--release', '--locked', '-p', 'fm-cli'] },
    {
      env: { ...env, ...bundleEnv },
      cmd: [
        'pnpm',
        '--filter',
        '@fm/desktop',
        'exec',
        'tauri',
        'build',
        '--ci',
        '--bundles',
        bundles,
        '--',
        '--locked',
      ],
    },
  ];
}

/** The steps of `cli`: fm-cli alone, reproducibly. */
export function cliSteps(ctx) {
  return [{ env: reproEnv(ctx), cmd: ['cargo', 'build', '--release', '--locked', '-p', 'fm-cli'] }];
}

/** One step as the shell line the doc shows. */
export function renderStep({ env, cmd }) {
  const vars = Object.entries(env).map(([k, v]) => {
    const value = Array.isArray(v) ? v.join(' ') : v;
    return /[\s$"]/.test(value) ? `${k}="${value}"` : `${k}=${value}`;
  });
  return [...vars, ...cmd].join(' ');
}

/** The placeholders the doc shows where `run` fills in real values. */
const DOC_CTX = { root: '$PWD', cargoHome: '$CARGO_HOME', epoch: '$(git log -1 --format=%ct)' };

/** The toolchain the repository pins, read from the files that pin it. */
export function toolchain(root = ROOT) {
  const rust = readFileSync(join(root, 'rust-toolchain.toml'), 'utf8').match(
    /^channel\s*=\s*"([^"]+)"/m,
  );
  const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
  const desktop = JSON.parse(readFileSync(join(root, 'apps', 'desktop', 'package.json'), 'utf8'));
  const tauri = desktop.devDependencies?.['@tauri-apps/cli'];
  const pnpm = /^pnpm@(.+)$/.exec(pkg.packageManager ?? '');
  if (!rust || !pnpm || !pkg.engines?.node || !tauri) {
    throw new Error('rust-toolchain.toml, packageManager, engines.node or @tauri-apps/cli is not pinned');
  }
  return { rust: rust[1], node: pkg.engines.node, pnpm: pnpm[1], tauri };
}

/** The generated blocks of the doc, by name. */
export function docBlocks(root = ROOT) {
  const t = toolchain(root);
  const toolchainBlock = [
    `- Rust ${t.rust} with rustfmt and clippy — \`rust-toolchain.toml\`; rustup installs it on the first \`cargo\` call`,
    `- Node.js ${t.node} — \`engines.node\` in \`package.json\` (CI uses Node 22)`,
    `- pnpm ${t.pnpm} — \`packageManager\` in \`package.json\`; \`corepack enable\` picks it up`,
    `- Tauri CLI ${t.tauri} — \`@tauri-apps/cli\` in \`apps/desktop/package.json\`, installed by \`pnpm install\``,
  ];
  const packagesBlock = [];
  for (const os of Object.keys(SYSTEM)) {
    packagesBlock.push(`**${os}**`, '');
    for (const note of SYSTEM[os].notes) packagesBlock.push(`- ${note}`);
    if (SYSTEM[os].packages.length) {
      packagesBlock.push(
        '',
        '```sh',
        `sudo apt-get install -y --no-install-recommends ${SYSTEM[os].packages.join(' ')}`,
        '```',
      );
    }
    packagesBlock.push('');
  }
  const commandsBlock = [];
  for (const os of Object.keys(BUNDLES)) {
    const kinds = BUNDLES[os].map((b) => b.as ?? b.ext).join(', ');
    commandsBlock.push(
      `**${os}** (${kinds}): \`node scripts/release/build.mjs installer --os ${os}\` runs`,
      '',
      '```sh',
      ...installerSteps(os, DOC_CTX).map(renderStep),
      '```',
      '',
    );
  }
  commandsBlock.push(
    `**fm-cli alone**: \`node scripts/release/build.mjs cli\` runs`,
    '',
    '```sh',
    ...cliSteps(DOC_CTX).map(renderStep),
    '```',
  );
  return {
    toolchain: toolchainBlock.join('\n'),
    packages: packagesBlock.join('\n').trimEnd(),
    commands: commandsBlock.join('\n'),
  };
}

const blockRe = (name) =>
  new RegExp(`(<!-- build\\.mjs:${name} -->)([\\s\\S]*?)(<!-- /build\\.mjs:${name} -->)`);

/** `doc` with every generated block replaced by `blocks`. Throws on a missing marker. */
export function fillDoc(doc, blocks) {
  let out = doc.replace(/\r\n/g, '\n');
  for (const [name, body] of Object.entries(blocks)) {
    const re = blockRe(name);
    if (!re.test(out)) throw new Error(`docs/building.md has no build.mjs:${name} block`);
    out = out.replace(re, (_, open, _old, close) => `${open}\n${body}\n${close}`);
  }
  return out;
}

/** The names of the blocks whose text in `doc` differs from `blocks`. */
export function staleBlocks(doc, blocks) {
  const text = doc.replace(/\r\n/g, '\n');
  return Object.entries(blocks)
    .filter(([name, body]) => {
      const m = blockRe(name).exec(text);
      return !m || m[2] !== `\n${body}\n`;
    })
    .map(([name]) => name);
}

/** SHA256 of a file, lower-case hex. */
export function sha256File(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

/** The SHA256SUMS text for the files in `dir`: `<hash>  <name>`, sorted by name. */
export function sumsFor(dir) {
  const names = readdirSync(dir)
    .filter((n) => n !== SUMS && statSync(join(dir, n)).isFile())
    .sort();
  if (!names.length) throw new Error(`no files to sum in ${dir}`);
  return names.map((n) => `${sha256File(join(dir, n))}  ${n}\n`).join('');
}

/**
 * The hash two `cli` runs agree on. Each line must be `<sha256>  <path>`; an empty or
 * other line (a build that failed before it printed its hash) is an error, not a match.
 */
export function sameHash(first, second) {
  const hash = (line, which) => {
    const m = /^([0-9a-f]{64}) {2}\S/.exec(String(line ?? '').trim());
    if (!m) throw new Error(`${which} build printed no SHA256 line: "${line ?? ''}"`);
    return m[1];
  };
  const a = hash(first, 'first');
  const b = hash(second, 'second');
  if (a !== b) throw new Error(`fm-cli is not reproducible: ${a} vs ${b}`);
  return a;
}

function git(args) {
  const r = spawnSync('git', args, { cwd: ROOT, encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`git ${args.join(' ')} failed: ${r.stderr}`);
  return r.stdout.trim();
}

function runCtx() {
  return {
    root: ROOT,
    cargoHome: process.env.CARGO_HOME ?? join(homedir(), '.cargo'),
    epoch: git(['log', '-1', '--format=%ct', 'HEAD']),
  };
}

function run(steps) {
  for (const step of steps) {
    // stderr: stdout of `cli` is the hash line alone, so a failed build prints no hash.
    console.error(`$ ${renderStep(step)}`);
    const env = { ...process.env };
    for (const [k, v] of Object.entries(step.env)) {
      if (k === 'RUSTFLAGS') env.CARGO_ENCODED_RUSTFLAGS = v.join('\x1f');
      else env[k] = v;
    }
    const [cmd, ...rest] = step.cmd;
    // pnpm and cargo are .cmd shims on Windows; no argument here holds a space.
    const r = spawnSync(cmd, rest, {
      cwd: ROOT,
      env,
      stdio: 'inherit',
      shell: process.platform === 'win32',
    });
    if (r.status !== 0) throw new Error(`${cmd} exited with ${r.status ?? r.signal}`);
  }
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
  if (cmd === 'installer') {
    checkOs(a.os);
    run(installerSteps(a.os, runCtx()));
  } else if (cmd === 'cli') {
    run(cliSteps(runCtx()));
    const os = { win32: 'windows', darwin: 'macos' }[process.platform] ?? 'linux';
    console.log(`${sha256File(join(ROOT, CLI_BINARY[os]))}  ${CLI_BINARY[os]}`);
  } else if (cmd === 'same-hash') {
    console.log(sameHash(a.first, a.second));
  } else if (cmd === 'packages') {
    checkOs(a.os);
    console.log(SYSTEM[a.os].packages.join(' '));
  } else if (cmd === 'sums') {
    if (!a.dir) throw new Error('sums needs --dir <dir>');
    writeFileSync(join(a.dir, SUMS), sumsFor(a.dir));
    process.stdout.write(readFileSync(join(a.dir, SUMS), 'utf8'));
  } else if (cmd === 'check-doc') {
    const stale = staleBlocks(readFileSync(DOC, 'utf8'), docBlocks());
    if (stale.length) {
      throw new Error(
        `docs/building.md is out of date (${stale.join(', ')}); run: node scripts/release/build.mjs write-doc`,
      );
    }
    console.log('docs/building.md matches scripts/release/build.mjs');
  } else if (cmd === 'write-doc') {
    writeFileSync(DOC, fillDoc(readFileSync(DOC, 'utf8'), docBlocks()));
  } else {
    throw new Error('usage: build.mjs installer|cli|same-hash|packages|sums|check-doc|write-doc [...]');
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    main(process.argv.slice(2));
  } catch (e) {
    console.error(`build: ${e.message}`);
    process.exit(1);
  }
}
