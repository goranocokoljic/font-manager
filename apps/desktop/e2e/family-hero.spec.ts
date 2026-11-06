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

// MockApi face 1 (Inter Regular): wght 100–900, default 400; instances Thin, Regular, Bold, Black.
// Its foundry is rsms, its copyright says 2016, its licence is the OFL. Face 6 is static.
async function open(page: Page, face = 1) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(`/${faceHash(face)}`);
  await expect(hero(page)).toBeVisible();
}

const wght = (page: Page) => axesRow(page).getByRole('slider', { name: /Weight/ });
const field = (page: Page) => axesRow(page).getByRole('textbox', { name: 'Weight (wght) value' });

/** Drags on the hero from its middle by `dx` pixels, in steps. */
async function dragHero(page: Page, dx: number, hold?: () => Promise<void>) {
  const box = (await hero(page).boundingBox())!;
  const [x, y] = [box.x + box.width / 2, box.y + box.height / 2];
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + dx, y, { steps: 4 });
  await hold?.();
  await page.mouse.up();
}

test('the hero is the preview text at 132 px on one line, cut with an ellipsis', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1000, height: 700 });
  await open(page);
  const specimen = hero(page);
  await expect(specimen).toHaveCSS('font-size', '132px');
  await expect(specimen).toHaveCSS('white-space', 'nowrap');
  await expect(specimen).toHaveCSS('text-overflow', 'ellipsis');
  await expect(specimen).toHaveCSS('overflow-x', 'hidden');
  // The text does not fit, and the hero stays inside the page's width.
  const [clientWidth, scrollWidth] = await specimen.evaluate((el) => [
    el.clientWidth,
    el.scrollWidth,
  ]);
  expect(scrollWidth).toBeGreaterThan(clientWidth!);
  expect(clientWidth).toBeLessThanOrEqual(1000);
});

test('the meta line tells foundry, year, axes and styles, licence and activation', async ({
  page,
}) => {
  await open(page);
  const meta = page.getByTestId('family-meta');
  await expect(meta).toContainText('rsms · 2016');
  await expect(meta).toContainText('Variable, 1 axis · 4 named styles');
  await expect(meta).toContainText('SIL Open Font License');
  await expect(meta).toContainText('Inactive');
  await page.getByRole('button', { name: 'Activate family' }).click();
  await expect(meta).toContainText('Active');
  await expect(meta).not.toContainText('Inactive');
});

test('a static family counts its styles and has no axes to scrub', async ({ page }) => {
  await open(page, 6);
  await expect(page.getByTestId('family-meta')).toContainText(/\d+ styles?/);
  await expect(page.getByTestId('family-meta')).not.toContainText('Variable');
  await expect(hero(page)).not.toHaveCSS('cursor', 'ew-resize');
  await dragHero(page, 100);
  await expect(hero(page)).toHaveCSS('font-variation-settings', 'normal');
});

test('moving the slider moves the hero, the Styles rows and the rail in the same frame', async ({
  page,
}) => {
  await open(page);
  const seen = await page.evaluate(() => {
    const read = (id: string) =>
      (document.querySelector(`[data-testid="${id}"]`) as HTMLElement).style.fontVariationSettings;
    const slider = document.querySelector<HTMLInputElement>('input[type="range"]')!;
    slider.value = '640';
    slider.dispatchEvent(new Event('input', { bubbles: true }));
    // Vue flushes in a microtask: this reads before the browser could paint another frame.
    return Promise.resolve().then(() => ({
      hero: read('family-specimen'),
      row: read('family-style-sample'),
      rail: read('family-rail-sample'),
    }));
  });
  expect(seen).toEqual({ hero: '"wght" 640', row: '"wght" 640', rail: '"wght" 640' });
});

