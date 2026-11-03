import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { WALL_FAMILIES } from '../src/api/mockWall';

const fixturePath = (path: string) =>
  fileURLToPath(new URL(`../../../fixtures/fonts/${path}`, import.meta.url));

/** fixtures/fonts/manifest.tsv: path → license (SPDX). */
const manifest = new Map(
  readFileSync(fixturePath('manifest.tsv'), 'utf8')
    .trim()
    .split(/\r?\n/)
    .slice(1)
    .map((line) => line.split('\t'))
    .map((cols) => [cols[4]!, cols[2]!] as const),
);

/** The axis tags of a TrueType file's `fvar` table. */
function fvarTags(path: string): string[] {
  const b = readFileSync(fixturePath(path));
  const tables = b.readUInt16BE(4);
  for (let i = 0; i < tables; i++) {
    const o = 12 + i * 16;
    if (b.toString('latin1', o, o + 4) !== 'fvar') continue;
    const fvar = b.readUInt32BE(o + 8);
    const first = fvar + b.readUInt16BE(fvar + 4);
    const count = b.readUInt16BE(fvar + 8);
    const size = b.readUInt16BE(fvar + 10);
    return Array.from({ length: count }, (_, k) =>
      b.toString('latin1', first + k * size, first + k * size + 4),
    );
  }
  return [];
}

// Preview rows load face files from `/fmfont/<id>`; serve one fixture for all.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page, query: string) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(`/${query}#/`);
  const tree = page.getByRole('tree', { name: 'Font list' });
  await expect(tree.getByRole('treeitem').first()).toBeVisible();
  return tree;
}

const names = (page: Page) =>
  page
    .getByRole('tree', { name: 'Font list' })
    .locator('[role="treeitem"][aria-level="1"]')
    .evaluateAll((rows) => rows.map((r) => r.getAttribute('aria-label')?.split(',')[0]));

test('?mockLibrary=wall serves the 60-family wall (E40.2)', async ({ page }) => {
  await open(page, '?mockLibrary=wall');
  // The rows shots.mjs relies on: row 1 Asap Sharp (face 1), row 2 Crimson Pro (six
  // styles); then one family of each class in turn.
  expect((await names(page)).slice(0, 7)).toEqual([
    'Asap Sharp',
    'Crimson Pro',
    'Archivo',
    'Roboto Slab',
    'Clicker Script',
    'JetBrains Mono',
    'Bebas Neue',
  ]);
  await page.getByRole('searchbox', { name: 'Search fonts' }).fill('Zeyada');
  await expect.poll(() => names(page)).toEqual(['Zeyada']);
});

test('?mockLibrary=small keeps the three sample families', async ({ page }) => {
  await open(page, '?mockLibrary=small');
  await expect.poll(() => names(page)).toEqual(['Inter', 'JetBrains Mono', 'Legacy Grotesk']);
});

test('every wall family renders from an OFL, Apache or UFL fixture of its own license', () => {
  const spdx = { ofl: 'OFL-1.1', apache: 'Apache-2.0', ufl: 'UFL-1.0' } as const;
  for (const f of WALL_FAMILIES) expect(manifest.get(f.file), f.file).toBe(spdx[f.license]);
});

test('every axis a wall family lists is in its fixture, so its sliders move the font', () => {
  for (const f of WALL_FAMILIES.filter((x) => x.axes?.length)) {
    const tags = fvarTags(f.file);
    for (const a of f.axes!) expect(tags, `${f.name} ${a.tag}`).toContain(a.tag);
  }
});
