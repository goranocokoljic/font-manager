// E16.1: the metrics overlay, the outline mode and the tracking slider, in every view.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

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
  await page.goto('/#/');
  await expect(page.locator('[data-face="1"]')).toHaveAttribute('data-state', 'ready');
}

const modeButton = (page: Page, name: string) =>
  page.getByRole('group', { name: 'View mode' }).getByRole('button', { name });

const controlsOf = (page: Page) => page.getByRole('group', { name: 'Preview settings' });

/** Distance of each metrics line of `face`'s list row from the top of its sample. */
async function metricLines(page: Page, face: number): Promise<Record<string, number>> {
  return page
    .locator(`[role="treeitem"] [data-face="${face}"] .sample`)
    .evaluate((sample) =>
      Object.fromEntries(
        [...sample.querySelectorAll<HTMLElement>('[data-line]')].map((l) => [
          l.dataset.line,
          l.getBoundingClientRect().top - sample.getBoundingClientRect().top,
        ]),
      ),
    );
}

async function openTableStrip(page: Page) {
  await modeButton(page, 'Table').click();
  await page
    .getByRole('treegrid', { name: 'Font table' })
    .getByRole('row', { name: 'JetBrains Mono, 1 style' })
    .click();
  const sample = page.getByRole('region', { name: 'Preview strip' }).locator('span.sample');
  await expect(sample).toBeVisible();
  return sample;
}

const fill = (el: Element) => getComputedStyle(el).getPropertyValue('-webkit-text-fill-color');
const stroke = (el: Element) => getComputedStyle(el).getPropertyValue('-webkit-text-stroke-width');

test('the metrics toggle draws baseline, x-height and cap height that follow size and line height', async ({
  page,
}) => {
  await open(page);
  const controls = controlsOf(page);
  const metrics = controls.getByRole('checkbox', { name: 'Metrics' });
  await expect(metrics).not.toBeChecked();
  await expect(page.locator('[data-line]')).toHaveCount(0);

  // Keyboard: Space on the focused, labelled checkbox turns it on.
  await metrics.focus();
  await page.keyboard.press('Space');
  await expect(metrics).toBeChecked();
  const rows = page.getByRole('tree', { name: 'Font list' }).getByRole('treeitem');
  const shown = await rows.count();
  expect(shown).toBeGreaterThan(1);
  await expect(page.locator('[role="treeitem"] [data-testid="metrics"]')).toHaveCount(shown);

  // Mock face 1: x-height 520 and cap height 720 per 1000 units; face 7 declares no x-height.
  await expect(page.locator('[data-face="1"] [data-line="xHeight"]')).toHaveCount(1);
  await expect(page.locator('[data-face="7"] [data-line="xHeight"]')).toHaveCount(0);
  await expect(page.locator('[data-face="7"] [data-line="capHeight"]')).toHaveCount(1);
  let at = await metricLines(page, 1);
  expect(at.baseline! - at.xHeight!).toBeCloseTo(0.52 * 28, 0);
  expect(at.baseline! - at.capHeight!).toBeCloseTo(0.72 * 28, 0);
  // The baseline lies inside the sample's line.
  expect(at.baseline!).toBeGreaterThan(0);
  expect(at.baseline!).toBeLessThan(28 * 1.2);

  await controls.getByLabel('Size in pixels').fill('56');
  await expect.poll(async () => (await metricLines(page, 1)).baseline).toBeGreaterThan(28);
  at = await metricLines(page, 1);
  expect(at.baseline! - at.xHeight!).toBeCloseTo(0.52 * 56, 0);
  expect(at.baseline! - at.capHeight!).toBeCloseTo(0.72 * 56, 0);

  // A taller line moves the baseline down by half the added leading; the gaps stay.
  const before = at.baseline!;
  await controls.getByLabel('Line height').fill('2');
  // Whole-pixel line boxes: within a pixel.
  await expect
    .poll(async () => Math.abs((await metricLines(page, 1)).baseline! - before - (56 * 0.8) / 2))
    .toBeLessThan(1.01);
  at = await metricLines(page, 1);
  expect(at.baseline! - at.xHeight!).toBeCloseTo(0.52 * 56, 0);

  // Persists for the view across a reload; the list stays virtualized.
  await page.reload();
  await expect(controls.getByRole('checkbox', { name: 'Metrics' })).toBeChecked();
  await expect(page.locator('[data-face="1"] [data-line="baseline"]')).toHaveCount(1);
  const total = Number(await rows.first().getAttribute('aria-setsize'));
  expect(await rows.count()).toBeLessThanOrEqual(total);

  await controls.getByRole('checkbox', { name: 'Metrics' }).uncheck();
  await expect(page.locator('[data-line]')).toHaveCount(0);
});

