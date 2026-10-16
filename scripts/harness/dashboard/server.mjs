#!/usr/bin/env node
// go-harness dashboard — zero-dependency live server (adapted from the toprope
// dev-cycle dashboard). Reads the harness telemetry fresh on every /api/data
// request, so the page always shows the latest run. No build step.
//
// Usage:  pnpm dashboard            (or: node scripts/harness/dashboard/server.mjs)
//         --port 8090 | FM_DASHBOARD_PORT
//         --data <dir> | FM_HARNESS_DIR     (default: <repo>/.harness)
//
// Sources: .harness/{stories,tasks,review-cycles}.jsonl, .harness/state/*.json
// (stories running now), dev-docs/harness/kb/lessons/*.json (review KB),
// dev-docs/harness/parking-lot.md (parked out-of-diff notes).

import http from 'node:http';
import { execFileSync } from 'node:child_process';
import { readFile, readdir, stat } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(__dirname, '..', '..', '..');
const PUBLIC = path.join(__dirname, 'public');

function argValue(flag, fallback) {
  const i = process.argv.indexOf(flag);
  return i !== -1 && process.argv[i + 1] ? process.argv[i + 1] : fallback;
}

/** A UTF-8 byte order mark (Windows editors add one) breaks JSON.parse. */
const stripBom = (s) => (s.charCodeAt(0) === 0xfeff ? s.slice(1) : s);

/** Tolerant JSONL reader: a torn last line (a write in progress) is skipped, not fatal. */
export async function readJsonl(file) {
  let raw;
  try {
    raw = await readFile(file, 'utf8');
  } catch {
    return { rows: [], skipped: 0, mtime: null };
  }
  const rows = [];
  let skipped = 0;
  for (const line of stripBom(raw).split(/\r?\n/)) {
    if (!line.trim()) continue;
    try { rows.push(JSON.parse(line)); } catch { skipped++; }
  }
  const info = await stat(file).catch(() => null);
  return { rows, skipped, mtime: info ? info.mtime.toISOString() : null };
}

/** Every `*.json` in a folder; `name` (the file name without .json) is added as `nameKey` when given. */
async function readJsonDir(dir, nameKey = null) {
  let names;
  try { names = (await readdir(dir)).filter((n) => n.endsWith('.json')); } catch { return []; }
  const out = [];
  for (const n of names) {
    try {
      const obj = JSON.parse(stripBom(await readFile(path.join(dir, n), 'utf8')));
      out.push(nameKey ? { [nameKey]: n.slice(0, -5), ...obj } : obj);
    } catch { /* skip a half-written file */ }
  }
  return out;
}

/** Parked lines: `- [E3.2 #61] path:line — note` under `## <date>` headings. */
export function parseParkingLot(text) {
  const items = [];
  let date = null;
  for (const line of text.split(/\r?\n/)) {
    const h = /^##\s+(.+?)\s*$/.exec(line);
    if (h) { date = h[1]; continue; }
    const m = /^\s*-\s+\[([^\]]+)\]\s+(.*)$/.exec(line);
    if (m) items.push({ date, story: m[1], note: m[2] });
  }
  return items;
}

function repoUrl() {
  try {
    const url = execFileSync('git', ['-C', REPO, 'remote', 'get-url', 'origin'], { encoding: 'utf8', windowsHide: true }).trim();
    const m = /github\.com[:/](.+?)(?:\.git)?$/.exec(url);
    return m ? `https://github.com/${m[1]}` : null;
  } catch {
    return null;
  }
}

export async function buildPayload(dataDir, repoRoot = REPO) {
  const [stories, runs, cycles] = await Promise.all(
    ['stories.jsonl', 'tasks.jsonl', 'review-cycles.jsonl'].map((f) => readJsonl(path.join(dataDir, f))),
  );
  const lanes = await readJsonDir(path.join(dataDir, 'state'), 'lane'); // state/<lane>.json
  const lessons = await readJsonDir(path.join(repoRoot, 'dev-docs', 'harness', 'kb', 'lessons'));
  let parking = [];
  try { parking = parseParkingLot(await readFile(path.join(repoRoot, 'dev-docs', 'harness', 'parking-lot.md'), 'utf8')); } catch { /* none yet */ }
  const info = (x) => ({ count: x.rows.length, skipped: x.skipped, mtime: x.mtime });
  return {
    generatedAt: new Date().toISOString(),
    dataDir,
    repoUrl: repoUrl(),
    files: { stories: info(stories), runs: info(runs), cycles: info(cycles) },
    stories: stories.rows,
    runs: runs.rows,
    cycles: cycles.rows,
    lanes,
    lessons,
    parking,
  };
}

const TYPES = { '.html': 'text/html; charset=utf-8', '.mjs': 'text/javascript; charset=utf-8' };

function startServer(port, dataDir) {
  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url, `http://${req.headers.host}`);
    try {
      if (url.pathname === '/api/data') {
        const body = JSON.stringify(await buildPayload(dataDir));
        res.writeHead(200, { 'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store' });
        res.end(body);
        return;
      }
      const file = { '/': path.join(PUBLIC, 'index.html'), '/index.html': path.join(PUBLIC, 'index.html'), '/stats.mjs': path.join(__dirname, 'stats.mjs') }[url.pathname];
      if (file) {
        res.writeHead(200, { 'Content-Type': TYPES[path.extname(file)], 'Cache-Control': 'no-store' });
        res.end(await readFile(file));
        return;
      }
      res.writeHead(404, { 'Content-Type': 'text/plain' });
      res.end('Not found');
    } catch (err) {
      res.writeHead(500, { 'Content-Type': 'text/plain' });
      res.end(`Server error: ${err.message}`);
    }
  });
  // Local tool: listen on loopback only.
  server.listen(port, '127.0.0.1', () => {
    console.log(`go-harness dashboard:  http://localhost:${port}`);
    console.log(`telemetry:             ${dataDir}`);
  });
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const port = Number(argValue('--port', process.env.FM_DASHBOARD_PORT || 8090));
  const dataDir = path.resolve(argValue('--data', process.env.FM_HARNESS_DIR || path.join(REPO, '.harness')));
  startServer(port, dataDir);
}
