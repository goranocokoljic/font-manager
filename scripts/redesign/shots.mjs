#!/usr/bin/env node
// Screenshots for the redesign loop (dev-docs/redesign/).
//
//   node scripts/redesign/shots.mjs <iteration> [sceneName ...]
//
// Drives the existing `@screens` Playwright spec (apps/desktop/e2e/screens.spec.ts) once per
// scene. That spec already renders every route in light and dark, seeds view preferences and
// mock settings, opens inspector tabs, presses keys and clicks buttons — all through env
// vars — so this file is only a table of scenes plus a runner. Output:
//   .harness/redesign/shots/<iteration>/<scene>-<theme>-<w>x<h>.png
// (.harness/ is git-ignored.) Viewports: 1280x800, 1000x700 (narrow: inspector hidden) and
// 1440x900; set FM_SCREENS_VIEWPORTS=1920x1080,... to choose others.
//
// Scene env keys (see the spec header for the full list):
//   routes      FM_SCREENS_ROUTES      '/'  '/?face=1@ControlOrMeta+k' (press after load)  '/studio?layout=magazine&face=1'
//   tab         FM_SCREENS_TAB         inspector tab name: Info | Axes | Features | Glyphs | Developer
//   click       FM_SCREENS_CLICK       first button whose accessible name starts with this
//   row         FM_SCREENS_ROW         click that list row (1-based) before the route's key is pressed
//   collection  FM_SCREENS_COLLECTION  make a collection of that name and open it
//   storage     FM_SCREENS_STORAGE     localStorage JSON (fm.views.v1 holds the view mode)
//   session     FM_SCREENS_SESSION     sessionStorage JSON (fm.mock.settings.v1 holds mock settings, e.g. density)
//   command     FM_SCREENS_COMMAND     a command-palette action to run before the shot

import { spawnSync } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';

const views = (mode) => JSON.stringify({ 'fm.views.v1': JSON.stringify({ home: { mode } }) });

/** @type {Record<string, Record<string, string>>} */
const scenes = {
  list: { routes: '/?face=1', storage: views('list') },
  table: { routes: '/?face=1', storage: views('table') },
  grid: { routes: '/?face=1', storage: views('grid') },
  waterfall: { routes: '/?face=1', storage: views('waterfall') },
  layout: { routes: '/?face=1', storage: views('layout') },
  'inspector-info': { routes: '/?face=1', tab: 'Info' },
  'inspector-axes': { routes: '/?face=1', tab: 'Axes' },
  'inspector-features': { routes: '/?face=1', tab: 'Features' },
  'inspector-glyphs': { routes: '/?face=1', tab: 'Glyphs' },
  'inspector-developer': { routes: '/?face=1', tab: 'Developer' },
  // Until the filters drawer exists (DESIGN.md item 4) there is no "Filters" button: this
  // is the chip wall as it is. Add `click: 'Filters'` when the drawer lands.
  filters: { routes: '/?face=1' },
  palette: { routes: '/?face=1@ControlOrMeta+k' },
  similar: { routes: '/?face=1', tab: 'Info', click: 'Find similar' },
  studio: { routes: '/studio?layout=magazine&face=1' },
  // ArrowRight expands the family only when the list has focus: click the row first.
  'expanded-family': { routes: '/?face=1@ArrowRight', row: '1' },
  'selected-row': { routes: '/?face=1' },
  // Browser mode starts without collections; the spec makes one and opens it.
  'empty-collection': { routes: '/', collection: 'Empty' },
  compact: {
    routes: '/?face=1',
    session: JSON.stringify({ 'fm.mock.settings.v1': JSON.stringify({ density: 'compact' }) }),
  },
  problems: { routes: '/problems' },
  settings: { routes: '/settings' },
};

const [iteration, ...only] = process.argv.slice(2);
if (!iteration) {
  console.error('usage: node scripts/redesign/shots.mjs <iteration> [scene ...]');
  process.exit(2);
}
const wanted = only.length ? only : Object.keys(scenes);
const unknown = wanted.filter((n) => !scenes[n]);
if (unknown.length) {
  console.error(`unknown scene(s): ${unknown.join(', ')}\nknown: ${Object.keys(scenes).join(', ')}`);
  process.exit(2);
}

const root = resolve(import.meta.dirname, '..', '..');
const outRoot = resolve(root, '.harness', 'redesign', 'shots', String(iteration));
mkdirSync(outRoot, { recursive: true });

const envKey = {
  routes: 'FM_SCREENS_ROUTES',
  tab: 'FM_SCREENS_TAB',
  click: 'FM_SCREENS_CLICK',
  row: 'FM_SCREENS_ROW',
  collection: 'FM_SCREENS_COLLECTION',
  storage: 'FM_SCREENS_STORAGE',
  session: 'FM_SCREENS_SESSION',
  command: 'FM_SCREENS_COMMAND',
  query: 'FM_SCREENS_QUERY',
};

const viewports = process.env.FM_SCREENS_VIEWPORTS || '1280x800,1000x700,1440x900';

let failed = 0;
for (const name of wanted) {
  // One flat folder per iteration, files named after the scene (FM_SCREENS_PREFIX), so a
  // before/after pair is the same file name in two iteration folders.
  const env = {
    ...process.env,
    FM_SCREENS_DIR: outRoot,
    FM_SCREENS_PREFIX: name,
    FM_SCREENS_VIEWPORTS: viewports,
  };
  for (const [k, v] of Object.entries(scenes[name])) env[envKey[k]] = v;
  process.stdout.write(`\n=== scene ${name} → ${outRoot}/${name}-*.png\n`);
  const r = spawnSync(
    'pnpm',
    ['--filter', '@fm/desktop', 'exec', 'playwright', 'test', '--grep', '@screens'],
    { cwd: root, env, stdio: 'inherit', shell: process.platform === 'win32' },
  );
  if (r.status !== 0) failed++;
}

console.log(`\nshots written under ${outRoot}${failed ? ` — ${failed} scene(s) failed` : ''}`);
process.exit(failed ? 1 : 0);
