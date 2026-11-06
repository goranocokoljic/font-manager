import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { axesRow, faceHash, hero } from './family-page';
import { selectOption } from './select';

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

// MockApi face 1 (Inter Regular): one axis, wght 100–900, default 400; named
// instances Thin, Regular, Bold, Black. Face 6 is static.
async function openAxes(page: Page, face = 1) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(`/${faceHash(face)}`);
  return axesRow(page);
}

const wghtSlider = (panel: ReturnType<Page['getByRole']>) =>
  panel.getByRole('slider', { name: /Weight/ });
const wghtField = (panel: ReturnType<Page['getByRole']>) =>
  panel.getByRole('textbox', { name: 'Weight (wght) value' });

test('the axis row sits under the hero and shows each axis with its range and default', async ({
  page,
}) => {
  const panel = await openAxes(page);
  const under = await hero(page).evaluate(
    (el, row) => el.compareDocumentPosition(row!) & Node.DOCUMENT_POSITION_FOLLOWING,
    await panel.elementHandle(),
  );
  expect(under).toBeTruthy();
  const slider = wghtSlider(panel);
  await expect(slider).toHaveAttribute('min', '100');
  await expect(slider).toHaveAttribute('max', '900');
  await expect(slider).toHaveValue('400');
  await expect(wghtField(panel)).toHaveValue('400');
  await expect(panel).toContainText('100 to 900, default 400');
});

test('a static face says it has no variation axes', async ({ page }) => {
  const panel = await openAxes(page, 6);
  await expect(panel).toContainText('No variable axes');
  await expect(panel.getByRole('slider')).toHaveCount(0);
});

test("moving the wght slider changes the hero and the row's font-variation-settings", async ({
  page,
}) => {
  const panel = await openAxes(page);
  const sample = hero(page);
  await expect(sample).toHaveCSS('font-variation-settings', '"wght" 400');
  await wghtSlider(panel).fill('700');
  await expect(sample).toHaveCSS('font-variation-settings', '"wght" 700');
  // Another face is untouched.
  await page.evaluate(() => (location.hash = '#/family/3?face=6'));
  await expect(page.getByTestId('family-panel')).toContainText('Regular');
  await expect(sample).toHaveCSS('font-variation-settings', 'normal');
  await page.evaluate(() => (location.hash = '#/family/1?face=1'));
  await expect(sample).toHaveCSS('font-variation-settings', '"wght" 700');
  // The list's row follows.
  await page.getByRole('button', { name: 'Library' }).click();
  const row = page.locator('[data-face="1"]').first();
  await expect(row.locator('.sample')).toHaveCSS('font-variation-settings', '"wght" 700');
  await expect(page.locator('[data-face="6"] .sample').first()).toHaveCSS(
    'font-variation-settings',
    'normal',
  );
  await page.evaluate(() => (location.hash = '#/family/1?face=1'));
  await panel.getByRole('button', { name: 'Reset' }).click();
  await expect(sample).toHaveCSS('font-variation-settings', '"wght" 400');
});

test('the preview strip follows the axes of its face', async ({ page }) => {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/#/');
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Table' })
    .click();
  await page
    .getByRole('treegrid', { name: 'Font table' })
    .getByRole('row', { name: /^Inter,/ })
    .click();
  const sample = page
    .getByRole('region', { name: 'Preview strip' })
    .locator('[data-face="1"] .sample');
  await expect(sample).toHaveCSS('font-variation-settings', '"wght" 400');
  await page.keyboard.press('o');
  await wghtSlider(axesRow(page)).fill('250');
  await page.getByRole('button', { name: 'Library' }).click();
  await expect(sample).toHaveCSS('font-variation-settings', '"wght" 250');
});

test('a typed value is clamped; a non-number is rejected without changing the preview', async ({
  page,
}) => {
  const panel = await openAxes(page);
  const sample = hero(page);
  const field = wghtField(panel);
  await field.fill('1200');
  await field.press('Enter');
  await expect(field).toHaveValue('900');
  await expect(sample).toHaveCSS('font-variation-settings', '"wght" 900');
  await field.fill('heavy');
  await field.press('Enter');
  await expect(field).toHaveAttribute('aria-invalid', 'true');
  await expect(panel.getByRole('alert')).toContainText('type a number');
  await expect(sample).toHaveCSS('font-variation-settings', '"wght" 900');
});