test('the outline toggle draws outlines in every view mode and turns off again', async ({
  page,
}) => {
  await open(page);
  const controls = controlsOf(page);
  const outline = controls.getByRole('checkbox', { name: 'Outline' });
  const sample = page.locator('[role="treeitem"] [data-face="1"] .sample');
  const solid = await sample.evaluate(fill);
  expect(solid).not.toBe('rgba(0, 0, 0, 0)');

  await outline.focus();
  await page.keyboard.press('Space');
  await expect(outline).toBeChecked();
  await expect.poll(() => sample.evaluate(fill)).toBe('rgba(0, 0, 0, 0)');
  expect(await sample.evaluate(stroke)).toBe('1px');

  await modeButton(page, 'Grid').click();
  const card = page
    .getByRole('listbox', { name: 'Font grid' })
    .getByTestId('card-live-text')
    .first();
  await expect(card).toBeVisible();
  expect(await card.evaluate(fill)).toBe('rgba(0, 0, 0, 0)');
  expect(await card.evaluate(stroke)).toBe('1px');

  await modeButton(page, 'Waterfall').click();
  const line = page.getByTestId('waterfall-line').first().locator('.sample');
  await expect(line).toBeVisible();
  expect(await line.evaluate(fill)).toBe('rgba(0, 0, 0, 0)');

  const strip = await openTableStrip(page);
  expect(await strip.evaluate(fill)).toBe('rgba(0, 0, 0, 0)');

  await controls.getByRole('checkbox', { name: 'Outline' }).uncheck();
  await expect.poll(() => strip.evaluate(fill)).toBe(solid);
  await modeButton(page, 'Grid').click();
  await expect(
    page.getByRole('listbox', { name: 'Font grid' }).getByTestId('thumbnail').first(),
  ).toBeVisible();
  await expect(page.getByTestId('card-live-text')).toHaveCount(0);
});

test('the tracking slider applies in every view mode and in the preview strip', async ({
  page,
}) => {
  await open(page);
  const controls = controlsOf(page);
  const slider = controls.getByRole('slider', { name: 'Tracking slider' });
  await expect(slider).toHaveAttribute('min', '-0.2');
  await expect(slider).toHaveAttribute('max', '1');
  await slider.fill('0.5');
  await expect(controls.getByLabel('Tracking in em')).toHaveValue('0.5');
  await expect(page.locator('[role="treeitem"] [data-face="1"] .sample')).toHaveCSS(
    'letter-spacing',
    '14px',
  );

  await modeButton(page, 'Grid').click();
  // Cards show live text at the 36 px thumbnail size while tracking is on.
  await expect(
    page.getByRole('listbox', { name: 'Font grid' }).getByTestId('card-live-text').first(),
  ).toHaveCSS('letter-spacing', '18px');

  await modeButton(page, 'Waterfall').click();
  const line = page.getByTestId('waterfall-line').first();
  const size = Number(await line.getAttribute('data-size'));
  await expect(line.locator('.sample')).toHaveCSS('letter-spacing', `${size * 0.5}px`);

  const strip = await openTableStrip(page);
  await expect(strip).toHaveCSS('letter-spacing', '14px');

  // Keyboard: an arrow on the slider steps it by 0.01 em.
  await slider.focus();
  await page.keyboard.press('ArrowLeft');
  await expect(controls.getByLabel('Tracking in em')).toHaveValue('0.49');
  await expect(strip).toHaveCSS('letter-spacing', '13.72px');
});
