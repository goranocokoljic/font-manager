// Shared helpers for go-harness scripts. Node 18+ ESM, no dependencies.
//
// Everything that talks to GitHub goes through `gh` (authenticated CLI), run
// from the current working directory so `{owner}/{repo}` resolves to the
// checkout the harness is driving (the lane worktree).

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
export const REPO_ROOT = resolve(__dirname, '..', '..');
export const CONFIG_PATH = resolve(REPO_ROOT, 'go-harness.config.json');

/** GitHub API version header for the issue-dependencies endpoints. */
export const DEPS_API_VERSION = '2026-03-10';

const SIZE_RANK = { S: 0, M: 1, L: 2 };
const SIZE_FACTOR = { S: 1, M: 2.5, L: 5 };

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------
export const DEFAULT_CONFIG = {
  merge: 'human',
  base_branch: 'main',
  budget_base_usd: 15,
  ci: {
    required_checks: ['ci / windows', 'ci / macos', 'ci / linux'],
    bench_check: 'bench / bench',
    bench_workflow: 'bench.yml',
    bench_artifact: 'bench-report',
    timeout_min: 75,
    poll_sec: 30,
    retry_budget: 3,
  },
  lanes: {
    core: { tracks: ['core', 'platform', 'data', 'ops', 'plugins'], worktree: '../font-manager-core' },
    ui: { tracks: ['ui'], worktree: '../font-manager-ui' },
  },
  lane_max_unmerged: 1,
  auto_merge: { exclude_tracks: ['platform'], max_branch_updates: 3, max_syncs: 2 },
  idle: { poll_min: 10, max_hours: 8 },
  main_red: { poll_min: 10, max_hours: 6 },
  review: { max_cycles: 3, model: 'inherit' },
  contract_epics: ['E1', 'E25'],
  close_epics: true,
  bot_login: '',
};

export function loadConfig(path = CONFIG_PATH) {
  if (!existsSync(path)) return structuredClone(DEFAULT_CONFIG);
  const user = JSON.parse(readFileSync(path, 'utf8'));
  return deepMerge(structuredClone(DEFAULT_CONFIG), user);
}

function deepMerge(base, over) {
  for (const [k, v] of Object.entries(over ?? {})) {
    if (k.startsWith('$') || k.startsWith('_')) continue; // $schema / _comment keys
    if (v && typeof v === 'object' && !Array.isArray(v) && base[k] && typeof base[k] === 'object') {
      base[k] = deepMerge(base[k], v);
    } else {
      base[k] = v;
    }
  }
  return base;
}

/** Per-story budget in USD: S = 1x, M = 2.5x, L = 5x the base amount. */
export function storyBudget(size, cfg) {
  return (SIZE_FACTOR[size] ?? SIZE_FACTOR.L) * cfg.budget_base_usd;
}

/** Which lane works a track. Returns null for a track no lane owns. */
export function laneOf(track, cfg) {
  for (const [lane, def] of Object.entries(cfg.lanes)) {
    if (def.tracks.includes(track)) return lane;
  }
  return null;
}

// ---------------------------------------------------------------------------
// Issue body parsing
// ---------------------------------------------------------------------------

/**
 * Parse the `<!-- fm-meta ... -->` block at the end of an issue body.
 * `key: value` lines; `depends` is a bracketed comma list; `human` is a boolean.
 * Inline `# comments` are stripped. Returns null when there is no block.
 */
