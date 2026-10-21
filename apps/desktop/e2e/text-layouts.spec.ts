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

const fileBar = (page: Page) => page.getByRole('toolbar', { name: 'Layout file' });
const action = (page: Page, name: string) =>
  fileBar(page).getByRole('button', { name, exact: true });

test('the layout view saves an edited block, reverts, duplicates and restores a version', async ({
  page,
}) => {
  await open(page);
  await modeButton(page, 'Layout').click();
  const book = layout(page, 'Book');
  await expect(blocks(book)).toHaveCount(2);
  await expect(action(page, 'Save')).toBeDisabled();
  await expect(action(page, 'Revert')).toBeDisabled();

  // Edit the heading in place: double-click opens a text box, leaving it keeps the text.
  await blocks(book).first().dblclick();
  const box = book.getByRole('textbox', { name: 'Block 1 text' });
  await expect(box).toBeFocused();
  await box.fill('Chapter 12');
  await box.blur();
  await expect(blocks(book).first()).toHaveText('Chapter 12');
  await expect(fileBar(page)).toContainText('Unsaved changes');

  // Revert drops it; edit again and save.
  await action(page, 'Revert').click();
  await expect(blocks(book).first()).toHaveText('Chapter One');
  await expect(action(page, 'Save')).toBeDisabled();
  await blocks(book).first().focus();
  await page.keyboard.press('Enter');
  await box.fill('Chapter Two');
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(blocks(book).first()).toBeFocused();
  await action(page, 'Save').click();
  await expect(fileBar(page)).toContainText('Saved.');
  await expect(action(page, 'Save')).toBeDisabled();

  // History has the content the save replaced; restoring it brings the original back.
  await action(page, 'History').click();
  const history = page.getByRole('region', { name: 'Layout history' });
  await expect(history.getByRole('listitem')).toHaveCount(1);
  await history.getByRole('button', { name: 'Restore version 1' }).click();
  await expect(blocks(book).first()).toHaveText('Chapter One');
  await expect(history.getByRole('listitem')).toHaveCount(2);

  // Duplicate opens an independent copy that the picker lists and keeps.
  await action(page, 'Duplicate').click();
  await expect(fileBar(page)).toContainText('Duplicated as “Book copy”.');
  await expect(picker(page)).toHaveValue('book-copy');
  const copy = layout(page, 'Book copy');
  await blocks(copy).first().dblclick();
  await copy.getByRole('textbox', { name: 'Block 1 text' }).fill('Copy heading');
  await page.keyboard.press('ControlOrMeta+Enter');
  await action(page, 'Save').click();
  await picker(page).selectOption('book');
  await expect(blocks(layout(page, 'Book')).first()).toHaveText('Chapter One');
  await picker(page).selectOption('book-copy');
  await expect(blocks(layout(page, 'Book copy')).first()).toHaveText('Copy heading');

  // Reset puts back the shipped preset; a broken file is listed but cannot be picked.
  await action(page, 'Reset to original').click();
  await expect(blocks(layout(page, 'Book copy')).first()).toHaveText('Chapter One');
  const broken = picker(page).getByRole('option', { name: 'draft (cannot be read)' });
  await expect(broken).toHaveAttribute('disabled', '');
});

