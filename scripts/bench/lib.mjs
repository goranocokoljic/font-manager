// lib.mjs — budgets, the JSON report, merging the per-OS reports and the trend
// table of scripts/bench.sh. Pure functions; bench.mjs does the processes and files.
//
// Per-OS report (bench.sh --out):
//   { schema: 1, os, arch, sha, date, library: { path, files, faces }, metrics: { <name>: Metric } }
//   first_scan is per SCAN_FILES files; it also carries { files, measured_ms }.
// bench-report.json (the `bench` CI job; read by scripts/harness/ci-wait.mjs and epic-sync.mjs):
//   { "<os>.<name>": Metric }
// Metric: { value: number|null, unit, target: number|null, pass: boolean|null, note? }
//   pass null = no budget, or not measured (note says why).
// On a PR, `bench.mjs gate` adds { baseline, change, regressed } to every entry (see gate()).

export const SCHEMA = 1;

/**
 * The budgets of CLAUDE.md rule 8, plus story budgets (preview_cache_hit: E4.3,
 * "a cache hit returns in under 2 ms"; smart_collections: E14.2, a search by a
 * smart collection and list_collections with 20 of them, each under 50 ms).
 * target null = reported, not budgeted.
 * `byOs` overrides the target on one OS; each override is recorded in an ADR.
 */
export const BUDGETS = {
  cold_start: { unit: 'ms', target: 1000 },
  first_scan: { unit: 'ms', target: 10000 },
  incremental_update: { unit: 'ms', target: null },
  search: { unit: 'ms', target: 50 },
  // macOS: one CoreText session registration per file (ADR 0008, Budget on macOS).
  activation_100: { unit: 'ms', target: 500, byOs: { macos: 1000 } },
  idle_memory: { unit: 'MB', target: 350 },
  preview_cache_hit: { unit: 'ms', target: 2 },
  smart_collections: { unit: 'ms', target: 50 },
};

export const OSES = ['windows', 'macos', 'linux'];

/** The library size of the first-scan budget: "10k-font first scan < 10 s". */
export const SCAN_FILES = 10000;

/**
 * The first-scan metric: the measured time scaled to SCAN_FILES files, so a
 * library of another size meets the same budget (E2.8). The metric carries
 * the file count and the measured time, and its note says both.
 */
export function firstScan(ms, files) {
  if (!files) return metric('first_scan', null, 'no font files scanned');
  const m = metric('first_scan', (ms * SCAN_FILES) / files,
    `${files} files in ${fmt(Math.round(ms) / 1000)} s, scaled to ${SCAN_FILES.toLocaleString('en-US')} files`);
  return { ...m, files, measured_ms: Math.round(ms * 10) / 10 };
}

/**
 * The 100-face activation metric (E6.6): the time of the one request, failed
 * when a face neither activated nor clashed with an installed font, since a
 * request whose registrations failed is fast and proves nothing.
 */
export function activation100(a, os = osName()) {
  const other = a.faces - a.done - a.conflicts;
  if (other <= 0) return metric('activation_100', a.ms, undefined, os);
  return { ...metric('activation_100', a.ms, `${other} of ${a.faces} faces were not activated`, os), pass: false };
}

/** The target of metric `name` on `os`. */
export function target(name, os = osName()) {
  const budget = BUDGETS[name];
  if (!budget) throw new Error(`unknown metric: ${name}`);
  return budget.byOs?.[os] ?? budget.target;
}

/**
 * One metric against its budget on `os` (default: this machine; bench.sh runs
 * on each OS). `value` null = not measured; give a `note`.
 */
export function metric(name, value, note, os = osName()) {
  const budget = BUDGETS[name];
  if (!budget) throw new Error(`unknown metric: ${name}`);
  const goal = target(name, os);
  const v = value == null ? null : Math.round(value * 10) / 10;
  const m = {
    value: v,
    unit: budget.unit,
    target: goal,
    pass: v == null || goal == null ? null : v <= goal,
  };
  if (note) m.note = note;
  return m;
}

export function osName(platform = process.platform) {
  return platform === 'win32' ? 'windows' : platform === 'darwin' ? 'macos' : 'linux';
}

