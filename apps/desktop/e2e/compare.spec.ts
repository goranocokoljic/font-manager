import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
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

/** Pins the cursor row with C, then moves the cursor down. */
async function pinAndNext(page: Page, expected: RegExp) {
  await page.keyboard.press('c');
  await expect(toast(page)).toContainText(expected);
  await page.keyboard.press('ArrowDown');
}

test('C and the palette pin up to four fonts; a fifth is refused; a pin can be removed', async ({
  page,
}) => {
  const tree = await open(page);
  await expect(compareButton(page)).toBeDisabled();

  await pinAndNext(page, /^Inter .* pinned to Compare \(1 of 4\)/);
  await pinAndNext(page, /^JetBrains Mono .* \(2 of 4\)/);
  await pinAndNext(page, /\(3 of 4\)/);
  // The fourth through the command palette.
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('pin the selected');
  await page.keyboard.press('Enter');
  await expect(toast(page)).toContainText('Sample 1');
  await expect(toast(page)).toContainText('(4 of 4)');
  await expect(compareButton(page)).toHaveAccessibleName('Compare 4/4');

  // The fifth is refused with a message.
  await tree.getByRole('treeitem').nth(4).click();
  await page.keyboard.press('c');
  await expect(toast(page)).toContainText('Compare holds 4 fonts. Remove one to pin Sample 2');
  await expect(compareButton(page)).toHaveAccessibleName('Compare 4/4');

  // Remove one in the sheet; the fifth then fits.
  await compareButton(page).click();
  const pinned = sheet(page).getByRole('list', { name: 'Side by side' }).getByRole('listitem');
  await expect(pinned).toHaveCount(4);
  await sheet(page)
    .getByRole('button', { name: /^Remove JetBrains Mono .* from Compare$/ })
    .click();
  await expect(pinned).toHaveCount(3);
  // The focus stays in the sheet, on the next Remove button.
  await expect(sheet(page).getByRole('button', { name: /^Remove Legacy Grotesk/ })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(sheet(page)).toBeHidden();
  await page.keyboard.press('c');
  await expect(toast(page)).toContainText('Sample 2');
  await expect(toast(page)).toContainText('(4 of 4)');
});

test('the sheet shows the pins in the sample text at one size, in three modes', async ({
  page,
}) => {
  await open(page);
  await pinAndNext(page, /\(1 of 4\)/);
  await pinAndNext(page, /\(2 of 4\)/);
  await pinAndNext(page, /\(3 of 4\)/);
  await compareButton(page).click();
  const dialog = sheet(page);
  await expect(dialog).toBeVisible();

  const modes = dialog.getByRole('radiogroup', { name: 'Mode' });
  await expect(modes.getByRole('radio')).toHaveCount(3);
  await expect(modes.getByRole('radio', { name: 'Side by side' })).toBeChecked();
  await expect(modes.getByRole('radio', { name: 'Side by side' })).toBeFocused();

  // Side by side: three columns on one row, the same text at the same size.
  const columns = dialog.getByRole('list', { name: 'Side by side' }).getByRole('listitem');
  await expect(columns).toHaveCount(3);
  const samples = dialog.locator('.sample');
  await expect(samples.first()).toHaveCSS('font-family', /fm-face-/);
  const boxes = await samples.evaluateAll((els) =>
    els.map((e) => ({
      text: e.textContent?.trim(),
      size: getComputedStyle(e).fontSize,
      top: e.getBoundingClientRect().top,
      left: e.getBoundingClientRect().left,
    })),
  );
  expect(new Set(boxes.map((b) => b.text)).size).toBe(1);
  expect(boxes[0]!.text).toContain('quick brown fox');
  // Auto follows the sheet's height: 64 px at the default 60 %.
  expect(boxes.map((b) => b.size)).toEqual(['64px', '64px', '64px']);
  expect(boxes[0]!.top).toBe(boxes[2]!.top);
  expect(boxes[0]!.left).toBeLessThan(boxes[1]!.left);

  // Arrow keys move through the radio group: Stacked puts the faces under each other.
  await page.keyboard.press('ArrowRight');
  await expect(modes.getByRole('radio', { name: 'Stacked' })).toBeChecked();
  const stacked = dialog.getByRole('list', { name: 'Stacked' }).locator('.sample');
  const tops = await stacked.evaluateAll((els) => els.map((e) => e.getBoundingClientRect().top));
  expect(tops[0]!).toBeLessThan(tops[1]!);
  expect(tops[1]!).toBeLessThan(tops[2]!);

  // Overlay: two faces on one baseline at 50% opacity; three pinned, so two pickers.
  await page.keyboard.press('ArrowRight');
  await expect(modes.getByRole('radio', { name: 'Overlay' })).toBeChecked();
  const overlay = dialog.getByRole('group', { name: 'Overlay' });
  const texts = overlay.locator('text');
  await expect(texts).toHaveCount(2);
  for (const t of await texts.all()) await expect(t).toHaveCSS('fill-opacity', '0.5');
  const fills = await texts.evaluateAll((els) => els.map((e) => getComputedStyle(e).fill));
  expect(fills[0]).not.toBe(fills[1]);
  await expect(overlay.getByRole('combobox')).toHaveCount(2);
  await selectOption(overlay.getByRole('combobox', { name: 'Second font' }), { index: 2 });
  await expect(overlay.getByRole('list', { name: 'Legend' })).toContainText('Legacy Grotesk');
});

test('focus stays in the sheet; Escape returns to the list with the selection kept', async ({
  page,
}) => {
  const tree = await open(page);
  await pinAndNext(page, /\(1 of 4\)/);
  await pinAndNext(page, /\(2 of 4\)/);
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
  await expect(
    dialog.getByRole('list', { name: 'Side by side' }).getByRole('listitem'),
  ).toHaveCount(2);
});

test('the sheet rises from the bottom, drags to full height and closes back to the row', async ({
  page,
}) => {
  const tree = await open(page);
  await pinAndNext(page, /^Inter .*\(1 of 4\)/);
  await pinAndNext(page, /\(2 of 4\)/);
  await pinAndNext(page, /\(3 of 4\)/);
  await pinAndNext(page, /\(4 of 4\)/);
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
