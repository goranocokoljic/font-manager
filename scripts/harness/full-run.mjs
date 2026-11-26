#!/usr/bin/env node
// full-run.mjs — should the keeper start a full CI run on main now?
//
//   node scripts/harness/full-run.mjs [--epics E57,E56]
//
// A push of frontend files runs only the `frontend` job on Linux (ci-scope.mjs),
// so Windows and macOS see those commits only at the next full run. The keeper
// (go-harness.ps1 -Lane keeper) starts one with `gh workflow run ci.yml --ref
// main` (a manual run is always full) when this script says `start`:
//   - a red full run is behind newer commits (a fix may have landed);
//   - an epic of a story merged since the keeper's last look (`--epics`) has no
//     story left the harness can work: every sub-issue is closed or carries
//     `human` or `harness:blocked`;
//   - the oldest uncovered commit is older than `ci.full_run_max_hours`
//     (default 24; 0 turns this fallback off).
// Never while a manual run is in flight, while a run on main has not decided
// its scope yet, or while every commit on main is covered by a full run.
//
// Verdict JSON (stdout):
//   { action: "start"|"none", reason, head_sha, covered_sha, uncovered: [{sha, title, date}],
//     oldest_hours, in_flight, epics_done: [], last_full: {run_id, sha, event, status, conclusion, url} | null }
// Exit: 0 on any verdict, 3 on error. Read-only: the keeper acts on it.

import { loadConfig, ghJson, subIssues, labelNames, parseArgs, isMain, standIns } from './lib.mjs';
import { mainRuns, mainState, runScope } from './main-check.mjs';

/** Labels that take a story out of the harness's hands. */
const NOT_WORKABLE = ['human', 'harness:blocked'];

/** True when the epic has no story left that the harness can work (pure). */
export function epicDone(subs) {
  return (subs ?? []).every((s) => String(s.state).toLowerCase() === 'closed'
    || labelNames(s).some((l) => NOT_WORKABLE.includes(l)));
}

/**
 * The decision (pure). `uncovered`: the commits on main after the newest full
 * run, oldest first. `oldestHours`: the age of the first of them (Infinity
 * when no full run is known). `inFlight`: a manual (keeper) run is queued or
 * running. `undecided`: a run on main is running and its scope is not known yet.
 * `fullRed`: the newest full run finished red.
 */
export function decideFullRun({ uncovered = [], oldestHours = 0, inFlight = false, undecided = false, epicsDone = [], fullRed = false }, maxHours = 24) {
  const none = (reason) => ({ action: 'none', reason });
  if (!uncovered.length) return none('every commit on main is covered by a full run');
  if (inFlight) return none('a full run started by the keeper is still running');
  if (undecided) return none('a CI run on main has not decided its scope yet');
  const n = `${uncovered.length} commit(s) on main no full run covered`;
  if (fullRed) return { action: 'start', reason: `the last full run is red and ${n} came after it` };
  if (epicsDone.length) return { action: 'start', reason: `epic ${epicsDone.join(', ')} has no story left the harness can work; ${n}` };
  if (Number(maxHours) > 0 && oldestHours >= Number(maxHours)) {
    return { action: 'start', reason: `the oldest of ${n} is ${Math.floor(oldestHours)} h old (ci.full_run_max_hours ${maxHours})` };
  }
  return none(`${n}; waiting for the end of an epic or ci.full_run_max_hours`);
}

/** The commits after `sha` on the base branch, oldest first, from a compare result. */
export function uncoveredCommits(cmp) {
  return (cmp?.commits ?? []).map((c) => ({
    sha: c.sha,
    title: String(c.commit?.message ?? '').split('\n')[0],
    date: c.commit?.committer?.date ?? c.commit?.author?.date ?? null,
  }));
}

function epicNumber(id) {
  const list = ghJson(['issue', 'list', '--label', 'type:epic', '--state', 'all', '--search', `${id} in:title`, '--json', 'number,title']) ?? [];
  return list.find((e) => new RegExp(`^\\[?${id}\\]?\\b`).test(e.title))?.number ?? null;
}

function gather(cfg, epics, now = Date.now()) {
  const runs = mainRuns(cfg);
  const stands = standIns(cfg);
  const state = mainState(runs, cfg.ci.required_checks, stands);
  const head = ghJson(['api', `repos/{owner}/{repo}/commits/${cfg.base_branch}`]);
  const full = state.fullAny;
  let uncovered;
  let oldestHours;
  if (!full) {
    uncovered = [{ sha: head.sha, title: String(head.commit?.message ?? '').split('\n')[0], date: head.commit?.committer?.date ?? null }];
    oldestHours = Infinity;
  } else {
    uncovered = full.headSha === head.sha ? [] : uncoveredCommits(ghJson(['api', `repos/{owner}/{repo}/compare/${full.headSha}...${head.sha}`]));
    oldestHours = uncovered[0]?.date ? (now - new Date(uncovered[0].date).getTime()) / 3600e3 : 0;
  }
  const running = runs.filter((r) => r.status !== 'completed');
  const epicsDone = [];
  for (const id of epics) {
    const n = epicNumber(id);
    if (n && epicDone(subIssues(n))) epicsDone.push(id);
  }
  return {
    head_sha: head.sha,
    covered_sha: full?.headSha ?? null,
    uncovered,
    oldestHours,
    inFlight: running.some((r) => r.event === 'workflow_dispatch'),
    undecided: running.some((r) => runScope(r, cfg.ci.required_checks, stands) === 'unknown'),
    epicsDone,
    fullRed: !!full && full.status === 'completed' && full.conclusion !== 'success',
    last_full: full ? { run_id: full.databaseId, sha: full.headSha, event: full.event, status: full.status, conclusion: full.conclusion, url: full.url } : null,
  };
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const cfg = loadConfig();
  const epics = String(args.epics ?? '').split(',').map((s) => s.trim()).filter((s) => /^E\d+$/.test(s));
  const f = gather(cfg, [...new Set(epics)]);
  const d = decideFullRun(f, cfg.ci.full_run_max_hours);
  const out = {
    ...d, head_sha: f.head_sha, covered_sha: f.covered_sha, uncovered: f.uncovered,
    oldest_hours: Number.isFinite(f.oldestHours) ? Math.round(f.oldestHours * 10) / 10 : null,
    in_flight: f.inFlight, epics_done: f.epicsDone, last_full: f.last_full,
  };
  process.stdout.write(JSON.stringify(out) + '\n');
}

if (isMain(import.meta.url)) {
  try { main(); } catch (e) { console.error(e.message); process.exit(3); }
}