export function median(values) {
  if (values.length === 0) return null;
  const s = [...values].sort((a, b) => a - b);
  const mid = s.length >> 1;
  return s.length % 2 ? s[mid] : (s[mid - 1] + s[mid]) / 2;
}

/**
 * Resident memory in MB from the output of the per-OS probe:
 * linux /proc/<pid>/status (VmRSS kB), macos `ps -o rss=` (kB), windows WorkingSet64 (bytes).
 */
export function parseRss(os, text) {
  if (os === 'linux') {
    const m = /^VmRSS:\s+(\d+)\s+kB/m.exec(text);
    return m ? Number(m[1]) / 1024 : null;
  }
  const n = Number(String(text).trim());
  if (!Number.isFinite(n) || String(text).trim() === '') return null;
  return os === 'windows' ? n / (1024 * 1024) : n / 1024;
}

/** The command that prints the resident memory of `pid` on `os`. */
export function rssProbe(os, pid) {
  if (os === 'linux') return null; // read /proc/<pid>/status directly
  if (os === 'macos') return ['ps', ['-o', 'rss=', '-p', String(pid)]];
  return ['powershell', ['-NoProfile', '-NonInteractive', '-Command', `(Get-Process -Id ${pid}).WorkingSet64`]];
}

export function failures(metrics) {
  return Object.entries(metrics).filter(([, m]) => m.pass === false).map(([k]) => k);
}

/** A PR fails the bench check when a budgeted metric is this much worse than main. */
export const REGRESSION_TOLERANCE = 0.1;
/**
 * The baseline is the slowest of this many runs on main. Shared CI runners swing
 * by 20-80% with GitHub's load, so a PR is only slower when it is slower than
 * every recent run of main.
 */
export const BASELINE_RUNS = 5;
/** A change smaller than this share of the budget is runner noise, never a regression. */
export const NOISE_FLOOR = 0.02;
/** Marks the sticky PR comment of the bench job, so a new push updates it. */
export const COMMENT_MARKER = '<!-- fm-bench -->';

/**
 * `gh run list` arguments for the runs on main whose reports are the gate's
 * baseline: the nightly ones and the ones started by hand on main (every event:
 * a PR run carries its own branch, never main). One nightly alone is too noisy
 * a baseline, so a fresh main can be measured on demand. Every conclusion counts:
 * a run with a metric over budget (the known-red first scan) is red, and it is
 * still the measurement of main. Its bench-report artifact is uploaded on a red
 * run too; a run without one is skipped.
 */
export function nightlyRunsArgs(limit) {
  return [
    'run', 'list', '--workflow', 'bench.yml', '--branch', 'main', '--status', 'completed',
    '--limit', String(limit), '--json', 'databaseId,headSha,createdAt,url',
  ];
}

/**
 * The PR regression gate. Each budgeted, measured metric of `current` gets
 * `baseline` (slowest of the last `runs` values on main), `change`
 * (fraction vs the baseline) and `regressed`:
 *   - more than `tolerance` worse than the baseline (and above the noise floor), or
 *   - over its budget while the baseline was within it.
 * A metric already over budget on main (known red) blocks only when it gets
 * slower still. Without a nightly value on main the budget is the gate:
 * `regressed` = over budget. Unbudgeted or unmeasured metrics get `regressed: null`.
 * `history`: [{ report }] newest first, as loadHistory returns it. Returns a new report.
 */
export function gate(current, history, { tolerance = REGRESSION_TOLERANCE, runs = BASELINE_RUNS } = {}) {
  const out = {};
  for (const [key, m] of Object.entries(current)) {
    if (m.target == null || m.value == null) {
      out[key] = { ...m, baseline: null, change: null, regressed: null };
      continue;
    }
    const values = history.slice(0, runs).map((r) => r.report?.[key]?.value).filter((v) => typeof v === 'number');
    const baseline = values.length ? Math.max(...values) : null;
    let regressed = m.pass === false;
    let change = null;
    if (baseline != null) {
      change = baseline === 0 ? null : Math.round(((m.value - baseline) / baseline) * 1000) / 1000;
      const slower = m.value - baseline > NOISE_FLOOR * m.target && m.value > baseline * (1 + tolerance);
      regressed = slower || (m.pass === false && baseline <= m.target);
    }
    out[key] = { ...m, baseline, change, regressed };
  }
  return out;
}

