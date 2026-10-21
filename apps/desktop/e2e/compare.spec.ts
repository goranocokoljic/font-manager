import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

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
  await expect(compareButton(page)).toHaveText('Compare 4/4');

  // The fifth is refused with a message.
  await tree.getByRole('treeitem').nth(4).click();
  await page.keyboard.press('c');
  await expect(toast(page)).toContainText('Compare holds 4 fonts. Remove one to pin Sample 2');
  await expect(compareButton(page)).toHaveText('Compare 4/4');

  // Remove one in the sheet; the fifth then fits.
  await compareButton(page).click();
  await expect(sheet(page).getByRole('listitem')).toHaveCount(4);
  await sheet(page)
    .getByRole('button', { name: /^Remove JetBrains Mono .* from Compare$/ })
    .click();
  await expect(sheet(page).getByRole('listitem')).toHaveCount(3);
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
  expect(boxes.map((b) => b.size)).toEqual(['28px', '28px', '28px']);
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
  await overlay.getByRole('combobox', { name: 'Second font' }).selectOption({ index: 2 });
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
  await expect(compareButton(page)).toHaveText('Compare 2/4');

  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  // Back in the list (opened from the palette, the focus goes to the list column).
  expect(await page.getByRole('main').evaluate((m) => m.contains(document.activeElement))).toBe(
    true,
  );
  await expect(selected).toHaveAttribute('data-key', before!);
  expect(page.url()).toBe(url);

  // The pins stay for the session: reopening shows them again.
  await compareButton(page).click();
  await expect(dialog.getByRole('listitem')).toHaveCount(2);
});
