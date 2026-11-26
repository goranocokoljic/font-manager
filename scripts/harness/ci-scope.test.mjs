// node --test scripts/harness/ — E57.1: CI scopes (light, frontend, full), the
// check the harness reads per scope, main's state with a red full run behind
// frontend commits, and when the keeper starts a full run. No network.

import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { readFileSync } from 'node:fs';

import { DEFAULT_CONFIG, withLight, standIns, HARNESS_LABELS, loadConfig } from './lib.mjs';
import { isFrontendPath, ciScope, FULL_LABEL } from './ci-scope.mjs';
import { missingLabels } from './issue.mjs';
import { matchCheck, verdictChecks } from './ci-wait.mjs';
import { decide } from './merge-check.mjs';
import { prVerdict, runScope, mainState } from './main-check.mjs';
import { decideFullRun, epicDone, uncoveredCommits } from './full-run.mjs';
import { changesNative } from '../bench/lib.mjs';

const cfg = structuredClone(DEFAULT_CONFIG);
const REQ = ['ci / windows', 'ci / macos', 'ci / linux'];
const STANDS = ['ci / light', 'ci / frontend'];
const MATRIX = '${{ matrix.name }}'; // the name GitHub gives a skipped matrix job
const read = (rel) => readFileSync(new URL(rel, import.meta.url), 'utf8');

describe('ci-scope: light, frontend, full', () => {
  test('frontend paths: desktop src, e2e, public and index.html', () => {
    for (const p of ['apps/desktop/src/App.vue', 'apps/desktop/src/components/FontList.vue', 'apps/desktop/src/api/client.ts',
      'apps/desktop/src/stores/library.test.ts', 'apps/desktop/e2e/screens.spec.ts', 'apps/desktop/public/favicon.svg', 'apps/desktop/index.html']) {
      assert.equal(isFrontendPath(p), true, p);
      assert.equal(ciScope([p]), 'frontend', p);
    }
  });

  test('never frontend: types.ts, src-tauri, crates, Cargo, package.json, lock file, configs, .github', () => {
    for (const p of ['apps/desktop/src/api/types.ts', 'apps/desktop/src-tauri/src/main.rs', 'apps/desktop/src-tauri/tauri.conf.json',
      'crates/fm-core/src/lib.rs', 'Cargo.toml', 'Cargo.lock', 'apps/desktop/src-tauri/Cargo.toml', 'package.json', 'apps/desktop/package.json',
      'pnpm-lock.yaml', 'apps/desktop/vite.config.ts', 'apps/desktop/playwright.config.ts', 'apps/desktop/tsconfig.json',
      'apps/desktop/src/tsconfig.json', 'apps/desktop/src/package.json', '.github/workflows/ci.yml', 'plugins/sketch/src/index.ts',
      'docs/user/index.md', 'apps/desktop/scripts/x.mjs']) {
      assert.equal(isFrontendPath(p), false, p);
      assert.equal(ciScope([p]), 'full', p);
    }
  });

  test('all light is still light; light plus frontend is frontend; anything else makes it full', () => {
    assert.equal(ciScope(['dev-docs/a.md', 'scripts/harness/lib.mjs']), 'light');
    assert.equal(ciScope(['dev-docs/a.md', 'apps/desktop/src/App.vue']), 'frontend');
    assert.equal(ciScope(['apps/desktop/src/App.vue', 'apps/desktop/e2e/a.spec.ts']), 'frontend');
    assert.equal(ciScope(['apps/desktop/src/App.vue', 'crates/fm-api/src/lib.rs']), 'full', 'mixed with Rust');
    assert.equal(ciScope(['apps/desktop/src/App.vue', 'apps/desktop/src/api/types.ts']), 'full', 'mixed with the generated types');
    assert.equal(ciScope(['apps/desktop/src/App.vue', 'dev-docs/a.md', 'pnpm-lock.yaml']), 'full', 'mixed with the lock file');
    assert.equal(ciScope(null), 'full', 'unknown files: full');
    assert.equal(ciScope([]), 'full');
  });

  test('a PR labelled ci:full is always full', () => {
    assert.equal(FULL_LABEL, 'ci:full');
    assert.equal(ciScope(['apps/desktop/src/App.vue'], { labels: ['ci:full'] }), 'full');
    assert.equal(ciScope(['dev-docs/a.md'], { labels: ['bench', 'ci:full'] }), 'full');
    assert.equal(ciScope(['apps/desktop/src/App.vue'], { labels: ['bench'] }), 'frontend');
  });

  test('ci:full is a harness label; /fix-main puts it on its PR; the manual names it', () => {
    assert.ok(HARNESS_LABELS.some((l) => l.name === 'ci:full' && /^[0-9A-F]{6}$/i.test(l.color) && l.description));
    const others = HARNESS_LABELS.map((l) => l.name).filter((n) => n !== 'ci:full');
    assert.deepEqual(missingLabels(others).map((l) => l.name), ['ci:full']);
    assert.deepEqual(missingLabels(HARNESS_LABELS.map((l) => l.name.toUpperCase())), [], 'existing labels are left alone');
    assert.match(read('../../.claude/skills/fix-main/SKILL.md'), /gh pr create [^\n]*--label ci:full/);
    const manual = read('../../dev-docs/harness/go-harness.md');
    assert.match(manual, /labelled `ci:full` is always full/);
    assert.match(manual, /A person adds the label to force the\s+matrix/);
    assert.match(manual, /proven on Linux only for\s+frontend changes/);
  });

  test('ci.yml: a frontend job on Linux without Rust, and the OS jobs skip that scope', () => {
    const yml = read('../../.github/workflows/ci.yml');
    const job = cfg.ci.frontend_check.split('/').pop().trim();
    const block = new RegExp(`^  ${job}:\\n([\\s\\S]*?)(?=^  [a-z]+:\\n)`, 'm').exec(yml)?.[1];
    assert.ok(block, 'the frontend job exists');
    assert.match(block, /^ {4}name: frontend$/m);
    assert.match(block, /runs-on: ubuntu-latest/);
    assert.match(block, /timeout-minutes: 20/);
    assert.match(block, /if: needs\.scope\.outputs\.scope == 'frontend'/);
    for (const cmd of ['pnpm lint:js', 'pnpm test', 'pnpm --filter @fm/desktop build', 'playwright install --with-deps chromium', 'pnpm test:e2e']) {
      assert.ok(block.includes(cmd), cmd);
    }
    assert.doesNotMatch(block, /rustup|cargo|rust-cache|tauri build/, 'no Rust toolchain in the frontend job');
    assert.match(yml, /needs\.scope\.outputs\.scope != 'light' && needs\.scope\.outputs\.scope != 'frontend'/, 'the OS jobs skip frontend');
    assert.match(yml, /cancel-in-progress: \$\{\{ github\.event_name == 'pull_request' \}\}/, 'runs on main never cancel each other');
  });
});

