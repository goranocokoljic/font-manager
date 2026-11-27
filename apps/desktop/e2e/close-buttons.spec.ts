import { expect, test, type Page } from '@playwright/test';

// E57.8: × closes. The Problems and Conflicts views close with the icon button at the end
// of their top bar: it is named for what it closes, its tooltip says that and Esc, and it
// goes home like Esc does. No top bar says "Close" in words.

async function checkClose(page: Page, route: string, title: string, name: string) {
  await page.goto(`/#/${route}`);
  await expect(page.getByRole('heading', { name: title, level: 1 })).toBeFocused();
  const bar = page.locator('[data-top-bar]');
  await expect(bar.getByRole('button', { name: 'Close', exact: true })).toHaveCount(0);
  const close = bar.getByRole('button', { name });
  await expect(close).toHaveAttribute('aria-keyshortcuts', 'Escape');
  // At the end of the bar, on the title's line.
  const box = (await close.boundingBox())!;
  const barBox = (await bar.boundingBox())!;
  expect(barBox.x + barBox.width - (box.x + box.width)).toBeLessThan(24);
  expect(box.y).toBeGreaterThanOrEqual(barBox.y);
  expect(box.y + box.height).toBeLessThanOrEqual(barBox.y + barBox.height);
  await close.hover();
  await expect(page.getByRole('tooltip')).toContainText(name);
  await expect(page.getByRole('tooltip')).toContainText('Esc');
  await close.click();
  await expect(page.getByRole('heading', { name: 'All fonts' })).toBeVisible();

  // Esc as before.
  await page.goto(`/#/${route}`);
  await expect(page.getByRole('heading', { name: title, level: 1 })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('heading', { name: 'All fonts' })).toBeVisible();
}

test('the Problems view closes with the × at the end of its top bar', async ({ page }) => {
  await checkClose(page, 'problems', 'Problems', 'Close problems');
});

test('the Conflicts view closes with the × at the end of its top bar', async ({ page }) => {
  await checkClose(page, 'conflicts', 'Font conflicts', 'Close font conflicts');
});

test('the × names read in German', async ({ page }) => {
  await page.addInitScript(() => {
    sessionStorage.setItem('fm.mock.settings.v1', JSON.stringify({ language: 'de' }));
  });
  await page.goto('/#/problems');
  await expect(page.getByRole('button', { name: 'Probleme schließen' })).toBeVisible();
  await page.goto('/#/conflicts');
  await expect(page.getByRole('button', { name: 'Schriftkonflikte schließen' })).toBeVisible();
});
