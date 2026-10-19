// node --test scripts/bench/lib.test.mjs — tests for scripts/bench.sh.
// The end-to-end test builds fm-bench (debug) and runs bench.sh on fixtures/fonts.

import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { execFileSync, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  BUDGETS, COMMENT_MARKER, OSES, SCAN_FILES, activation100, blocking, failures, firstScan, gate, median, merge, metric, target,
  nightlyRunsArgs, osName, parseCli, parseRss, renderTable, renderTrend, rssProbe,
} from './lib.mjs';
import { loadHistory } from './bench.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
const tmp = () => mkdtempSync(join(tmpdir(), 'fm-bench-test-'));

const report = (os, values) => ({
  schema: 1,
  os,
  metrics: Object.fromEntries(Object.entries(values).map(([k, v]) => [k, metric(k, v)])),
});

describe('metric', () => {
  test('a value at the budget passes, one above fails', () => {
    assert.equal(metric('search', 50).pass, true);
    assert.equal(metric('search', 50.1).pass, false);
    assert.deepEqual(metric('cold_start', 412.345), { value: 412.3, unit: 'ms', target: 1000, pass: true });
  });

  test('idle memory is budgeted in MB', () => {
    assert.deepEqual(metric('idle_memory', 351), { value: 351, unit: 'MB', target: 350, pass: false });
  });

  test('a metric without a budget is reported, never failed', () => {
    assert.equal(metric('incremental_update', 99999).pass, null);
  });

  test('a metric not measured carries its reason and does not pass', () => {
    const m = metric('activation_100', null, 'no FontActivator yet (E6)', 'linux');
    assert.deepEqual(m, { value: null, unit: 'ms', target: 500, pass: null, note: 'no FontActivator yet (E6)' });
    assert.deepEqual(failures({ a: m }), []);
  });

  test('every budget of CLAUDE.md rule 8 is in the report', () => {
    assert.deepEqual(Object.keys(BUDGETS).sort(), [
      'activation_100', 'cold_start', 'first_scan', 'idle_memory', 'incremental_update', 'preview_cache_hit',
      'search',
    ]);
    assert.equal(BUDGETS.preview_cache_hit.target, 2);
    assert.equal(BUDGETS.first_scan.target, 10000);
    assert.equal(BUDGETS.activation_100.target, 500);
  });

  test('an OS with its own budget is measured against it (ADR 0008)', () => {
    assert.equal(target('activation_100', 'macos'), 1000);
    assert.equal(target('activation_100', 'windows'), 500);
    assert.equal(target('activation_100', 'linux'), 500);
    assert.equal(target('first_scan', 'macos'), 10000);
    assert.equal(metric('activation_100', 800, undefined, 'macos').pass, true);
    assert.equal(metric('activation_100', 800, undefined, 'windows').pass, false);
    assert.equal(metric('activation_100', 800, undefined, 'macos').target, 1000);
  });

  test('an unknown metric is an error', () => {
    assert.throws(() => metric('fps', 1), /unknown metric/);
    assert.throws(() => target('fps'), /unknown metric/);
  });
});

describe('100-face activation', () => {
  test('times a request whose faces all activated or clashed', () => {
    const m = activation100({ ms: 270.4, faces: 100, done: 99, conflicts: 1 }, 'windows');
    assert.equal(m.value, 270.4);
    assert.equal(m.pass, true);
    assert.equal(m.note, undefined);
  });

  test('fails a fast request whose registrations failed', () => {
    const m = activation100({ ms: 12, faces: 100, done: 0, conflicts: 0 }, 'macos');
    assert.equal(m.value, 12);
    assert.equal(m.pass, false);
    assert.equal(m.note, '100 of 100 faces were not activated');
  });

  test('keeps the per-OS budget', () => {
    assert.equal(activation100({ ms: 800, faces: 100, done: 100, conflicts: 0 }, 'macos').pass, true);
    assert.equal(activation100({ ms: 800, faces: 100, done: 100, conflicts: 0 }, 'linux').pass, false);
  });
});