/**
 * The entries that make the bench check red. A gated report (a PR): the
 * regressions and the missing OS reports. Otherwise (nightly): every metric over budget.
 */
export function blocking(report) {
  const entries = Object.entries(report);
  const gated = entries.some(([, m]) => 'regressed' in m);
  return entries
    .filter(([, m]) => (gated ? m.regressed === true || (m.target == null && m.pass === false) : m.pass === false))
    .map(([k]) => k);
}

/** Why `key` blocks, in one line (a GitHub annotation, the PR comment). */
export function reason(key, m) {
  if (m.target == null) return `${key}: ${m.note ?? 'no report'}`;
  const vs = m.baseline == null
    ? `budget ${fmt(m.target)} ${m.unit}`
    : `${fmt(m.baseline)} ${m.unit} on main${m.change == null ? '' : `, ${pct(m.change)}`}`;
  const over = m.pass === false ? `, over the ${fmt(m.target)} ${m.unit} budget` : '';
  return `${key}: ${fmt(m.value)} ${m.unit} vs ${vs}${over}`;
}

function pct(fraction) {
  const p = fraction * 100;
  return `${p > 0 ? '+' : ''}${p.toFixed(0)}%`;
}

/** The markdown section of the PR regression gate (renderTrend appends it). */
export function renderGate(gated, { tolerance = REGRESSION_TOLERANCE } = {}) {
  const lines = [
    `### Regression gate — this PR vs main (fails at more than ${Math.round(tolerance * 100)}% worse)`,
    '',
    '| Metric | Target | Baseline | Now | Change | Verdict |',
    '|---|---|---|---|---|---|',
  ];
  for (const key of Object.keys(gated).sort()) {
    const m = gated[key];
    if (m.target == null || m.value == null) continue;
    const verdict = m.regressed ? '✗ regressed' : m.pass === false ? 'over budget on main too' : '✓';
    const baseline = m.baseline == null ? '– (budget)' : `${fmt(m.baseline)} ${m.unit}`;
    lines.push(`| ${key} | ${fmt(m.target)} ${m.unit} | ${baseline} | ${fmt(m.value)} ${m.unit} | ${m.change == null ? '' : pct(m.change)} | ${verdict} |`);
  }
  const blocked = blocking(gated);
  lines.push('', blocked.length
    ? `**Bench check failed:**\n${blocked.map((k) => `- ${reason(k, gated[k])}`).join('\n')}`
    : `No budgeted metric regressed by more than ${Math.round(tolerance * 100)}%.`);
  return lines.join('\n');
}

/**
 * bench-report.json from the per-OS reports. An OS in `expect` without a
 * report gets a failed `<os>.report` entry, so a crashed run is not green.
 */
export function merge(reports, expect = OSES) {
  const out = {};
  for (const os of expect) {
    const r = reports.find((x) => x.os === os);
    if (!r) {
      out[`${os}.report`] = { value: null, unit: '', target: null, pass: false, note: 'no report: the run failed' };
      continue;
    }
    for (const [name, m] of Object.entries(r.metrics)) out[`${os}.${name}`] = m;
  }
  return out;
}

function fmt(v) {
  if (v == null) return '–';
  return Number.isInteger(v) ? String(v) : v.toFixed(1);
}

function delta(now, prev) {
  if (now == null || prev == null || prev === 0) return '';
  const pct = ((now - prev) / prev) * 100;
  return `${pct > 0 ? '+' : ''}${pct.toFixed(0)}%`;
}

/**
 * Markdown trend: one row per metric, now vs the previous run and the last
 * runs (oldest first). `history`: [{ date, sha, url?, report }] newest first.
 */
