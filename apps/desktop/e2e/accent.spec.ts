import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { showSection } from './family-page';

// The accent per theme (E40.3), as the page really paints it: rings and fills take the
// accent, words on a fill take on-accent. styles/contrast.test.ts checks the ratios.

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page) {
  await page.route('**/fmfont/*', (r) =>
    r.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/#/');
  await expect(page.getByRole('treeitem').first()).toBeVisible();
}

const css = (el: Locator, property: string) =>
  el.evaluate((node, p) => getComputedStyle(node).getPropertyValue(p), property);

for (const [colorScheme, accent, onAccent] of [
  ['light', 'rgb(197, 110, 178)', 'rgb(20, 20, 19)'],
  ['dark', 'rgb(171, 75, 150)', 'rgb(255, 255, 255)'],
] as const) {
  test.describe(`the ${colorScheme} theme`, () => {
    test.beforeEach(async ({ page }) => {
      await page.emulateMedia({ colorScheme });
      await open(page);
    });

    test('the active segment is an accent fill with on-accent text', async ({ page }) => {
      const modes = page.getByRole('group', { name: 'View mode' });
      const segment = modes.locator('[aria-pressed=true]');
      // The view switch slides (E46.2): its fill is the thumb, which lies under the segment.
      const thumb = modes.locator('.thumb');
      await expect(thumb).toHaveCSS('background-color', accent);
      await expect
        .poll(async () => {
          const [t, b] = await Promise.all([thumb.boundingBox(), segment.boundingBox()]);
          return [Math.round(t!.x - b!.x), Math.round(t!.width - b!.width)];
        })
        .toEqual([0, 0]);
      await expect(segment).toHaveCSS('color', onAccent);
    });

    test('the focus ring takes the accent', async ({ page }) => {
      await page.keyboard.press('Tab');
      const focused = page.locator(':focus');
      await expect(focused).toHaveCount(1);
      expect(await css(focused, 'outline-color')).toBe(accent);
    });

    test('the activation toggle is an accent fill with on-accent text', async ({ page }) => {
      await page.goto('/#/family/1?face=1');
      const info = await showSection(page, 'Info');
      const toggle = info.locator('.activate');
      await expect(toggle).toBeVisible();
      await expect(toggle).toHaveAttribute('aria-pressed', 'false');
      await expect(toggle).toHaveCSS('background-color', accent);
      await expect(toggle).toHaveCSS('color', onAccent);
    });

    test('the focus ring of a filled button sits outside the fill', async ({ page }) => {
      await page.keyboard.press('ControlOrMeta+k');
      await page.keyboard.type("Check a document's fonts");
      await page.keyboard.press('Enter');
      const activate = page
        .getByRole('region', { name: 'Document fonts' })
        .getByRole('button', { name: /^Activate / });
      await expect(activate).toHaveCSS('background-color', accent);
      await activate.focus();
      await expect(activate).toBeFocused();
      expect(await css(activate, 'outline-color')).toBe(accent);
      expect(parseFloat(await css(activate, 'outline-offset'))).toBeGreaterThanOrEqual(0);
    });

    test('a toast action is an accent fill with on-accent text', async ({ page }) => {
      await page.getByRole('treeitem').first().click();
      await page.keyboard.press('a');
      const action = page.getByTestId('toast').getByRole('button', { name: 'Undo' });
      await expect(action).toBeVisible();
      await expect(action).toHaveCSS('background-color', accent);
      await expect(action).toHaveCSS('color', onAccent);
    });
  });
}
