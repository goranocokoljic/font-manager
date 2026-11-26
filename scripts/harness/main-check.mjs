#!/usr/bin/env node
// main-check.mjs — the facts the keeper lane needs, without waiting:
//
//   node scripts/harness/main-check.mjs [--out .harness/ci/main.json]
//     The state of main. Exit 0 green, 1 red, 2 pending, 3 error.
//     A push of frontend files or harness scripts runs only a short job on Linux
//     (ci-scope.mjs), so the newest run does not prove Windows and macOS. The
//     state is the newest FINISHED FULL run (the three OS jobs ran) when that one
//     is red, whatever short runs came after it (via "full"), and otherwise the
//     newest run (via "newest").
//     { status: "green"|"red"|"pending", via, run_id, attempt, sha, url, kind, jobs[], failed[],
//       last_green_sha, suspects[] }
//     kind    — "infra" when every red job died without a failed step (runner never
//               started, cancelled, timed out): a re-run helps, no code fix does.
//               "test" when a step failed: a test, lint or build problem in the tree.
//     failed  — the red jobs with their failed step and a log excerpt (kind "test").
//     suspects— the commits on main since the last green run, newest last. For a red
//               full run: since the last green FULL run, up to the red run's commit.
//
//   node scripts/harness/main-check.mjs --pr 512
//     The keeper's own fix PR, without waiting. Exit 0 green, 1 red, 2 pending.
//     { status: "green"|"red"|"pending"|"merged"|"closed", pr, sha, checks: {windows: "SUCCESS", ...} }
//
// Everything here is read-only; the keeper (go-harness.ps1 -Lane keeper) acts on it.

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import { loadConfig, gh, ghJson, parseArgs, isMain, withLight, standIns } from './lib.mjs';
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

/**
 * The verdict for the fix PR's checks: every required job green, any red, or
 * still running. The `light` job counts for the OS jobs ci.yml skipped.
 */
export function prVerdict(checks, required, light = null) {
  const want = required.map((n) => n.toLowerCase().split('/').pop().trim());
  const byName = {};
  for (const c of withLight(checks ?? [], required, light, 'state')) byName[String(c.name).toLowerCase()] = String(c.state ?? '').toUpperCase();
  const states = want.map((w) => byName[w] ?? 'PENDING');
  if (states.some((s) => ['FAILURE', 'TIMED_OUT', 'CANCELLED', 'ERROR', 'ACTION_REQUIRED', 'STARTUP_FAILURE'].includes(s))) return 'red';
  if (states.every((s) => s === 'SUCCESS')) return 'green';
  return 'pending';
}

const jobName = (n) => String(n ?? '').toLowerCase().split('/').pop().trim();

/**
 * What a CI run on main proved. "full": the three OS jobs ran (a manual run,
 * `workflow_dispatch`, is always full). "short": a stand-in job ran instead
 * (light, frontend). "unknown": the scope job has not decided yet (or no jobs
 * were read). `run.jobs` is [{ name, conclusion }] from `gh run view --json jobs`.
 */
export function runScope(run, required, stands = []) {
  if (run?.event === 'workflow_dispatch') return 'full';
  const jobs = run?.jobs;
  if (!Array.isArray(jobs)) return 'unknown';
  const ran = (j) => String(j.conclusion ?? '').toLowerCase() !== 'skipped';
  const req = (required ?? []).map(jobName);
  const st = (stands ?? []).map(jobName);
  if (jobs.some((j) => req.includes(jobName(j.name)) && ran(j))) return 'full';
  if (jobs.some((j) => st.includes(jobName(j.name)) && ran(j))) return 'short';
  return 'unknown';
}

/**
 * The run that decides main's state (pure; `runs` newest first, each with
 * `jobs` when read). The newest finished full run when it is red, whatever
 * short runs came after it; otherwise the newest run. Also returns the newest
 * finished full run and the newest green one, for suspects and coverage.
 */
export function mainState(runs, required, stands = []) {
  const list = runs ?? [];
  const full = (r) => runScope(r, required, stands) === 'full';
  const fullAny = list.find(full) ?? null;
  const fullDone = list.find((r) => r.status === 'completed' && full(r)) ?? null;
  const lastGreenFull = list.find((r) => r.status === 'completed' && r.conclusion === 'success' && full(r)) ?? null;
  const out = { fullAny, fullDone, lastGreenFull };
  // A red full run being re-run (attempt 2+) is still that run: pending, not
  // the older green one behind it.
  if (fullAny && fullAny.status !== 'completed' && (fullAny.attempt ?? 1) > 1) return { run: fullAny, via: 'full', ...out };
  if (fullDone && fullDone.conclusion !== 'success') return { run: fullDone, via: 'full', ...out };
  return { run: list[0] ?? null, via: 'newest', ...out };
}

/**
 * The CI runs on main, newest first, with their jobs read until the newest
 * finished full run and the newest green full run are found (at most `limit`
 * runs). Runs further back carry no `jobs` (scope "unknown").
 */
export function mainRuns(cfg, { limit = 40 } = {}) {
  const list = ghJson(['run', 'list', '--branch', cfg.base_branch, '--workflow', 'ci.yml', '--limit', String(limit),
    '--json', 'databaseId,headSha,status,conclusion,url,event,createdAt']) ?? [];
  const stands = standIns(cfg);
  for (const r of list) {
    const view = ghJson(['run', 'view', String(r.databaseId), '--json', 'attempt,jobs'], { allowFail: true }) ?? {};
    r.attempt = view.attempt ?? 1;
    r.jobs = view.jobs ?? null;
    const s = mainState(list.filter((x) => x.jobs !== undefined), cfg.ci.required_checks, stands);
    if (s.fullDone && s.lastGreenFull) break;
  }
  return list;
}

function mainStatus(cfg) {
  const runs = mainRuns(cfg);
  const state = mainState(runs, cfg.ci.required_checks, standIns(cfg));
  const run = state.run;
  if (!run) return { status: 'pending', note: 'no CI run on main yet' };
  // A newer full run is already running (the keeper started one after a fix):
  // the keeper waits for it instead of re-running or fixing the old red one.
  const newerFull = state.fullAny && state.fullAny !== run && state.fullAny.status !== 'completed';
  const base = { via: state.via, run_id: run.databaseId, sha: run.headSha, url: run.url, newer_full_running: !!newerFull };
  if (run.status !== 'completed') return { status: 'pending', ...base };
  if (run.conclusion === 'success') return { status: 'green', ...base };
  const view = { attempt: run.attempt, jobs: run.jobs ?? [] };
  const { kind, failed } = classifyJobs(view.jobs);
  for (const f of failed) {
    if (f.infra || !f.id) continue;
    const log = gh(['api', `repos/{owner}/{repo}/actions/jobs/${f.id}/logs`], { allowFail: true }) ?? '';
    f.excerpt = excerpt(log);
  }
  // A red full run: the suspects are the commits since the last green FULL run
  // (a green short run proved nothing on Windows and macOS).
  const green = state.via === 'full'
    ? state.lastGreenFull
    : (ghJson(['run', 'list', '--branch', cfg.base_branch, '--workflow', 'ci.yml', '--status', 'success', '--limit', '1',
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
  return { status: prVerdict(checks, cfg.ci.required_checks, standIns(cfg)), pr: Number(pr), sha: v.headRefOid, mergeable: v.mergeable, checks: out };
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
