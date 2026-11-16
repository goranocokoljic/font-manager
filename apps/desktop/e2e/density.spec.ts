import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// E51.5: the compact density left the interface. Every list has the comfortable rows, and a
// `density: compact` saved before this story is read and ignored (fm-api keeps the field).

// Preview rows load face files from `/fmfont/<id>`; serve one fixture for all.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

const firstRow = (page: Page) =>
  page.getByRole('tree', { name: 'Font list' }).getByRole('treeitem').first();
const rowHeight = async (page: Page) => (await firstRow(page).boundingBox())?.height;

test.beforeEach(async ({ page }) => {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  // The mock's saved settings, as an older version of the app left them.
  await page.addInitScript(() =>
    sessionStorage.setItem('fm.mock.settings.v1', JSON.stringify({ density: 'compact' })),
  );
  await page.goto('/#/');
  await expect(firstRow(page)).toBeVisible();
});

test('a saved compact density gives comfortable rows in the list and the table', async ({
  page,
}) => {
  await expect.poll(() => rowHeight(page)).toBe(64);
  await expect(page.locator('html')).not.toHaveAttribute('data-density', /.*/);

  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Table' })
    .click();
  const tableRow = page
    .getByRole('treegrid', { name: 'Font table' })
    .locator('.spacer [role="row"]');
  await expect.poll(async () => (await tableRow.first().boundingBox())?.height).toBe(32);
});

test('no toolbar button, palette action or setting offers a density', async ({ page }) => {
  await expect(page.getByRole('button', { name: 'Compact rows' })).toHaveCount(0);

  await page.keyboard.press('ControlOrMeta+k');
  const palette = page.getByRole('dialog', { name: 'Command palette' });
  await expect(palette).toBeVisible();
  await page.keyboard.type('rows');
  await expect(palette.getByRole('option').filter({ hasText: /compact rows/i })).toHaveCount(0);
  await expect(palette.getByRole('option').filter({ hasText: /comfortable rows/i })).toHaveCount(0);
  await page.keyboard.press('Escape');

  await page.goto('/#/settings?section=appearance');
  await expect(page.getByRole('radio', { name: 'Dark' })).toBeVisible();
  await expect(page.getByRole('group', { name: 'Density' })).toHaveCount(0);
  await expect(page.getByRole('radio', { name: 'Compact' })).toHaveCount(0);
});
