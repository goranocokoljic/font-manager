#!/usr/bin/env node
// The stdio shim as a Tauri sidecar (E27.7). The desktop bundle ships `fm-mcp-stdio`
// through `externalBin`, and tauri-build fails every build of fm-app (cargo build,
// clippy, tests) when the file is missing. This script builds the shim and copies it to
// the path Tauri expects:
//
//   apps/desktop/src-tauri/binaries/fm-mcp-stdio-<target-triple>[.exe]
//
// It runs before Lint in ci.yml, from Tauri's beforeDevCommand and beforeBuildCommand,
// from `pnpm lint`, and in the release build (scripts/release/build.mjs).
//
//   node scripts/release/sidecar.mjs [--release] [--locked] [--target <triple>]
//
// The profile is release with --release or when Tauri runs a release build
// (TAURI_ENV_DEBUG=false); debug otherwise. The triple is --target, else Tauri's
// TAURI_ENV_TARGET_TRIPLE, else the host's (`rustc -vV`).

import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, isAbsolute, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');

/** The [[bin]] name in crates/fm-mcp/Cargo.toml, and `externalBin` in tauri.conf.json. */
export const SHIM = 'fm-mcp-stdio';

/** Where Tauri looks for the sidecar, relative to the repository root. */
export const BINARIES = join('apps', 'desktop', 'src-tauri', 'binaries');

/** The host triple from `rustc -vV` output. */
export function hostTriple(rustcVersion) {
  const m = /^host:\s*(\S+)/m.exec(rustcVersion);
  if (!m) throw new Error('rustc -vV printed no host triple');
  return m[1];
}

/**
 * What to build and where to copy it. `argv`: this script's arguments; `env`: the
 * environment (Tauri's TAURI_ENV_*, CARGO_TARGET_DIR); `host`: the host triple.
 */
export function plan({ argv = [], env = {}, host, root = ROOT }) {
  const opts = { release: false, locked: false, target: undefined };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--release') opts.release = true;
    else if (argv[i] === '--locked') opts.locked = true;
    else if (argv[i] === '--target' && argv[i + 1]) opts.target = argv[++i];
    else throw new Error(`unexpected argument: ${argv[i]}`);
  }
  const release = opts.release || env.TAURI_ENV_DEBUG === 'false';
  const triple = opts.target ?? env.TAURI_ENV_TARGET_TRIPLE ?? host;
  const cross = triple !== host;
  const exe = triple.includes('windows') ? '.exe' : '';
  const targetDir = env.CARGO_TARGET_DIR
    ? isAbsolute(env.CARGO_TARGET_DIR)
      ? env.CARGO_TARGET_DIR
      : join(root, env.CARGO_TARGET_DIR)
    : join(root, 'target');
  const profile = release ? 'release' : 'debug';
  const cargo = [
    'cargo',
    'build',
    ...(release ? ['--release'] : []),
    ...(opts.locked || release ? ['--locked'] : []),
    '-p',
    'fm-mcp',
    '--bin',
    SHIM,
    ...(cross ? ['--target', triple] : []),
  ];
  return {
    cargo,
    source: join(targetDir, ...(cross ? [triple] : []), profile, `${SHIM}${exe}`),
    dest: join(root, BINARIES, `${SHIM}-${triple}${exe}`),
  };
}

function run([cmd, ...rest]) {
  // cargo and rustc may be .cmd shims on Windows; no argument here holds a space.
  const r = spawnSync(cmd, rest, {
    cwd: ROOT,
    stdio: ['ignore', 'pipe', 'inherit'],
    encoding: 'utf8',
    shell: process.platform === 'win32',
  });
  if (r.status !== 0) throw new Error(`${cmd} ${rest.join(' ')} exited with ${r.status ?? r.signal}`);
  return r.stdout;
}

function main(argv) {
  const host = hostTriple(run(['rustc', '-vV']));
  const p = plan({ argv, env: process.env, host });
  console.error(`$ ${p.cargo.join(' ')}`);
  process.stderr.write(run(p.cargo));
  mkdirSync(dirname(p.dest), { recursive: true });
  copyFileSync(p.source, p.dest);
  console.error(`sidecar: ${p.dest}`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    main(process.argv.slice(2));
  } catch (e) {
    console.error(`sidecar: ${e.message}`);
    process.exit(1);
  }
}
