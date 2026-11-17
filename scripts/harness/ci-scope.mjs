#!/usr/bin/env node
// ci-scope.mjs — does this change need the three-OS CI matrix?
//
//   node scripts/harness/ci-scope.mjs        # prints `scope=light` or `scope=full`
//
// Run by the `scope` job of .github/workflows/ci.yml, which appends the line to
// $GITHUB_OUTPUT. "light" means: every changed file is a harness script or a
// developer doc, which the app, the CLI and the plugins never read. ci.yml then
// runs the `light` job alone (ESLint and the script tests on Linux) instead of
// lint, tests, end-to-end tests and a Tauri build on Windows, macOS and Linux.
//
// Everything this script cannot tell (no file list, an API error, a push of
// 300 files or more, a manual run) is "full": the careful answer.

import { execFileSync } from 'node:child_process';
import { isMain } from './lib.mjs';

/**
 * Paths that cannot change what ships or what the OS jobs test.
 * Not light on purpose: `docs/user` (the in-app help), `README.md` (the docs
 * site), `scripts/bench` and `scripts/release` (they run on all three OS),
 * `.github/` (a workflow change must prove itself on the matrix).
 */
const LIGHT = [
  /^scripts\/harness\//,
  /^dev-docs\//,
  /^docs\/adr\//,
  /^\.claude\//,
  /^go-harness\.(ps1|config\.json)$/,
  /^CLAUDE\.md$/,
];

export function isLightPath(path) {
  return LIGHT.some((re) => re.test(path));
}

/** `paths`: the changed files (old and new name of a rename), or null when unknown. */
export function ciScope(paths) {
  return paths && paths.length > 0 && paths.every(isLightPath) ? 'light' : 'full';
}

function api(args) {
  return execFileSync('gh', ['api', ...args], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
}

function changedFiles(env) {
  const repo = env.GITHUB_REPOSITORY;
  const names = '.filename, (.previous_filename // empty)';
  if (env.GITHUB_EVENT_NAME === 'pull_request' && env.PR_NUMBER) {
    const out = api(['--paginate', `repos/${repo}/pulls/${env.PR_NUMBER}/files?per_page=100`, '--jq', `.[] | ${names}`]);
    return out.split('\n').filter(Boolean);
  }
  if (env.GITHUB_EVENT_NAME === 'push' && env.PUSH_BEFORE && !/^0+$/.test(env.PUSH_BEFORE)) {
    const cmp = JSON.parse(api([`repos/${repo}/compare/${env.PUSH_BEFORE}...${env.GITHUB_SHA}`]));
    const files = cmp.files ?? [];
    if (files.length >= 300) return null; // the API lists 300 files at most
    return files.flatMap((f) => [f.filename, f.previous_filename].filter(Boolean));
  }
  return null;
}

function main() {
  let paths = null;
  try {
    paths = changedFiles(process.env);
  } catch (e) {
    console.error(`ci-scope: cannot list the changed files, running everything: ${e.message}`);
  }
  const scope = ciScope(paths);
  console.error(`ci-scope: ${scope} (${paths ? paths.length : 'unknown'} changed files)`);
  process.stdout.write(`scope=${scope}\n`);
}

if (isMain(import.meta.url)) main();
