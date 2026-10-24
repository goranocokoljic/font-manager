import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// E16.3: the MockApi color face (`?mockColor=1`, face 8, "Color Play") has two palettes.
// The webview gets CairoPlay with its CPAL replaced by a version 1 table of two
// palettes of different colors (as in fm-core's palettes tests), so choosing the second
// palette visibly redraws the row.
const cairo = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/cairoplay/CairoPlay[slnt,wght].ttf',
      import.meta.url,
    ),
  ),
);

function withTwoPalettes(font: Buffer): Buffer {
  const cpal = Buffer.alloc(80);
  [1, 5, 2, 10].forEach((v, i) => cpal.writeUInt16BE(v, i * 2)); // version, entries, palettes, records
  cpal.writeUInt32BE(28, 8); // color records
  cpal.writeUInt16BE(0, 12); // first record of palette 0
  cpal.writeUInt16BE(5, 14); // first record of palette 1
  cpal.writeUInt32BE(68, 16); // palette types
  cpal.writeUInt32BE(76, 20); // palette labels
  cpal.writeUInt32BE(0, 24); // no entry labels
  for (let p = 0; p < 2; p++)
    for (let e = 0; e < 5; e++) {
      const at = 28 + (p * 5 + e) * 4;
      // BGRA: palette 0 is reds, palette 1 is greens.
      cpal.set(p === 0 ? [0x20, 0x20, 0xe0, 0xff] : [0x20, 0xd0, 0x20, 0xff], at);
    }
  cpal.writeUInt32BE(1, 68);
  cpal.writeUInt32BE(2, 72);
  cpal.writeUInt16BE(0xffff, 76);
  cpal.writeUInt16BE(0xffff, 78);

  const padded = Buffer.concat([font, Buffer.alloc((4 - (font.length % 4)) % 4)]);
  const tables = padded.readUInt16BE(4);
  for (let i = 0; i < tables; i++) {
    const at = 12 + 16 * i;
    if (padded.toString('latin1', at, at + 4) === 'CPAL') {
      padded.writeUInt32BE(padded.length, at + 8);
      padded.writeUInt32BE(cpal.length, at + 12);
    }
  }
  return Buffer.concat([padded, cpal]);
}

const font = withTwoPalettes(cairo);

async function open(page: Page) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: font }),
  );
  await page.goto('/?mockColor=1#/?face=8');
  await expect(page.getByRole('radiogroup', { name: 'Color palette' })).toBeVisible();
}

/** The sample of Color Play's row in the font list, once its font has loaded. */
async function listSample(page: Page) {
  const row = page.getByRole('tree', { name: 'Font list' }).locator('.row[data-face="8"]');
  await expect(row).toHaveAttribute('data-state', 'ready');
  return row.locator('.sample');
}

const palette = (sample: Awaited<ReturnType<typeof listSample>>) =>
  sample.evaluate((el) => getComputedStyle(el).fontPalette);

test('picking the second palette by keyboard redraws the row in it and the choice stays', async ({
  page,
}) => {
  await open(page);
  const group = page.getByRole('radiogroup', { name: 'Color palette' });
  await expect(group.getByRole('radio')).toHaveCount(2);
  await expect(group.getByRole('radio', { name: 'Daylight' })).toBeChecked();
  await expect(group.getByRole('radio', { name: 'Palette 2' })).not.toBeChecked();
  await expect(group.locator('label').nth(1).getByTestId('palette-swatch')).toHaveCount(5);

  const sample = await listSample(page);
  expect(await palette(sample)).toBe('normal');
  const before = await sample.screenshot();

  // Keyboard: focus the chosen palette, then the arrow key moves to the next one.
  await group.getByRole('radio', { name: 'Daylight' }).focus();
  await page.keyboard.press('ArrowDown');
  await expect(group.getByRole('radio', { name: 'Palette 2' })).toBeChecked();
  await expect(group.getByRole('radio', { name: 'Palette 2' })).toBeFocused();

  await expect.poll(() => palette(sample)).toBe('--fm-palette-8-1');
  // The webview draws the face's COLR glyphs in the other palette.
  await expect.poll(async () => (await sample.screenshot()).equals(before)).toBe(false);

  // ArrowUp goes back.
  await page.keyboard.press('ArrowUp');
  await expect(group.getByRole('radio', { name: 'Daylight' })).toBeChecked();
  await expect.poll(() => palette(sample)).toBe('normal');
  // Back in the default palette, the row looks as it did: the change was the palette.
  await expect.poll(async () => (await sample.screenshot()).equals(before)).toBe(true);
  await page.keyboard.press('ArrowDown');

  // The choice is a view preference: it is there after a reload.
  await page.reload();
  await expect(
    page
      .getByRole('radiogroup', { name: 'Color palette' })
      .getByRole('radio', { name: 'Palette 2' }),
  ).toBeChecked();
  expect(await palette(await listSample(page))).toBe('--fm-palette-8-1');
});

test('the chosen palette applies in every view mode', async ({ page }) => {
  await open(page);
  await page
    .getByRole('radiogroup', { name: 'Color palette' })
    .getByRole('radio', { name: 'Palette 2' })
    .check();
  const modes = page.getByRole('group', { name: 'View mode' });
  const computed = (selector: string) =>
    page
      .locator(selector)
      .first()
      .evaluate((el) => getComputedStyle(el).fontPalette);

  // The table's preview strip shows the family under the cursor.
  await modes.getByRole('button', { name: 'Table' }).click();
  await page.getByRole('row', { name: /^Color Play,/ }).click();
  await expect.poll(() => computed('.row[data-face="8"] .sample')).toBe('--fm-palette-8-1');

  await modes.getByRole('button', { name: 'Grid' }).click();
  await expect(page.getByTestId('card-live-text').first()).toBeVisible();
  await expect.poll(() => computed('[data-testid="card-live-text"]')).toBe('--fm-palette-8-1');

  await modes.getByRole('button', { name: 'Waterfall' }).click();
  await expect.poll(() => computed('.row[data-face="8"] .sample')).toBe('--fm-palette-8-1');

  await modes.getByRole('button', { name: 'Layout' }).click();
  await expect.poll(() => computed('[data-testid="layout-page"]')).toBe('--fm-palette-8-1');

  // Another family keeps its default palette.
  await modes.getByRole('button', { name: 'List' }).click();
  await expect.poll(() => computed('.row[data-face="1"] .sample')).toBe('normal');
});
