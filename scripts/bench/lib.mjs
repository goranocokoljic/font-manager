// lib.mjs — budgets, the JSON report, merging the per-OS reports and the trend
// table of scripts/bench.sh. Pure functions; bench.mjs does the processes and files.
//
// Per-OS report (bench.sh --out):
//   { schema: 1, os, arch, sha, date, library: { path, files, faces }, metrics: { <name>: Metric } }
// bench-report.json (the `bench` CI job; read by scripts/harness/ci-wait.mjs and epic-sync.mjs):
//   { "<os>.<name>": Metric }
// Metric: { value: number|null, unit, target: number|null, pass: boolean|null, note? }
//   pass null = no budget, or not measured (note says why).

export const SCHEMA = 1;

/** The budgets of CLAUDE.md rule 8. target null = reported, not budgeted. */
export const BUDGETS = {
  cold_start: { unit: 'ms', target: 1000 },
  first_scan: { unit: 'ms', target: 10000 },
  incremental_update: { unit: 'ms', target: null },
  search: { unit: 'ms', target: 50 },
  activation_100: { unit: 'ms', target: 500 },
  idle_memory: { unit: 'MB', target: 350 },
};

export const OSES = ['windows', 'macos', 'linux'];

/** One metric against its budget. `value` null = not measured; give a `note`. */
export function metric(name, value, note) {
  const budget = BUDGETS[name];
  if (!budget) throw new Error(`unknown metric: ${name}`);
  const v = value == null ? null : Math.round(value * 10) / 10;
  const m = {
    value: v,
    unit: budget.unit,
    target: budget.target,
    pass: v == null || budget.target == null ? null : v <= budget.target,
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
    else if (a === '--help' || a === '-h') opts.help = true;
    else if (!a.startsWith('--') && command !== 'run') opts.files.push(a);
    else throw new Error(`unknown argument: ${a}`);
  }
  if (!['run', 'merge', 'history', 'trend'].includes(command)) throw new Error(`unknown command: ${command}`);
  if (command === 'run') opts.library ??= env.FM_REFERENCE_LIBRARY;
  return opts;
}

function positiveInt(flag, v) {
  const n = Number(v);
  if (!Number.isInteger(n) || n < 1) throw new Error(`${flag} needs a positive integer`);
  return n;
}
