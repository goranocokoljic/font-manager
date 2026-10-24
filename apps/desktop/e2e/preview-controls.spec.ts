import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/#/');
  await expect(page.locator('[data-face="1"]')).toHaveAttribute('data-state', 'ready');
}

test('preview controls apply to all rows live and persist after a reload', async ({ page }) => {
  await open(page);
  const controls = page.getByRole('group', { name: 'Preview settings' });
  const samples = page.locator('[role="treeitem"] .sample');
  const count = await samples.count();
  expect(count).toBeGreaterThan(1);

  await controls.getByLabel('Sample').selectOption('pangram');
  await controls.getByLabel('Language').selectOption('de');
  await controls.getByLabel('Size in pixels').fill('72');
  await controls.getByLabel('Line height').fill('1.5');
  await controls.getByLabel('Tracking').fill('0.1');
  await controls.getByRole('button', { name: 'Center' }).click();
  await controls.getByLabel('Text color').fill('#aa0000');
  await controls.getByLabel('Background color').fill('#ffeecc');
  await controls.getByLabel('Surface').selectOption('dark');

  for (let i = 0; i < count; i += 1) {
    const sample = samples.nth(i);
    await expect(sample).toHaveText(/Victor jagt zwölf Boxkämpfer/);
    await expect(sample).toHaveCSS('font-size', '72px');
    await expect(sample).toHaveCSS('line-height', '108px');
    await expect(sample).toHaveCSS('letter-spacing', '7.2px');
    await expect(sample).toHaveCSS('text-align', 'center');
    await expect(sample).toHaveCSS('color', 'rgb(170, 0, 0)');
  }
  const list = page.getByRole('tree', { name: 'Font list' });
  await expect(list).toHaveCSS('background-color', 'rgb(255, 238, 204)');
  await expect(list).toHaveAttribute('data-surface', 'dark');

  await page.reload();
  await expect(page.locator('[data-face="1"] .sample')).toHaveCSS('font-size', '72px');
  await expect(page.locator('[data-face="1"] .sample')).toHaveText(/Victor jagt/);
  await expect(controls.getByLabel('Surface')).toHaveValue('dark');

  await controls.getByRole('button', { name: 'Reset' }).click();
  await expect(page.locator('[data-face="1"] .sample')).toHaveCSS('font-size', '28px');
});

test('the font-name and custom samples show per-face text', async ({ page }) => {
  await open(page);
  const controls = page.getByRole('group', { name: 'Preview settings' });
  await controls.getByLabel('Sample').selectOption('fontName');
  await expect(page.locator('[data-face="1"] .sample')).toHaveText('Inter Regular');
  await expect(page.locator('[data-face="6"] .sample')).toHaveText('JetBrains Mono Regular');
  await controls.getByLabel('Sample').selectOption('custom');
  await controls.getByLabel('Custom text').fill('Hamburgefonstiv');
  await expect(page.locator('[data-face="6"] .sample')).toHaveText('Hamburgefonstiv');
});

test('auto size scales each row by its x-height or cap height, by keyboard, and persists', async ({
  page,
}) => {
  await open(page);
  const controls = page.getByRole('group', { name: 'Preview settings' });
  const autoSize = controls.getByLabel('Auto size');
  const size = (face: number) =>
    page
      .locator(`[data-face="${face}"] .sample`)
      .evaluate((el) => parseFloat(getComputedStyle(el).fontSize));

  await expect(autoSize).toHaveValue('off');
  // Keyboard-reachable: Tab from the size field lands on it (line height comes after).
  await controls.getByLabel('Size in pixels').focus();
  await page.keyboard.press('Tab');
  await expect(autoSize).toBeFocused();

  await autoSize.selectOption('xHeight');
  // Mock faces 1 and 6 have x-heights of 520 and 550 per 1000; face 7 declares none.
  await expect.poll(() => size(1)).toBeCloseTo((28 * 0.5) / 0.52, 1);
  await expect.poll(() => size(6)).toBeCloseTo((28 * 0.5) / 0.55, 1);
  expect(await size(7)).toBe(28);
  // Every scaled face has the same x-height in pixels.
  expect(Math.abs((await size(1)) * 0.52 - (await size(6)) * 0.55)).toBeLessThan(1);

  await autoSize.selectOption('capHeight');
  await expect.poll(() => size(1)).toBeCloseTo((28 * 0.7) / 0.72, 1);

  await page.reload();
  await expect(controls.getByLabel('Auto size')).toHaveValue('capHeight');
  await expect.poll(() => size(1)).toBeCloseTo((28 * 0.7) / 0.72, 1);

  await controls.getByLabel('Auto size').selectOption('off');
  await expect.poll(() => size(1)).toBe(28);
});

test('the pangram picker lists the API languages and shows Serbian (Cyrillic)', async ({
  page,
}) => {
  await open(page);
  const controls = page.getByRole('group', { name: 'Preview settings' });
  await controls.getByLabel('Sample').selectOption('pangram');
  const language = controls.getByLabel('Language');
  await expect(language).toHaveValue('en');
  await expect(language.locator('option', { hasText: 'Serbian (Cyrillic)' })).toHaveCount(1);
  // A preset without a pangram is not offered.
  await expect(language.locator('option', { hasText: 'Maltese' })).toHaveCount(0);
  await language.selectOption({ label: 'Serbian (Cyrillic)' });
  await expect(page.locator('[data-face="1"] .sample')).toHaveText(
    'Љубазни фењерџија чађавог лица хоће да ми покаже штос',
  );
  await page.reload();
  await expect(controls.getByLabel('Language')).toHaveValue('sr-Cyrl');
  await expect(page.locator('[data-face="1"] .sample')).toHaveText(/фењерџија/);
});
