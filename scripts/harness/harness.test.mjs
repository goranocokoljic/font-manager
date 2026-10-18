// node --test scripts/harness/ — unit tests for the go-harness scripts.
// No network: GitHub calls are replaced by fakes (planLane takes checkDeps).

import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import {
  parseMeta, parseCriteria, isNumericCriterion, compareStories, areaOf, areasOverlap,
  predictAreas, storyBudget, laneOf, DEFAULT_CONFIG,
} from './lib.mjs';
import { staticReadiness, planLane, humanWaits, HUMAN_MERGE } from './pick.mjs';
import { editCriterion } from './issue.mjs';
import { countFindings, markerLine } from './review-counts.mjs';
import { applyItems, matchesPaths, retrieveDigest, rulesMarkdown, readStore } from './kb.mjs';
import { matchCheck, osOf, excerpt, infraReason, latestRuns } from './ci-wait.mjs';
import { summaryLine } from './epic-sync.mjs';
import { contractRemovals, commitCi, decide, latestPerName } from './merge-check.mjs';

const cfg = structuredClone(DEFAULT_CONFIG);

const BODY = `**Epic:** E1 Foundation and fm-api
**Size:** S

## Acceptance criteria

- [ ] tracing with a rotating log file
- [x] Config lives in the platform app-data folder
- [ ] Search answers in < 50 ms on the reference library

## Definition of done

- [ ] Merged to \`main\` with a PR that references this issue

<!-- fm-meta
id: E1.6
epic: E1
phase: 1
size: S            # S ≤ 1 day
track: core
human: false
depends: [E1.1, E1.4]
-->`;