describe('first scan per 10,000 files', () => {
  test('the measured time is scaled to the budgeted library size', () => {
    assert.equal(SCAN_FILES, 10000);
    // The first bench run (E9.1): macOS, 15,329 files in 22.4 s.
    const m = firstScan(22438.5, 15329);
    assert.equal(m.value, 14637.9);
    assert.equal(m.pass, false);
    assert.equal(m.target, BUDGETS.first_scan.target);
    assert.equal(m.files, 15329);
    assert.equal(m.measured_ms, 22438.5);
    assert.equal(m.note, '15329 files in 22.4 s, scaled to 10,000 files');
  });

  test('a library of exactly 10,000 files keeps its time', () => {
    const m = firstScan(9999, 10000);
    assert.equal(m.value, 9999);
    assert.equal(m.pass, true);
  });

  test('a smaller library is scaled up, so the budget compares like with like', () => {
    const m = firstScan(1000, 2500);
    assert.equal(m.value, 4000);
    assert.equal(m.files, 2500);
  });

  test('no files is not measured, never a pass', () => {
    const m = firstScan(12, 0);
    assert.equal(m.value, null);
    assert.equal(m.pass, null);
    assert.match(m.note, /no font files/);
  });

  test('the terminal table shows the file count next to the scaled time', () => {
    const table = renderTable({ metrics: { first_scan: firstScan(4407.3, 15329) } });
    assert.match(table, /first_scan\s+2875\.1 ms\s+<= 10000 ms\s+ok \(15329 files in 4\.4 s, scaled to 10,000 files\)/);
  });
});

describe('helpers', () => {
  test('median of odd, even and empty', () => {
    assert.equal(median([3, 1, 2]), 2);
    assert.equal(median([4, 1, 3, 2]), 2.5);
    assert.equal(median([]), null);
  });

  test('os names match the CI matrix', () => {
    assert.equal(osName('win32'), 'windows');
    assert.equal(osName('darwin'), 'macos');
    assert.equal(osName('linux'), 'linux');
  });

  test('resident memory is read per OS', () => {
    assert.equal(parseRss('linux', 'Name:\tfm-bench\nVmRSS:\t   20480 kB\nVmData: 1 kB\n'), 20);
    assert.equal(parseRss('macos', '  10240\n'), 10);
    assert.equal(parseRss('windows', '52428800\r\n'), 50);
    assert.equal(parseRss('macos', ''), null);
    assert.equal(parseRss('linux', 'no rss here'), null);
    assert.equal(rssProbe('linux', 1), null);
    assert.deepEqual(rssProbe('macos', 42), ['ps', ['-o', 'rss=', '-p', '42']]);
    assert.match(rssProbe('windows', 42)[1].at(-1), /Get-Process -Id 42/);
  });

  test('the table names each verdict', () => {
    const t = renderTable(report('linux', { search: 80, cold_start: 10, incremental_update: 5 }));
    assert.match(t, /search\s+80 ms\s+<= 50 ms\s+OVER BUDGET/);
    assert.match(t, /cold_start\s+10 ms\s+<= 1000 ms\s+ok/);
    assert.match(t, /incremental_update\s+5 ms\s+no budget/);
  });
});

describe('merge', () => {
  test('bench-report.json is flat, keyed <os>.<metric>, as ci-wait reads it', () => {
    const merged = merge(OSES.map((os) => report(os, { search: 10 })));
    assert.deepEqual(Object.keys(merged), ['windows.search', 'macos.search', 'linux.search']);
    // ci-wait.mjs bench(): failed = entries with pass === false.
    assert.deepEqual(Object.entries(merged).filter(([, m]) => m.pass === false), []);
  });

  test('a metric over budget on one OS makes the report red', () => {
    const merged = merge([report('windows', { search: 10 }), report('macos', { search: 70 }), report('linux', { search: 1 })]);
    assert.deepEqual(failures(merged), ['macos.search']);
  });

  test('a missing OS report is a failure, not a silent pass', () => {
    const merged = merge([report('linux', { search: 1 })]);
    assert.deepEqual(failures(merged), ['windows.report', 'macos.report']);
  });
});

