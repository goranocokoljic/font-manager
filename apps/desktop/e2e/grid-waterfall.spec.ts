import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// Preview rows load face files from `/fmfont/<id>`; serve one fixture for all.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page, query = '') {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(`/${query}#/`);
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
}

const modeButton = (page: Page, name: string) =>
  page.getByRole('group', { name: 'View mode' }).getByRole('button', { name });

test('grid mode shows a card with a rendered thumbnail per family', async ({ page }) => {
  await open(page);
  await modeButton(page, 'Grid').click();
  const grid = page.getByRole('listbox', { name: 'Font grid' });
  const inter = grid.getByRole('option', { name: 'Inter, 1 style, not active' });
  await expect(inter).toBeVisible();
  await expect(inter).toContainText('Inter');
  const thumb = inter.getByTestId('thumbnail');
  await expect(thumb).toHaveAttribute('data-state', 'ready');
  const mask = await thumb.evaluate(
    (el) => getComputedStyle(el).maskImage || getComputedStyle(el).webkitMaskImage,
  );
  expect(mask).toMatch(/^url\("blob:/);
  // The mode is a saved view preference.
  await page.reload();
  await expect(page.getByRole('listbox', { name: 'Font grid' })).toBeVisible();
});

test('waterfall mode shows the sample text at six sizes per family', async ({ page }) => {
  await open(page);
  // Key 4 is the waterfall (keys 1–4: list, table, grid, waterfall).
  await page.keyboard.press('4');
  await expect(modeButton(page, 'Waterfall')).toHaveAttribute('aria-pressed', 'true');
  const fall = page.getByRole('tree', { name: 'Font waterfall' });
  const block = fall.getByRole('treeitem', { name: 'Inter, 1 style, not active' });
  const lines = block.getByTestId('waterfall-line');
  await expect(lines).toHaveCount(6);
  const sizes = await lines.evaluateAll((ls) =>
    ls.map((l) => getComputedStyle(l.querySelector('.sample')!).fontSize),
  );
  expect(sizes).toEqual(['12px', '16px', '24px', '32px', '48px', '72px']);
  await expect(lines.first().locator('.sample')).toHaveText(
    'The quick brown fox jumps over the lazy dog',
  );
});

test('switching views keeps the selection and the family at the top', async ({ page }) => {
  await open(page, '?mockFaces=2000');
  const tree = page.getByRole('tree', { name: 'Font list' });
  // Scroll to the top of row 300, so one row starts at the top.
  await tree.evaluate((el) => {
    const row = el.querySelector<HTMLElement>('[role="treeitem"]')!;
    el.scrollTop = 300 * row.offsetHeight;
  });
  // The rows whose top is the first in the viewport (a line of cards in the grid).
  const topLine = (viewport: ReturnType<Page['getByRole']>) =>
    viewport.evaluate((el) => {
      const box = el.getBoundingClientRect();
      // The table's sticky header covers the top of its viewport.
      const head = el.querySelector('.header')?.getBoundingClientRect().height ?? 0;
      const items = [...el.querySelectorAll<HTMLElement>('[data-key]')]
        .map((i) => ({ key: i.dataset.key!, y: i.getBoundingClientRect().top - box.top }))
        .filter((i) => i.y >= head - 1);
      const first = Math.min(...items.map((i) => i.y));
      return items.filter((i) => i.y < first + 1).map((i) => i.key);
    });
  await expect.poll(async () => (await topLine(tree)).length).toBe(1);
  const [anchor] = await topLine(tree);
  expect(anchor).toMatch(/^family:/);
  await tree.locator(`[data-key="${anchor}"]`).click();
  await page.keyboard.press('Shift+ArrowDown');
  const selected = (viewport: ReturnType<Page['getByRole']>) =>
    viewport.locator('[data-key][aria-selected="true"]');
  await expect(selected(tree)).toHaveCount(2);
  const keys = await selected(tree).evaluateAll((els) =>
    els.map((e) => (e as HTMLElement).dataset.key).sort(),
  );

  const views = [
    ['Grid', page.getByRole('listbox', { name: 'Font grid' })],
    ['Waterfall', page.getByRole('tree', { name: 'Font waterfall' })],
    ['Table', page.getByRole('treegrid', { name: 'Font table' })],
    ['List', tree],
  ] as const;
  for (const [mode, viewport] of views) {
    await modeButton(page, mode).click();
    await expect(viewport).toBeVisible();
    await expect.poll(() => topLine(viewport), { message: mode }).toContain(anchor);
    await expect(selected(viewport), mode).toHaveCount(2);
    const now = await selected(viewport).evaluateAll((els) =>
      els.map((e) => (e as HTMLElement).dataset.key).sort(),
    );
    expect(now, mode).toEqual(keys);
  }
});