export function parseMeta(body) {
  const m = /<!--\s*fm-meta\s*\r?\n([\s\S]*?)-->/.exec(body ?? '');
  if (!m) return null;
  const meta = {};
  for (const raw of m[1].split(/\r?\n/)) {
    const line = raw.replace(/\s+#.*$/, '').trim();
    const kv = /^([A-Za-z_][\w-]*)\s*:\s*(.*)$/.exec(line);
    if (!kv) continue;
    const [, key, value] = kv;
    meta[key] = value.trim();
  }
  meta.depends = parseList(meta.depends);
  meta.human = meta.human === 'true';
  if (meta.phase !== undefined) meta.phase = Number(meta.phase);
  return meta;
}

function parseList(v) {
  if (!v) return [];
  const inner = v.replace(/^\[/, '').replace(/\]$/, '');
  return inner.split(',').map((s) => s.trim()).filter(Boolean);
}

/** Split a story id like "E3.12" into numbers for ordering. */
export function idParts(id) {
  const m = /^E(\d+)(?:\.(\d+))?$/.exec(id ?? '');
  return m ? { epic: Number(m[1]), story: Number(m[2] ?? 0) } : { epic: 9999, story: 9999 };
}

/** Ordering inside a lane: phase, epic number, story number, size. */
export function compareStories(a, b) {
  const pa = idParts(a.meta.id);
  const pb = idParts(b.meta.id);
  return (
    (a.meta.phase ?? 9) - (b.meta.phase ?? 9) ||
    pa.epic - pb.epic ||
    pa.story - pb.story ||
    (SIZE_RANK[a.meta.size] ?? 9) - (SIZE_RANK[b.meta.size] ?? 9)
  );
}

/**
 * The `## Acceptance criteria` checkbox list. Returns
 * [{ index, line, text, checked, numeric }] where `line` is the 0-based line
 * number in the body and `numeric` marks a criterion with a number + unit
 * (ms, s, fps, MB) that needs the bench job.
 */
export function parseCriteria(body) {
  const lines = (body ?? '').split(/\r?\n/);
  const start = lines.findIndex((l) => /^##\s+Acceptance criteria\s*$/i.test(l.trim()));
  if (start < 0) return [];
  const out = [];
  for (let i = start + 1; i < lines.length; i++) {
    const l = lines[i];
    if (/^##\s/.test(l.trim())) break;
    const m = /^\s*[-*]\s+\[( |x|X)\]\s+(.*)$/.exec(l);
    if (!m) continue;
    out.push({
      index: out.length + 1,
      line: i,
      text: m[2].trim(),
      checked: m[1].toLowerCase() === 'x',
      numeric: isNumericCriterion(m[2]),
    });
  }
  return out;
}

export function isNumericCriterion(text) {
  return /(?:^|[^\w.])\d[\d.,]*\s*(?:ms|s|fps|MB|GB|KB)\b/.test(text ?? '');
}

// ---------------------------------------------------------------------------
// Areas (lane overlap)
// ---------------------------------------------------------------------------

/** Files that every lane regenerates; they never count as overlap. */
const GENERATED = new Set(['apps/desktop/src/api/types.ts', 'Cargo.lock', 'pnpm-lock.yaml']);

/**
 * The area a changed file belongs to: a crate (`crates/fm-core`), a frontend
 * subtree (`apps/desktop/src/components`), the Tauri host
 * (`apps/desktop/src-tauri`), or the top-level directory.
 */
export function areaOf(path) {
  const p = path.replace(/\\/g, '/');
  if (GENERATED.has(p)) return null;
  const seg = p.split('/');
  if (seg[0] === 'crates' && seg[1]) return `crates/${seg[1]}`;
  if (seg[0] === 'apps' && seg[1] === 'desktop') {
    if (seg[2] === 'src' && seg.length > 4) return `apps/desktop/src/${seg[3]}`;
    if (seg[2]) return `apps/desktop/${seg[2]}`;
  }
  return seg.length > 1 ? seg[0] : null; // root files (README, configs) do not count
}

/** Two areas overlap when one is a prefix of the other. */
export function areasOverlap(a, b) {
  return a === b || a.startsWith(`${b}/`) || b.startsWith(`${a}/`);
}

/**
 * The areas a not-yet-started story will probably touch: the crates and
 * frontend paths its body names, plus its track's home area.
 */
export function predictAreas(story) {
  const areas = new Set();
  const body = story.body ?? '';
  for (const m of body.matchAll(/\b(fm-(?:core|api|platform|cli|mcp))\b/g)) areas.add(`crates/${m[1]}`);
  for (const m of body.matchAll(/\bapps\/desktop\/(src(?:\/[\w-]+)?|src-tauri)/g)) areas.add(`apps/desktop/${m[1]}`);
  const home = {
    core: 'crates/fm-core',
    platform: 'crates/fm-platform',
    ui: 'apps/desktop/src',
    data: 'data',
    plugins: 'plugins',
    ops: '.github',
  }[story.meta?.track];
  if (home) areas.add(home);
  return [...areas];
}

// ---------------------------------------------------------------------------
// gh
// ---------------------------------------------------------------------------
export function gh(args, { input, allowFail = false } = {}) {
  try {
    return execFileSync('gh', args, {
      encoding: 'utf8',
      input,
      maxBuffer: 64 * 1024 * 1024,
      windowsHide: true,
      stdio: ['pipe', 'pipe', 'pipe'],
    });
  } catch (e) {
    if (allowFail) return null;
    const msg = (e.stderr || e.message || '').toString().trim();
    throw new Error(`gh ${args.slice(0, 3).join(' ')} failed: ${msg}`, { cause: e });
  }
}

export function ghJson(args, opts) {
  const out = gh(args, opts);
  return out == null || out.trim() === '' ? null : JSON.parse(out);
}

/** All pages of a REST list endpoint, flattened. */
export function ghApiList(path, headers = []) {
  const h = headers.flatMap((x) => ['-H', x]);
  const out = gh(['api', '--paginate', '--slurp', ...h, path]);
  return JSON.parse(out).flat();
}

export function blockedBy(number) {
  return ghApiList(`repos/{owner}/{repo}/issues/${number}/dependencies/blocked_by`, [
    `X-GitHub-Api-Version: ${DEPS_API_VERSION}`,
  ]);
}

export function subIssues(number) {
  return ghApiList(`repos/{owner}/{repo}/issues/${number}/sub_issues`);
}

export function currentLogin() {
  return (gh(['api', 'user', '-q', '.login'], { allowFail: true }) ?? '').trim();
}

export function labelNames(issue) {
  return (issue.labels ?? []).map((l) => (typeof l === 'string' ? l : l.name));
}

/** Parse `--flag value` / `--flag=value` / `--bool` argv into an object. */
export function parseArgs(argv) {
  const out = { _: [] };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (!a.startsWith('--')) {
      out._.push(a);
      continue;
    }
    const eq = a.indexOf('=');
    if (eq !== -1) out[a.slice(2, eq)] = a.slice(eq + 1);
    else if (argv[i + 1] !== undefined && !argv[i + 1].startsWith('--')) out[a.slice(2)] = argv[++i];
    else out[a.slice(2)] = true;
  }
  return out;
}

/** True when this module is the entry point (`node script.mjs`). */
export function isMain(metaUrl) {
  return process.argv[1] && resolve(process.argv[1]) === fileURLToPath(metaUrl);
}
