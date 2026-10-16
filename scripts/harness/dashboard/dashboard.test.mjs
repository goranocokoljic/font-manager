// node --test for the go-harness dashboard: the numbers (stats.mjs) and the
// telemetry reader (server.mjs buildPayload). No network, no browser.

import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import {
  collapseStories, filterStories, scope, kpis, costByDay, mustTrend, lensRows, cycleRows, stageRows,
  ciByOs, budgetRows, epicRows, median,
} from './stats.mjs';
import { buildPayload, parseParkingLot, readJsonl } from './server.mjs';

const story = (issue, id, extra = {}) => ({
  issue, id, epic: id.split('.')[0], size: 'S', track: 'core', lane: 'core', outcome: 'ready', pr: issue + 100,
  budget_usd: 15, billed_usd: 10, ci_attempts: 0, ci_failed_platforms: [], review_rounds: 1, must_total: 0,
  must_fixed: 0, should_total: 0, rejected_total: 0, purecontext_calls: 20, claim_ts: '2026-10-20T08:00:00Z',
  green_ts: '2026-10-20T09:00:00Z', claim_to_green_sec: 3600, wall_sec: 4000, ts: '2026-10-20T10:00:00Z', ...extra,
});
const run = (issue, stage, billed, ts, extra = {}) => ({ issue, id: `E1.${issue}`, stage, billed_cost_usd: billed, total_sec: 600, purecontext_calls: 5, ts, ...extra });
const cycle = (issue, n, findings, byLens, extra = {}) => ({ issue, id: `E1.${issue}`, review_cycle: n, ts: `2026-10-2${n}T10:00:00Z`, findings, by_lens: byLens, ...extra });

describe('stories', () => {
  test('a re-run after a block adds its cost and attempts; the newest record sets the outcome', () => {
    const rows = collapseStories([
      story(42, 'E1.6', { outcome: 'blocked', billed_usd: 9, ci_attempts: 3, ci_failed_platforms: ['macos', 'macos', 'macos'], claim_ts: '2026-10-19T08:00:00Z', ts: '2026-10-19T10:00:00Z' }),
      story(42, 'E1.6', { outcome: 'ready', billed_usd: 4, ci_attempts: 1, ci_failed_platforms: ['linux'], claim_ts: '2026-10-20T08:00:00Z' }),
    ]);
    assert.equal(rows.length, 1);
    assert.equal(rows[0].outcome, 'ready');
    assert.equal(rows[0].billed_usd, 13);
    assert.equal(rows[0].ci_attempts, 4);
    assert.deepEqual(rows[0].ci_failed_platforms, ['macos', 'macos', 'macos', 'linux']);
    assert.equal(rows[0].attempts, 2);
    assert.equal(rows[0].first_claim_ts, '2026-10-19T08:00:00Z'); // the first attempt's claim, not the re-run's
  });

  test('filters by period, lane and outcome', () => {
    const all = collapseStories([
      story(1, 'E1.1', { ts: '2026-10-01T10:00:00Z' }),
      story(2, 'E5.1', { lane: 'ui', outcome: 'blocked', ts: '2026-10-20T10:00:00Z' }),
      story(3, 'E1.3', { outcome: 'merged', ts: '2026-10-21T10:00:00Z' }),
    ]);
    const now = new Date('2026-10-22T00:00:00Z');
    assert.deepEqual(filterStories(all, { period: '7' }, now).map((s) => s.issue), [2, 3]);
    assert.deepEqual(filterStories(all, { lane: 'ui' }, now).map((s) => s.issue), [2]);
    assert.deepEqual(filterStories(all, { outcome: 'done' }, now).map((s) => s.issue), [1, 3]);
    assert.deepEqual(filterStories(all, { outcome: 'blocked' }, now).map((s) => s.issue), [2]);
    assert.deepEqual(scope([{ issue: 2 }, { issue: 9 }], filterStories(all, { lane: 'ui' }, now)), [{ issue: 2 }]);
  });
});

