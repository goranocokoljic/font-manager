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
  BUDGETS, OSES, SCAN_FILES, failures, firstScan, median, merge, metric, osName, parseCli, parseRss, renderTable, renderTrend, rssProbe,
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
    const m = metric('activation_100', null, 'no FontActivator yet (E6)');
    assert.deepEqual(m, { value: null, unit: 'ms', target: 500, pass: null, note: 'no FontActivator yet (E6)' });
    assert.deepEqual(failures({ a: m }), []);
  });

  test('every budget of CLAUDE.md rule 8 is in the report', () => {
    assert.deepEqual(Object.keys(BUDGETS).sort(), [
      'activation_100', 'cold_start', 'first_scan', 'idle_memory', 'incremental_update', 'search',
    ]);
    assert.equal(BUDGETS.first_scan.target, 10000);
    assert.equal(BUDGETS.activation_100.target, 500);
  });

  test('an unknown metric is an error', () => {
    assert.throws(() => metric('fps', 1), /unknown metric/);
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
      for (const name of ['cold_start', 'first_scan', 'incremental_update', 'search', 'idle_memory']) {
        assert.ok(rep.metrics[name].value > 0, `${name}: ${JSON.stringify(rep.metrics[name])}`);
      }
      assert.equal(rep.metrics.activation_100.value, null);
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

  test('every OS of the ci matrix is measured', () => {
    for (const os of OSES) assert.match(yml, new RegExp(`- name: ${os}\\n\\s+os: `));
  });
});
