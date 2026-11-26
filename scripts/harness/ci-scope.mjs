#!/usr/bin/env node
// ci-scope.mjs — does this change need the three-OS CI matrix?
//
//   node scripts/harness/ci-scope.mjs        # prints `scope=light`, `scope=frontend` or `scope=full`
//
// Run by the `scope` job of .github/workflows/ci.yml, which appends the line to
// $GITHUB_OUTPUT. Three answers:
//   light    — every changed file is a harness script or a developer doc, which
//              the app, the CLI and the plugins never read. ci.yml runs the
//              `light` job alone (ESLint and the script tests on Linux).
//   frontend — every changed file is Vue/TypeScript frontend code, its tests or
//              its static files (or light). ci.yml runs the `frontend` job alone
//              (lint:js, Vitest, the desktop build, Playwright on Linux; no Rust).
//   full     — anything else: lint, tests, end-to-end tests and a Tauri build
//              on Windows, macOS and Linux.
//
// A pull request labelled `ci:full` is always full, and so is a push to main
// whose merged pull request carries it. The labels are read from the API when
// the job runs, so a re-run after adding the label sees it.
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

/** The label that forces the three-OS matrix on a pull request. */
export const FULL_LABEL = 'ci:full';

/**
 * Frontend files the `frontend` job proves on Linux: the Vue/TypeScript code,
 * its tests and its static files. Never frontend: `src/api/types.ts` (generated
 * from fm-api, so a Rust change), `src-tauri/`, every `package.json`, the lock
 * file and the Vite, Playwright and TypeScript configs (they sit outside these
 * folders, and NEVER_FRONTEND names them again so a later pattern cannot let
 * them in).
 */
const FRONTEND = [
  /^apps\/desktop\/src\//,
  /^apps\/desktop\/e2e\//,
  /^apps\/desktop\/public\//,
  /^apps\/desktop\/index\.html$/,
];

const NEVER_FRONTEND = [
  /^apps\/desktop\/src\/api\/types\.ts$/,
  /^apps\/desktop\/src-tauri\//,
  /^crates\//,
  /(^|\/)Cargo\.[^/]+$/,
  /(^|\/)package\.json$/,
  /(^|\/)pnpm-(lock|workspace)\.yaml$/,
  /(^|\/)(vite|vitest|playwright)\.config\.[cm]?[jt]s$/,
  /(^|\/)tsconfig[^/]*\.json$/,
  /^\.github\//,
];

export function isFrontendPath(path) {
  return !NEVER_FRONTEND.some((re) => re.test(path)) && FRONTEND.some((re) => re.test(path));
}

/**
 * `paths`: the changed files (old and new name of a rename), or null when unknown.
 * `labels`: the labels of the pull request (or of the merged one, for a push).
 */
export function ciScope(paths, { labels = [] } = {}) {
  if (!paths || paths.length === 0 || labels.includes(FULL_LABEL)) return 'full';
  if (paths.every(isLightPath)) return 'light';
  if (paths.every((p) => isLightPath(p) || isFrontendPath(p))) return 'frontend';
  return 'full';
}

function api(args) {
  return execFileSync('gh', ['api', ...args], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
}

/** The labels of the pull request, or of the pull requests a pushed commit came from. */
function prLabels(env) {
  const repo = env.GITHUB_REPOSITORY;
  if (env.GITHUB_EVENT_NAME === 'pull_request' && env.PR_NUMBER) {
    return api([`repos/${repo}/pulls/${env.PR_NUMBER}`, '--jq', '.labels[].name']).split('\n').filter(Boolean);
  }
  if (env.GITHUB_EVENT_NAME === 'push' && env.GITHUB_SHA) {
    return api([`repos/${repo}/commits/${env.GITHUB_SHA}/pulls`, '--jq', '.[].labels[].name']).split('\n').filter(Boolean);
  }
  return [];
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
  let paths;
  let names = [];
  try {
    paths = changedFiles(process.env);
    names = prLabels(process.env);
  } catch (e) {
    console.error(`ci-scope: cannot list the changed files or the labels, running everything: ${e.message}`);
    paths = null;
  }
  const scope = ciScope(paths, { labels: names });
  const forced = names.includes(FULL_LABEL) ? `, label ${FULL_LABEL}` : '';
  console.error(`ci-scope: ${scope} (${paths ? paths.length : 'unknown'} changed files${forced})`);
  process.stdout.write(`scope=${scope}\n`);
}

if (isMain(import.meta.url)) main();