for (const scheme of ['light', 'dark'] as const) {
  test(`the block text box is outlined at 3:1 or more (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await open(page);
    await modeButton(page, 'Layout').click();
    await blocks(layout(page, 'Book')).first().dblclick();
    const box = page.getByRole('textbox', { name: 'Block 1 text' });
    expect(await borderContrast(box)).toBeGreaterThanOrEqual(3);
  });
}

test('a size badge beside each block opens its properties editor; changes show at once', async ({
  page,
}) => {
  await open(page);
  await page.getByRole('tree', { name: 'Font list' }).getByRole('treeitem').first().click();
  await modeButton(page, 'Layout').click();
  await picker(page).selectOption('magazine');
  const mag = layout(page, 'Magazine');
  await expect(mag).toHaveAttribute('data-state', 'ready');
  const badges = mag.getByTestId('block-size-badge');
  await expect(badges).toHaveCount(await blocks(mag).count());
  const others = await blocks(mag).evaluateAll((bs) => bs.map((b) => getComputedStyle(b).fontSize));

  // Click opens the editor of that block.
  const badge = mag.getByRole('button', { name: /^Block 2 properties, / });
  await badge.click();
  const editor = page.getByRole('dialog', { name: 'Block 2 properties' });
  await expect(editor).toBeVisible();
  await expect(badge).toHaveAttribute('aria-expanded', 'true');

  const size = editor.getByLabel('Size');
  await size.fill('30');
  await size.press('Enter');
  await expect(blocks(mag).nth(1)).toHaveCSS('font-size', '40px');
  await expect(badge).toHaveText('30 pt');
  // Out of range: clamped to the core's 600 pt.
  await size.fill('9999');
  await size.press('Enter');
  await expect(size).toHaveValue('600');
  await expect(blocks(mag).nth(1)).toHaveCSS('font-size', '800px');
  await size.fill('30');
  await size.press('Enter');

  await editor.getByLabel('Columns').selectOption('3');
  await expect(blocks(mag).nth(1)).toHaveCSS('column-count', '3');
  await expect(blocks(mag).nth(1)).toHaveCSS('column-fill', 'balance');
  await editor.getByLabel('Alignment').selectOption('end');
  await expect(blocks(mag).nth(1)).toHaveCSS('text-align', 'end');
  await editor.getByLabel('Overflow').selectOption('clip');
  await expect(blocks(mag).nth(1)).toHaveAttribute('data-overflow', 'clip');
  const lh = editor.getByLabel('Line height');
  await lh.fill('2');
  await lh.press('Enter');
  await expect(blocks(mag).nth(1)).toHaveCSS('line-height', '80px');
  const tr = editor.getByLabel('Tracking');
  await tr.fill('100');
  await tr.press('Enter');
  await expect(blocks(mag).nth(1)).toHaveCSS('letter-spacing', '4px');
  const ps = editor.getByLabel('Paragraph spacing');
  await ps.fill('6');
  await ps.press('Enter');
  await expect(blocks(mag).nth(1).locator('p').first()).toHaveCSS('margin-bottom', '8px');
  const ind = editor.getByLabel('First-line indent');
  await ind.fill('1');
  await ind.press('Enter');
  await expect(blocks(mag).nth(1).locator('p').first()).toHaveCSS('text-indent', '40px');

  // The other blocks keep their values.
  const after = await blocks(mag).evaluateAll((bs) => bs.map((b) => getComputedStyle(b).fontSize));
  expect(after.filter((_, i) => i !== 1)).toEqual(others.filter((_, i) => i !== 1));
});

test('the properties editor works from the keyboard; Escape returns focus to the badge', async ({
  page,
}) => {
  await open(page);
  await page.getByRole('tree', { name: 'Font list' }).getByRole('treeitem').first().click();
  await modeButton(page, 'Layout').click();
  const book = layout(page, 'Book');
  await expect(book).toHaveAttribute('data-state', 'ready');
  const badge = book.getByRole('button', { name: /^Block 1 properties, / });
  await badge.focus();
  await page.keyboard.press('Enter');
  const editor = page.getByRole('dialog', { name: 'Block 1 properties' });
  await expect(editor).toBeVisible();

  // Every control has a label and Tab reaches each in turn.
  const names = [
    'Size',
    'Line height',
    'Tracking',
    'Paragraph spacing',
    'First-line indent',
    'Columns',
    'Alignment',
    'Overflow',
  ];
  await expect(editor.getByLabel('Size')).toBeFocused();
  for (const name of names.slice(1)) {
    await page.keyboard.press('Tab');
    await expect(editor.getByLabel(name, { exact: true })).toBeFocused();
  }
  await page.keyboard.press('Tab');
  await expect(editor.getByRole('button', { name: 'Close' })).toBeFocused();

  // The keyboard steps a number by the core's step (1 pt of size). A closed select is not
  // stepped here: on macOS an arrow key opens its menu instead of changing the value.
  for (let i = 0; i < names.length; i++) await page.keyboard.press('Shift+Tab');
  const size = editor.getByLabel('Size');
  await expect(size).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await expect(size).toHaveValue('25');
  await expect(blocks(book).first()).toHaveAttribute('data-size', '25');
  await expect(badge).toHaveText('25 pt');

  await page.keyboard.press('Escape');
  await expect(editor).toBeHidden();
  await expect(badge).toBeFocused();
  await expect(badge).toHaveAttribute('aria-expanded', 'false');
});

for (const scheme of ['light', 'dark'] as const) {
  test(`the size badge and the properties fields are outlined at 3:1 or more (${scheme})`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await open(page);
    await modeButton(page, 'Layout').click();
    const badge = layout(page, 'Book').getByRole('button', { name: /^Block 1 properties, / });
    expect(await borderContrast(badge)).toBeGreaterThanOrEqual(3);
    await badge.click();
    const editor = page.getByRole('dialog', { name: 'Block 1 properties' });
    for (const name of ['Size', 'Columns', 'Alignment', 'Overflow']) {
      expect(await borderContrast(editor.getByLabel(name, { exact: true }))).toBeGreaterThanOrEqual(
        3,
      );
    }
  });
}

const widthLock = (page: Page) => page.getByRole('button', { name: 'Lock width', exact: true });
/** The rendered width of the text column of the first block, in CSS px. */
const columnWidth = (region: Locator) =>
  blocks(region)
    .first()
    .evaluate((el) => el.getBoundingClientRect().width);
/** The number of characters on the first line of the first paragraph of block `i`. */
const charsOnALine = (region: Locator, i: number) =>
  blocks(region)
    .nth(i)
    .evaluate((el) => {
      const p = el.querySelector('p') ?? el;
      const text = p.firstChild as Text;
      const range = document.createRange();
      const tops: number[] = [];
      for (let c = 0; c < text.length; c++) {
        range.setStart(text, c);
        range.setEnd(text, c + 1);
        const rect = range.getClientRects()[0];
        tops.push(rect ? Math.round(rect.top) : NaN);
      }
      return tops.filter((t) => t === tops[0]).length;
    });

test('by default the layout is a readable width, centered, and a wider window does not widen it', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1400, height: 800 });
  await open(page);
  await modeButton(page, 'Layout').click();
  const book = layout(page, 'Book');
  await expect(book).toHaveAttribute('data-state', 'ready');
  await expect(widthLock(page)).toHaveAttribute('aria-pressed', 'true');
  // About 65–75 characters on a line of the 11 pt body block (its first line is indented).
  const chars = await charsOnALine(book, 1);
  expect(chars).toBeGreaterThanOrEqual(63);
  expect(chars).toBeLessThanOrEqual(77);
  const narrow = await columnWidth(book);
  // Centered: as much room on the left of the text as on the right.
  const page1 = await book.getByTestId('layout-page').boundingBox();
  const cell = await blocks(book).nth(1).boundingBox();
  const left = cell!.x - page1!.x;
  const right = page1!.x + page1!.width - (cell!.x + cell!.width);
  expect(Math.abs(left - right)).toBeLessThanOrEqual(2);

  await page.setViewportSize({ width: 1800, height: 800 });
  await expect
    .poll(() => book.getByTestId('layout-page').evaluate((e) => e.clientWidth))
    .toBeGreaterThan(1200);
  expect(await columnWidth(book)).toBeCloseTo(narrow, 0);
  expect(await charsOnALine(book, 1)).toBe(chars);
});

test('unlocking follows the window; locking again keeps the width it had through later resizes', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1600, height: 800 });
  await open(page);
  await modeButton(page, 'Layout').click();
  const book = layout(page, 'Book');
  await expect(book).toHaveAttribute('data-state', 'ready');
  const locked = await columnWidth(book);

  await widthLock(page).click();
  await expect(widthLock(page)).toHaveAttribute('aria-pressed', 'false');
  await expect.poll(() => columnWidth(book)).toBeGreaterThan(locked + 100);
  const wide = await columnWidth(book);
  await page.setViewportSize({ width: 1900, height: 800 });
  await expect.poll(() => columnWidth(book)).toBeGreaterThan(wide + 200);
  const atLock = await columnWidth(book);

  // Locked again: the width of that moment, kept through a narrower and a wider window.
  await widthLock(page).click();
  await expect(widthLock(page)).toHaveAttribute('aria-pressed', 'true');
  await page.setViewportSize({ width: 2300, height: 800 });
  await expect(book).toBeVisible();
  expect(Math.abs((await columnWidth(book)) - atLock)).toBeLessThanOrEqual(1);
  await page.setViewportSize({ width: 2100, height: 800 });
  await expect.poll(() => columnWidth(book)).toBeLessThanOrEqual(atLock + 1);
  expect(Math.abs((await columnWidth(book)) - atLock)).toBeLessThanOrEqual(1);
  // The lock and the width are part of the layout: saving keeps them with the file.
  await action(page, 'Save').click();
  await expect(action(page, 'Save')).toBeDisabled();
});

test('the width lock works from the keyboard and the command palette', async ({ page }) => {
  await open(page);
  await modeButton(page, 'Layout').click();
  await expect(layout(page, 'Book')).toHaveAttribute('data-state', 'ready');
  const lock = widthLock(page);
  await lock.focus();
  await expect(lock).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(lock).toHaveAttribute('aria-pressed', 'false');
  await page.keyboard.press('Space');
  await expect(lock).toHaveAttribute('aria-pressed', 'true');

  await page.keyboard.press('ControlOrMeta+k');
  const palette = page.getByRole('dialog', { name: 'Command palette' });
  await page.keyboard.type('Lock or unlock the layout width');
  await expect(palette.getByRole('option').first()).toContainText(
    'Lock or unlock the layout width',
  );
  await page.keyboard.press('Enter');
  await expect(lock).toHaveAttribute('aria-pressed', 'false');
});

for (const scheme of ['light', 'dark'] as const) {
  test(`the width lock button is outlined at 3:1 or more (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await open(page);
    await modeButton(page, 'Layout').click();
    expect(await borderContrast(widthLock(page))).toBeGreaterThanOrEqual(3);
  });
}
