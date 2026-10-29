#!/usr/bin/env node
// webview.mjs — E35.1: measure the real desktop app (webview and all) against
// the spec's scroll and memory targets with the reference library loaded.
//
//   node scripts/bench/webview.mjs [--bin EXE] [--library DIR] [--home DIR]
//        [--fresh] [--out FILE] [--port N] [--settle-ms N] [--scroll-ms N]
//
// What it does, in order:
//   1. Makes a scratch home (default target/bench/webview-home) whose
//      config.json turns every optional feature on. The user's own folders
//      are not touched: the app is started with LOCALAPPDATA, APPDATA and
//      USERNAME pointed at the scratch home (Windows); on macOS and Linux with
//      HOME and the XDG folders.
//   2. Starts the release app with the webview's remote-debugging port open
//      and attaches to the page over the Chrome DevTools Protocol.
//   3. Adds the library through the app's local API when the scratch index
//      does not have it yet, and waits for the font list to settle.
//   4. Idles for --settle-ms and samples the memory of the whole process tree
//      (the app and every webview process it owns).
//   5. For each list view (List, Table, Grid, Waterfall) scrolls the main
//      area at three paces for --scroll-ms each, recording every animation
//      frame of the page's main thread: "wheel" (one notch every 50 ms, a
//      hand on the wheel), "fling" (two notches every frame, a touchpad
//      flick) and "drag" (the whole list top to bottom in --scroll-ms, what a
//      scrollbar drag does). Wheel and fling are real input events over CDP;
//      drag moves scrollTop from the page. Memory is sampled after each view.
//   6. Idles again, samples memory, writes the JSON report, prints a table.
//
// Budgets (spec "Non-functional requirements"): scroll at 60 fps in every view
// mode, no jank on cache misses; idle memory with 10k fonts < 350 MB (webview
// UI). A pass here means: in every view and pace at most 1 % of frames run
// longer than 18 ms (16.7 ms at 60 Hz plus a little) and no frame runs longer
// than 50 ms; and the private working set of the process tree stays under
// 350 MB after the idle wait. The report keeps every number so that the ADR
// (E35.3) can judge the margins, not only the verdict.
//
// The Layout view is left out: it shows one family at a time, not the list.
//
// Needs: a release build (pnpm tauri build --no-bundle), Node 22, and the
// desktop app's Playwright (pnpm install). Windows is the only OS this has
// run on; the process-tree memory sampler has a macOS/Linux branch (ps) and
// the webview attach there is untested.