test('choosing a named instance sets the slider to its coordinates', async ({ page }) => {
  const panel = await openAxes(page);
  const presets = panel.getByRole('group', { name: 'Named instances' }).getByRole('button');
  await expect(presets).toHaveText(['Thin', 'Regular', 'Bold', 'Black']);
  await panel.getByRole('button', { name: 'Bold', exact: true }).click();
  await expect(wghtSlider(panel)).toHaveValue('700');
  await expect(panel.getByRole('button', { name: 'Bold', exact: true })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(hero(page)).toHaveCSS('font-variation-settings', '"wght" 700');
});

test('"Copy CSS" copies font-variation-settings for the moved axes, as the preview applies them', async ({
  page,
  context,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  const panel = await openAxes(page);
  const sample = hero(page);
  const copy = panel.getByRole('button', { name: 'Copy CSS for the axes' });
  const status = panel.getByRole('status').filter({ hasText: /Copied|Could not copy/ });
  const clipboard = () => page.evaluate(() => navigator.clipboard.readText());

  // Nothing moved yet: `normal`.
  await copy.click();
  await expect(status).toHaveText('Copied');
  expect(await clipboard()).toBe('font-variation-settings: normal;');

  await wghtField(panel).fill('650');
  await wghtField(panel).press('Enter');
  await expect(sample).toHaveCSS('font-variation-settings', '"wght" 650');
  await copy.click();
  await expect.poll(clipboard).toBe('font-variation-settings: "wght" 650;');
  // The copied value, as the browser reads it, is what the row applies.
  const copied = (await clipboard()).replace(/^font-variation-settings: (.*);$/, '$1');
  const [applied, parsed] = await sample.evaluate((el, value) => {
    const probe = document.createElement('span');
    probe.style.fontVariationSettings = value;
    return [(el as HTMLElement).style.fontVariationSettings, probe.style.fontVariationSettings];
  }, copied);
  expect(parsed).toBe(applied);
});

test('"Copy CSS" says so when the clipboard refuses', async ({ page }) => {
  const panel = await openAxes(page);
  await page.evaluate(() => {
    navigator.clipboard.writeText = () => Promise.reject(new Error('refused'));
  });
  await panel.getByRole('button', { name: 'Copy CSS for the axes' }).click();
  await expect(panel.getByRole('status').filter({ hasText: 'Could not copy' })).toBeVisible();
});

test('"Copy CSS" is keyboard-reachable after the last axis control', async ({ page }) => {
  const panel = await openAxes(page);
  const copy = panel.getByRole('button', { name: 'Copy CSS for the axes' });
  const pin = panel.getByRole('button', { name: 'Pin 400 as my instance' });
  await pin.focus();
  await page.keyboard.press('Tab');
  await expect(copy).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await expect(pin).toBeFocused();
  await page.keyboard.press('Tab');
  await page.keyboard.press('Enter');
  await expect(
    panel.getByRole('status').filter({ hasText: /Copied|Could not copy/ }),
  ).toBeVisible();
});

test('sliders, fields, presets and the pin button are keyboard-reachable with labels', async ({
  page,
}) => {
  const panel = await openAxes(page);
  const slider = wghtSlider(panel);
  await slider.focus();
  await page.keyboard.press('ArrowRight');
  await expect(slider).toHaveValue('401');
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.press('ArrowLeft');
  await expect(slider).toHaveValue('399');
  await page.keyboard.press('Tab');
  await expect(wghtField(panel)).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(panel.getByRole('button', { name: 'Thin' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(slider).toHaveValue('100');
});

test('"Pin … as my instance" saves a virtual style that shows as its own row in the collection', async ({
  page,
}) => {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/#/');
  // A manual collection to pin into.
  const sidebar = page.getByRole('region', { name: 'Collections' });
  await sidebar.getByRole('button', { name: 'New collection', exact: true }).click();
  const input = sidebar.getByRole('textbox', { name: 'New collection name' });
  await input.fill('Brand');
  await input.press('Enter');
  await expect(sidebar.getByRole('listitem', { name: 'Brand', exact: true })).toBeVisible();

  const tree = page.getByRole('tree');
  await tree.getByRole('treeitem', { name: /^Inter,/ }).click();
  await page.keyboard.press('o');
  const panel = axesRow(page);
  await wghtSlider(panel).fill('620');
  await panel.getByRole('textbox', { name: 'Style name' }).fill('Semibold Pin');
  await selectOption(panel.getByRole('combobox', { name: 'Save in' }), { label: 'Brand' });
  const pin = panel.getByRole('button', { name: 'Pin 620 as my instance' });
  await pin.focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Pinned “Semibold Pin” to Brand.')).toBeVisible();
  await page.getByRole('button', { name: 'Library' }).click();

  // Open the collection: Inter is listed for its pin; its expanded rows show only the pin.
  await sidebar
    .getByRole('listitem', { name: 'Brand', exact: true })
    .getByRole('button', { name: 'Brand', exact: true })
    .click();
  await tree.getByRole('treeitem', { name: /^Inter,/ }).click();
  await page.keyboard.press('ArrowRight');
  const style = tree.getByRole('treeitem', { name: /Semibold Pin, pinned instance/ });
  await expect(style).toBeVisible();
  await expect(tree.getByRole('treeitem', { level: 2 })).toHaveCount(1);
  await expect(style.locator('.sample')).toHaveCSS('font-variation-settings', '"wght" 620');
  // Moving the face on its family page does not move the pinned row.
  await tree.getByRole('treeitem', { name: /^Inter,/ }).click();
  await page.keyboard.press('o');
  await panel.getByRole('button', { name: 'Reset' }).click();
  await page.getByRole('button', { name: 'Library' }).click();
  await expect(style.locator('.sample')).toHaveCSS('font-variation-settings', '"wght" 620');
});

for (const colorScheme of ['light', 'dark'] as const) {
  test(`axis field outlines are at least 3:1 in the ${colorScheme} theme`, async ({ page }) => {
    await page.emulateMedia({ colorScheme });
    const panel = await openAxes(page);
    const ratio = (el: Element) => {
      const rgb = (c: string) => (c.match(/[\d.]+/g) ?? []).slice(0, 3).map(Number);
      const lum = (c: number[]) => {
        const lin = c.map((v) => {
          const s = v / 255;
          return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
        });
        return 0.2126 * lin[0]! + 0.7152 * lin[1]! + 0.0722 * lin[2]!;
      };
      const [a, b] = [
        lum(rgb(getComputedStyle(el).borderTopColor)),
        lum(rgb(getComputedStyle(document.body).backgroundColor)),
      ].sort((x, y) => y - x);
      return (a! + 0.05) / (b! + 0.05);
    };
    expect(await wghtField(panel).evaluate(ratio)).toBeGreaterThanOrEqual(3);
  });
}

// Screenshots of the Axes tab for UI reviews: FM_SCREENS_DIR=<dir> pnpm test:e2e --grep @screens
test.describe('@screens axes tab', () => {
  const dir = process.env.FM_SCREENS_DIR;
  test.skip(!dir, 'FM_SCREENS_DIR is not set');
  for (const scheme of ['light', 'dark'] as const) {
    for (const vp of [
      { width: 1280, height: 800 },
      { width: 1000, height: 700 },
    ]) {
      test(`axes-${scheme}-${vp.width}x${vp.height}`, async ({ page }) => {
        await page.setViewportSize(vp);
        await page.emulateMedia({ colorScheme: scheme });
        const panel = await openAxes(page);
        await wghtSlider(panel).fill('700');
        await expect(hero(page)).toHaveCSS('font-variation-settings', '"wght" 700');
        await page.screenshot({
          path: `${dir}/axes-${scheme}-${vp.width}x${vp.height}.png`,
          fullPage: true,
          animations: 'disabled',
        });
      });
    }
  }
});
