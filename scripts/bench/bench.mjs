#!/usr/bin/env node
// bench.mjs — CLI of scripts/bench.sh: the performance budgets on this machine.
//
//   node scripts/bench/bench.mjs [run] [--library DIR] [--out FILE] [--runs N] [--rounds N]
//                                      [--settle-ms N] [--bin PATH] [--no-build] [--home DIR]
//   node scripts/bench/bench.mjs merge --out bench-report.json bench-*.json
//   node scripts/bench/bench.mjs history --history DIR [--limit N]        # previous nightly reports (gh)
//   node scripts/bench/bench.mjs gate --current bench-report.json [--history DIR] [--out FILE]   # PR only
//   node scripts/bench/bench.mjs trend --current bench-report.json [--history DIR] [--sha SHA] [--title T]
//   node scripts/bench/bench.mjs comment --pr N --body trend.md          # sticky PR comment (gh)
//
// run: builds fm-bench in release, then measures against --library (default
// $FM_REFERENCE_LIBRARY): first scan into a fresh index (budgeted per 10,000
// files; the report gives the file count and the measured time), incremental rescan,
// cold start (median of --runs fresh processes, spawn to first page), search
// latency (p95 over --rounds rounds of the query set), idle memory (resident
// set of a started process after --settle-ms) and 100-face activation (not
// measured until the FontActivator lands, E6). Writes the per-OS JSON report
// (default target/bench/bench-<os>.json) and prints a table.
// The library is only read; the index lives in a throwaway home folder.
// gate: marks each budgeted metric against the median of the last nightly runs on
// main (lib.mjs gate()); a PR fails at more than 10% worse, or newly over budget.
// It writes the marked report (default: in place) and a GitHub annotation per regression.
//
// Exit code: 0 within budget (gate: no regression), 1 a metric over budget
// (gate: a regression), 2 usage or setup error.

import { execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  COMMENT_MARKER, SCHEMA, blocking, failures, firstScan, gate, median, merge, metric, nightlyRunsArgs, osName,
  parseCli, parseRss, reason, renderTable, renderTrend, rssProbe,
} from './lib.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
const EXE = process.platform === 'win32' ? '.exe' : '';

/** Runs fm-bench to completion; returns its JSON line. */
function bench(bin, args) {
  const out = execFileSync(bin, args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] });
  return JSON.parse(out.trim().split('\n').pop());
}

/**
 * Starts fm-bench and resolves when its first JSON line arrives, with the
 * wall time from spawn to that line. The child keeps running when `keep`.
 */
function startProcess(bin, args, { keep = false } = {}) {
  return new Promise((resolveP, reject) => {
    const begin = process.hrtime.bigint();
    const child = spawn(bin, args, { stdio: ['pipe', 'pipe', 'inherit'] });
    let buf = '';
    let done = false;
    child.on('error', reject);
    child.stdout.on('data', (d) => {
      buf += d;
      const nl = buf.indexOf('\n');
      if (done || nl < 0) return;
      done = true;
      const ms = Number(process.hrtime.bigint() - begin) / 1e6;
      resolveP({ ms, line: JSON.parse(buf.slice(0, nl)), child });
    });
    child.on('exit', (code) => {
      if (!done) reject(new Error(`fm-bench ${args[0]} exited with ${code} before it was ready`));
    });
    if (!keep) child.stdin.end();
  });
}

