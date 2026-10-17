#!/usr/bin/env node
// ci-wait.mjs — wait for the CI matrix (and optionally the bench job) on a PR's
// head commit and report a verdict. The harness runs this BETWEEN agent
// sessions, so waiting for CI costs no model tokens.
//
// Usage:
//   node scripts/harness/ci-wait.mjs --pr 57 [--bench] [--out .harness/ci/issue-42.json]
//
// Verdict JSON (stdout, and --out when given):
//   { status: "green"|"red"|"conflict"|"infra"|"timeout"|"no-ci", sha, checks: [{name, os, conclusion, url}],
//     failed: [{name, os, url, excerpt}], bench: null | {status, metrics, failed[], note} }
//   conflict — the PR conflicts with main, so GitHub starts no CI for it
//   infra    — GitHub did not run the jobs (billing, spending limit): no code fix helps
// Exit: 0 green, 1 red (a required check or a bench metric failed), 2 anything else, 3 error.
//
// Required checks come from go-harness.config.json `ci.required_checks`
// ("ci / windows" ...). A check run matches when its name equals the entry or
// its job part ("windows"). A story is green only when ALL of them conclude
// `success` — a Windows-only pass is not a pass.

import { existsSync, mkdirSync, readFileSync, writeFileSync, mkdtempSync, rmSync, readdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { loadConfig, gh, ghJson, parseArgs, isMain } from './lib.mjs';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export function matchCheck(required, runs) {
  const want = required.toLowerCase();
  const job = want.split('/').pop().trim();
  return (
    runs.find((r) => r.name.toLowerCase() === want) ??
    runs.find((r) => r.name.toLowerCase() === job) ??
    runs.find((r) => new RegExp(`\\b${job.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\b`).test(r.name.toLowerCase())) ??
    null
  );
}

export function osOf(name) {
  const n = name.toLowerCase();
  return n.includes('windows') ? 'windows' : n.includes('mac') ? 'macos' : n.includes('linux') || n.includes('ubuntu') ? 'linux' : null;
}

/** The useful part of a job log: lines near compiler errors, failed tests and panics. */
export function excerpt(log, max = 120) {
  const lines = log.split(/\r?\n/).map((l) => l.replace(/^\d{4}-\d\d-\d\dT[\d:.]+Z\s?/, ''));
  const hit = /(error(\[E\d+\])?:|^error\b|FAILED|FAIL\b|panicked at|failures:|assertion .*failed|Error: |✗|× )/;
  const keep = new Set();
  lines.forEach((l, i) => {
    if (hit.test(l)) for (let k = Math.max(0, i - 3); k <= Math.min(lines.length - 1, i + 8); k++) keep.add(k);
  });
  let picked = [...keep].sort((a, b) => a - b).map((i) => lines[i]);
  if (!picked.length) picked = lines.slice(-60);
  return picked.slice(-max).join('\n');
}

/** GitHub refused to run the job (account problem), from its annotations; null when it ran. */
export function infraReason(annotations) {
  const hit = (annotations ?? []).find((a) =>
    /not started because|spending limit|payments have failed|billing/i.test(a.message ?? ''));
  return hit ? hit.message.trim() : null;
}

function annotations(id) {
  return ghJson(['api', `repos/{owner}/{repo}/check-runs/${id}/annotations`], { allowFail: true }) ?? [];
}

function mergeable(pr) {
  return ghJson(['pr', 'view', String(pr), '--json', 'mergeable'], { allowFail: true })?.mergeable ?? 'UNKNOWN';
}

function checkRuns(sha) {
  const pages = ghJson(['api', '--paginate', '--slurp', `repos/{owner}/{repo}/commits/${sha}/check-runs?per_page=100`]) ?? [];
  // Keep the newest run per name (a re-run leaves the old one behind).
  const byName = new Map();
  for (const r of pages.flatMap((p) => p.check_runs ?? [])) {
    const prev = byName.get(r.name);
    if (!prev || new Date(r.started_at) > new Date(prev.started_at)) byName.set(r.name, r);
  }
  return [...byName.values()];
}

function jobLog(id) {
  return gh(['api', `repos/{owner}/{repo}/actions/jobs/${id}/logs`], { allowFail: true }) ?? '';
}

async function waitFor(sha, names, cfg, deadline, pr = null) {
  for (;;) {
    const runs = checkRuns(sha);
    const found = names.map((n) => ({ want: n, run: matchCheck(n, runs) }));
    // No run yet and the PR conflicts with main: GitHub will never start one.
    if (pr && found.every((f) => !f.run) && mergeable(pr) === 'CONFLICTING') return 'conflict';
    const done = found.every((f) => f.run && f.run.status === 'completed');
    const failedEarly = found.some((f) => f.run && f.run.status === 'completed' && f.run.conclusion !== 'success');
    // A red required check settles the verdict; do not wait for the slow OS.
    if (done || failedEarly) return found;
    if (Date.now() > deadline) return null;
    await sleep(cfg.ci.poll_sec * 1000);
  }
}

async function bench(prNumber, sha, cfg, deadline) {
  if (!existsSync('.github/workflows/' + cfg.ci.bench_workflow)) {
    return { status: 'unavailable', note: 'no bench workflow yet: verify numeric criteria locally (pending E9.1)' };
  }
  gh(['label', 'create', 'bench', '--color', 'C5DEF5', '--description', 'Runs the bench workflow on this PR'], { allowFail: true }); // exists -> no-op error
  gh(['pr', 'edit', String(prNumber), '--add-label', 'bench'], { allowFail: true });
  const found = await waitFor(sha, [cfg.ci.bench_check], cfg, deadline);
  if (!found || !found[0].run) return { status: 'timeout', note: 'bench check did not finish' };
  const run = found[0].run;
  const runId = /\/actions\/runs\/(\d+)/.exec(run.details_url ?? '')?.[1];
  if (!runId) return { status: 'error', note: 'cannot find the bench workflow run' };
  const dir = mkdtempSync(join(tmpdir(), 'go-bench-'));
  try {
    gh(['run', 'download', runId, '-n', cfg.ci.bench_artifact, '-D', dir]);
    const file = readdirSync(dir, { recursive: true }).find((f) => String(f).endsWith('bench-report.json'));
    if (!file) return { status: 'error', note: 'bench-report.json missing from the artifact' };
    const metrics = JSON.parse(readFileSync(join(dir, String(file)), 'utf8'));
    const failed = Object.entries(metrics).filter(([, m]) => m.pass === false).map(([k, m]) => ({ metric: k, ...m }));
    return { status: failed.length ? 'red' : 'green', metrics, failed, conclusion: run.conclusion };
  } catch (e) {
    return { status: 'error', note: e.message };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const cfg = loadConfig();
  if (!args.pr) throw new Error('usage: ci-wait.mjs --pr <number> [--bench] [--out <file>]');
  const pr = ghJson(['pr', 'view', String(args.pr), '--json', 'number,headRefOid']);
  const sha = pr.headRefOid;
  const deadline = Date.now() + cfg.ci.timeout_min * 60 * 1000;

  let verdict;
  if (!existsSync('.github/workflows/ci.yml')) {
    verdict = { status: 'no-ci', sha, checks: [], failed: [], bench: null, note: '.github/workflows/ci.yml does not exist (E1.2 not merged?)' };
  } else {
    const found = await waitFor(sha, cfg.ci.required_checks, cfg, deadline, pr.number);
    if (found === 'conflict') {
      verdict = { status: 'conflict', sha, checks: [], failed: [], bench: null, note: 'the PR conflicts with main, so no CI runs' };
    } else if (!found) {
      verdict = { status: 'timeout', sha, checks: [], failed: [], bench: null, note: `required checks not finished after ${cfg.ci.timeout_min} min` };
    } else {
      const checks = found.map((f) => ({
        name: f.want, os: osOf(f.want), conclusion: f.run?.conclusion ?? 'missing', url: f.run?.html_url ?? null, id: f.run?.id ?? null,
      }));
      const failed = checks
        .filter((c) => c.conclusion !== 'success' && c.conclusion !== 'missing' && c.id)
        .map((c) => ({ name: c.name, os: c.os, url: c.url, excerpt: excerpt(jobLog(c.id)) }));
      const infra = failed.map((c) => infraReason(annotations(checks.find((k) => k.name === c.name).id))).find(Boolean);
      verdict = infra
        ? { status: 'infra', sha, checks, failed, bench: null, note: infra }
        : { status: failed.length ? 'red' : 'green', sha, checks, failed, bench: null };
    }
  }
  if (args.bench && verdict.status === 'green') {
    verdict.bench = await bench(pr.number, sha, cfg, deadline);
    if (verdict.bench.status === 'red') verdict.status = 'red';
  }

  const json = JSON.stringify(verdict, null, 2);
  if (args.out) {
    mkdirSync(dirname(String(args.out)), { recursive: true });
    writeFileSync(String(args.out), json + '\n', 'utf8');
  }
  process.stdout.write(json + '\n');
  process.exit({ green: 0, red: 1 }[verdict.status] ?? 2);
}

if (isMain(import.meta.url)) {
  main().catch((e) => { console.error(e.message); process.exit(3); });
}
