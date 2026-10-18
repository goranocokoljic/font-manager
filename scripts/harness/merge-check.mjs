#!/usr/bin/env node
// merge-check.mjs — may the harness merge this PR by itself (merge: auto)?
// The harness runs it after finalize; it changes nothing on GitHub.
//
// Usage:
//   node scripts/harness/merge-check.mjs --pr 57 --track core --epic E3
//
// Verdict JSON (stdout):
//   { action: "merge"|"update"|"sync"|"human"|"block"|"stop", reasons[], sha, behind_by, main_ci }
//   merge  — green, reviewed, up to date with main: merge at `sha`
//   update — behind main: bring the branch up to date, then CI runs again
//   sync   — conflicts with main: the sync stage merges main and resolves them
//   human  — a merge a person must decide (excluded track, a changed fm-api type)
//   block  — cannot merge (draft, closed)
//   stop   — main itself is red: merge nothing until main is green again
// Exit: 0 on any verdict, 3 on error.

import { loadConfig, ghJson, parseArgs, isMain } from './lib.mjs';

const TYPES_TS = 'apps/desktop/src/api/types.ts';

/**
 * Lines of the generated TypeScript types that a PR removes or rewrites. An
 * additive change only adds lines; a removed line means an existing type
 * changed shape. Ignored: the API_VERSION line, doc comments, and the last
 * member of a union that only gains new members (`"a";` becomes `"a" |`).
 */
export function contractRemovals(diff) {
  const out = [];
  const added = new Set();
  let inTypes = false;
  for (const line of diff.split(/\r?\n/)) {
    if (line.startsWith('diff --git ')) inTypes = line.endsWith(` b/${TYPES_TS}`);
    else if (inTypes && line.startsWith('+') && !line.startsWith('+++')) added.add(line.slice(1).trim());
  }
  inTypes = false;
  for (const line of diff.split(/\r?\n/)) {
    if (line.startsWith('diff --git ')) {
      inTypes = line.endsWith(` b/${TYPES_TS}`);
      continue;
    }
    if (!inTypes || !line.startsWith('-') || line.startsWith('---')) continue;
    const text = line.slice(1).trim();
    if (!text || text.startsWith('export const API_VERSION')) continue;
    // Doc comments change no type: a reworded comment is not a contract change.
    if (/^(\/\*\*?|\*\/?|\/\/)/.test(text)) continue;
    // A union that gains a member: its old last member only gets a ` |`.
    const base = text.endsWith(';') ? `${text.slice(0, -1).trimEnd()} |` : null;
    if (base && [...added].some((a) => a.startsWith(base))) continue;
    out.push(text);
  }
  return out;
}

/** The newest run per check name: a re-run leaves the old (failed) one behind. */
export function latestPerName(runs) {
  const byName = new Map();
  for (const r of runs) {
    const prev = byName.get(r.name);
    if (!prev || new Date(r.started_at ?? 0) > new Date(prev.started_at ?? 0)) byName.set(r.name, r);
  }
  return [...byName.values()];
}

/** Overall conclusion of the check runs on one commit: success, failure or pending. */
export function commitCi(runs) {
  if (!runs.length) return 'pending';
  if (runs.some((r) => ['failure', 'timed_out', 'cancelled', 'action_required'].includes(r.conclusion))) return 'failure';
  if (runs.some((r) => r.status !== 'completed')) return 'pending';
  return 'success';
}

/** The decision, from facts gathered by `gather` (pure, so it is testable). */
export function decide(f, cfg) {
  const reasons = [];
  if (f.state !== 'OPEN') return { action: 'block', reasons: [`PR is ${f.state.toLowerCase()}`] };
  if (f.isDraft) return { action: 'block', reasons: ['PR is still a draft'] };
  if (f.mainCi === 'failure') return { action: 'stop', reasons: [`main is red at ${f.baseSha.slice(0, 7)}; merge nothing until it is fixed`] };
  if (f.mergeable === 'CONFLICTING') return { action: 'sync', reasons: ['the branch conflicts with main'] };
  if ((cfg.auto_merge?.exclude_tracks ?? []).includes(f.track)) reasons.push(`track ${f.track} is merged by a human`);
  if (f.removals.length && !(cfg.contract_epics ?? []).includes(f.epic)) {
    reasons.push(`changes existing fm-api types (${f.removals.length} line(s) of ${TYPES_TS} removed or rewritten)`);
  }
  if (reasons.length) return { action: 'human', reasons };
  if (f.behindBy > 0) return { action: 'update', reasons: [`${f.behindBy} commit(s) behind main`] };
  return { action: 'merge', reasons: [] };
}

/** The PR's patch of the generated types file, as a one-file git diff ('' when untouched). */
function typesPatch(pr) {
  const pages = ghJson(['api', '--paginate', '--slurp', `repos/{owner}/{repo}/pulls/${pr}/files?per_page=100`]) ?? [];
  const file = pages.flat().find((f) => f.filename === TYPES_TS);
  if (!file) return '';
  if (file.patch === undefined) return `diff --git a/${TYPES_TS} b/${TYPES_TS}\n-(patch too large to inspect)`;
  return `diff --git a/${TYPES_TS} b/${TYPES_TS}\n${file.patch}`;
}

function gather(pr, track, epic, cfg) {
  const v = ghJson(['pr', 'view', String(pr), '--json', 'state,isDraft,mergeable,headRefOid,baseRefName']);
  const base = ghJson(['api', `repos/{owner}/{repo}/commits/${v.baseRefName ?? cfg.base_branch}`]);
  const cmp = ghJson(['api', `repos/{owner}/{repo}/compare/${base.sha}...${v.headRefOid}`]);
  const runs = ghJson(['api', `repos/{owner}/{repo}/commits/${base.sha}/check-runs?per_page=100`])?.check_runs ?? [];
  const required = (cfg.ci.required_checks ?? []).map((n) => n.toLowerCase().split('/').pop().trim());
  const mainRuns = latestPerName(runs).filter((r) => required.includes(r.name.toLowerCase()) || required.includes(r.name.toLowerCase().split('/').pop().trim()));
  return {
    state: v.state,
    isDraft: v.isDraft,
    mergeable: v.mergeable,
    sha: v.headRefOid,
    baseSha: base.sha,
    behindBy: cmp.behind_by ?? 0,
    mainCi: commitCi(mainRuns),
    // The per-file API, not `gh pr diff`: a full diff over 300 files is refused.
    removals: contractRemovals(typesPatch(pr)),
    track,
    epic,
  };
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (!args.pr) throw new Error('usage: merge-check.mjs --pr <number> --track <track> --epic <E3>');
  const cfg = loadConfig();
  const f = gather(args.pr, String(args.track ?? ''), String(args.epic ?? ''), cfg);
  const d = decide(f, cfg);
  process.stdout.write(JSON.stringify({ ...d, sha: f.sha, behind_by: f.behindBy, main_ci: f.mainCi }) + '\n');
}

if (isMain(import.meta.url)) {
  try { main(); } catch (e) { console.error(e.message); process.exit(3); }
}