describe('fm-meta and criteria', () => {
  test('parses the fm-meta block, strips comments, reads depends and human', () => {
    const m = parseMeta(BODY);
    assert.equal(m.id, 'E1.6');
    assert.equal(m.size, 'S');
    assert.equal(m.phase, 1);
    assert.equal(m.human, false);
    assert.deepEqual(m.depends, ['E1.1', 'E1.4']);
    assert.deepEqual(parseMeta('[E1]\n<!-- fm-meta\nid: E1\ndepends: []\n-->').depends, []);
    assert.equal(parseMeta('no block'), null);
  });

  test('reads only the acceptance-criteria checkboxes, with numeric detection', () => {
    const c = parseCriteria(BODY);
    assert.equal(c.length, 3);
    assert.deepEqual(c.map((x) => x.checked), [false, true, false]);
    assert.deepEqual(c.map((x) => x.numeric), [false, false, true]);
    assert.ok(isNumericCriterion('cold start < 1 s'));
    assert.ok(isNumericCriterion('idle memory < 350 MB'));
    assert.ok(!isNumericCriterion('E3.2 handles 2 fonts'));
  });

  test('tick flips exactly the chosen box and keeps the rest of the body', () => {
    const { body } = editCriterion(BODY, 1, 'tick');
    assert.match(body, /- \[x\] tracing with a rotating log file/);
    assert.match(body, /- \[ \] Search answers/);
    assert.match(body, /- \[ \] Merged to/); // DoD untouched
    const pend = editCriterion(BODY, 3, 'pending').body;
    assert.match(pend, /- \[ \] Search answers in < 50 ms on the reference library _\(verified locally, pending E9\.1\)_/);
    assert.equal(editCriterion(pend, 3, 'pending').body, pend, 'pending is idempotent');
    assert.match(editCriterion(pend, 3, 'tick').body, /- \[x\] Search answers in < 50 ms on the reference library$/m);
    assert.throws(() => editCriterion(BODY, 9, 'tick'), /no acceptance criterion #9/);
  });
});

const story = (id, extra = {}) => {
  const { labels = ['type:story'], phase = 1, size = 'M', track = 'core', depends = [], body = '', ...rest } = extra;
  return { number: Number(id.replace(/\D/g, '')), title: `${id} t`, body, labels, state: 'OPEN', assignees: [], meta: { id, phase, size, track, depends, human: labels.includes('human') }, ...rest };
};

describe('ordering, lanes, areas', () => {
  test('orders by phase, epic number, story number, then size', () => {
    const list = [story('E10.1'), story('E2.3', { phase: 2 }), story('E2.10'), story('E2.2', { size: 'L' })];
    assert.deepEqual(list.sort(compareStories).map((s) => s.meta.id), ['E2.2', 'E2.10', 'E10.1', 'E2.3']);
  });

  test('tracks map to lanes and budgets follow size', () => {
    assert.equal(laneOf('platform', cfg), 'core');
    assert.equal(laneOf('ui', cfg), 'ui');
    assert.equal(storyBudget('S', cfg), 15);
    assert.equal(storyBudget('M', cfg), 37.5);
    assert.equal(storyBudget('L', cfg), 75);
  });

  test('areas: crates, frontend subtrees, generated files ignored', () => {
    assert.equal(areaOf('crates/fm-core/src/scan.rs'), 'crates/fm-core');
    assert.equal(areaOf('apps/desktop/src/components/List.vue'), 'apps/desktop/src/components');
    assert.equal(areaOf('apps/desktop/src-tauri/src/main.rs'), 'apps/desktop/src-tauri');
    assert.equal(areaOf('apps/desktop/src/api/types.ts'), null);
    assert.equal(areaOf('Cargo.lock'), null);
    assert.ok(areasOverlap('apps/desktop/src', 'apps/desktop/src/components'));
    assert.ok(!areasOverlap('crates/fm-core', 'crates/fm-core-x'));
    assert.deepEqual(predictAreas(story('E5.1', { track: 'ui', body: 'needs a new fm-api call' })).sort(), ['apps/desktop/src', 'crates/fm-api']);
  });
});

describe('Definition of Ready and lane planning', () => {
  test('static readiness rejects excluded labels, human meta and foreign assignees', () => {
    assert.equal(staticReadiness(story('E1.4'), 'bot'), null);
    assert.match(staticReadiness(story('E1.4', { labels: ['type:story', 'criteria:tbd'] }), 'bot'), /criteria:tbd/);
    assert.match(staticReadiness(story('E1.1', { labels: ['type:story', 'human'] }), 'bot'), /human/);
    assert.match(staticReadiness({ ...story('E1.4'), assignees: [{ login: 'someone' }] }, 'bot'), /assigned/);
    assert.equal(staticReadiness({ ...story('E1.4'), assignees: [{ login: 'bot' }] }, 'bot'), null);
  });

  const world = (stories, prs = []) => ({ stories, prs });
  const deps = (map) => (n) => map[n] ?? [];

  test('starts the first ready story in lane order, skipping open blockers', () => {
    const w = world([story('E1.5'), story('E1.4'), story('E2.1', { track: 'ui' })]);
    const p = planLane('core', w, cfg, { checkDeps: deps({ 14: [{ number: 37, state: 'open' }] }) });
    assert.equal(p.action, 'start');
    assert.equal(p.story.id, 'E1.5');
    assert.match(p.reasons[0].why, /blocked by open #37/);
  });

  test('resumes a claimed story whose PR is still a draft', () => {
    const w = world([story('E1.4', { labels: ['type:story', 'harness:in-progress'] }), story('E1.5')],
      [{ number: 70, storyId: 'E1.4', isDraft: true, areas: ['crates/fm-core'], headRefName: 'E1.4-ref' }]);
    const p = planLane('core', w, cfg, { checkDeps: deps({}) });
    assert.equal(p.action, 'resume');
    assert.equal(p.story.id, 'E1.4');
    assert.equal(p.story.pr.number, 70);
  });

  test('waits when the lane already has a ready PR for a human (lane_max_unmerged 1)', () => {
    const w = world([story('E1.4', { labels: ['type:story', 'harness:in-progress'] }), story('E1.5')],
      [{ number: 70, storyId: 'E1.4', isDraft: false, areas: [], headRefName: 'E1.4-ref' }]);
    assert.equal(planLane('core', w, cfg, { checkDeps: deps({}) }).action, 'wait');
  });

  test('a candidate overlapping the other lane\'s open PR waits; a disjoint one starts', () => {
    const w = world([
      story('E5.1', { track: 'ui', body: 'touches apps/desktop/src/stores' }),
      story('E5.2', { track: 'ui', body: 'only crates? no: fm-cli' }),
      story('E3.1', { labels: ['type:story', 'harness:in-progress'], track: 'core' }),
    ], [{ number: 80, storyId: 'E3.1', isDraft: true, areas: ['apps/desktop/src/stores', 'crates/fm-core'], headRefName: 'E3.1-x' }]);
    // E5.2 predicts apps/desktop/src (ui home) which overlaps apps/desktop/src/stores too -> both wait.
    const p = planLane('ui', w, cfg, { checkDeps: deps({}) });
    assert.equal(p.action, 'idle');
    assert.equal(p.reasons.length, 2);
    assert.match(p.reasons[0].why, /overlaps apps\/desktop\/src\/stores \(PR #80, E3.1\)/);
    const w2 = world(w.stories, [{ ...w.prs[0], areas: ['crates/fm-core'] }]);
    assert.equal(planLane('ui', w2, cfg, { checkDeps: deps({}) }).story.id, 'E5.1');
  });

  test('a blocked or human-merge PR of the other lane holds no area', () => {
    const pr = { number: 80, storyId: 'E3.1', isDraft: true, areas: ['apps/desktop/src'], headRefName: 'E3.1-x' };
    const ui = story('E5.1', { track: 'ui' });
    for (const extra of ['harness:blocked', HUMAN_MERGE]) {
      const w = world([ui, story('E3.1', { labels: ['type:story', 'harness:in-progress', extra] })], [pr]);
      assert.equal(planLane('ui', w, cfg, { checkDeps: deps({}) }).story.id, 'E5.1', extra);
    }
    const blockedOnly = world([ui, story('E3.1', { labels: ['type:story', 'harness:blocked'] })], [pr]);
    assert.equal(planLane('ui', blockedOnly, cfg, { checkDeps: deps({}) }).story.id, 'E5.1');
  });

  test('lists human stories with the stories they block', () => {
    const hw = humanWaits([story('E1.1', { labels: ['type:story', 'human'] }), story('E1.4', { depends: ['E1.1'] })]);
    assert.deepEqual(hw.map((h) => [h.id, h.blocks]), [['E1.1', ['E1.4']]]);
  });
});

const REVIEW = `# fm-review — E1.6 #42 — cycle 1 (full)

## Findings index

| # | Sev | Status | Where | Headline | Lens IDs |
|---|-----|--------|-------|----------|----------|
| 1 | must | CONFIRMED | crates/fm-core/src/scan.rs:88 | panics on truncated head table | COR-1, TST-2 |
| 2 | should | ADVISORY | crates/fm-core/src/log.rs:10 | rotation size hard-coded | SPEC-1 |
| 3 | must | REJECTED | crates/fm-core/src/index.rs:12 | lock across await | COR-2 |

Counts: must=1 should=1 rejected=1 resolved=0 (raw=4 unique=3)

## Verifier
COR-1 | CONFIRMED | ...

## [COR] Correctness
### Must
#### COR-9. should never be counted (lens section, not index)
`;

describe('review counts', () => {
  test('counts verified rows of the index only', () => {
    const c = countFindings(REVIEW);
    const { by_lens: byLens, ...counts } = c;
    assert.deepEqual(counts, { must: 1, should: 1, rejected: 1, resolved: 0, raw: 4, unique: 3, source: 'index' });
    assert.deepEqual(byLens, {
      COR: { must: 1, should: 0, rejected: 1, resolved: 0 },
      TST: { must: 1, should: 0, rejected: 0, resolved: 0 },
      SPEC: { must: 0, should: 1, rejected: 0, resolved: 0 },
    });
    assert.equal(markerLine(c, 1), 'GOCYCLE_METRIC: review_done | review_cycle=1 | must=1 should=1 rejected=1 resolved=0 raw=4');
  });

  test('verify mode: OPEN and NEW block, RESOLVED does not', () => {
    const v = REVIEW.replace('| must | CONFIRMED |', '| must | RESOLVED |').replace('| should | ADVISORY |', '| must | NEW |');
    const c = countFindings(v);
    assert.equal(c.must, 1);
    assert.equal(c.resolved, 1);
    assert.equal(countFindings('# no index').source, 'none');
  });
});

describe('review KB', () => {
  const item = (n, extra = {}) => ({ title: 'Never panic on font data', category: 'error-handling', rule: 'Return Health::Broken.', severity: 'should', source_issue: n, file_globs: ['crates/fm-core/src/parse/**'], ...extra });

  test('a lesson seen in two stories becomes active and keeps the stronger severity', () => {
    const lessons = [];
    applyItems(lessons, [item(42)]);
    assert.equal(lessons[0].status, 'candidate');
    const { summary } = applyItems(lessons, [item(43, { match_id: 'never-panic-on-font-data', severity: 'must' })]);
    assert.equal(lessons.length, 1);
    assert.equal(lessons[0].status, 'active');
    assert.equal(lessons[0].severity, 'must');
    assert.deepEqual(lessons[0].source_issues, [42, 43]);
    assert.equal(summary.promoted, 1);
    assert.throws(() => applyItems([], [item(1, { severity: 'low' })]), /must\|should/);
  });

  test('retrieval matches by area and stays silent otherwise', () => {
    const l = { id: 'x', title: 'X', category: 'testing', rule: 'r', severity: 'must', status: 'active', source_issues: [1, 2], file_globs: ['crates/fm-core/src/parse/**'] };
    assert.ok(matchesPaths(l, ['crates/fm-core/**']));
    assert.ok(!matchesPaths(l, ['apps/desktop/src/**']));
    assert.match(retrieveDigest([l], ['crates/fm-core/src/parse/head.rs']), /Review-KB pitfalls/);
    assert.equal(retrieveDigest([l], ['apps/desktop/src/**']), '<!-- review-KB: no relevant active lessons -->');
    assert.match(rulesMarkdown([{ ...l, status: 'graduated' }]), /## testing\n\n- \*\*X\.\*\* r/);
  });

  test('an empty or missing store reads as no lessons', () => {
    const d = mkdtempSync(join(tmpdir(), 'kb-'));
    try { assert.deepEqual(readStore(join(d, 'missing')), []); } finally { rmSync(d, { recursive: true, force: true }); }
  });
});

describe('ci-wait helpers', () => {
  const runs = [{ name: 'windows' }, { name: 'macos' }, { name: 'ci / linux' }, { name: 'bench' }];
  test('matches a required check by full name or job part', () => {
    assert.equal(matchCheck('ci / windows', runs).name, 'windows');
    assert.equal(matchCheck('ci / linux', runs).name, 'ci / linux');
    assert.equal(matchCheck('bench / bench', runs).name, 'bench');
    assert.equal(matchCheck('ci / freebsd', runs), null);
    assert.equal(osOf('ci / macos'), 'macos');
    assert.equal(osOf('ubuntu-latest'), 'linux');
  });

  test('the bench wait skips the skipped bench check an unlabeled push left behind', () => {
    // E9.1 review: every PR push runs bench.yml with its jobs skipped; the
    // labeled run's `bench` check appears only after the library and measure jobs.
    const skipped = { name: 'bench', conclusion: 'skipped', status: 'completed', started_at: '2026-10-18T10:00:00Z' };
    const real = { name: 'bench', conclusion: null, status: 'in_progress', started_at: '2026-10-18T11:00:00Z' };
    assert.equal(matchCheck('bench / bench', latestRuns([skipped], { ignoreSkipped: true })), null, 'keep waiting');
    assert.equal(matchCheck('bench / bench', latestRuns([skipped, real], { ignoreSkipped: true })), real);
    // A skipped run newer than the real one (another label added) does not hide it.
    const later = { ...skipped, started_at: '2026-10-18T12:00:00Z' };
    assert.equal(matchCheck('bench / bench', latestRuns([real, later], { ignoreSkipped: true })), real);
    // The required ci checks keep the newest run whatever its conclusion.
    assert.equal(latestRuns([real, later])[0], later);
  });

  test('log excerpt keeps the error and its context, drops timestamps', () => {
    const log = ['2026-10-16T10:00:00.000Z Compiling fm-core', ...Array(50).fill('noise'), '2026-10-16T10:01:00.000Z error[E0308]: mismatched types', '  --> crates/fm-core/src/a.rs:3:5'].join('\n');
    const ex = excerpt(log);
    assert.match(ex, /^noise/m);
    assert.match(ex, /error\[E0308\]: mismatched types\n {2}--> crates/);
    assert.doesNotMatch(ex, /2026-10-16T/);
    assert.ok(ex.split('\n').length < 20);
  });

  test('a job GitHub refused to start is an infra problem, a failed test is not', () => {
    const billing = [{ message: "The job was not started because recent account payments have failed or your spending limit needs to be increased. Please check the 'Billing & plans' section in your settings" }];
    assert.match(infraReason(billing), /not started because/);
    assert.equal(infraReason([{ message: 'Process completed with exit code 100.' }]), null);
    assert.equal(infraReason([]), null);
  });
});

describe('auto merge', () => {
  const auto = { ...structuredClone(DEFAULT_CONFIG), merge: 'auto' };
  const facts = (extra = {}) => ({
    state: 'OPEN', isDraft: false, mergeable: 'MERGEABLE', sha: 'abc', baseSha: 'def1234567', behindBy: 0,
    mainCi: 'success', removals: [], track: 'core', epic: 'E3', ...extra,
  });

  test('merges a green, reviewed, up-to-date PR', () => {
    assert.equal(decide(facts(), auto).action, 'merge');
    assert.equal(decide(facts({ mainCi: 'pending' }), auto).action, 'merge');
  });

  test('brings a PR that is behind main up to date first', () => {
    assert.deepEqual(decide(facts({ behindBy: 2 }), auto), { action: 'update', reasons: ['2 commit(s) behind main'] });
  });

  test('leaves platform PRs and changed fm-api types for a human; contract epics may change types', () => {
    assert.equal(decide(facts({ track: 'platform' }), auto).action, 'human');
    assert.match(decide(facts({ removals: ['total: number,'] }), auto).reasons[0], /changes existing fm-api types/);
    assert.equal(decide(facts({ removals: ['total: number,'], epic: 'E1' }), auto).action, 'merge');
  });

  test('syncs on conflicts, blocks drafts, stops when main is red', () => {
    assert.equal(decide(facts({ mergeable: 'CONFLICTING' }), auto).action, 'sync');
    assert.equal(decide(facts({ isDraft: true }), auto).action, 'block');
    assert.equal(decide(facts({ mainCi: 'failure', track: 'platform' }), auto).action, 'stop');
  });

  test('only removed or rewritten lines of the generated types count as a contract change', () => {
    const diff = [
      'diff --git a/crates/fm-api/src/search.rs b/crates/fm-api/src/search.rs',
      '-    pub old: u32,',
      'diff --git a/apps/desktop/src/api/types.ts b/apps/desktop/src/api/types.ts',
      '--- a/apps/desktop/src/api/types.ts',
      '+++ b/apps/desktop/src/api/types.ts',
      '-export const API_VERSION: ApiVersion = { major: 0, minor: 2, patch: 0 };',
      '+export const API_VERSION: ApiVersion = { major: 0, minor: 3, patch: 0 };',
      '+\tnewField?: string | null,',
      '-"permanent";',
      '+"permanent" | "pinned";',
      '-\tweight?: WeightRange | null,',
      '-"unsupported";',
      '+"unsupported" | ',
      '+"sidecarConflict";',
      '-\t/**  Weight range. */',
      '+\t/**  Weight range; a variable face passes when its `wght` axis reaches it. */',
      '- *  Old wording of a long comment.',
      '- */',
    ].join('\n');
    // A union that gains members is additive; a removed field is not.
    assert.deepEqual(contractRemovals(diff), ['weight?: WeightRange | null,']);
    assert.deepEqual(contractRemovals(diff.split('\n').filter((l) => !l.startsWith('-\tweight')).join('\n')), []);
  });

  test('main CI is red if any required check failed, pending until all completed', () => {
    assert.equal(commitCi([{ status: 'completed', conclusion: 'success' }, { status: 'in_progress', conclusion: null }]), 'pending');
    assert.equal(commitCi([{ status: 'completed', conclusion: 'failure' }, { status: 'queued', conclusion: null }]), 'failure');
    assert.equal(commitCi([{ status: 'completed', conclusion: 'success' }]), 'success');
    assert.equal(commitCi([]), 'pending');
  });

  test('a re-run that passed replaces the failed run of the same check', () => {
    const runs = [
      { name: 'linux', status: 'completed', conclusion: 'failure', started_at: '2026-10-17T16:30:00Z' },
      { name: 'linux', status: 'completed', conclusion: 'success', started_at: '2026-10-17T17:00:00Z' },
      { name: 'windows', status: 'completed', conclusion: 'success', started_at: '2026-10-17T16:30:00Z' },
    ];
    assert.equal(commitCi(runs), 'failure');
    assert.equal(commitCi(latestPerName(runs)), 'success');
  });

  test('a ready PR left for a human does not hold the lane in auto mode', () => {
    const w = {
      stories: [story('E6.2', { track: 'platform', labels: ['type:story', 'harness:in-progress', HUMAN_MERGE] }), story('E1.6')],
      prs: [{ number: 90, storyId: 'E6.2', isDraft: false, areas: [], headRefName: 'E6.2-x' }],
    };
    assert.equal(planLane('core', w, cfg, { checkDeps: () => [] }).action, 'wait');
    assert.equal(planLane('core', w, auto, { checkDeps: () => [] }).story.id, 'E1.6');
  });

  test('a ready PR not left for a human goes back to the merge gate in auto mode', () => {
    const w = {
      stories: [story('E3.6', { labels: ['type:story', 'harness:in-progress'] }), story('E1.6')],
      prs: [{ number: 226, storyId: 'E3.6', isDraft: false, areas: [], headRefName: 'E3.6-x' }],
    };
    const p = planLane('core', w, auto, { checkDeps: () => [] });
    assert.equal(p.action, 'resume');
    assert.equal(p.story.id, 'E3.6');
    assert.equal(p.story.pr.draft, false);
  });

  test('in_flight counts claimed stories of every lane, not those left for a human', () => {
    const w = {
      stories: [
        story('E5.9', { track: 'ui', labels: ['type:story', 'harness:in-progress'] }),
        story('E6.2', { track: 'platform', labels: ['type:story', 'harness:in-progress', HUMAN_MERGE] }),
        story('E2.6', { body: 'fm-core' }),
      ],
      prs: [
        { number: 225, storyId: 'E5.9', isDraft: true, areas: ['crates/fm-core'], headRefName: 'E5.9-x' },
        { number: 230, storyId: 'E6.2', isDraft: false, areas: ['crates/fm-platform'], headRefName: 'E6.2-x' },
      ],
    };
    const p = planLane('core', w, auto, { checkDeps: () => [] });
    assert.equal(p.action, 'idle');
    assert.equal(p.in_flight, 1);
  });

});

test('epic summary line names cost, CI and review', () => {
  const line = summaryLine({ id: 'E1.6', issue: 42, pr: 57, billed_usd: 12.4, budget_usd: 15, ci_attempts: 2, ci_failed_platforms: ['macos'], review_rounds: 2, must_fixed: 1, should_total: 2, claim_to_green_sec: 4320 });
  assert.equal(line, '**E1.6 #42** closed — PR #57 · $12.40 of $15 budget · CI attempts 2 (failed: macos) · review rounds 2, must fixed 1, advisory 2 · claim→green 1h12m');
});