function rssMb(os, pid) {
  if (os === 'linux') return parseRss(os, readFileSync(`/proc/${pid}/status`, 'utf8'));
  const [cmd, args] = rssProbe(os, pid);
  return parseRss(os, execFileSync(cmd, args, { encoding: 'utf8' }));
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function idleMemory(os, bin, home, settleMs) {
  const { child } = await startProcess(bin, ['idle', '--home', home], { keep: true });
  try {
    await sleep(settleMs);
    const samples = [];
    for (let i = 0; i < 3; i++) {
      samples.push(rssMb(os, child.pid));
      await sleep(250);
    }
    return Math.max(...samples.filter((s) => s != null));
  } finally {
    child.stdin.end();
    await new Promise((r) => (child.exitCode != null ? r() : child.on('exit', r)));
  }
}

function gitSha() {
  try {
    return execFileSync('git', ['rev-parse', 'HEAD'], { cwd: ROOT, encoding: 'utf8' }).trim();
  } catch {
    return process.env.GITHUB_SHA ?? '';
  }
}

async function run(opts) {
  if (!opts.library) {
    throw usage('no library: pass --library DIR or set FM_REFERENCE_LIBRARY (scripts/build-reference-library.sh downloads it)');
  }
  const library = resolve(opts.library);
  if (!existsSync(library)) throw usage(`library not found: ${library}`);
  const os = osName();
  const bin = resolve(opts.bin ?? join(ROOT, 'target', 'release', `fm-bench${EXE}`));
  if (opts.build && !opts.bin) {
    console.log('building fm-bench (release)…');
    execFileSync('cargo', ['build', '--release', '-q', '-p', 'fm-bench'], { cwd: ROOT, stdio: 'inherit' });
  }
  if (!existsSync(bin)) throw usage(`fm-bench not found: ${bin} (build it: cargo build --release -p fm-bench)`);
  const home = opts.home ? resolve(opts.home) : mkdtempSync(join(tmpdir(), 'fm-bench-'));
  const out = resolve(opts.out ?? join(ROOT, 'target', 'bench', `bench-${os}.json`));
  try {
    console.log(`library: ${library}\nhome: ${home}`);
    const scan = bench(bin, ['scan', '--library', library, '--home', home]);
    console.log(`first scan: ${scan.files} files, ${scan.faces} faces, ${scan.failed} unreadable`);
    const inc = bench(bin, ['incremental', '--library', library, '--home', home]);
    const starts = [];
    for (let i = 0; i < opts.runs; i++) starts.push((await startProcess(bin, ['start', '--home', home])).ms);
    const search = bench(bin, ['search', '--home', home, '--rounds', String(opts.rounds)]);
    const idle = await idleMemory(os, bin, home, opts.settleMs);
    const report = {
      schema: SCHEMA,
      os,
      arch: process.arch,
      sha: gitSha(),
      date: new Date().toISOString(),
      library: { path: library, files: scan.files, faces: scan.faces, unreadable: scan.failed },
      metrics: {
        cold_start: metric('cold_start', median(starts)),
        first_scan: firstScan(scan.ms, scan.files),
        incremental_update: metric('incremental_update', inc.ms),
        search: metric('search', search.p95_ms),
        activation_100: metric('activation_100', null, 'no FontActivator yet (E6)'),
        idle_memory: metric('idle_memory', idle),
      },
      detail: { cold_start_runs: starts, scan, incremental: inc, search },
    };
    mkdirSync(dirname(out), { recursive: true });
    writeFileSync(out, JSON.stringify(report, null, 2) + '\n');
    console.log(`\n${renderTable(report)}\n\nreport: ${out}`);
    return failures(report.metrics).length ? 1 : 0;
  } finally {
    if (!opts.home) rmSync(home, { recursive: true, force: true });
  }
}

function readJson(file) {
  return JSON.parse(readFileSync(file, 'utf8'));
}

function mergeCmd(opts) {
  if (!opts.out) throw usage('merge needs --out FILE');
  const merged = merge(opts.files.map(readJson));
  mkdirSync(dirname(resolve(opts.out)), { recursive: true });
  writeFileSync(opts.out, JSON.stringify(merged, null, 2) + '\n');
  const failed = failures(merged);
  console.log(failed.length ? `over budget or missing: ${failed.join(', ')}` : 'every metric within budget');
  return failed.length ? 1 : 0;
}

/** Downloads the bench-report of the last nightly runs on main into <history>/<run id>/. */
function historyCmd(opts) {
  if (!opts.history) throw usage('history needs --history DIR');
  const gh = (args) => execFileSync('gh', args, { encoding: 'utf8' });
  const runs = JSON.parse(gh(nightlyRunsArgs(opts.limit ?? 14)));
  for (const r of runs) {
    const dir = join(opts.history, String(r.databaseId));
    try {
      gh(['run', 'download', String(r.databaseId), '-n', 'bench-report', '-D', dir]);
      writeFileSync(join(dir, 'run.json'), JSON.stringify({ date: r.createdAt, sha: r.headSha, url: r.url }) + '\n');
    } catch (e) {
      console.warn(`skipped run ${r.databaseId}: ${e.message.split('\n')[0]}`);
    }
  }
  console.log(`history: ${runs.length} run(s) in ${opts.history}`);
  return 0;
}

/** Earlier reports under `dir`, newest first. */
export function loadHistory(dir) {
  if (!dir || !existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true })
    .filter((d) => d.isDirectory())
    .map((d) => join(dir, d.name))
    .filter((d) => existsSync(join(d, 'run.json')) && existsSync(join(d, 'bench-report.json')))
    .map((d) => ({ ...readJson(join(d, 'run.json')), report: readJson(join(d, 'bench-report.json')) }))
    .sort((a, b) => b.date.localeCompare(a.date));
}