export function renderTrend(current, history, { title = 'Bench', sha = '', limit = 10 } = {}) {
  const runs = history.slice(0, limit);
  const lines = [
    `## ${title}${sha ? ` — ${sha.slice(0, 7)}` : ''}`,
    '',
    runs.length
      ? `Compared with ${runs.length} earlier nightly run(s) on main; newest ${runs[0].date.slice(0, 10)} (${runs[0].sha.slice(0, 7)}).`
      : 'No earlier nightly run on main to compare with.',
    '',
    '| Metric | Target | Now | Previous | Change | Trend (oldest → now) |',
    '|---|---|---|---|---|---|',
  ];
  for (const key of Object.keys(current).sort()) {
    const m = current[key];
    const prev = runs[0]?.report?.[key]?.value ?? null;
    const series = [...runs].reverse().map((r) => fmt(r.report?.[key]?.value ?? null));
    const status = m.pass === false ? ' ✗' : m.pass === true ? ' ✓' : '';
    const target = m.target == null ? '–' : `${fmt(m.target)} ${m.unit}`;
    const now = m.value == null ? `– ${m.note ? `(${m.note})` : ''}`.trim() : `${fmt(m.value)} ${m.unit}${status}`;
    lines.push(`| ${key} | ${target} | ${now} | ${fmt(prev)} | ${delta(m.value, prev)} | ${[...series, fmt(m.value)].join(' → ')} |`);
  }
  const failed = failures(current);
  lines.push('', failed.length ? `**Over budget:** ${failed.join(', ')}` : 'Every measured metric is within its budget.');
  if (Object.values(current).some((m) => 'regressed' in m)) lines.push('', renderGate(current));
  return lines.join('\n') + '\n';
}

/** A plain-text table of one per-OS report, for the terminal. */
export function renderTable(report) {
  const rows = Object.entries(report.metrics).map(([k, m]) => {
    const verdict = m.pass === true ? 'ok' : m.pass === false ? 'OVER BUDGET' : m.value == null ? 'not measured' : 'no budget';
    const target = m.target == null ? '' : `<= ${m.target} ${m.unit}`;
    return [k, m.value == null ? '–' : `${fmt(m.value)} ${m.unit}`, target, verdict + (m.note ? ` (${m.note})` : '')];
  });
  const w = [0, 1, 2].map((i) => Math.max(...rows.map((r) => r[i].length), 6));
  return rows.map((r) => `${r[0].padEnd(w[0])}  ${r[1].padStart(w[1])}  ${r[2].padEnd(w[2])}  ${r[3]}`).join('\n');
}

export function parseCli(argv, env = process.env) {
  const [command = 'run', ...rest] = argv[0]?.startsWith('--') || argv.length === 0 ? ['run', ...argv] : argv;
  const opts = { command, runs: 5, rounds: 20, settleMs: 2000, build: true, files: [] };
  for (let i = 0; i < rest.length; i++) {
    const a = rest[i];
    const value = () => {
      const v = rest[++i];
      if (v === undefined) throw new Error(`${a} needs a value`);
      return v;
    };
    if (a === '--library') opts.library = value();
    else if (a === '--out') opts.out = value();
    else if (a === '--bin') opts.bin = value();
    else if (a === '--home') opts.home = value();
    else if (a === '--runs') opts.runs = positiveInt(a, value());
    else if (a === '--rounds') opts.rounds = positiveInt(a, value());
    else if (a === '--settle-ms') opts.settleMs = positiveInt(a, value());
    else if (a === '--no-build') opts.build = false;
    else if (a === '--current') opts.current = value();
    else if (a === '--history') opts.history = value();
    else if (a === '--limit') opts.limit = positiveInt(a, value());
    else if (a === '--title') opts.title = value();
    else if (a === '--sha') opts.sha = value();
    else if (a === '--pr') opts.pr = positiveInt(a, value());
    else if (a === '--body') opts.body = value();
    else if (a === '--help' || a === '-h') opts.help = true;
    else if (!a.startsWith('--') && command !== 'run') opts.files.push(a);
    else throw new Error(`unknown argument: ${a}`);
  }
  if (!['run', 'merge', 'history', 'gate', 'trend', 'comment'].includes(command)) throw new Error(`unknown command: ${command}`);
  if (command === 'run') opts.library ??= env.FM_REFERENCE_LIBRARY;
  return opts;
}

function positiveInt(flag, v) {
  const n = Number(v);
  if (!Number.isInteger(n) || n < 1) throw new Error(`${flag} needs a positive integer`);
  return n;
}
