#!/usr/bin/env node
// main-check.mjs — the facts the keeper lane needs, without waiting:
//
//   node scripts/harness/main-check.mjs [--out .harness/ci/main.json]
//     The newest CI run on main. Exit 0 green, 1 red, 2 pending, 3 error.
//     { status: "green"|"red"|"pending", run_id, attempt, sha, url, kind, jobs[], failed[],
//       last_green_sha, suspects[] }
//     kind    — "infra" when every red job died without a failed step (runner never
//               started, cancelled, timed out): a re-run helps, no code fix does.
//               "test" when a step failed: a test, lint or build problem in the tree.
//     failed  — the red jobs with their failed step and a log excerpt (kind "test").
//     suspects— the commits on main since the last green run, newest last.
//
//   node scripts/harness/main-check.mjs --pr 512
//     The keeper's own fix PR, without waiting. Exit 0 green, 1 red, 2 pending.
//     { status: "green"|"red"|"pending"|"merged"|"closed", pr, sha, checks: {windows: "SUCCESS", ...} }
//
// Everything here is read-only; the keeper (go-harness.ps1 -Lane keeper) acts on it.

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import { loadConfig, gh, ghJson, parseArgs, isMain } from './lib.mjs';
import { excerpt } from './ci-wait.mjs';

const DEAD = new Set(['cancelled', 'timed_out', 'action_required', 'stale', 'startup_failure']);

/**
 * Sort the red jobs of a run into "infra" (no step of the job failed: the runner
 * never started, the job was cancelled or timed out) and "test" (a step failed).
 * Pure, so it is testable. Returns { kind, failed: [{ name, conclusion, step, steps_done }] }.
 */
export function classifyJobs(jobs) {
  const red = (jobs ?? []).filter((j) => j.conclusion && j.conclusion !== 'success' && j.conclusion !== 'skipped' && j.conclusion !== 'neutral');
  const failed = red.map((j) => {
    const steps = j.steps ?? [];
    const bad = steps.find((s) => s.conclusion === 'failure');
    return {
      name: j.name,
      id: j.databaseId ?? null,
      url: j.url ?? null,
      conclusion: j.conclusion,
      step: bad?.name ?? null,
      steps_done: steps.filter((s) => s.conclusion === 'success').length,
      infra: !bad && (DEAD.has(j.conclusion) || j.conclusion === 'failure'),
    };
  });
  if (!failed.length) return { kind: 'none', failed };
  return { kind: failed.every((f) => f.infra) ? 'infra' : 'test', failed };
}

/** The verdict for the fix PR's checks: every required job green, any red, or still running. */
export function prVerdict(checks, required) {
  const want = required.map((n) => n.toLowerCase().split('/').pop().trim());
  const byName = {};
  for (const c of checks ?? []) byName[String(c.name).toLowerCase()] = String(c.state ?? '').toUpperCase();
  const states = want.map((w) => byName[w] ?? 'PENDING');
  if (states.some((s) => ['FAILURE', 'TIMED_OUT', 'CANCELLED', 'ERROR', 'ACTION_REQUIRED', 'STARTUP_FAILURE'].includes(s))) return 'red';
  if (states.every((s) => s === 'SUCCESS')) return 'green';
  return 'pending';
}

function mainStatus(cfg) {
  const run = (ghJson(['run', 'list', '--branch', cfg.base_branch, '--workflow', 'ci.yml', '--limit', '1',
    '--json', 'databaseId,headSha,status,conclusion,url']) ?? [])[0];
  if (!run) return { status: 'pending', note: 'no CI run on main yet' };
  const base = { run_id: run.databaseId, sha: run.headSha, url: run.url };
  if (run.status !== 'completed') return { status: 'pending', ...base };
  if (run.conclusion === 'success') return { status: 'green', ...base };
  const view = ghJson(['run', 'view', String(run.databaseId), '--json', 'attempt,jobs']) ?? {};
  const { kind, failed } = classifyJobs(view.jobs);
  for (const f of failed) {
    if (f.infra || !f.id) continue;
    const log = gh(['api', `repos/{owner}/{repo}/actions/jobs/${f.id}/logs`], { allowFail: true }) ?? '';
    f.excerpt = excerpt(log);
  }
  const green = (ghJson(['run', 'list', '--branch', cfg.base_branch, '--workflow', 'ci.yml', '--status', 'success', '--limit', '1',
    '--json', 'headSha']) ?? [])[0];
  let suspects = [];
  if (green?.headSha && green.headSha !== run.headSha) {
    const cmp = ghJson(['api', `repos/{owner}/{repo}/compare/${green.headSha}...${run.headSha}`], { allowFail: true });
    suspects = (cmp?.commits ?? []).map((c) => ({ sha: c.sha, title: String(c.commit?.message ?? '').split('\n')[0] }));
  }
  return {
    status: 'red', ...base, attempt: view.attempt ?? 1, conclusion: run.conclusion, kind,
    jobs: (view.jobs ?? []).map((j) => ({ name: j.name, conclusion: j.conclusion })),
    failed, last_green_sha: green?.headSha ?? null, suspects,
  };
}

function prStatus(pr, cfg) {
  const v = ghJson(['pr', 'view', String(pr), '--json', 'state,headRefOid,mergeable']);
  if (v.state === 'MERGED') return { status: 'merged', pr: Number(pr), sha: v.headRefOid };
  if (v.state !== 'OPEN') return { status: 'closed', pr: Number(pr), sha: v.headRefOid };
  const checks = ghJson(['pr', 'checks', String(pr), '--json', 'name,state'], { allowFail: true }) ?? [];
  const out = {};
  for (const c of checks) out[c.name] = c.state;
  return { status: prVerdict(checks, cfg.ci.required_checks), pr: Number(pr), sha: v.headRefOid, mergeable: v.mergeable, checks: out };
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const cfg = loadConfig();
  const v = args.pr ? prStatus(args.pr, cfg) : mainStatus(cfg);
  const json = JSON.stringify(v, null, 2);
  if (args.out) {
    mkdirSync(dirname(String(args.out)), { recursive: true });
    writeFileSync(String(args.out), json + '\n', 'utf8');
  }
  process.stdout.write(json + '\n');
  process.exit({ green: 0, merged: 0, red: 1 }[v.status] ?? 2);
}

if (isMain(import.meta.url)) {
  try { main(); } catch (e) { console.error(e.message); process.exit(3); }
}
