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
const card = (panel: Locator) => panel.getByRole('dialog', { name: 'Selected glyph' });
const more = (panel: Locator) => card(panel).getByRole('button', { name: 'More details' }).click();
const chip = (panel: Locator, name: string) =>
  panel
    .getByRole('group', { name: 'Blocks' })
    .getByRole('button', { name: new RegExp(`^${name} \\d`) });

/** The columns the CSS grid made, its cell size and the glyph's font size, from the first row. */
const measureRow = (panel: Locator) =>
  panel
    .getByRole('row')
    .first()
    .evaluate((row) => {
      const cell = row.querySelector('[role="gridcell"]')!;
      const r = cell.getBoundingClientRect();
      return {
        tracks: getComputedStyle(row).gridTemplateColumns.split(' ').length,
        width: r.width,
        height: r.height,
        font: parseFloat(getComputedStyle(cell.querySelector('.char')!).fontSize),
      };
    });

test('the grid fills its rows with 88 px cells, glyphs at half the cell, virtualized', async ({
  page,
}) => {
  const panel = await openGlyphs(page);
  await expect(panel).toContainText('32,543 characters');
  await expect(panel.getByRole('rowheader')).toHaveCount(0);
  // 30k+ characters, a few dozen cells in the DOM.
  const cells = await panel.getByRole('gridcell').count();
  expect(cells).toBeGreaterThan(10);
  expect(cells).toBeLessThan(400);
  // The CSS grid's auto-fill tracks are the columns the script counts; every cell is at
  // least 88 px wide and square, and the glyph is half of it.
  const cols = Number(await grid(panel).getAttribute('aria-colcount'));
  const m = await measureRow(panel);
  expect(m.tracks).toBe(cols);
  await expect(panel.getByRole('row').first().getByRole('gridcell')).toHaveCount(cols);
  expect(m.width).toBeGreaterThanOrEqual(88);
  expect(m.width).toBeLessThan(88 * 2);
  expect(Math.abs(m.height - m.width)).toBeLessThan(1.5);
  expect(m.font).toBe(Math.round(m.height / 2));
  // The cells use the face's own font, loaded through the font URL.
  const a = panel.getByRole('gridcell', { name: 'U+0041 A', exact: true });
  await expect(a.locator('.char')).toHaveCSS('font-family', /fm-face-1/);
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

test('a click opens a 360 px popover beside its cell; outside it closes, focus on the grid', async ({
  page,
}) => {
  const panel = await openGlyphs(page, { cjk: false });
  const cell = panel.getByRole('gridcell', { name: 'U+0041 A', exact: true });
  await cell.click();
  await expect(card(panel)).toBeVisible();
  const c = await cell.boundingBox();
  const p = await card(panel).boundingBox();
  expect(Math.round(p!.width)).toBe(360);
  expect(Math.abs(p!.y - c!.y)).toBeLessThan(2);
  // Beside the cell, not over it.
  expect(p!.x >= c!.x + c!.width || p!.x + p!.width <= c!.x).toBe(true);
  await panel.getByTestId('glyph-specimen').click();
  await expect(card(panel)).toBeHidden();
  await expect(grid(panel)).toBeFocused();
  await expect(grid(panel)).toHaveAttribute(
    'aria-activedescendant',
    (await cell.getAttribute('id'))!,
  );
});

test("'Details on click' off: a click selects only; the context menu opens the detail", async ({
  page,
}) => {
  const panel = await openGlyphs(page, { cjk: false });
  await panel.getByRole('switch', { name: 'Details on click' }).click();
  const cell = panel.getByRole('gridcell', { name: 'U+0061 a', exact: true });
  await cell.click();
  await expect(cell).toHaveAttribute('aria-selected', 'true');
  await expect(card(panel)).toBeHidden();
  await cell.click({ button: 'right' });
  const menu = page.getByRole('menu', { name: 'Actions for U+0061' });
  await expect(menu.getByRole('menuitem').first()).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(card(panel).getByTestId('glyph-code')).toHaveText('U+0061');
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
    await expect(card(panel).getByTestId('glyph-code')).toHaveText('U+0416');
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
  await expect(card(panel).getByTestId('glyph-code')).toHaveText('U+AC01');
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
  await expect(card(panel)).toBeHidden();
  await page.keyboard.press('Enter');
  await expect(card(panel).getByTestId('glyph-code')).toHaveText('U+0021');
  await page.keyboard.press('ArrowDown');
  const cols = Number(await grid(panel).getAttribute('aria-colcount'));
  await expect(grid(panel)).toHaveAttribute('aria-activedescendant', `fm-glyph-${1 + cols}`);
  await expect(card(panel).getByTestId('glyph-code')).not.toHaveText('U+0021');
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

test('the popover shows the glyph at 92 px with code, names, feature chips, styles and live alternates', async ({
  page,
}) => {
  const panel = await openGlyphs(page, { cjk: false });
  await panel.getByRole('searchbox', { name: 'Search glyphs' }).fill('U+0061');
  const detail = card(panel);
  await expect(detail.getByTestId('glyph-code')).toHaveText('U+0061');
  await expect(detail.getByTestId('glyph-names')).toHaveText('LATIN SMALL LETTER A · glyph a');
  await expect(detail.getByRole('list', { name: 'Features' })).toContainText('ss01');
  const big = detail.getByTestId('glyph-big');
  await expect(big).toHaveCSS('font-size', '92px');
  await expect(big).toHaveCSS('font-family', /fm-face-1/);
  await expect(big).toHaveText('a');
  await more(panel);
  await expect(detail.getByLabel('In styles', { exact: true })).toHaveText('all 4');
  const alternate = detail.getByRole('list', { name: 'Alternates' }).getByRole('listitem').first();
  await expect(alternate).toContainText('Stylistic set 1');
  await expect(alternate.locator('.alternate-char')).toHaveCSS('font-feature-settings', /"ss01"/);
});

test('the card draws the axis ends of the glyph live', async ({ page }) => {
  const panel = await openGlyphs(page, { cjk: false });
  await panel.getByRole('searchbox', { name: 'Search glyphs' }).fill('U+0041');
  await more(panel);
  const list = card(panel).getByRole('list', { name: 'Alternates' });
  for (const value of [100, 900])
    await expect(
      list
        .getByRole('listitem')
        .filter({ hasText: `Weight ${value}` })
        .locator('.alternate-char'),
    ).toHaveCSS('font-variation-settings', `"wght" ${value}`);
});

test('End on a long list selects the last character, and the card follows', async ({ page }) => {
  const panel = await openGlyphs(page);
  await grid(panel).focus();
  await page.keyboard.press('End');
  await page.keyboard.press('Enter');
  await expect(card(panel).getByTestId('glyph-code')).toHaveText('U+D7A3');
  await expect(panel.getByRole('gridcell', { name: /^U\+D7A3 / })).toHaveAttribute(
    'aria-selected',
    'true',
  );
});

test('Tab reaches the popover; Escape closes it and returns to the grid', async ({ page }) => {
  const panel = await openGlyphs(page, { cjk: false });
  await panel.getByRole('searchbox', { name: 'Search glyphs' }).fill('U+0061');
  const detail = card(panel);
  await expect(detail).toBeVisible();
  await detail.getByRole('button', { name: 'Close glyph details' }).focus();
  await page.keyboard.press('Tab');
  await expect(detail.getByRole('button', { name: 'Copy feature tag: smcp' })).toBeFocused();
  const moreButton = detail.getByRole('button', { name: 'More details' });
  await moreButton.focus();
  await page.keyboard.press('Enter');
  await expect(detail.getByLabel('Block', { exact: true })).toHaveText('Basic Latin');
  // No metric without a comparison.
  await expect(detail).not.toContainText(/advance|units/i);
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
  await more(panel);
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