describe('the check the harness reads per scope', () => {
  test('config: ci.frontend_check and ci.full_run_max_hours, with defaults in lib.mjs', () => {
    assert.equal(DEFAULT_CONFIG.ci.frontend_check, 'ci / frontend');
    assert.equal(DEFAULT_CONFIG.ci.full_run_max_hours, 24);
    const repo = loadConfig();
    assert.equal(repo.ci.frontend_check, 'ci / frontend');
    assert.equal(repo.ci.full_run_max_hours, 24);
    assert.deepEqual(standIns(DEFAULT_CONFIG), STANDS);
  });

  test('the frontend job stands in for the three OS jobs; a red one goes to ci-fix as frontend on linux', () => {
    const front = (conclusion) => [{ name: 'scope', id: 1, status: 'completed', conclusion: 'success' },
      { name: 'light', id: 2, status: 'completed', conclusion: 'skipped' },
      { name: 'frontend', id: 3, status: 'completed', conclusion }];
    const ok = withLight(front('success'), REQ, STANDS);
    for (const r of REQ) assert.equal(matchCheck(r, ok).conclusion, 'success', r);
    const red = withLight(front('failure'), REQ, STANDS);
    const rows = verdictChecks(REQ.map((want) => ({ want, run: matchCheck(want, red) })));
    assert.deepEqual([...new Set(rows.map((r) => `${r.name} ${r.os} ${r.conclusion} ${r.id}`))], ['ci / frontend linux failure 3']);
    assert.equal(rows[0].stands_for, 'ci / windows');
    // A full run: both stand-ins skipped, the OS jobs decide.
    const full = [{ name: 'windows', conclusion: 'success' }, { name: 'frontend', conclusion: 'skipped' }, { name: 'light', conclusion: 'skipped' }];
    assert.equal(withLight(full, REQ, STANDS), full);
    assert.deepEqual(verdictChecks([{ want: 'ci / windows', run: full[0] }]).map((r) => [r.name, r.os]), [['ci / windows', 'windows']]);
  });

  test('the keeper reads the fix PR the same way (gh pr checks states)', () => {
    const checks = (state) => [{ name: 'frontend', state }, { name: 'light', state: 'SKIPPED' }, { name: MATRIX, state: 'SKIPPED' }];
    assert.equal(prVerdict(checks('SUCCESS'), REQ, STANDS), 'green');
    assert.equal(prVerdict(checks('FAILURE'), REQ, STANDS), 'red');
    assert.equal(prVerdict(checks('SUCCESS'), REQ, ['ci / light']), 'pending', 'only the configured stand-ins count');
  });

  test('a frontend PR never asks for the bench label: it changes no native code', () => {
    assert.equal(changesNative(['apps/desktop/src/App.vue', 'apps/desktop/e2e/a.spec.ts', 'apps/desktop/public/x.svg', 'apps/desktop/index.html']), false);
  });
});