import { spawn, execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..', '..');
const require = createRequire(path.join(root, 'apps', 'desktop', 'package.json'));
const { chromium } = require('@playwright/test');

const MODES = ['List', 'Table', 'Grid', 'Waterfall'];
const PACES = {
  wheel: { everyMs: 50, notches: 1 }, // ~2,000 px/s
  fling: { everyMs: 16, notches: 2 }, // ~12,500 px/s
  drag: null, // the whole list in --scroll-ms, from the page
};
const NOTCH_PX = 100;
const FRAME_BUDGET_MS = 18; // 16.7 ms at 60 Hz plus a little
const LONG_FRAME_MS = 50; // visible jank
const MEMORY_BUDGET_MB = 350;

function parseArgs(argv) {
  const opts = {
    bin: path.join(root, 'target', 'release', os.platform() === 'win32' ? 'fm-app.exe' : 'fm-app'),
    library: process.env.FM_REFERENCE_LIBRARY ?? '',
    home: path.join(root, 'target', 'bench', 'webview-home'),
    fresh: false,
    out: path.join(root, 'target', 'bench', `webview-${osName()}.json`),
    port: 9229,
    settleMs: 60_000,
    scrollMs: 5_000,
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const next = () => argv[++i];
    if (a === '--bin') opts.bin = path.resolve(next());
    else if (a === '--library') opts.library = path.resolve(next());
    else if (a === '--home') opts.home = path.resolve(next());
    else if (a === '--fresh') opts.fresh = true;
    else if (a === '--out') opts.out = path.resolve(next());
    else if (a === '--port') opts.port = Number(next());
    else if (a === '--settle-ms') opts.settleMs = Number(next());
    else if (a === '--scroll-ms') opts.scrollMs = Number(next());
    else if (a === '--help' || a === '-h') {
      console.log(fs.readFileSync(fileURLToPath(import.meta.url), 'utf8').split('\n\n')[0]);
      process.exit(0);
    } else usage(`unknown option ${a}`);
  }
  if (!opts.library) usage('--library DIR or FM_REFERENCE_LIBRARY is required');
  if (!fs.existsSync(opts.library)) usage(`no library at ${opts.library}`);
  if (!fs.existsSync(opts.bin)) usage(`no app at ${opts.bin} (pnpm tauri build --no-bundle)`);
  return opts;
}

function usage(msg) {
  console.error(`webview: ${msg}`);
  process.exit(2);
}

function osName() {
  return { win32: 'windows', darwin: 'macos', linux: 'linux' }[os.platform()] ?? os.platform();
}

// --- the scratch home ------------------------------------------------------

function prepareHome(opts) {
  if (opts.fresh) fs.rmSync(opts.home, { recursive: true, force: true });
  const win = os.platform() === 'win32';
  const local = win ? path.join(opts.home, 'Local') : path.join(opts.home, '.local', 'share');
  const roaming = win ? path.join(opts.home, 'Roaming') : path.join(opts.home, '.config');
  const app = win ? 'FontManager' : 'font-manager';
  const data = path.join(local, app, 'data');
  const config = path.join(roaming, app);
  fs.mkdirSync(data, { recursive: true });
  fs.mkdirSync(config, { recursive: true });
  // Every optional feature on; nothing that phones home.
  fs.writeFileSync(
    path.join(config, 'config.json'),
    JSON.stringify(
      {
        mcpEnabled: true,
        telemetry: false,
        checkUpdatesAutomatically: false,
        keepInTray: false,
        reactivateOnLaunch: false,
      },
      null,
      2,
    ),
  );
  const env = { ...process.env };
  if (win) {
    env.LOCALAPPDATA = local;
    env.APPDATA = roaming;
    env.USERNAME = 'fm-bench'; // its own IPC pipe, beside a running app
    // Tauri picks the WebView2 user-data folder from the known-folder API,
    // not from LOCALAPPDATA, so a running Font Manager would share it and
    // WebView2 refuses a second environment with other arguments
    // (0x8007139F). This variable overrides the folder the app passes.
    env.WEBVIEW2_USER_DATA_FOLDER = path.join(local, 'FontManager', 'EBWebView');
    env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = `--remote-debugging-port=${opts.port}`;
  } else {
    env.HOME = opts.home;
    env.XDG_DATA_HOME = local;
    env.XDG_CONFIG_HOME = roaming;
    env.XDG_CACHE_HOME = path.join(opts.home, '.cache');
    env.XDG_RUNTIME_DIR = path.join(opts.home, 'run');
    fs.mkdirSync(env.XDG_RUNTIME_DIR, { recursive: true });
    env.WEBKIT_INSPECTOR_HTTP_SERVER = `127.0.0.1:${opts.port}`;
  }
  return { env, data };
}

// --- the process tree and its memory ---------------------------------------

function processTable() {
  if (os.platform() === 'win32') {
    const script = [
      '$p = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name',
      '$m = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process | Select-Object IDProcess, WorkingSetPrivate, WorkingSet, PrivateBytes',
      '$mem = @{}; foreach ($r in $m) { $mem[[int]$r.IDProcess] = $r }',
      '$p | ForEach-Object { $x = $mem[[int]$_.ProcessId]; [pscustomobject]@{ pid = $_.ProcessId; ppid = $_.ParentProcessId; name = $_.Name; private = [int64]($x.WorkingSetPrivate); ws = [int64]($x.WorkingSet); commit = [int64]($x.PrivateBytes) } } | ConvertTo-Json -Compress',
    ].join('; ');
    const out = execFileSync('powershell', ['-NoProfile', '-NonInteractive', '-Command', script], {
      encoding: 'utf8',
      maxBuffer: 64 << 20,
    });
    return JSON.parse(out);
  }
  // ps: pid, ppid, rss (KiB), vsz (KiB), comm. No private working set here.
  const out = execFileSync('ps', ['-axo', 'pid=,ppid=,rss=,vsz=,comm='], { encoding: 'utf8' });
  return out
    .trim()
    .split('\n')
    .map((line) => {
      const [pid, ppid, rss, vsz, ...comm] = line.trim().split(/\s+/);
      return {
        pid: Number(pid),
        ppid: Number(ppid),
        name: comm.join(' '),
        private: Number(rss) * 1024,
        ws: Number(rss) * 1024,
        commit: Number(vsz) * 1024,
      };
    });
}

function sampleTree(rootPid) {
  const table = processTable();
  const byParent = new Map();
  for (const p of table) {
    if (!byParent.has(p.ppid)) byParent.set(p.ppid, []);
    byParent.get(p.ppid).push(p);
  }
  const members = [];
  const stack = [rootPid];
  const seen = new Set();
  while (stack.length) {
    const pid = stack.pop();
    if (seen.has(pid)) continue;
    seen.add(pid);
    const self = table.find((p) => p.pid === pid);
    if (self) members.push(self);
    for (const child of byParent.get(pid) ?? []) stack.push(child.pid);
  }
  const sum = (k, pick = () => true) => members.filter(pick).reduce((n, p) => n + (p[k] || 0), 0);
  const isApp = (p) => p.pid === rootPid;
  return {
    processes: members.length,
    private_mb: mb(sum('private')),
    app_private_mb: mb(sum('private', isApp)),
    webview_private_mb: mb(sum('private', (p) => !isApp(p))),
    working_set_mb: mb(sum('ws')),
    commit_mb: mb(sum('commit')),
    members: members.map((p) => ({ pid: p.pid, name: p.name, private_mb: mb(p.private) })),
  };
}

const mb = (bytes) => Math.round((bytes / (1024 * 1024)) * 10) / 10;

// --- the app ---------------------------------------------------------------

async function startApp(opts, env) {
  const child = spawn(opts.bin, [], { env, stdio: 'ignore', windowsHide: false });
  const t0 = performance.now();
  const deadline = t0 + 60_000;
  let endpoint;
  while (performance.now() < deadline) {
    try {
      const res = await fetch(`http://127.0.0.1:${opts.port}/json/version`);
      if (res.ok) {
        endpoint = await res.json();
        break;
      }
    } catch {
      /* not up yet */
    }
    await sleep(100);
  }
  if (!endpoint) {
    child.kill();
    throw new Error(`the webview did not open its debugging port ${opts.port} within 60 s`);
  }
  const browser = await chromium.connectOverCDP(`http://127.0.0.1:${opts.port}`);
  let page;
  const pageDeadline = performance.now() + 30_000;
  while (!page && performance.now() < pageDeadline) {
    page = browser
      .contexts()
      .flatMap((c) => c.pages())
      .find((p) => /^(tauri:\/\/|https?:\/\/tauri\.localhost)/.test(p.url()));
    if (!page) await sleep(100);
  }
  if (!page) throw new Error('no app page found over CDP');
  return { child, browser, page, t0 };
}

const tree = (page) => page.getByRole('tree', { name: 'Font list' });
const modeGroup = (page) => page.getByRole('group', { name: 'View mode' });

// The app's local API (ADR 0015): `endpoint.json` in the data folder says
// where it is and carries the token.
async function localApi(data) {
  const file = path.join(data, 'endpoint.json');
  const deadline = performance.now() + 30_000;
  while (!fs.existsSync(file) && performance.now() < deadline) await sleep(200);
  const { url, token } = JSON.parse(fs.readFileSync(file, 'utf8'));
  return async (method, body) => {
    const res = await fetch(`${url}/v1/${method}`, {
      method: 'POST',
      headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    if (!res.ok) throw new Error(`${method}: ${res.status} ${await res.text()}`);
    return res.json();
  };
}

// Adds the library as a source unless the scratch index has it already.
// Sources live in the index, so there is no file to seed.
async function ensureSource(api, library) {
  const sources = await api('list_sources');
  const want = path.resolve(library).toLowerCase();
  if (sources.some((s) => path.resolve(s.path).toLowerCase() === want)) return 'in the index';
  const added = await api('add_source', { path: library, mode: { kind: 'indexInPlace' } });
  await api('scan', { source: added.id }); // as the Sources sidebar does after "Add folder"
  return 'added, scanning';
}

async function waitForList(page) {
  const start = performance.now();
  let last = -1;
  let stableSince = performance.now();
  let families = 0;
  while (performance.now() - start < 10 * 60_000) {
    const item = tree(page).getByRole('treeitem').first();
    const size = Number((await item.getAttribute('aria-setsize').catch(() => null)) ?? 0);
    if (size !== last) {
      last = size;
      stableSince = performance.now();
    } else if (size > 0 && performance.now() - stableSince > 8_000) {
      families = size;
      break;
    }
    await sleep(1_000);
  }
  if (!families) throw new Error('the font list never settled');
  return { families, stableAfterMs: Math.round(performance.now() - start - 8_000) };
}

// --- scrolling and frames --------------------------------------------------

const RECORDER = `
(() => {
  const s = (window.__fm = { frames: [], long: [], on: true });
  const loop = (t) => { if (!s.on) return; s.frames.push(t); requestAnimationFrame(loop); };
  requestAnimationFrame(loop);
  try {
    s.obs = new PerformanceObserver((list) => {
      for (const e of list.getEntries()) s.long.push({ start: e.startTime, duration: e.duration, blocking: e.blockingDuration });
    });
    s.obs.observe({ type: 'long-animation-frame', buffered: false });
  } catch {}
})()`;

const STOP = `
(() => {
  const s = window.__fm; s.on = false; s.obs?.disconnect();
  const f = s.frames, d = [];
  for (let i = 1; i < f.length; i++) d.push(f[i] - f[i - 1]);
  return { intervals: d, long: s.long };
})()`;

// The scrollable element the view shows: the tallest scrolling box in the main area.
const SCROLLER = `
(() => {
  const main = document.querySelector('main') ?? document.body;
  let best = null;
  for (const el of main.querySelectorAll('*')) {
    const cs = getComputedStyle(el);
    if (!/(auto|scroll)/.test(cs.overflowY)) continue;
    if (el.scrollHeight <= el.clientHeight + 50) continue;
    if (!best || el.clientHeight > best.clientHeight) best = el;
  }
  if (!best) return null;
  best.scrollTop = 0;
  const r = best.getBoundingClientRect();
  window.__fmScroller = best;
  return { x: r.left + r.width / 2, y: r.top + r.height / 2, height: best.scrollHeight, view: best.clientHeight, label: best.getAttribute('aria-label') ?? best.className };
})()`;

// scrollTop from top to bottom over `ms`, one step per animation frame.
const DRAG = (ms) => `
new Promise((done) => {
  const el = window.__fmScroller, end = el.scrollHeight - el.clientHeight, t0 = performance.now();
  const step = () => {
    const k = Math.min(1, (performance.now() - t0) / ${ms});
    el.scrollTop = Math.round(end * k);
    if (k < 1) requestAnimationFrame(step); else done(el.scrollTop);
  };
  requestAnimationFrame(step);
})`;

async function scrollRun(page, cdp, pace, ms) {
  const box = await page.evaluate(SCROLLER);
  if (!box) throw new Error('no scrollable element in the main area');
  await page.mouse.move(box.x, box.y);
  await sleep(500);
  await page.evaluate(RECORDER);
  let sent = 0;
  if (pace === 'drag') {
    await page.evaluate(DRAG(ms));
  } else {
    const { everyMs, notches } = PACES[pace];
    const end = performance.now() + ms;
    while (performance.now() < end) {
      cdp
        .send('Input.dispatchMouseEvent', {
          type: 'mouseWheel',
          x: box.x,
          y: box.y,
          deltaX: 0,
          deltaY: NOTCH_PX * notches,
        })
        .catch(() => {});
      sent++;
      await sleep(everyMs);
    }
  }
  await sleep(300); // let the last smooth-scroll animation finish
  const { intervals, long } = await page.evaluate(STOP);
  const scrolled = await page.evaluate('window.__fmScroller.scrollTop');
  return {
    ...frameStats(intervals),
    long_frames: long.length,
    long: long.map((l) => ({ start_ms: round(l.start), duration_ms: round(l.duration), blocking_ms: round(l.blocking) })),
    scrolled_px: Math.round(scrolled),
    list_px: box.height,
    wheel_events: sent,
    scroller: box.label,
  };
}

function frameStats(intervals) {
  const sorted = [...intervals].sort((a, b) => a - b);
  const q = (p) => (sorted.length ? sorted[Math.min(sorted.length - 1, Math.floor(p * sorted.length))] : 0);
  const over = intervals.filter((d) => d > FRAME_BUDGET_MS).length;
  const jank = intervals.filter((d) => d > LONG_FRAME_MS).length;
  return {
    frames: intervals.length,
    median_ms: round(q(0.5)),
    p95_ms: round(q(0.95)),
    p99_ms: round(q(0.99)),
    max_ms: round(sorted.at(-1) ?? 0),
    over_budget: over,
    over_budget_pct: intervals.length ? round((100 * over) / intervals.length) : 0,
    jank_frames: jank,
  };
}

const round = (n) => Math.round(n * 10) / 10;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// What the page itself holds: the font faces the document has loaded (each
// row's font comes in through `@font-face`) and the JS heap.
async function pageStats(page) {
  return page.evaluate(`({
    font_faces: document.fonts.size,
    font_faces_loaded: [...document.fonts].filter((f) => f.status === 'loaded').length,
    js_heap_mb: Math.round((performance.memory?.usedJSHeapSize ?? 0) / 1048576),
    dom_nodes: document.getElementsByTagName('*').length,
  })`);
}

async function selectMode(page, name) {
  await modeGroup(page).getByRole('button', { name, exact: true }).click();
  await sleep(1_000);
}

// --- main ------------------------------------------------------------------

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const { env, data } = prepareHome(opts);
  console.log(`webview: starting ${opts.bin}`);
  const { child, browser, page, t0 } = await startApp(opts, env);
  const report = {
    os: osName(),
    date: new Date().toISOString(),
    bin: opts.bin,
    library: opts.library,
    home: opts.home,
    fresh: opts.fresh,
    settle_ms: opts.settleMs,
    scroll_ms: opts.scrollMs,
    display: {},
    memory: {},
    scroll: {},
  };
  const failShot = async (e) => {
    const shot = path.join(path.dirname(opts.out), 'webview-fail.png');
    await page.screenshot({ path: shot }).catch(() => {});
    const text = await page.evaluate('document.body.innerText.slice(0, 600)').catch(() => '');
    throw new Error(`${e.message}\nurl ${page.url()}\nscreenshot ${shot}\npage text: ${text}`);
  };
  try {
    await modeGroup(page).waitFor({ state: 'visible', timeout: 60_000 }).catch(failShot);
    report.window_ms = Math.round(performance.now() - t0);
    const api = await localApi(data);
    const source = await ensureSource(api, opts.library);
    console.log(`webview: window up after ${report.window_ms} ms; library ${source}`);
    await selectMode(page, 'List'); // the app remembers the last view
    await tree(page).waitFor({ state: 'visible', timeout: 300_000 }).catch(failShot);
    report.list_ms = Math.round(performance.now() - t0);
    const list = await waitForList(page);
    report.families = list.families;
    report.faces = (await api('list_sources')).reduce((n, s) => n + (s.faceCount ?? 0), 0);
    report.list_stable_ms = report.list_ms + list.stableAfterMs;
    console.log(`webview: ${report.faces} faces in ${list.families} families; idling ${opts.settleMs} ms`);
    await selectMode(page, 'List');
    await sleep(opts.settleMs);
    report.memory.idle = sampleTree(child.pid);
    report.memory.idle.page = await pageStats(page);
    // The display's refresh interval, from an idle second of frames.
    await page.evaluate(RECORDER);
    await sleep(1_000);
    const idle = await page.evaluate(STOP);
    report.display.frame_interval_ms = frameStats(idle.intervals).median_ms;
    report.display.hz = Math.round(1000 / Math.max(report.display.frame_interval_ms, 1));
    console.log(
      `webview: idle ${report.memory.idle.private_mb} MB private (app ${report.memory.idle.app_private_mb}, webview ${report.memory.idle.webview_private_mb}) over ${report.memory.idle.processes} processes; display ~${report.display.hz} Hz`,
    );

    const cdp = await page.context().newCDPSession(page);
    for (const mode of MODES) {
      await selectMode(page, mode);
      report.scroll[mode] = {};
      for (const pace of Object.keys(PACES)) {
        const r = await scrollRun(page, cdp, pace, opts.scrollMs);
        report.scroll[mode][pace] = r;
        console.log(
          `webview: ${mode.padEnd(9)} ${pace.padEnd(5)} ${String(r.frames).padStart(4)} frames  p95 ${String(r.p95_ms).padStart(6)} ms  max ${String(r.max_ms).padStart(6)} ms  over ${String(r.over_budget_pct).padStart(5)} %  jank ${String(r.jank_frames).padStart(3)}  (${r.scrolled_px} of ${r.list_px} px, "${r.scroller}")`,
        );
      }
      const sample = sampleTree(child.pid);
      sample.page = await pageStats(page);
      report.memory[`after_${mode.toLowerCase()}`] = sample;
      console.log(
        `webview: after ${mode}: ${sample.private_mb} MB private; ${sample.page.font_faces} font faces loaded, JS heap ${sample.page.js_heap_mb} MB`,
      );
    }
    await selectMode(page, 'List');
    await sleep(opts.settleMs);
    report.memory.idle_after = sampleTree(child.pid);
    report.memory.idle_after.page = await pageStats(page);
  } finally {
    await browser.close().catch(() => {});
    killTree(child.pid);
  }
  report.verdict = verdict(report);
  fs.mkdirSync(path.dirname(opts.out), { recursive: true });
  fs.writeFileSync(opts.out, JSON.stringify(report, null, 2));
  printTable(report);
  console.log(`webview: report ${opts.out}`);
  process.exit(report.verdict.pass ? 0 : 1);
}

function verdict(report) {
  const failures = [];
  const idle = report.memory.idle?.private_mb ?? Infinity;
  if (idle >= MEMORY_BUDGET_MB) failures.push(`idle memory ${idle} MB >= ${MEMORY_BUDGET_MB} MB`);
  for (const [mode, paces] of Object.entries(report.scroll)) {
    for (const [pace, r] of Object.entries(paces)) {
      if (r.over_budget_pct > 1) failures.push(`${mode}/${pace}: ${r.over_budget_pct} % of frames over ${FRAME_BUDGET_MS} ms`);
      if (r.jank_frames > 0) failures.push(`${mode}/${pace}: ${r.jank_frames} frame(s) over ${LONG_FRAME_MS} ms`);
    }
  }
  return {
    pass: failures.length === 0,
    failures,
    frame_budget_ms: FRAME_BUDGET_MS,
    long_frame_ms: LONG_FRAME_MS,
    memory_budget_mb: MEMORY_BUDGET_MB,
  };
}

function printTable(report) {
  const m = report.memory;
  console.log('');
  console.log(
    `${report.faces} faces, ${report.families} families  window ${report.window_ms} ms  list ${report.list_ms} ms  stable ${report.list_stable_ms} ms  display ~${report.display.hz} Hz`,
  );
  console.log('');
  console.log('memory, private working set (MB)   total      app  webview  processes');
  for (const [k, s] of Object.entries(m)) {
    console.log(
      `  ${k.padEnd(30)} ${String(s.private_mb).padStart(7)}  ${String(s.app_private_mb).padStart(7)}  ${String(s.webview_private_mb).padStart(7)}  ${String(s.processes).padStart(9)}`,
    );
  }
  console.log(`  budget (idle)                   ${String(MEMORY_BUDGET_MB).padStart(7)}`);
  console.log('');
  console.log('view      pace   frames  median    p95    p99     max   >18ms  >50ms  scrolled');
  for (const [mode, paces] of Object.entries(report.scroll)) {
    for (const [pace, r] of Object.entries(paces)) {
      console.log(
        `${mode.padEnd(9)} ${pace.padEnd(6)} ${String(r.frames).padStart(5)}  ${String(r.median_ms).padStart(6)} ${String(r.p95_ms).padStart(6)} ${String(r.p99_ms).padStart(6)}  ${String(r.max_ms).padStart(6)}  ${String(r.over_budget_pct + ' %').padStart(6)}  ${String(r.jank_frames).padStart(5)}  ${String(r.scrolled_px + ' px').padStart(9)}`,
      );
    }
  }
  console.log('');
  console.log(report.verdict.pass ? 'within budget' : `over budget:\n  ${report.verdict.failures.join('\n  ')}`);
}

function killTree(pid) {
  try {
    if (os.platform() === 'win32') execFileSync('taskkill', ['/PID', String(pid), '/T', '/F'], { stdio: 'ignore' });
    else process.kill(pid, 'SIGTERM');
  } catch {
    /* already gone */
  }
}

main().catch((e) => {
  console.error(`webview: ${e.message}`);
  process.exit(2);
});
