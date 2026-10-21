import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';

// Layout blocks load face files from `/fmfont/<id>`; serve one fixture for all.
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
  await page.goto('/');
  await page.evaluate(() => localStorage.clear());
  await page.goto('/#/');
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
}

const modeButton = (page: Page, name: string) =>
  page.getByRole('group', { name: 'View mode' }).getByRole('button', { name, exact: true });
const picker = (page: Page) => page.getByRole('combobox', { name: 'Preset' });
const layout = (page: Page, name: string) => page.getByRole('region', { name: `${name} layout` });
const blocks = (region: Locator) => region.getByTestId('layout-block');

test('the layout mode shows the selected family; switching presets keeps it and the selection', async ({
  page,
}) => {
  await open(page);
  const tree = page.getByRole('tree', { name: 'Font list' });
  const mono = tree.getByRole('treeitem', { name: /^JetBrains Mono/ });
  await mono.click();
  await expect(mono).toHaveAttribute('aria-selected', 'true');
  const url = page.url();

  await modeButton(page, 'Layout').click();
  await expect(modeButton(page, 'Layout')).toHaveAttribute('aria-pressed', 'true');
  const book = layout(page, 'Book');
  await expect(book).toBeVisible();
  await expect(book).toContainText('JetBrains Mono');
  await expect(blocks(book)).toHaveCount(2);

  for (const [value, name] of [
    ['columns', 'Columns'],
    ['magazine', 'Magazine'],
    ['quote', 'Quote'],
    ['characters', 'Characters'],
    ['waterfall', 'Waterfall'],
  ] as const) {
    await picker(page).selectOption(value);
    const region = layout(page, name);
    await expect(region).toBeVisible();
    await expect(region).toContainText('JetBrains Mono');
    expect(page.url()).toBe(url);
  }

  // Back in the list, the family is still selected; the preset is a saved preference.
  await modeButton(page, 'List').click();
  await expect(tree.getByRole('treeitem', { name: /^JetBrains Mono/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await page.reload();
  await modeButton(page, 'Layout').click();
  await expect(picker(page)).toHaveValue('waterfall');
  await expect(layout(page, 'Waterfall')).toContainText('JetBrains Mono');
});

test('blocks are live text in the loaded face: columns balance, clip hides, wrap wraps', async ({
  page,
}) => {
  await open(page);
  await page.getByRole('tree', { name: 'Font list' }).getByRole('treeitem').first().click();
  await modeButton(page, 'Layout').click();
  await picker(page).selectOption('columns');
  const columns = layout(page, 'Columns');
  await expect(columns).toHaveAttribute('data-state', 'ready');
  const body = blocks(columns).nth(1);
  const css = await body.evaluate((el) => {
    const s = getComputedStyle(el);
    return {
      columnCount: s.columnCount,
      columnFill: s.columnFill,
      fontFamily: s.fontFamily,
      loaded: document.fonts.check(`12px ${s.fontFamily}`),
    };
  });
  expect(css.columnCount).toBe('3');
  expect(css.columnFill).toBe('balance');
  expect(css.fontFamily).toMatch(/^"?fm-face-\d+"?,/);
  expect(css.loaded).toBe(true);
  // A wrap block breaks its paragraphs into several lines.
  const lines = await body
    .locator('p')
    .first()
    .evaluate((p) =>
      Math.round(p.getBoundingClientRect().height / parseFloat(getComputedStyle(p).lineHeight)),
    );
  expect(lines).toBeGreaterThan(1);
  // Live text, not rasterized images.
  await expect(columns.locator('img, canvas')).toHaveCount(0);

  await picker(page).selectOption('waterfall');
  const fall = layout(page, 'Waterfall');
  await expect(blocks(fall)).toHaveCount(8);
  const top = blocks(fall).first();
  const clip = await top.locator('p').evaluate((p) => ({
    overflow: getComputedStyle(p).overflow,
    whiteSpace: getComputedStyle(p).whiteSpace,
    overflows: p.scrollWidth > p.clientWidth,
    oneLine: Math.round(p.clientHeight / parseFloat(getComputedStyle(p).lineHeight)) === 1,
  }));
  expect(clip).toEqual({
    overflow: 'hidden',
    whiteSpace: 'nowrap',
    overflows: true,
    oneLine: true,
  });
  const sizes = await blocks(fall).evaluateAll((bs) => bs.map((b) => getComputedStyle(b).fontSize));
  // 72 pt … 9 pt, falling.
  expect(sizes[0]).toBe('96px');
  expect(sizes.map(parseFloat)).toEqual([...sizes.map(parseFloat)].sort((a, b) => b - a));
});

test('the mode switch and the preset picker work from the keyboard and the palette', async ({
  page,
}) => {
  await open(page);
  // Key 5 is the layout mode (keys 1–5: list, table, grid, waterfall, layout).
  await page.getByRole('main').focus();
  await page.keyboard.press('5');
  await expect(modeButton(page, 'Layout')).toHaveAttribute('aria-pressed', 'true');
  await expect(layout(page, 'Book')).toBeVisible();

  // Tab goes from the mode buttons to the labelled picker.
  await modeButton(page, 'Layout').focus();
  await page.keyboard.press('Tab');
  await expect(picker(page)).toBeFocused();

  // Back to the list, then the palette picks a preset and switches to the layout mode.
  await page.keyboard.press('Shift+Tab');
  await page.keyboard.press('ControlOrMeta+k');
  const palette = page.getByRole('dialog', { name: 'Command palette' });
  await expect(
    palette.getByRole('option').filter({ hasText: 'Show fonts as a layout' }),
  ).toHaveCount(1);
  for (const name of ['Book', 'Columns', 'Magazine', 'Quote', 'Characters', 'Waterfall'])
    await expect(
      palette.getByRole('option').filter({ hasText: `Show the ${name} layout` }),
    ).toHaveCount(1);
  await page.keyboard.press('Escape');
  await page.keyboard.press('1');
  await expect(modeButton(page, 'List')).toHaveAttribute('aria-pressed', 'true');
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('Show the Quote layout');
  await expect(palette.getByRole('option').first()).toContainText('Show the Quote layout');
  await page.keyboard.press('Enter');
  await expect(layout(page, 'Quote')).toBeVisible();
  await expect(picker(page)).toHaveValue('quote');
});

/** WCAG contrast ratio of the field's border against the page background. */
const borderContrast = (field: Locator) =>
  field.evaluate((el) => {
    const rgb = (c: string) => (c.match(/\d+(\.\d+)?/g) ?? []).slice(0, 3).map(Number);
    const lum = (c: string) => {
      const [r, g, b] = rgb(c).map((v) => {
        const s = v / 255;
        return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
      });
      return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!;
    };
    const a = lum(getComputedStyle(el).borderTopColor);
    const b = lum(getComputedStyle(document.body).backgroundColor);
    return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
  });

for (const scheme of ['light', 'dark'] as const) {
  test(`the preset picker is outlined at 3:1 or more (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await open(page);
    await modeButton(page, 'Layout').click();
    expect(await borderContrast(picker(page))).toBeGreaterThanOrEqual(3);
  });
}

test('a start in the layout mode with nothing selected shows the first family', async ({
  page,
}) => {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.addInitScript(() =>
    localStorage.setItem(
      'fm.views.v1',
      JSON.stringify({ all: { mode: 'layout', preset: 'quote' } }),
    ),
  );
  await page.goto('/#/');
  const quote = layout(page, 'Quote');
  await expect(quote).toHaveAttribute('data-state', 'ready');
  await expect(quote).toContainText('Inter');
  await expect(blocks(quote)).toHaveCount(2);
});