test('ticks mark the named instances; the label names one within 12 units', async ({ page }) => {
  await open(page);
  const row = axesRow(page);
  const ticks = row.locator('.tick');
  await expect(ticks).toHaveCount(4);
  const lefts = await ticks.evaluateAll((els) =>
    els.map((el) => [el.getAttribute('data-name'), el.getBoundingClientRect().left] as const),
  );
  expect(lefts.map(([name]) => name)).toEqual(['Thin', 'Regular', 'Bold', 'Black']);
  expect(lefts[0]![1]).toBeLessThan(lefts[1]![1]);
  expect(lefts[1]![1]).toBeLessThan(lefts[2]![1]);
  // Regular (400) is 3/8 along the track.
  const track = (await wght(page).boundingBox())!;
  const expected = track.x + 7 + (3 / 8) * (track.width - 14);
  expect(Math.abs(lefts[1]![1] - expected)).toBeLessThanOrEqual(1);

  const label = page.getByTestId('axis-instance');
  await expect(label).toHaveText('= Regular');
  for (const [value, text] of [
    ['412', '= Regular'],
    ['413', 'between instances'],
    ['388', '= Regular'],
    ['387', 'between instances'],
    ['700', '= Bold'],
  ] as const) {
    await wght(page).fill(value);
    await expect(label).toHaveText(text);
    await expect(field(page)).toHaveValue(value);
  }
});

test('dragging the hero scrubs the weight: 1 px is 2 units, Shift halves, with pointer capture', async ({
  page,
}) => {
  await open(page);
  const specimen = hero(page);
  await expect(specimen).toHaveCSS('cursor', 'ew-resize');
  // Chromium's mouse pointer has the id 1.
  await dragHero(page, 50, async () =>
    expect(await specimen.evaluate((el) => el.hasPointerCapture(1))).toBe(true),
  );
  await expect(specimen).toHaveCSS('font-variation-settings', '"wght" 500');
  await expect(wght(page)).toHaveValue('500');

  await page.keyboard.down('Shift');
  await dragHero(page, -50);
  await page.keyboard.up('Shift');
  await expect(specimen).toHaveCSS('font-variation-settings', '"wght" 450');

  // The axis range is the end of a drag.
  await dragHero(page, 2000);
  await expect(wght(page)).toHaveValue('900');
});

test('a press alone does not scrub, and the pointer leaving keeps the capture to the end', async ({
  page,
}) => {
  await open(page);
  await dragHero(page, 0);
  await expect(hero(page)).toHaveCSS('font-variation-settings', '"wght" 400');
  const box = (await hero(page).boundingBox())!;
  await page.mouse.move(box.x + 300, box.y + 20);
  await page.mouse.down();
  // Far under the hero: the capture still sends the moves to it.
  await page.mouse.move(box.x + 310, box.y + 500);
  await expect(hero(page)).toHaveCSS('font-variation-settings', '"wght" 420');
  await page.mouse.up();
  await page.mouse.move(box.x + 400, box.y + 20);
  await expect(hero(page)).toHaveCSS('font-variation-settings', '"wght" 420');
});

test('"Pin 537 as my instance" pins the weight in one click and the style appears in Styles', async ({
  page,
}) => {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/#/');
  const sidebar = page.getByRole('region', { name: 'Collections' });
  await sidebar.getByRole('button', { name: 'New collection', exact: true }).click();
  const input = sidebar.getByRole('textbox', { name: 'New collection name' });
  await input.fill('Brand');
  await input.press('Enter');
  await expect(sidebar.getByRole('listitem', { name: 'Brand', exact: true })).toBeVisible();
  await page.evaluate(() => (location.hash = '#/family/1?face=1'));

  await wght(page).fill('537');
  await selectOption(axesRow(page).getByRole('combobox', { name: 'Save in' }), { label: 'Brand' });
  await axesRow(page).getByRole('button', { name: 'Pin 537 as my instance' }).click();
  await expect(page.getByText('Pinned “Weight 537” to Brand.')).toBeVisible();
  const pinned = page.getByTestId('family-pin');
  await expect(pinned).toHaveCount(1);
  await expect(pinned).toContainText('Weight 537');
  // The pin keeps its coordinates when the axis moves on.
  await wght(page).fill('200');
  await expect(pinned.locator('.family-sample')).toHaveCSS('font-variation-settings', '"wght" 537');
});

test('"Copy CSS" on the row puts font-variation-settings on the clipboard', async ({
  page,
  context,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await open(page);
  await dragHero(page, 100);
  await axesRow(page).getByRole('button', { name: 'Copy CSS for the axes' }).click();
  await expect
    .poll(() => page.evaluate(() => navigator.clipboard.readText()))
    .toBe('font-variation-settings: "wght" 600;');
});
