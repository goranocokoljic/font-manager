// E58.4 TEST BUILD ONLY (never merged): the checks of criterion 3 in the real app window on
// Windows. Starts target/debug/fm-app.exe (built from the E58.4-test-build branch) once per
// interface size and way, with a scratch home (its own APPDATA, LOCALAPPDATA, USERPROFILE,
// USERNAME and WebView2 profile, so the user's library and app are never touched). The app runs
// checker.js in its window (FM_TEST_CHECKS) and writes a report (FM_TEST_REPORT), then quits.
// Usage: node apps/desktop/e2e-window/interface-size.mjs   (FM_MODES=none:1,css:1.25 to pick)
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { inflateSync } from 'node:zlib';

const ROOT = resolve(import.meta.dirname, '..', '..', '..');
const EXE = join(ROOT, 'target', 'debug', 'fm-app.exe');
const FIXTURES = join(ROOT, 'fixtures', 'fonts');
const MODES = process.env.FM_MODES ? process.env.FM_MODES.split(',') : ['none:1', 'webview:1.25', 'webview:1.5', 'css:1.25', 'css:1.5'];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Page boxes and text sizes of a PDF: every MediaBox, every `Tf` size and `Tm` scale. */
function pdfFacts(buf) {
  const text = buf.toString('latin1');
  const boxes = [...text.matchAll(/\/MediaBox\s*\[([^\]]+)\]/g)].map((m) => m[1].trim().split(/\s+/).map((n) => Number(n).toFixed(2)).join(' '));
  const sizes = {};
  const re = /stream\r?\n/g;
  let m;
  while ((m = re.exec(text))) {
    const start = m.index + m[0].length;
    const end = text.indexOf('endstream', start);
    let body;
    try {
      body = inflateSync(buf.subarray(start, end)).toString('latin1');
    } catch {
      continue;
    }
    const add = (k) => (sizes[k] = (sizes[k] ?? 0) + 1);
    for (const t of body.matchAll(/([-\d.]+)\s+Tf\b/g)) add(`Tf ${Number(t[1]).toFixed(2)}`);
    for (const t of body.matchAll(/([-\d.]+)\s+([-\d.]+)\s+([-\d.]+)\s+([-\d.]+)\s+[-\d.]+\s+[-\d.]+\s+Tm\b/g)) {
      add(`Tm ${Math.hypot(Number(t[1]), Number(t[2])).toFixed(2)}`);
    }
  }
  return { pages: boxes.length, boxes: [...new Set(boxes)], sizes };
}

function run(mode) {
  const home = mkdtempSync(join(tmpdir(), 'fm-e58-'));
  const env = { ...process.env, USERNAME: 'fm-e58-probe' };
  for (const [k, d] of Object.entries({ APPDATA: 'roaming', LOCALAPPDATA: 'local', USERPROFILE: 'profile', WEBVIEW2_USER_DATA_FOLDER: 'webview' })) {
    env[k] = join(home, d);
    mkdirSync(env[k], { recursive: true });
  }
  delete env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS;
  if (mode.startsWith('none')) delete env.FM_TEST_INTERFACE;
  else env.FM_TEST_INTERFACE = mode;
  const checker = join(home, 'checker.js');
  writeFileSync(checker, `window.__fmFixtures=${JSON.stringify(FIXTURES)};\n` + readFileSync(join(import.meta.dirname, 'checker.js'), 'utf8'));
  env.FM_TEST_CHECKS = checker;
  env.FM_TEST_REPORT = join(home, 'report.json');
  return new Promise((done) => {
    const app = spawn(EXE, [], { env, stdio: 'ignore' });
    const timer = setTimeout(() => app.kill(), 240000);
    app.on('exit', async () => {
      clearTimeout(timer);
      await sleep(1000);
      const report = existsSync(env.FM_TEST_REPORT) ? JSON.parse(readFileSync(env.FM_TEST_REPORT, 'utf8')) : { mode, errors: ['no report: the app quit or timed out'] };
      rmSync(home, { recursive: true, force: true, maxRetries: 5, retryDelay: 500 });
      done({ run: mode, ...report });
    });
  });
}

const report = [];
for (const mode of MODES) {
  const r = await run(mode);
  if (r.pdf?.base64) {
    const buf = Buffer.from(r.pdf.base64, 'base64');
    r.pdf = { paper: r.pdf.paper, bytes: buf.length, ...pdfFacts(buf) };
  }
  report.push(r);
  const ok = (k) => r[k]?.ok ?? `missing`;
  console.log(JSON.stringify({ run: mode, input: r.input?.scale, tooltip: ok('tooltip'), popover: ok('popover'), textFieldMenu: ok('textFieldMenu'), list: ok('list'), dragGhost: ok('dragGhost'), pdfPages: r.pdf?.pages, errors: r.errors }));
}
const base = report.find((r) => r.run === 'none:1');
for (const r of report) {
  if (r.pdf && base?.pdf) r.pdfSameAsBase = JSON.stringify([r.pdf.boxes, r.pdf.sizes]) === JSON.stringify([base.pdf.boxes, base.pdf.sizes]);
}
const outFile = join(ROOT, 'target', 'e58-4-window-checks.json');
writeFileSync(outFile, JSON.stringify(report, null, 2));
console.log(`report: ${outFile}`);
for (const r of report) console.log(r.run, `pdfSameAsBase=${r.pdfSameAsBase}`, r.pdf ? JSON.stringify({ boxes: r.pdf.boxes, sizes: r.pdf.sizes }) : '');