describe("main's state with a red full run behind frontend commits", () => {
  const run = (id, { status = 'completed', conclusion = 'success', event = 'push', scope = 'full', attempt = 1 } = {}) => ({
    databaseId: id, headSha: `sha${id}`, status, conclusion: status === 'completed' ? conclusion : null, event, attempt,
    jobs: scope === 'unknown' ? [{ name: 'scope', conclusion: null }]
      : scope === 'full' ? [{ name: 'scope', conclusion: 'success' }, { name: 'windows', conclusion }, { name: 'frontend', conclusion: 'skipped' }]
        : [{ name: 'scope', conclusion: 'success' }, { name: MATRIX, conclusion: 'skipped' }, { name: scope, conclusion }],
  });

  test('runScope: full (the OS jobs ran, or a manual run), short (light, frontend), unknown', () => {
    assert.equal(runScope(run(1), REQ, STANDS), 'full');
    assert.equal(runScope(run(1, { scope: 'frontend' }), REQ, STANDS), 'short');
    assert.equal(runScope(run(1, { scope: 'light' }), REQ, STANDS), 'short');
    assert.equal(runScope(run(1, { scope: 'unknown', status: 'in_progress' }), REQ, STANDS), 'unknown');
    assert.equal(runScope({ event: 'workflow_dispatch', status: 'queued' }, REQ, STANDS), 'full', 'a manual run is always full');
    assert.equal(runScope({ event: 'push' }, REQ, STANDS), 'unknown', 'no jobs read');
  });

  test('the red full run decides, whatever green frontend runs came after it', () => {
    const s = mainState([run(3, { scope: 'frontend' }), run(2, { scope: 'frontend' }), run(1, { conclusion: 'failure' })], REQ, STANDS);
    assert.equal(s.via, 'full');
    assert.equal(s.run.databaseId, 1);
  });

  test("otherwise the newest commit's own checks", () => {
    let s = mainState([run(3, { scope: 'frontend', conclusion: 'failure' }), run(2, { scope: 'frontend' }), run(1)], REQ, STANDS);
    assert.equal(s.via, 'newest');
    assert.equal(s.run.databaseId, 3, 'a red frontend run on the newest commit is red');
    assert.equal(s.lastGreenFull.databaseId, 1);
    s = mainState([run(4, { scope: 'frontend' }), run(3), run(2, { scope: 'frontend' }), run(1, { conclusion: 'failure' })], REQ, STANDS);
    assert.equal(s.via, 'newest', 'a newer green full run settles an older red one');
    assert.equal(s.run.databaseId, 4);
    assert.equal(mainState([], REQ, STANDS).run, null);
  });

  test('a new full run in flight after a red one: still red; a re-run of the red one: that run, pending', () => {
    let s = mainState([run(5, { status: 'in_progress', event: 'workflow_dispatch' }), run(4, { scope: 'frontend' }),
      run(3, { conclusion: 'failure' }), run(2)], REQ, STANDS);
    assert.equal(s.via, 'full');
    assert.equal(s.run.databaseId, 3);
    assert.equal(s.fullAny.databaseId, 5);
    assert.equal(s.lastGreenFull.databaseId, 2, 'suspects start at the last green full run');
    s = mainState([run(4, { scope: 'frontend' }), run(3, { status: 'in_progress', attempt: 2 }), run(2)], REQ, STANDS);
    assert.equal(s.via, 'full');
    assert.equal(s.run.databaseId, 3);
    assert.equal(s.run.status, 'in_progress');
  });

  test('the merge gate stops on it', () => {
    const f = { state: 'OPEN', isDraft: false, mergeable: 'MERGEABLE', behindBy: 0, removals: [], track: 'ui', epic: 'E57',
      baseSha: 'a'.repeat(40), mainCi: 'failure', mainRedSha: 'b'.repeat(40), mainRedVia: 'full' };
    const d = decide(f, cfg);
    assert.equal(d.action, 'stop');
    assert.match(d.reasons[0], /last full CI run on main is red at bbbbbbb/);
    assert.equal(decide({ ...f, mainCi: 'success' }, cfg).action, 'merge');
  });
});