function gateCmd(opts) {
  if (!opts.current) throw usage('gate needs --current bench-report.json');
  const gated = gate(readJson(opts.current), loadHistory(opts.history));
  writeFileSync(opts.out ?? opts.current, JSON.stringify(gated, null, 2) + '\n');
  const blocked = blocking(gated);
  // GitHub turns these lines into annotations on the PR's checks page.
  for (const k of blocked) console.log(`::error title=Bench regression::${reason(k, gated[k])}`);
  console.log(blocked.length ? `regressed: ${blocked.join(', ')}` : 'no budgeted metric regressed');
  return blocked.length ? 1 : 0;
}

/** Creates or updates the one bench comment on the PR (found by COMMENT_MARKER). */
function commentCmd(opts) {
  if (!opts.pr || !opts.body) throw usage('comment needs --pr N --body FILE');
  const dir = mkdtempSync(join(tmpdir(), 'fm-bench-comment-'));
  const file = join(dir, 'body.md');
  try {
    writeFileSync(file, `${COMMENT_MARKER}\n${readFileSync(opts.body, 'utf8')}`);
    const gh = (args) => execFileSync('gh', args, { encoding: 'utf8' }).trim();
    const id = gh([
      'api', '--paginate', `repos/{owner}/{repo}/issues/${opts.pr}/comments`,
      '--jq', `.[] | select(.body | startswith(${JSON.stringify(COMMENT_MARKER)})) | .id`,
    ]).split('\n')[0];
    if (id) gh(['api', '--method', 'PATCH', `repos/{owner}/{repo}/issues/comments/${id}`, '-F', `body=@${file}`]);
    else gh(['api', '--method', 'POST', `repos/{owner}/{repo}/issues/${opts.pr}/comments`, '-F', `body=@${file}`]);
    console.log(`${id ? 'updated' : 'posted'} the bench comment on PR #${opts.pr}`);
    return 0;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

function trendCmd(opts) {
  if (!opts.current) throw usage('trend needs --current bench-report.json');
  const md = renderTrend(readJson(opts.current), loadHistory(opts.history), {
    title: opts.title ?? 'Bench', sha: opts.sha ?? '', limit: opts.limit ?? 10,
  });
  if (opts.out) writeFileSync(opts.out, md);
  else process.stdout.write(md);
  return 0;
}

function usage(message) {
  const e = new Error(message);
  e.usage = true;
  return e;
}

async function main() {
  try {
    const opts = parseCli(process.argv.slice(2));
    if (opts.help) {
      console.log(readFileSync(fileURLToPath(import.meta.url), 'utf8').split('\n').slice(1, 26).map((l) => l.replace(/^\/\/ ?/, '')).join('\n'));
      return 0;
    }
    const commands = { run, merge: mergeCmd, history: historyCmd, gate: gateCmd, trend: trendCmd, comment: commentCmd };
    return await commands[opts.command](opts);
  } catch (err) {
    console.error(`bench: ${err.message}`);
    return 2;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = await main();
}
