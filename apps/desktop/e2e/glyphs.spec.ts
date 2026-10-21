import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// A fixture with Cyrillic, so the webview draws the cells from a real font.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-static/robotoslab/RobotoSlab-ExtraBold.ttf',
      import.meta.url,
    ),
  ),
);

// MockApi face 1 (Inter Regular); `?mockCjk=1` gives it 32,164 more characters.
async function openGlyphs(page: Page, { cjk = true } = {}) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(`/${cjk ? '?mockCjk=1' : ''}#/?face=1`);
  await page.getByRole('tab', { name: 'Glyphs' }).click();
  const panel = page.getByRole('tabpanel', { name: 'Glyphs' });
  await expect(panel.getByRole('gridcell').first()).toBeVisible();
  return panel;
}

test('the Glyphs tab draws the face under block headings, virtualized', async ({ page }) => {
  const panel = await openGlyphs(page);
  await expect(panel.getByRole('rowheader').first()).toContainText('Basic Latin');
  await expect(panel).toContainText('32,543 characters');
  // 30k+ characters, a few dozen cells in the DOM.
  const cells = await panel.getByRole('gridcell').count();
  expect(cells).toBeGreaterThan(10);
  expect(cells).toBeLessThan(200);
  // The cells use the face's own font, loaded through the font URL.
  const a = panel.getByRole('gridcell', { name: 'U+0041 A', exact: true });
  await expect(a.locator('.char')).toHaveCSS('font-family', /fm-face-1/);
  // Scrolling to the end renders the last rows, still not every cell.
  const grid = panel.getByRole('grid', { name: 'Glyphs' });
  await grid.evaluate((el) => (el.scrollTop = el.scrollHeight));
  await expect(panel.getByRole('gridcell', { name: /^U\+D7A3 / })).toBeVisible();
  expect(await panel.getByRole('gridcell').count()).toBeLessThan(200);
});

test('the block picker lists one block', async ({ page }) => {
  const panel = await openGlyphs(page);
  await panel.getByRole('combobox', { name: 'Block' }).selectOption('Cyrillic');
  await expect(panel.getByRole('rowheader')).toHaveCount(1);
  await expect(panel.getByRole('rowheader')).toContainText('Cyrillic');
  await expect(panel.getByRole('gridcell', { name: 'U+0416 uni0416' })).toBeVisible();
});

test('search by code point scrolls to and selects the glyph', async ({ page }) => {
  const panel = await openGlyphs(page);
  await panel.getByRole('searchbox', { name: 'Search glyphs' }).fill('U+AC01');
  const cell = panel.getByRole('gridcell', { name: /^U\+AC01 / });
  await expect(cell).toBeInViewport();
  await expect(cell).toHaveAttribute('aria-selected', 'true');
  await expect(panel.getByRole('region', { name: 'Selected glyph' })).toContainText('U+AC01');
  await expect(panel.getByRole('region', { name: 'Selected glyph' })).toContainText(
    'Hangul Syllables',
  );
});

test('arrow keys move between cells, Enter and a click select', async ({ page }) => {
  const panel = await openGlyphs(page, { cjk: false });
  const grid = panel.getByRole('grid', { name: 'Glyphs' });
  await grid.focus();
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('ArrowRight');
  await expect(grid).toHaveAttribute('aria-activedescendant', 'fm-glyph-1');
  await page.keyboard.press('Enter');
  const excl = panel.getByRole('gridcell', { name: 'U+0021 uni0021' });
  await expect(excl).toHaveAttribute('aria-selected', 'true');
  // Every key-reached cell is labelled with code point and name.
  await page.keyboard.press('End');
  const last = await grid.getAttribute('aria-activedescendant');
  await expect(page.locator(`#${last}`)).toHaveAttribute('aria-label', /^U\+[0-9A-F]{4,6} \S/);
  await page.keyboard.press('Home');
  await panel.getByRole('gridcell', { name: 'U+0041 A', exact: true }).click();
  await expect(panel.getByRole('gridcell', { name: 'U+0041 A', exact: true })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await expect(excl).toHaveAttribute('aria-selected', 'false');
});

test('a glyph selected by keyboard opens its detail with code point, advance and features', async ({
  page,
}) => {
  const panel = await openGlyphs(page, { cjk: false });
  await panel.getByRole('searchbox', { name: 'Search glyphs' }).fill('U+0061');
  const grid = panel.getByRole('grid', { name: 'Glyphs' });
  await grid.focus();
  await page.keyboard.press('Enter');
  const detail = panel.getByRole('region', { name: 'Selected glyph' });
  await expect(detail.getByLabel('Code point', { exact: true })).toHaveText('U+0061');
  await expect(detail.getByLabel('Advance width', { exact: true })).toHaveText(/^\d[\d,]* units$/);
  await expect(detail.getByLabel('Glyph name', { exact: true })).toHaveText('a');
  await expect(detail.getByRole('list', { name: 'Replaced by' })).toContainText('Small capitals');
  // Tab reaches the detail: Close, Compare with…, then the feature tags.
  await page.keyboard.press('Tab');
  await expect(detail.getByRole('button', { name: 'Close glyph detail' })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(detail.getByRole('button', { name: 'Compare with…' })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(detail.getByRole('button', { name: 'Copy feature tag: smcp' })).toBeFocused();
  // Escape closes it and returns to the grid.
  await page.keyboard.press('Escape');
  await expect(detail).toBeHidden();
  await expect(grid).toBeFocused();
});

/** WCAG contrast ratio of the field's border against the page background. */
const borderContrast = (page: Page, name: string) =>
  page.getByLabel(name, { exact: true }).evaluate((el) => {
    const rgb = (c: string) => (c.match(/\d+(\.\d+)?/g) ?? []).slice(0, 3).map(Number);
    const lum = (c: string) => {
      const [r, g, b] = rgb(c).map((v) => {
        const s = v / 255;
        return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
      });
      return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    };
    const a = lum(getComputedStyle(el).borderTopColor);
    const b = lum(getComputedStyle(document.body).backgroundColor);
    return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
  });

for (const scheme of ['light', 'dark'] as const) {
  test(`the search field and the block picker are outlined at 3:1 or more (${scheme})`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await openGlyphs(page, { cjk: false });
    for (const name of ['Search glyphs', 'Block'])
      expect(await borderContrast(page, name)).toBeGreaterThanOrEqual(3);
  });
}
