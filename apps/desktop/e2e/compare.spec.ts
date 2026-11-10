import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { addRow, compareRows } from './compare-pins';
import { selectOption } from './select';

// Preview rows and Compare load face files from `/fmfont/<id>`; serve one fixture for all.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

/** The list with three sample families after the mock's three: six family rows. */
async function open(page: Page) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/?mockFaces=3#/');
  await page.evaluate(() => localStorage.clear());
  const tree = page.getByRole('tree', { name: 'Font list' });
  await expect(tree.getByRole('treeitem')).toHaveCount(6);
  await tree.getByRole('treeitem').first().click();
  return tree;
}

const toast = (page: Page) => page.getByTestId('toast');
const sheet = (page: Page) => page.getByRole('dialog', { name: 'Compare' });
const compareButton = (page: Page) => page.getByRole('button', { name: /^Compare \d\/4$/ });

test('C starts a new comparison, the row icon adds up to four, a fifth is refused; a new C replaces the set', async ({
  page,
}) => {
  const tree = await open(page);
  await expect(compareButton(page)).toBeDisabled();

  // C: a new comparison with the cursor row as its base, in the open sheet (E48.5).
  await page.keyboard.press('c');
  await expect(toast(page)).toContainText(/^New comparison with Inter .* as its base\./);
  await expect(sheet(page)).toBeVisible();
  await expect(sheet(page).getByTestId('compare-count')).toHaveText(/^1 of 4 · base Inter /);
  await page.keyboard.press('Escape');
  await expect(sheet(page)).toBeHidden();

  // The row icon adds to the set and never clears it.
  await addRow(page, tree, 1, /^JetBrains Mono .* pinned to Compare \(2 of 4\)/);
  await addRow(page, tree, 2, /\(3 of 4\)/);
  await addRow(page, tree, 3, /^Sample 1 .*\(4 of 4\)/);
  await expect(compareButton(page)).toHaveAccessibleName('Compare 4/4');

  // The fifth is refused with a message.
  await addRow(page, tree, 4, 'Compare holds 4 fonts. Remove one to pin Sample 2');
  await expect(compareButton(page)).toHaveAccessibleName('Compare 4/4');

  // Remove one in the sheet; the fifth then fits.
  await compareButton(page).click();
  const pinned = sheet(page).getByRole('list', { name: 'Stacked' }).getByRole('listitem');
  await expect(pinned).toHaveCount(4);
  await pinned.getByRole('button', { name: /^Remove JetBrains Mono .* from Compare$/ }).click();
  await expect(pinned).toHaveCount(3);
  // The focus stays in the sheet, on the next Remove button.
  await expect(pinned.getByRole('button', { name: /^Remove Legacy Grotesk/ })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(sheet(page)).toBeHidden();
  await addRow(page, tree, 4, /^Sample 2 .*\(4 of 4\)/);

  // A new C (here from the palette) while a set exists replaces it: no stale pins.
  await tree.getByRole('treeitem').nth(5).click();
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('compare the selected');
  await page.keyboard.press('Enter');
  await expect(toast(page)).toContainText(/^New comparison with Sample 3 /);
  await expect(sheet(page).getByTestId('compare-count')).toHaveText(/^1 of 4 · base Sample 3 /);
  await expect(pinned).toHaveCount(1);
  await page.keyboard.press('Escape');
  await expect(compareButton(page)).toHaveAccessibleName('Compare 1/4');
});

test('Only this keeps one face as the base; the toast undoes it', async ({ page }) => {
  const tree = await open(page);
  await compareRows(page, tree, 3);
  await compareButton(page).click();
  const dialog = sheet(page);
  const faces = dialog.getByRole('list', { name: 'Stacked' }).getByRole('listitem');
  await expect(faces).toHaveCount(3);
  // The base wears the tag; the header names it.
  await expect(dialog.getByTestId('face-base')).toHaveCount(1);
  await expect(faces.first().getByTestId('face-base')).toHaveText('base');
  await faces.nth(2).hover();
  await faces.nth(2).getByTestId('face-only').click();
  await expect(dialog.getByTestId('compare-count')).toHaveText(/^1 of 4 · base Legacy Grotesk /);
  await expect(toast(page)).toContainText(/^Only Legacy Grotesk .* is left in Compare\./);
  // The suggestions follow the new base.
  await expect(dialog.getByTestId('compare-similar')).toContainText('Similar to Legacy Grotesk');
  await toast(page).getByRole('button', { name: 'Undo' }).click();
  await expect(dialog.getByTestId('compare-count')).toHaveText(/^3 of 4 · base Inter /);

  // By keyboard alone: Tab to 'Only this', Enter, then Ctrl/Cmd+Z in the sheet undoes it.
  const only = faces.nth(1).getByTestId('face-only');
  for (let i = 0; i < 30; i++) {
    if (await only.evaluate((el) => el === document.activeElement)) break;
    await page.keyboard.press('Tab');
  }
  await expect(only).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(dialog.getByTestId('compare-count')).toHaveText(/^1 of 4 · base JetBrains Mono /);
  await expect(toast(page)).toContainText('Z undoes it.');
  await page.keyboard.press('ControlOrMeta+z');
  await expect(dialog.getByTestId('compare-count')).toHaveText(/^3 of 4 · base Inter /);
  await expect(dialog).toBeVisible();
});

test('the sheet shows the pins in the sample text at one size, in three modes', async ({
  page,
}) => {
  const tree = await open(page);
  await compareRows(page, tree, 3);
  await compareButton(page).click();
  const dialog = sheet(page);
  await expect(dialog).toBeVisible();

  const modes = dialog.getByRole('radiogroup', { name: 'Mode' });
  // Stacked · Grid · Overlay, each with an icon; a new comparison opens Stacked.
  await expect(modes.getByRole('radio')).toHaveCount(3);
  await expect(modes.locator('label')).toHaveText(['Stacked', 'Grid', 'Overlay']);
  await expect(modes.locator('label svg')).toHaveCount(3);
  await expect(modes.getByRole('radio', { name: 'Stacked' })).toBeChecked();
  await expect(modes.getByRole('radio', { name: 'Stacked' })).toBeFocused();
  await expect(modes.getByRole('radio', { name: 'Side by side' })).toHaveCount(0);

  // Stacked: the faces under each other, the same text at the same size.
  const rows = dialog.getByRole('list', { name: 'Stacked' }).getByRole('listitem');
  await expect(rows).toHaveCount(3);
  const samples = dialog.locator('.sample');
  await expect(samples.first()).toHaveCSS('font-family', /fm-face-/);
  const boxesOf = () =>
    samples.evaluateAll((els) =>
      els.map((e) => ({
        text: e.textContent?.trim(),
        size: getComputedStyle(e).fontSize,
        top: Math.round(e.getBoundingClientRect().top),
        left: Math.round(e.getBoundingClientRect().left),
      })),
    );
  const stacked = await boxesOf();
  expect(new Set(stacked.map((b) => b.text)).size).toBe(1);
  expect(stacked[0]!.text).toContain('quick brown fox');
  // Auto follows the sheet's height: 64 px at the default 60 %.
  expect(stacked.map((b) => b.size)).toEqual(['64px', '64px', '64px']);
  expect(stacked[0]!.top).toBeLessThan(stacked[1]!.top);
  expect(stacked[1]!.top).toBeLessThan(stacked[2]!.top);

  // Arrow keys move through the radio group: Grid puts two faces per row, so three
  // faces fill a 2×2 grid (the third starts the second row, under the first).
  await page.keyboard.press('ArrowRight');
  await expect(modes.getByRole('radio', { name: 'Grid' })).toBeChecked();
  await expect(dialog.getByRole('list', { name: 'Grid' }).getByRole('listitem')).toHaveCount(3);
  const grid = await boxesOf();
  expect(grid[0]!.top).toBe(grid[1]!.top);
  expect(grid[0]!.left).toBeLessThan(grid[1]!.left);
  expect(grid[2]!.top).toBeGreaterThan(grid[0]!.top);
  expect(grid[2]!.left).toBe(grid[0]!.left);

  // Overlay: two faces on one baseline at 50% opacity; three pinned, so two pickers.
  await page.keyboard.press('ArrowRight');
  await expect(modes.getByRole('radio', { name: 'Overlay' })).toBeChecked();
  const overlay = dialog.getByRole('group', { name: 'Overlay' });
  const texts = overlay.locator('text');
  await expect(texts).toHaveCount(2);
  for (const t of await texts.all()) await expect(t).toHaveCSS('fill-opacity', '0.5');
  const fills = await texts.evaluateAll((els) => els.map((e) => getComputedStyle(e).fill));
  expect(fills[0]).not.toBe(fills[1]);
  const ys = await texts.evaluateAll((els) => els.map((e) => e.getAttribute('y')));
  expect(ys[0]).toBe(ys[1]);
  await expect(overlay.getByRole('combobox')).toHaveCount(2);
  await selectOption(overlay.getByRole('combobox', { name: 'Second font' }), { index: 2 });
  // The legend names the two faces, family and style.
  const legend = overlay.getByRole('list', { name: 'Legend' }).getByRole('listitem');
  await expect(legend).toHaveCount(2);
  await expect(legend.nth(1)).toContainText(/Legacy Grotesk\s+\S/);

  // Back to Grid with two faces: one row.
  await modes.getByRole('radio', { name: 'Grid' }).check();
  await dialog
    .getByTestId('face-remove')
    .and(dialog.getByRole('button', { name: /^Remove Legacy Grotesk/ }))
    .click();
  await expect(dialog.getByRole('list', { name: 'Grid' }).getByRole('listitem')).toHaveCount(2);
  const pair = await boxesOf();
  expect(pair[0]!.top).toBe(pair[1]!.top);
  expect(pair[0]!.left).toBeLessThan(pair[1]!.left);
});

test('focus stays in the sheet; Escape returns to the list with the selection kept', async ({
  page,
}) => {
  const tree = await open(page);
  await compareRows(page, tree, 2);
  const selected = tree.locator('[aria-selected="true"]');
  const before = await selected.getAttribute('data-key');
  const url = page.url();

  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('open compare');
  await page.keyboard.press('Enter');
  const dialog = sheet(page);
  await expect(dialog).toBeVisible();

  // Tab and Shift+Tab never leave the sheet.
  for (let i = 0; i < 8; i++) {
    await page.keyboard.press('Tab');
    expect(await dialog.evaluate((d) => d.contains(document.activeElement))).toBe(true);
  }
  for (let i = 0; i < 8; i++) {
    await page.keyboard.press('Shift+Tab');
    expect(await dialog.evaluate((d) => d.contains(document.activeElement))).toBe(true);
  }
  // C in the sheet does not pin the list's face behind it.
  await page.keyboard.press('c');
  await expect(compareButton(page)).toHaveAccessibleName('Compare 2/4');

  // A click on the sample text keeps the focus in the sheet: Tab stays inside, and
  // A does not activate the list's face behind the modal.
  await dialog.locator('.sample').first().click();
  expect(await dialog.evaluate((d) => d.contains(document.activeElement))).toBe(true);
  await page.keyboard.press('a');
  await page.keyboard.press('Tab');
  expect(await dialog.evaluate((d) => d.contains(document.activeElement))).toBe(true);
  await page.keyboard.press('Shift+Tab');
  expect(await dialog.evaluate((d) => d.contains(document.activeElement))).toBe(true);
  await dialog.locator('.sample').first().click();

  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  // Back in the list (opened from the palette, the focus goes to the list column).
  expect(await page.getByRole('main').evaluate((m) => m.contains(document.activeElement))).toBe(
    true,
  );
  await expect(selected).toHaveAttribute('data-key', before!);
  expect(page.url()).toBe(url);
  await expect(page.getByTestId('selected-row-state')).toHaveText('not active');

  // The pins stay for the session: reopening shows them again.
  await compareButton(page).click();
  await expect(dialog.getByRole('list', { name: 'Stacked' }).getByRole('listitem')).toHaveCount(2);
});

test('the sheet rises from the bottom, drags to full height and closes back to the row', async ({
  page,
}) => {
  const tree = await open(page);
  await compareRows(page, tree, 4);
  // Opened from the command palette over the list: the focus comes back to the row.
  const row = tree.locator('[aria-selected="true"]');
  const key = await row.getAttribute('data-key');
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('open compare');
  await page.keyboard.press('Enter');
  const dialog = sheet(page);
  await expect(dialog).toBeVisible();
  const viewport = page.viewportSize()!;
  const box = async () => (await dialog.boundingBox())!;

  // Anchored to the bottom edge, full width, 60 % high; the wall shows above under the scrim.
  await expect(async () => {
    const b = await box();
    expect(Math.round(b.y + b.height)).toBe(viewport.height);
    expect(Math.round(b.height)).toBe(Math.round(viewport.height * 0.6));
  }).toPass();
  expect(Math.round((await box()).width)).toBe(viewport.width);
  const scrim = page.getByTestId('compare-scrim');
  await expect(scrim).toHaveCSS('background-color', 'rgba(0, 0, 0, 0.28)');
  await expect(dialog).not.toHaveCSS('border-top-left-radius', '0px');

  // The header is one row at 1280 px: every control on one line, nothing clipped.
  const head = dialog.locator('header').first();
  await expect(head.getByTestId('compare-count')).toHaveText(/^4 of 4 · base Inter /);
  const line = await head.evaluate((h) => {
    const items = [...h.children].filter(
      (c) => c.id !== 'fm-compare-saved' && c.getBoundingClientRect().width > 0,
    );
    const mids = items.map((c) => {
      const r = c.getBoundingClientRect();
      return Math.round((r.top + r.bottom) / 2);
    });
    const count = h.querySelector<HTMLElement>('[data-testid="compare-count"]')!;
    return {
      spread: Math.max(...mids) - Math.min(...mids),
      overflow: h.scrollWidth - h.clientWidth,
      countClipped: count.scrollWidth - count.clientWidth,
    };
  });
  expect(line.spread).toBeLessThanOrEqual(2);
  expect(line.overflow).toBeLessThanOrEqual(0);
  expect(line.countClipped).toBeLessThanOrEqual(0);

  // Drag the handle up to 80 %: the specimen in Auto grows with it.
  const handle = dialog.getByRole('separator', { name: 'Compare height' });
  const drag = async (toY: number) => {
    const h = (await handle.boundingBox())!;
    await page.mouse.move(h.x + h.width / 2, h.y + h.height / 2);
    await page.mouse.down();
    await page.mouse.move(h.x + h.width / 2, toY, { steps: 5 });
    await page.mouse.up();
  };
  await drag(viewport.height * 0.2);
  await expect(handle).toHaveAttribute('aria-valuenow', '80');
  expect(Math.round((await box()).height)).toBe(Math.round(viewport.height * 0.8));
  await expect(dialog.locator('.sample').first()).toHaveCSS('font-size', '72px');
  // Down past 40 % it stops at 40 %, with 56 px specimens.
  await drag(viewport.height - 20);
  await expect(handle).toHaveAttribute('aria-valuenow', '40');
  await expect(dialog.locator('.sample').first()).toHaveCSS('font-size', '56px');

  // A double-click on the handle: the whole window, no radius, no scrim.
  await handle.dblclick();
  await expect(handle).toHaveAttribute('aria-valuenow', '100');
  const full = await box();
  expect(Math.round(full.y)).toBe(0);
  expect(Math.round(full.height)).toBe(viewport.height);
  await expect(dialog).toHaveCSS('border-top-left-radius', '0px');
  await expect(scrim).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
  await expect(dialog.locator('.sample').first()).toHaveCSS('font-size', '80px');
  // The maximize button goes back to the last height, and up again.
  await dialog.getByRole('button', { name: 'Restore height' }).click();
  await expect(handle).toHaveAttribute('aria-valuenow', '40');
  await dialog.getByRole('button', { name: 'Full height' }).click();
  await expect(handle).toHaveAttribute('aria-valuenow', '100');

  // Escape closes it; the focus is back in the list, on the row that opened it.
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  await expect(tree).toBeFocused();
  await expect(tree.locator('[aria-selected="true"]')).toHaveAttribute('data-key', key!);

  // The height is remembered: the next open is full height again.
  await compareButton(page).click();
  await expect(handle).toHaveAttribute('aria-valuenow', '100');
  // At 40 % again, a press on the scrim above the sheet closes it.
  await dialog.getByRole('button', { name: 'Restore height' }).click();
  await expect(handle).toHaveAttribute('aria-valuenow', '40');
  await page.mouse.click(viewport.width / 2, 40);
  await expect(dialog).toBeHidden();
});
