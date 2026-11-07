import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { openSection } from './family-page';

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
  const panel = await openSection(page, 1, 'Characters', cjk ? '?mockCjk=1' : '');
  await expect(panel.getByRole('gridcell').first()).toBeVisible();
  return panel;
}

const scroller = (page: Page) => page.locator('main.scroll');
const grid = (panel: Locator) => panel.getByRole('grid', { name: 'Glyphs' });
const card = (panel: Locator) => panel.getByRole('region', { name: 'Selected glyph' });
const chip = (panel: Locator, name: string) =>
  panel
    .getByRole('group', { name: 'Blocks' })
    .getByRole('button', { name: new RegExp(`^${name} \\d`) });

test('the grid draws the face at 26 px in twelve columns, virtualized', async ({ page }) => {
  const panel = await openGlyphs(page);
  await expect(panel).toContainText('32,543 characters');
  await expect(panel.getByRole('rowheader')).toHaveCount(0);
  await expect(grid(panel)).toHaveAttribute('aria-colcount', '12');
  // 30k+ characters, a few dozen cells in the DOM.
  const cells = await panel.getByRole('gridcell').count();
  expect(cells).toBeGreaterThan(10);
  expect(cells).toBeLessThan(400);
  // Twelve to a row.
  const firstRow = panel.getByRole('row').first().getByRole('gridcell');
  await expect(firstRow).toHaveCount(12);
  // The cells use the face's own font, loaded through the font URL, at 26 px.
  const a = panel.getByRole('gridcell', { name: 'U+0041 A', exact: true });
  await expect(a.locator('.char')).toHaveCSS('font-family', /fm-face-1/);
  await expect(a.locator('.char')).toHaveCSS('font-size', '26px');
  // Scrolling the page to the end renders the last rows, still not every cell.
  await scroller(page).evaluate((el) => (el.scrollTop = el.scrollHeight));
  await expect(panel.getByRole('gridcell', { name: /^U\+D7A3 / })).toBeAttached();
  expect(await panel.getByRole('gridcell').count()).toBeLessThan(600);
});

test('no inner scrollbars: the grid grows with the page', async ({ page }) => {
  const panel = await openGlyphs(page);
  const box = await grid(panel).evaluate((el) => ({
    overflow: getComputedStyle(el).overflowY,
    height: el.clientHeight,
    scrolled: el.scrollHeight - el.clientHeight,
    rows: Number(el.getAttribute('aria-rowcount')),
    cell: el.querySelector('[role="gridcell"]')!.getBoundingClientRect().height,
  }));
  expect(box.overflow).toBe('visible');
  expect(box.scrolled).toBe(0);
  expect(box.height).toBeCloseTo(box.rows * box.cell, -1);
  const pageScroll = await scroller(page).evaluate((el) => el.scrollHeight);
  expect(pageScroll).toBeGreaterThan(box.height);
  // Nothing in the panel scrolls by itself.
  const inner = await panel.evaluate(
    (el) =>
      [...el.querySelectorAll('*')].filter((e) => {
        const o = getComputedStyle(e).overflowY;
        return (o === 'auto' || o === 'scroll') && e.scrollHeight > e.clientHeight;
      }).length,
  );
  expect(inner).toBe(0);
});

test('the detail card is sticky on a tall window, and not on a short one', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 1000 });
  const panel = await openGlyphs(page);
  await panel.getByRole('gridcell', { name: 'U+0041 A', exact: true }).click();
  await expect(card(panel)).toBeVisible();
  await scroller(page).evaluate((el) => (el.scrollTop = 6000));
  await expect(card(panel)).toBeInViewport();
  const top = await card(panel).evaluate((el) => el.getBoundingClientRect().top);
  await scroller(page).evaluate((el) => (el.scrollTop = 9000));
  await expect
    .poll(() => card(panel).evaluate((el) => el.getBoundingClientRect().top))
    .toBeCloseTo(top, 0);

  await page.setViewportSize({ width: 1280, height: 700 });
  await expect(card(panel)).toHaveCSS('position', 'static');
});

test('a chip lists its block with its count, and a second press lists every block', async ({
  page,
}) => {
  const panel = await openGlyphs(page);
  await chip(panel, 'Cyrillic').click();
  await expect(chip(panel, 'Cyrillic')).toHaveAttribute('aria-pressed', 'true');
  await expect(panel.getByRole('gridcell', { name: 'U+0416 uni0416' })).toBeVisible();
  await expect(panel.getByRole('gridcell', { name: 'U+0041 A', exact: true })).toHaveCount(0);
  await chip(panel, 'Cyrillic').click();
  await expect(chip(panel, 'Cyrillic')).toHaveAttribute('aria-pressed', 'false');
  await expect(panel.getByRole('gridcell', { name: 'U+0041 A', exact: true })).toBeVisible();
});

test('a search takes a character, a code point or a glyph name', async ({ page }) => {
  const panel = await openGlyphs(page, { cjk: false });
  const field = panel.getByRole('searchbox', { name: 'Search glyphs' });
  for (const query of ['Ж', 'U+0416', 'uni0416']) {
    await field.fill(query);
    await expect(card(panel).getByLabel('Code', { exact: true })).toHaveText('U+0416');
    await field.fill('');
  }
  await field.fill('nothing like this');
  await expect(panel.getByRole('status').first()).toContainText('No glyph matches');
});