describe('when the keeper starts a full run', () => {
  const base = { uncovered: [{ sha: 's1', title: 'c1', date: null }], oldestHours: 2, inFlight: false, undecided: false, epicsDone: [], fullRed: false };

  test('the end of an epic starts it at once; young uncovered commits wait', () => {
    assert.equal(decideFullRun(base, 24).action, 'none');
    const d = decideFullRun({ ...base, epicsDone: ['E57'] }, 24);
    assert.equal(d.action, 'start');
    assert.match(d.reason, /epic E57 has no story left/);
  });

  test('the fallback: the oldest uncovered commit is older than ci.full_run_max_hours; 0 turns it off', () => {
    assert.equal(decideFullRun({ ...base, oldestHours: 25 }, 24).action, 'start');
    assert.equal(decideFullRun({ ...base, oldestHours: 23.9 }, 24).action, 'none');
    assert.equal(decideFullRun({ ...base, oldestHours: 25 }, 0).action, 'none');
    assert.equal(decideFullRun({ ...base, oldestHours: Infinity }, 24).action, 'start', 'no full run known');
  });

  test('never two in flight, none while a run has not decided its scope, none without uncovered commits', () => {
    assert.equal(decideFullRun({ ...base, oldestHours: 99, inFlight: true }, 24).action, 'none');
    assert.equal(decideFullRun({ ...base, epicsDone: ['E57'], undecided: true }, 24).action, 'none');
    assert.equal(decideFullRun({ ...base, uncovered: [], epicsDone: ['E57'], oldestHours: 99 }, 24).action, 'none');
  });

  test('a red full run with commits after it starts a new one (a fix may have landed)', () => {
    assert.equal(decideFullRun({ ...base, fullRed: true }, 24).action, 'start');
  });

  test('an epic is done when every sub-issue is closed, human or harness:blocked', () => {
    const i = (state, labels = []) => ({ state, labels: labels.map((name) => ({ name })) });
    assert.equal(epicDone([i('closed'), i('CLOSED'), i('open', ['human']), i('open', ['type:story', 'harness:blocked'])]), true);
    assert.equal(epicDone([i('closed'), i('open', ['type:story'])]), false);
    assert.equal(epicDone([i('closed'), i('open', ['harness:in-progress'])]), false);
  });

  test('uncovered commits: sha, first line, date', () => {
    assert.deepEqual(uncoveredCommits({ commits: [{ sha: 'x', commit: { message: 'Fix it\n\nbody', committer: { date: '2026-11-26T10:00:00Z' } } }] }),
      [{ sha: 'x', title: 'Fix it', date: '2026-11-26T10:00:00Z' }]);
    assert.deepEqual(uncoveredCommits(null), []);
  });
});