describe('trend', () => {
  const run = (date, sha, values) => ({ date, sha, report: merge([report('linux', values)], ['linux']) });

  test('now is compared with the previous run and the series runs oldest to now', () => {
    const current = merge([report('linux', { search: 12, cold_start: 300 })], ['linux']);
    const history = [run('2026-10-17T03:00:00Z', 'bbbbbbbbb', { search: 10, cold_start: 300 }),
      run('2026-10-16T03:00:00Z', 'aaaaaaaaa', { search: 8, cold_start: 310 })];
    const md = renderTrend(current, history, { title: 'Nightly bench', sha: 'cccccccccc' });
    assert.match(md, /## Nightly bench — ccccccc/);
    assert.match(md, /newest 2026-10-17 \(bbbbbbb\)/);
    assert.match(md, /\| linux\.search \| 50 ms \| 12 ms ✓ \| 10 \| \+20% \| 8 → 10 → 12 \|/);
    assert.match(md, /\| linux\.cold_start \| 1000 ms \| 300 ms ✓ \| 300 \| 0% \| 310 → 300 → 300 \|/);
    assert.match(md, /within its budget/);
  });

  test('no history and an over-budget metric are both said plainly', () => {
    const current = merge([report('linux', { search: 90 })], ['linux']);
    const md = renderTrend(current, []);
    assert.match(md, /No earlier nightly run/);
    assert.match(md, /90 ms ✗/);
    assert.match(md, /\*\*Over budget:\*\* linux\.search/);
  });

  test('history is read newest first and incomplete downloads are skipped', () => {
    const dir = tmp();
    try {
      for (const [id, date] of [['1', '2026-10-15T03:00:00Z'], ['2', '2026-10-17T03:00:00Z']]) {
        mkdirSync(join(dir, id));
        writeFileSync(join(dir, id, 'run.json'), JSON.stringify({ date, sha: id.repeat(7) }));
        writeFileSync(join(dir, id, 'bench-report.json'), JSON.stringify({ 'linux.search': metric('search', Number(id)) }));
      }
      mkdirSync(join(dir, '3')); // download failed: no report
      writeFileSync(join(dir, '3', 'run.json'), JSON.stringify({ date: '2026-10-18T03:00:00Z', sha: 'x' }));
      const h = loadHistory(dir);
      assert.deepEqual(h.map((r) => r.sha), ['2222222', '1111111']);
      assert.equal(h[0].report['linux.search'].value, 2);
      assert.deepEqual(loadHistory(join(dir, 'missing')), []);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe('regression gate (E9.2)', () => {
  const linux = (values) => merge([report('linux', values)], ['linux']);
  const nightly = (...values) =>
    values.map((v, i) => ({ date: `2026-10-${String(17 - i).padStart(2, '0')}T03:00:00Z`, sha: 'a', report: linux(v) }));

  test('more than 10% worse than main fails, 10% or less passes', () => {
    const history = nightly({ cold_start: 400 });
    const bad = gate(linux({ cold_start: 441 }), history)['linux.cold_start'];
    assert.deepEqual([bad.baseline, bad.change, bad.regressed], [400, 0.103, true]);
    assert.equal(gate(linux({ cold_start: 440 }), history)['linux.cold_start'].regressed, false);
    assert.equal(gate(linux({ cold_start: 300 }), history)['linux.cold_start'].regressed, false, 'faster is fine');
  });

  test('every budgeted target of the non-functional table is gated', () => {
    const budgeted = Object.entries(BUDGETS).filter(([, b]) => b.target != null).map(([k]) => k);
    const base = Object.fromEntries(budgeted.map((k) => [k, BUDGETS[k].target / 2]));
    const worse = Object.fromEntries(budgeted.map((k) => [k, (BUDGETS[k].target / 2) * 1.2]));
    assert.deepEqual(blocking(gate(linux(worse), nightly(base))).sort(), budgeted.map((k) => `linux.${k}`).sort());
  });

  test('the baseline is the median of the last nightly runs, so one noisy night does not move it', () => {
    const history = nightly({ search: 30 }, { search: 10 }, { search: 10 }, { search: 11 }, { search: 10 }, { search: 99 });
    const m = gate(linux({ search: 12 }), history)['linux.search'];
    assert.equal(m.baseline, 10, 'median of the newest 5; the 6th run is not used');
    assert.equal(m.regressed, true);
  });

  test('a metric over budget on main too blocks only when it gets slower still', () => {
    const history = nightly({ first_scan: 20000 });
    assert.deepEqual(blocking(gate(linux({ first_scan: 21000 }), history)), [], 'known red, not 10% slower');
    assert.deepEqual(blocking(gate(linux({ first_scan: 23000 }), history)), ['linux.first_scan']);
  });

  test('a red nightly on main is still the baseline (review E9.2: the known-red scan made every nightly red)', () => {
    const args = nightlyRunsArgs(14);
    assert.equal(args[args.indexOf('--status') + 1], 'completed', 'every conclusion, not only success');
    assert.deepEqual(args.slice(0, 9), ['run', 'list', '--workflow', 'bench.yml', '--branch', 'main', '--event', 'schedule', '--status']);
    const dir = tmp();
    try {
      // What `history` leaves on disk for a red nightly: its run.json and its bench-report.
      mkdirSync(join(dir, '1'));
      writeFileSync(join(dir, '1', 'run.json'), JSON.stringify({ date: '2026-10-17T03:00:00Z', sha: 'a' }));
      writeFileSync(join(dir, '1', 'bench-report.json'), JSON.stringify(linux({ first_scan: 16000, search: 10 })));
      const gated = gate(linux({ first_scan: 16500, search: 10 }), loadHistory(dir));
      assert.equal(gated['linux.first_scan'].baseline, 16000);
      assert.deepEqual(blocking(gated), [], 'over budget on main too and not 10% slower: not blocked');
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  test('crossing the budget fails even by less than 10%', () => {
    assert.equal(gate(linux({ cold_start: 1020 }), nightly({ cold_start: 980 }))['linux.cold_start'].regressed, true);
  });

  test('without a nightly value on main the budget is the gate', () => {
    assert.deepEqual(blocking(gate(linux({ search: 51 }), [])), ['linux.search']);
    const m = gate(linux({ search: 40 }), [])['linux.search'];
    assert.deepEqual([m.baseline, m.change, m.regressed], [null, null, false]);
  });

  test('a change below the noise floor of the budget is not a regression', () => {
    // 2 -> 2.5 ms is +25% but 1% of the 50 ms budget: runner jitter.
    assert.equal(gate(linux({ search: 2.5 }), nightly({ search: 2 }))['linux.search'].regressed, false);
  });

  test('unbudgeted and unmeasured metrics never block; a missing OS report does', () => {
    const current = merge([report('linux', { incremental_update: 900, activation_100: null })], ['linux', 'macos']);
    const gated = gate(current, nightly({ incremental_update: 100, activation_100: 100 }));
    assert.equal(gated['linux.incremental_update'].regressed, null);
    assert.equal(gated['linux.activation_100'].regressed, null);
    assert.deepEqual(blocking(gated), ['macos.report']);
  });

  test('an ungated (nightly) report blocks on every metric over budget', () => {
    const r = linux({ first_scan: 20000, search: 10 });
    assert.deepEqual(blocking(r), failures(r));
  });

  test('the trend shows the gate with the reason of each regression', () => {
    const history = nightly({ search: 10, cold_start: 300 });
    const md = renderTrend(gate(linux({ search: 20, cold_start: 300 }), history), history);
    assert.match(md, /### Regression gate — this PR vs main \(fails at more than 10% worse\)/);
    assert.match(md, /\| linux\.search \| 50 ms \| 10 ms \| 20 ms \| \+100% \| ✗ regressed \|/);
    assert.match(md, /\| linux\.cold_start \| 1000 ms \| 300 ms \| 300 ms \| 0% \| ✓ \|/);
    assert.match(md, /\*\*Bench check failed:\*\*\n- linux\.search: 20 ms vs 10 ms on main, \+100%/);
    assert.doesNotMatch(renderTrend(linux({ search: 20 }), []), /Regression gate/, 'nightly has no gate');
  });

  test('gate on the command line annotates the regression, writes the report and exits 1', () => {
    const dir = tmp();
    try {
      mkdirSync(join(dir, 'h', '1'), { recursive: true });
      writeFileSync(join(dir, 'h', '1', 'run.json'), JSON.stringify({ date: '2026-10-17T03:00:00Z', sha: 'a' }));
      writeFileSync(join(dir, 'h', '1', 'bench-report.json'), JSON.stringify(linux({ search: 10, cold_start: 300 })));
      const cli = (values) => {
        const file = join(dir, 'bench-report.json');
        writeFileSync(file, JSON.stringify(linux(values)));
        const args = [join(ROOT, 'scripts', 'bench', 'bench.mjs'), 'gate', '--current', file, '--history', join(dir, 'h')];
        const r = spawnSync(process.execPath, args, { encoding: 'utf8' });
        return { ...r, report: JSON.parse(readFileSync(file, 'utf8')) };
      };
      const bad = cli({ search: 20, cold_start: 300 });
      assert.equal(bad.status, 1, bad.stderr);
      assert.match(bad.stdout, /^::error title=Bench regression::linux\.search: 20 ms vs 10 ms on main, \+100%$/m);
      assert.equal(bad.report['linux.search'].regressed, true);
      const good = cli({ search: 10.5, cold_start: 290 });
      assert.equal(good.status, 0, good.stderr);
      assert.doesNotMatch(good.stdout, /::error/);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  test('the PR comment carries a marker so a new push updates it', () => {
    assert.match(COMMENT_MARKER, /^<!-- .+ -->$/);
    assert.equal(parseCli(['comment', '--pr', '7', '--body', 't.md']).pr, 7);
    assert.equal(parseCli(['gate', '--current', 'r.json']).current, 'r.json');
  });
});

describe('cli', () => {
  test('run is the default command and the library comes from the environment', () => {
    const o = parseCli(['--runs', '3'], { FM_REFERENCE_LIBRARY: '/lib' });
    assert.equal(o.command, 'run');
    assert.equal(o.runs, 3);
    assert.equal(o.library, '/lib');
    assert.equal(parseCli([], {}).library, undefined);
  });

  test('merge takes the report files', () => {
    const o = parseCli(['merge', '--out', 'r.json', 'a.json', 'b.json'], {});
    assert.deepEqual([o.command, o.out, o.files], ['merge', 'r.json', ['a.json', 'b.json']]);
  });

  test('bad input is refused', () => {
    assert.throws(() => parseCli(['--runs', '0'], {}), /positive integer/);
    assert.throws(() => parseCli(['--library'], {}), /needs a value/);
    assert.throws(() => parseCli(['frobnicate'], {}), /unknown command/);
    assert.throws(() => parseCli(['--what'], {}), /unknown argument/);
  });
});

describe('bench.sh end to end', () => {
  const bash = process.platform === 'win32' ? 'bash' : '/bin/bash';

  test('without a library it says how to get one', () => {
    const r = spawnSync(bash, [join(ROOT, 'scripts', 'bench.sh'), '--no-build'], {
      encoding: 'utf8', env: { ...process.env, FM_REFERENCE_LIBRARY: '' },
    });
    assert.equal(r.status, 2, r.stderr);
    assert.match(r.stderr, /build-reference-library\.sh/);
  });

  test('measures every metric on the fixtures and writes the report', { timeout: 600_000 }, () => {
    execFileSync('cargo', ['build', '-q', '-p', 'fm-bench'], { cwd: ROOT, stdio: 'inherit' });
    const bin = join(ROOT, 'target', 'debug', `fm-bench${process.platform === 'win32' ? '.exe' : ''}`);
    const dir = tmp();
    const out = join(dir, 'bench.json');
    try {
      const r = spawnSync(bash, [join(ROOT, 'scripts', 'bench.sh'), '--library', join(ROOT, 'fixtures', 'fonts'),
        '--bin', bin, '--out', out, '--runs', '2', '--rounds', '2', '--settle-ms', '200'], { encoding: 'utf8' });
      // 200 fixtures scaled to 10,000 files on a debug build mostly measure
      // the fixed start-up cost: first_scan alone may go over on a slow
      // runner. Any other metric over budget, or a setup error, still fails.
      assert.ok([0, 1].includes(r.status), r.stdout + r.stderr);
      const rep = JSON.parse(readFileSync(out, 'utf8'));
      assert.deepEqual(failures(rep.metrics).filter((m) => m !== 'first_scan'), [], r.stdout);
      assert.equal(r.status, failures(rep.metrics).length ? 1 : 0, 'the exit code follows the report');
      assert.equal(rep.os, osName());
      assert.ok(rep.library.files >= 150, JSON.stringify(rep.library));
      assert.deepEqual(Object.keys(rep.metrics).sort(), Object.keys(BUDGETS).sort());
      for (const name of ['cold_start', 'first_scan', 'incremental_update', 'search', 'activation_100', 'idle_memory']) {
        assert.ok(rep.metrics[name].value > 0, `${name}: ${JSON.stringify(rep.metrics[name])}`);
      }
      assert.equal(rep.detail.activation.faces, 100, JSON.stringify(rep.detail.activation));
      assert.equal(rep.detail.activation.done + rep.detail.activation.conflicts, 100,
        `every face activated or clashed: ${JSON.stringify(rep.detail.activation)}`);
      assert.equal(rep.metrics.first_scan.files, rep.library.files, 'the scaled time names its file count');
      assert.equal(rep.metrics.first_scan.measured_ms, Math.round(rep.detail.scan.ms * 10) / 10);
      assert.equal(rep.detail.cold_start_runs.length, 2);
      assert.equal(rep.detail.incremental.added, 0, 'the rescan reads nothing new');
      assert.ok(existsSync(join(ROOT, 'fixtures', 'fonts', 'manifest.tsv')), 'the library is left in place');
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe('bench workflow', () => {
  const yml = readFileSync(join(ROOT, '.github', 'workflows', 'bench.yml'), 'utf8');
  const cfg = JSON.parse(readFileSync(join(ROOT, 'go-harness.config.json'), 'utf8'));

  test('the check and the artifact are the ones ci-wait waits for', () => {
    const [workflow, job] = cfg.ci.bench_check.split('/').map((s) => s.trim());
    assert.match(yml, new RegExp(`^name: ${workflow}$`, 'm'));
    assert.match(yml, new RegExp(`^  ${job}:\n    name: ${job}$`, 'm'));
    assert.equal(cfg.ci.bench_workflow, 'bench.yml');
    assert.match(yml, new RegExp(`name: ${cfg.ci.bench_artifact}\\n\\s+path: out/`));
  });

  test('no bench job shadows a required ci check', () => {
    const names = [...yml.matchAll(/^ {4}name: (.+)$/gm)].map((m) => m[1].trim());
    for (const required of cfg.ci.required_checks) {
      const job = required.split('/').pop().trim();
      assert.ok(!names.includes(job), `${job} is a ci job name`);
    }
  });

  test('it runs nightly, by hand and on the bench label', () => {
    assert.match(yml, /schedule:\n\s+- cron:/);
    assert.match(yml, /workflow_dispatch:/);
    assert.match(yml, /types: \[[^\]]*labeled[^\]]*\]/);
    assert.match(yml, /contains\(github\.event\.pull_request\.labels\.\*\.name, 'bench'\)/);
  });

  test('every workflow expression closes before a YAML comment cuts it off', () => {
    // E9.1 review: an unquoted `#` inside `${{ }}` made GitHub reject bench.yml.
    for (const file of ['bench.yml', 'ci.yml']) {
      const text = readFileSync(join(ROOT, '.github', 'workflows', file), 'utf8');
      text.split('\n').forEach((line, i) => {
        const m = /^\s*(?:- )?[\w-]+:\s+(.*)$/.exec(line);
        if (!m || !m[1].includes('${{') || /^['"|>]/.test(m[1])) return;
        const value = m[1].split(' #')[0];
        const opened = value.split('${{').length - 1;
        const closed = value.split('}}').length - 1;
        assert.equal(closed, opened, `${file}:${i + 1}: unclosed or commented-out expression: ${line.trim()}`);
      });
    }
  });

  test('a PR is gated against main and flagged with a comment (E9.2)', () => {
    assert.match(yml, /if \[ -n "\$PR_NUMBER" \]; then[\s\S]*?bench\.mjs gate --current out\/bench-report\.json --history history \|\| status=\$\?/);
    const gateAt = yml.indexOf('bench.mjs gate');
    assert.ok(gateAt > yml.indexOf('bench.mjs history') && gateAt < yml.indexOf('bench.mjs trend'), 'history, gate, trend');
    assert.match(yml, /if: always\(\) && github\.event_name == 'pull_request'\n\s+continue-on-error: true[\s\S]*?bench\.mjs comment --pr "\$PR_NUMBER" --body out\/trend\.md/);
    assert.match(yml, / {2}bench:\n[\s\S]*?permissions:\n\s+contents: read\n\s+actions: read\n\s+pull-requests: write/);
  });

  test('every OS of the ci matrix is measured', () => {
    for (const os of OSES) assert.match(yml, new RegExp(`- name: ${os}\\n\\s+os: `));
  });
});
