#!/usr/bin/env node
// epic-sync.mjs — tell each epic its stories' story.
//
// Usage: node scripts/harness/epic-sync.mjs [--dir .harness] [--dry-run]
//
// For every story the harness finished (.harness/stories.jsonl) whose issue is
// now CLOSED (a human merged its PR, or merge: auto-sm did), post one summary
// comment on the parent epic — once (.harness/epic-posted.json remembers).
// With `close_epics: true` in go-harness.config.json, an epic whose sub-issues
// are all closed is closed too. Run by go-harness at lane start and lane end;
// safe to run by hand.

import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { REPO_ROOT, loadConfig, gh, ghJson, subIssues, parseArgs, isMain } from './lib.mjs';

export function summaryLine(r) {
  const dur = r.claim_to_green_sec ? `${Math.floor(r.claim_to_green_sec / 3600)}h${String(Math.round((r.claim_to_green_sec % 3600) / 60)).padStart(2, '0')}m` : 'n/a';
  const failed = r.ci_failed_platforms?.length ? ` (failed: ${r.ci_failed_platforms.join(', ')})` : '';
  const bench = r.bench ? Object.entries(r.bench).map(([k, m]) => `${k} ${m.value}${m.unit ?? ''}${m.target != null ? `/${m.target}` : ''}`).join(', ') : '';
  return [
    `**${r.id} #${r.issue}** closed — PR #${r.pr ?? '?'}`,
    `$${Number(r.billed_usd ?? 0).toFixed(2)} of $${r.budget_usd} budget`,
    `CI attempts ${r.ci_attempts ?? 0}${failed}`,
    `review rounds ${r.review_rounds ?? 0}, must fixed ${r.must_fixed ?? 0}, advisory ${r.should_total ?? 0}`,
    `claim→green ${dur}`,
    bench && `bench: ${bench}`,
  ].filter(Boolean).join(' · ');
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const cfg = loadConfig();
  const dir = resolve(String(args.dir ?? resolve(REPO_ROOT, '.harness')));
  const storiesFile = resolve(dir, 'stories.jsonl');
  const postedFile = resolve(dir, 'epic-posted.json');
  if (!existsSync(storiesFile)) { console.log('epic-sync: no stories yet'); return; }

  const latest = new Map();
  for (const l of readFileSync(storiesFile, 'utf8').split(/\r?\n/)) {
    if (!l.trim()) continue;
    try { const r = JSON.parse(l); if (['ready', 'merged'].includes(r.outcome)) latest.set(r.issue, r); } catch { /* skip */ }
  }
  const posted = new Set(existsSync(postedFile) ? JSON.parse(readFileSync(postedFile, 'utf8')) : []);
  const epics = ghJson(['issue', 'list', '--label', 'type:epic', '--state', 'all', '--limit', '200', '--json', 'number,title,state']) ?? [];
  const epicOf = (id) => epics.find((e) => e.title.startsWith(`${String(id).split('.')[0]} `));
  const touched = new Set();

  for (const r of latest.values()) {
    if (posted.has(r.issue)) continue;
    const st = ghJson(['issue', 'view', String(r.issue), '--json', 'state'], { allowFail: true });
    if (st?.state !== 'CLOSED') continue;
    const epic = epicOf(r.id);
    if (!epic) continue;
    const line = summaryLine(r);
    if (args['dry-run']) console.log(`[dry] #${epic.number}: ${line}`);
    else { gh(['issue', 'comment', String(epic.number), '--body-file', '-'], { input: line }); posted.add(r.issue); }
    touched.add(epic.number);
  }
  if (!args['dry-run']) writeFileSync(postedFile, JSON.stringify([...posted]) + '\n', 'utf8');

  if (cfg.close_epics) {
    for (const n of touched) {
      const subs = subIssues(n);
      if (subs.length && subs.every((s) => s.state === 'closed')) {
        if (args['dry-run']) console.log(`[dry] would close epic #${n}`);
        else gh(['issue', 'close', String(n), '--comment', `All ${subs.length} stories are closed. Closed by go-harness.`]);
      }
    }
  }
  console.log(`epic-sync: ${touched.size} epic(s) updated`);
}

if (isMain(import.meta.url)) {
  try { main(); } catch (e) { console.error(`epic-sync: ${e.message}`); process.exit(2); }
}