describe('review numbers follow the new rules', () => {
  const cycles = [
    cycle(1, 1, { must: 2, should: 1, rejected: 3, resolved: 0 }, { COR: { must: 2, should: 0, rejected: 1 }, TST: { must: 0, should: 1, rejected: 2 } }),
    cycle(1, 2, { must: 0, should: 0, rejected: 0, resolved: 2 }, null, { dispositions: { fixed: 2, disputed: 0 }, review_sec: 120, fix_sec: 300 }),
    cycle(2, 1, { must: 0, should: 2, rejected: 1, resolved: 0 }, { SPEC: { must: 0, should: 2, rejected: 1 } }),
  ];

  test('verifier verdicts per lens come from first reviews only', () => {
    const rows = lensRows(cycles);
    assert.deepEqual(rows.map((r) => [r.lens, r.must, r.should, r.rejected]), [['SPEC', 0, 2, 1], ['COR', 2, 0, 1], ['TST', 0, 1, 2]]);
    assert.equal(rows.find((r) => r.lens === 'COR').rejectRate, 1 / 3);
    assert.equal(rows.find((r) => r.lens === 'TST').rejectRate, 1); // 2 rejected, 0 confirmed
  });

  test('KPIs: must per story from cycle 1, reject rate over judged musts, advisory never mixed in', () => {
    const stories = collapseStories([story(1, 'E1.1', { must_fixed: 2 }), story(2, 'E1.2', { outcome: 'blocked', purecontext_calls: 0, budget_usd: 15, billed_usd: 20 })]);
    const k = kpis(stories, [run(1, 'build', 5, '2026-10-20T08:00:00Z'), run(1, 'review', 3, '2026-10-20T09:00:00Z')], cycles);
    assert.equal(k.stories, 2);
    assert.equal(k.done, 1);
    assert.equal(k.blocked, 1);
    assert.equal(k.billed, 8);
    assert.equal(k.mustPerStory, 1); // median of [2, 0]
    assert.equal(k.mustFixed, 2);
    assert.equal(k.should, 3);
    assert.equal(k.rejectRate, 4 / 6); // rejected 4 (1+2+1) of judged 6 (2 confirmed + 4 rejected)
    assert.equal(k.overBudget, 1);
    assert.equal(k.pcZero, 1);
  });

  test('cycle table separates the full review from verify cycles', () => {
    const rows = cycleRows(cycles);
    assert.deepEqual(rows.map((r) => [r.cycle, r.mode, r.runs, r.must, r.resolved, r.fixed]), [[1, 'full', 2, 2, 0, 0], [2, 'verify', 1, 0, 2, 2]]);
    assert.equal(rows[1].medianFixSec, 300);
  });

  test('the must trend uses each story\'s first review and a rolling average', () => {
    const pts = mustTrend(cycles, collapseStories([story(1, 'E1.1'), story(2, 'E1.2')]));
    assert.deepEqual(pts.map((p) => [p.issue, p.must, p.roll]), [[1, 2, 2], [2, 0, 1]]);
  });
});

describe('cost, stages, CI, budget, epics', () => {
  test('billed cost per day is split by stage and keeps empty days', () => {
    const days = costByDay([run(1, 'build', 5, '2026-10-20T08:00:00'), run(1, 'review', 2, '2026-10-20T15:00:00'), run(2, 'ci-fix', 1, '2026-10-22T09:00:00')]);
    assert.equal(days.length, 3);
    assert.deepEqual(days.map((d) => d.total), [7, 0, 1]);
    assert.equal(days[0].sums.review, 2);
  });

  test('stage rows use real billed cost and add the CI wait as harness time', () => {
    const rows = stageRows([run(1, 'build', 6, 'x'), run(1, 'review', 2, 'x')], [story(1, 'E1.1', { wall_sec: 3000 })]);
    const build = rows.find((r) => r.stage === 'build');
    assert.equal(build.share, 0.75);
    assert.equal(rows.find((r) => r.stage === 'ci wait').medianSec, 1800); // 3000 wall - 2 x 600 claude
  });

  test('CI failures count per platform; budget rows flag overspend; epics group stories', () => {
    const stories = collapseStories([story(1, 'E1.4', { ci_failed_platforms: ['macos', 'bench'] }), story(2, 'E2.1', { billed_usd: 40, budget_usd: 37.5, size: 'M', outcome: 'blocked' })]);
    assert.deepEqual(ciByOs(stories).filter((r) => r.failures).map((r) => [r.os, r.failures]), [['macos', 1], ['bench', 1]]);
    assert.deepEqual(budgetRows(stories).map((r) => [r.id, r.over]), [['E1.4', false], ['E2.1', true]]);
    assert.deepEqual(epicRows(stories).map((r) => [r.epic, r.done, r.blocked]), [['E1', 1, 0], ['E2', 0, 1]]);
    assert.equal(median([3, null, 1]), 2);
  });
});

describe('server data', () => {
  test('reads telemetry, running lanes, KB lessons and the parking lot; skips torn lines', async () => {
    const root = mkdtempSync(join(tmpdir(), 'dash-'));
    try {
      const data = join(root, '.harness');
      mkdirSync(join(data, 'state'), { recursive: true });
      mkdirSync(join(root, 'dev-docs', 'harness', 'kb', 'lessons'), { recursive: true });
      writeFileSync(join(data, 'stories.jsonl'), JSON.stringify(story(1, 'E1.1')) + '\n{"torn":');
      writeFileSync(join(data, 'state', 'ui.json'), JSON.stringify({ issue: 64, id: 'E5.1', stage: 'review' }));
      writeFileSync(join(root, 'dev-docs', 'harness', 'kb', 'lessons', 'x.json'), JSON.stringify({ id: 'x', status: 'active' }));
      writeFileSync(join(root, 'dev-docs', 'harness', 'parking-lot.md'), '# Parking lot\n\n## 2026-10-20\n- [E1.1 #1] crates/fm-core/src/a.rs:3 — old unwrap\n');
      const p = await buildPayload(data, root);
      assert.equal(p.stories.length, 1);
      assert.equal(p.files.stories.skipped, 1);
      assert.deepEqual(p.runs, []);
      assert.deepEqual(p.lanes, [{ lane: 'ui', issue: 64, id: 'E5.1', stage: 'review' }]);
      assert.equal(p.lessons[0].id, 'x');
      assert.deepEqual(p.parking, [{ date: '2026-10-20', story: 'E1.1 #1', note: 'crates/fm-core/src/a.rs:3 — old unwrap' }]);
      assert.deepEqual(await readJsonl(join(data, 'missing.jsonl')), { rows: [], skipped: 0, mtime: null });
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test('parking-lot lines outside the list format are ignored', () => {
    assert.deepEqual(parseParkingLot('Format: `- [<story id> #<issue>] path:line`\n- not a parked line'), []);
  });
});
