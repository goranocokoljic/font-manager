#!/usr/bin/env node
// loop-check.mjs — convergence guard, run by go-harness after every finished story.
//
// Usage: node scripts/harness/loop-check.mjs [--dir .harness] [--budget]
// Exit: 0 ok, 1 warn (log and continue), 2 stop (halt the lane).
// Last stdout line: LOOPCHECK: <ok|warn|stop> | <reasons or ->
//
// Reads .harness/stories.jsonl (one record per finished story). Signals:
//   S1 budget burn   — stories finishing over their size budget; a signal only with
//                      --budget (the lane runs with -Budget / budget_enforced), else info
//   S2 no convergence — confirmed must-findings per story not going down
//   S3 blocked streak — consecutive stories that ended harness:blocked
//   S4 PureContext   — stories that ran with zero PureContext calls (the index is
//                      not being used: stale index, MCP not loaded, or the skill drifted)
// The profile/toprope history this replaces: 562 findings over 23 cycles, 74% Low,
// review the dominant cost of every task. The guard exists so a loop like that
// stops a lane instead of quietly spending the night.

import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { REPO_ROOT, parseArgs } from './lib.mjs';

const W = 5;
const median = (a) => { if (!a.length) return 0; const s = [...a].sort((x, y) => x - y); const m = s.length >> 1; return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2; };

const args = parseArgs(process.argv.slice(2));
const file = resolve(String(args.dir ?? resolve(REPO_ROOT, '.harness')), 'stories.jsonl');
const recs = existsSync(file)
  ? readFileSync(file, 'utf8').split(/\r?\n/).filter((l) => l.trim()).map((l) => { try { return JSON.parse(l); } catch { return null; } }).filter(Boolean)
  : [];
recs.sort((a, b) => String(a.ts).localeCompare(String(b.ts)));
const last = recs.slice(-W);
const prev = recs.slice(-2 * W, -W);
const warns = []; const stops = []; const info = [];

// S1 budget burn
if (last.length) {
  const over = last.filter((r) => r.budget_usd && r.billed_usd > r.budget_usd);
  info.push(`over budget: ${over.length}/${last.length} recent stories`);
  // Budgets are opt-in (#275): without --budget, spend is shown, never a stop.
  if (args.budget) {
    if (over.length >= 3) stops.push(`${over.length} of the last ${last.length} stories went over budget`);
    else if (over.length >= 2) warns.push(`${over.length} of the last ${last.length} stories went over budget`);
  }
}

// S2 must-findings not converging
if (last.length === W) {
  const mLast = median(last.map((r) => r.must_total ?? 0));
  const mPrev = prev.length === W ? median(prev.map((r) => r.must_total ?? 0)) : null;
  const newest = last[last.length - 1].must_total ?? 0;
  info.push(`confirmed must-findings/story median: ${mLast}${mPrev !== null ? ` (prev ${mPrev})` : ''}`);
  if (mLast >= 6) stops.push(`median ${mLast} confirmed must-findings per story`);
  else if (mLast >= 3 && newest >= 3 && (mPrev === null || mLast >= mPrev)) warns.push(`must-findings not converging (median ${mLast}, newest ${newest})`);
}

// S3 blocked streak
let streak = 0;
for (let i = recs.length - 1; i >= 0 && recs[i].outcome === 'blocked'; i--) streak++;
if (streak >= 3) stops.push(`${streak} stories in a row ended blocked`);
else if (streak === 2) warns.push('2 stories in a row ended blocked');

// S4 PureContext not used
const noPc = last.filter((r) => (r.purecontext_calls ?? 0) === 0 && r.outcome !== 'blocked');
if (noPc.length >= 2) warns.push(`${noPc.length} recent stories made zero PureContext calls (check: claude mcp list; index freshness)`);

const verdict = stops.length || warns.length >= 3 ? 'stop' : warns.length ? 'warn' : 'ok';
for (const l of info) console.log(`  ${l}`);
for (const w of warns) console.log(`  WARN: ${w}`);
for (const s of stops) console.log(`  STOP-SIGNAL: ${s}`);
if (verdict !== 'ok') console.log('  Pause the lane, read .harness/fm-run-summary.md and the last review files, then decide.');
console.log(`LOOPCHECK: ${verdict} | ${[...stops, ...warns].join('; ') || '-'}`);
process.exit(verdict === 'stop' ? 2 : verdict === 'warn' ? 1 : 0);
