#!/usr/bin/env node
// review-counts.mjs — count the findings in an /fm-review file.
//
// Usage:
//   node scripts/harness/review-counts.mjs .harness/reviews/issue-42-review-1.md
//     -> {"must":1,"should":2,"rejected":3,"resolved":0,"raw":7,"unique":6}
//   node scripts/harness/review-counts.mjs .harness/reviews/issue-42-review-1.md --marker 1
//     -> GOCYCLE_METRIC: review_done | review_cycle=1 | must=1 should=2 rejected=3 resolved=0 raw=7
//
// The file's "## Findings index" table has one row per unique finding:
//   | # | Sev | Status | Where | Headline | Lens IDs |
// Sev is `must` or `should`. Status is set by the review's verify step:
//   CONFIRMED  a must finding the verifier reproduced from the code   -> counts as must (blocker)
//   NEW        verify mode: a must-level regression in the fix delta   -> counts as must
//   OPEN       verify mode: a prior must finding the fix did not solve -> counts as must
//   RESOLVED   verify mode: a prior must finding the fix solved        -> resolved
//   REJECTED   the verifier could not reproduce it                     -> rejected (not a blocker)
//   ADVISORY   a should finding (never verified, never blocks)         -> should
// Rows are the truth; `raw` comes from the "Counts: ... (raw=N unique=M)" line.

import { readFileSync } from 'node:fs';
import { isMain } from './lib.mjs';

const MUST_STATUSES = new Set(['confirmed', 'new', 'open']);

export function countFindings(text) {
  const lines = text.split(/\r?\n/);
  const start = lines.findIndex((l) => /^##\s+Findings index/i.test(l.trim()));
  const c = { must: 0, should: 0, rejected: 0, resolved: 0, raw: 0, unique: 0 };
  if (start < 0) return { ...c, source: 'none' };
  let header = null;
  for (let i = start + 1; i < lines.length; i++) {
    const line = lines[i].trim();
    if (/^##\s/.test(line)) break;
    const m = /\(raw=(\d+)\s+unique=(\d+)\)/.exec(line);
    if (m) c.raw = Number(m[1]);
    if (!line.startsWith('|')) continue;
    const cells = line.split('|').slice(1, -1).map((x) => x.trim().toLowerCase());
    if (cells[0] === '#') { header = cells; continue; }
    if (!header || /^:?-+:?$/.test(cells[0])) continue;
    const sev = cells[header.indexOf('sev')] ?? '';
    const status = cells[header.indexOf('status')] ?? '';
    c.unique++;
    if (status === 'rejected') c.rejected++;
    else if (status === 'resolved') c.resolved++;
    else if (sev === 'must' && MUST_STATUSES.has(status)) c.must++;
    else if (sev === 'should') c.should++;
  }
  if (!c.raw) c.raw = c.unique;
  return { ...c, source: 'index' };
}

export function markerLine(c, cycle) {
  return `GOCYCLE_METRIC: review_done | review_cycle=${cycle} | must=${c.must} should=${c.should} rejected=${c.rejected} resolved=${c.resolved} raw=${c.raw}`;
}

if (isMain(import.meta.url)) {
  const argv = process.argv.slice(2);
  const file = argv.find((a) => !a.startsWith('--'));
  const mi = argv.indexOf('--marker');
  const cycle = mi >= 0 ? Number(argv[mi + 1]) : null;
  if (!file || (mi >= 0 && !(cycle >= 1))) {
    console.error('usage: review-counts.mjs <review-file.md> [--marker <cycle>]');
    process.exit(2);
  }
  const counts = countFindings(readFileSync(file, 'utf8'));
  process.stdout.write((cycle ? markerLine(counts, cycle) : JSON.stringify(counts)) + '\n');
}
