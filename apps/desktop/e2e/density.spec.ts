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

const compactRows = (page: Page) => page.getByRole('button', { name: 'Compact rows' });
const firstRow = (page: Page) =>
  page.getByRole('tree', { name: 'Font list' }).getByRole('treeitem').first();
const rowHeight = async (page: Page) => (await firstRow(page).boundingBox())?.height;

test.beforeEach(async ({ page }) => {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/#/');
  await expect(firstRow(page)).toBeVisible();
});

test('the density switch changes the row height and survives a reload', async ({ page }) => {
  // Comfortable by default: the switch is off.
  await expect(compactRows(page)).toHaveAttribute('aria-pressed', 'false');
  await expect(compactRows(page)).toHaveAttribute('title', /Rows: Comfortable/);
  await expect.poll(() => rowHeight(page)).toBe(64);
  const sample = firstRow(page).locator('.sample');
  const size = await sample.evaluate((el) => getComputedStyle(el).fontSize);

  // By keyboard: Tab from the last view mode button reaches it, Enter turns it on.
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Layout' })
    .focus();
  await page.keyboard.press('Tab');
  await expect(compactRows(page)).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(compactRows(page)).toHaveAttribute('aria-pressed', 'true');
  await expect(compactRows(page)).toHaveAttribute('title', /Rows: Compact/);
  await expect.poll(() => rowHeight(page)).toBe(54);
  // The sample keeps the size the user chose.
  await expect(sample).toHaveCSS('font-size', size);

  await page.reload();
  await expect(firstRow(page)).toBeVisible();
  await expect(compactRows(page)).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('html')).toHaveAttribute('data-density', 'compact');
  await expect.poll(() => rowHeight(page)).toBe(54);

  // The table follows the same density.
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Table' })
    .click();
  const tableRow = page
    .getByRole('treegrid', { name: 'Font table' })
    .locator('.spacer [role="row"]');
  await expect.poll(async () => (await tableRow.first().boundingBox())?.height).toBe(24);

  // The Settings view shows the same choice.
  await page.goto('/#/settings?section=appearance');
  await expect(page.getByRole('radio', { name: 'Compact' })).toBeChecked();
});

test('the command palette switches the density, in step with Settings', async ({ page }) => {
  await page.keyboard.press('ControlOrMeta+k');
  const palette = page.getByRole('dialog', { name: 'Command palette' });
  await expect(palette).toBeVisible();
  await page.keyboard.type('use compact rows');
  await expect(palette.getByRole('option').first()).toContainText('Use compact rows');
  await page.keyboard.press('Enter');
  await expect(palette).toBeHidden();
  await expect.poll(() => rowHeight(page)).toBe(54);
  await expect(compactRows(page)).toHaveAttribute('aria-pressed', 'true');

  // Settings changes it back; the toolbar follows.
  await page.goto('/#/settings?section=appearance');
  await page.getByRole('radio', { name: 'Comfortable' }).check();
  await page.getByRole('button', { name: 'Close' }).click();
  await expect(compactRows(page)).toHaveAttribute('aria-pressed', 'false');
  await expect.poll(() => rowHeight(page)).toBe(64);
});