test('search by code point scrolls the page to the glyph and selects it', async ({ page }) => {
  const panel = await openGlyphs(page);
  await panel.getByRole('searchbox', { name: 'Search glyphs' }).fill('U+AC01');
  const cell = panel.getByRole('gridcell', { name: /^U\+AC01 / });
  await expect(cell).toBeInViewport();
  await expect(cell).toHaveAttribute('aria-selected', 'true');
  await expect(card(panel).getByLabel('Code', { exact: true })).toHaveText('U+AC01');
});

test('the arrow keys move the selection and the card follows; they do not change the family', async ({
  page,
}) => {
  const panel = await openGlyphs(page, { cjk: false });
  const url = page.url();
  await grid(panel).focus();
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('ArrowRight');
  await expect(grid(panel)).toHaveAttribute('aria-activedescendant', 'fm-glyph-1');
  const excl = panel.getByRole('gridcell', { name: 'U+0021 uni0021' });
  await expect(excl).toHaveAttribute('aria-selected', 'true');
  await expect(card(panel).getByLabel('Code', { exact: true })).toHaveText('U+0021');
  await page.keyboard.press('ArrowDown');
  await expect(grid(panel)).toHaveAttribute('aria-activedescendant', 'fm-glyph-13');
  await expect(card(panel).getByLabel('Code', { exact: true })).not.toHaveText('U+0021');
  expect(page.url()).toBe(url);
  // Every key-reached cell is labelled with code point and name.
  await page.keyboard.press('End');
  const last = await grid(panel).getAttribute('aria-activedescendant');
  await expect(page.locator(`#${last}`)).toHaveAttribute('aria-label', /^U\+[0-9A-F]{4,6} \S/);
  await expect(page.locator(`#${last}`)).toBeInViewport();
  await page.keyboard.press('Home');
  await panel.getByRole('gridcell', { name: 'U+0041 A', exact: true }).click();
  await expect(panel.getByRole('gridcell', { name: 'U+0041 A', exact: true })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await expect(excl).toHaveAttribute('aria-selected', 'false');
});

test('the card shows the glyph at 160 px with name, code, styles and live alternates', async ({
  page,
}) => {
  const panel = await openGlyphs(page, { cjk: false });
  await panel.getByRole('searchbox', { name: 'Search glyphs' }).fill('U+0061');
  const detail = card(panel);
  await expect(detail.getByLabel('Code', { exact: true })).toHaveText('U+0061');
  await expect(detail.getByLabel('Name', { exact: true })).toHaveText('a');
  await expect(detail.getByLabel('In styles', { exact: true })).toHaveText(
    /^(all \d+|\d+ of \d+)$/,
  );
  const big = detail.getByTestId('glyph-big');
  await expect(big).toHaveCSS('font-size', '160px');
  await expect(big).toHaveCSS('font-family', /fm-face-1/);
  await expect(big).toHaveText('a');
  const alternate = detail.getByRole('list', { name: 'Alternates' }).getByRole('listitem').first();
  await expect(alternate).toContainText('Stylistic set 1');
  await expect(alternate.locator('.alternate-char')).toHaveCSS('font-feature-settings', /"ss01"/);
});

test('Tab reaches the card; Escape closes it and returns to the grid', async ({ page }) => {
  const panel = await openGlyphs(page, { cjk: false });
  await panel.getByRole('searchbox', { name: 'Search glyphs' }).fill('U+0061');
  const detail = card(panel);
  await expect(detail).toBeVisible();
  // The coverage list comes after the grid in its column, so start from the card's first button.
  await detail.getByRole('button', { name: 'Copy glyph' }).focus();
  await page.keyboard.press('Tab');
  await expect(detail.getByRole('button', { name: 'Compare with…' })).toBeFocused();
  await page.keyboard.press('Tab');
  const more = detail.getByRole('button', { name: 'More details' });
  await expect(more).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(detail.getByLabel('Advance width', { exact: true })).toHaveText(/^\d[\d,]* units$/);
  await expect(detail.getByRole('list', { name: 'Replaced by' })).toContainText('Small capitals');
  await page.keyboard.press('Escape');
  await expect(detail).toBeHidden();
  await expect(grid(panel)).toBeFocused();
  await expect(page).toHaveURL(/#\/family\//);
});

test('Copy glyph puts the character on the clipboard', async ({ page, context, browserName }) => {
  test.skip(browserName !== 'chromium', 'clipboard permissions are Chromium only');
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  const panel = await openGlyphs(page, { cjk: false });
  await panel.getByRole('searchbox', { name: 'Search glyphs' }).fill('U+0416');
  await card(panel).getByRole('button', { name: 'Copy glyph' }).click();
  await expect(card(panel)).toContainText('Copied');
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('Ж');
});

/** WCAG contrast of the element's top border against the page background. */
const edgeContrast = (target: Locator) =>
  target.evaluate((el) => {
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
  test(`the search field, the pressed chip and the selected cell reach 3:1 (${scheme})`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: scheme });
    const panel = await openGlyphs(page, { cjk: false });
    expect(
      await edgeContrast(page.getByLabel('Search glyphs', { exact: true })),
    ).toBeGreaterThanOrEqual(3);
    await chip(panel, 'Basic Latin').click();
    await expect(chip(panel, 'Basic Latin')).toHaveAttribute('aria-pressed', 'true');
    await expect.poll(() => edgeContrast(chip(panel, 'Basic Latin'))).toBeGreaterThanOrEqual(3);
    // A selected cell away from the cursor: the mark is not the focus ring.
    const cell = panel.getByRole('gridcell', { name: 'U+0041 A', exact: true });
    await cell.click();
    await expect(cell).toHaveAttribute('aria-selected', 'true');
    await expect.poll(() => edgeContrast(cell)).toBeGreaterThanOrEqual(3);
  });
}
