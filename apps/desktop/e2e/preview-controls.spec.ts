import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { optionLabels, selectOption, showPreviewOptions } from './select';

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

/** A theme token as a computed colour (`--fm-fg` → `rgb(…)`), from a probe in the page. */
const themeColor = (page: Page, token: string) =>
  page.evaluate((t) => {
    const probe = document.createElement('span');
    probe.style.color = `var(${t})`;
    document.body.append(probe);
    const c = getComputedStyle(probe).color;
    probe.remove();
    return c;
  }, token);

async function open(page: Page) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/#/');
  await expect(page.locator('[data-face="1"]')).toHaveAttribute('data-state', 'ready');
  await showPreviewOptions(page);
}

test('preview controls apply to all rows live and persist after a reload', async ({ page }) => {
  await open(page);
  const controls = page.getByRole('group', { name: 'Preview settings' });
  const samples = page.locator('[role="treeitem"] .sample');
  const count = await samples.count();
  expect(count).toBeGreaterThan(1);

  await selectOption(controls.getByLabel('Sample'), 'pangram');
  await selectOption(controls.getByLabel('Language'), 'de');
  // A press on the row outside the popover closes it.
  await showPreviewOptions(page);
  await controls.getByLabel('Size in pixels').fill('72');
  await controls.getByRole('textbox', { name: 'Line height', exact: true }).fill('1.5');
  await controls.getByLabel('Tracking in em').fill('0.1');
  await controls.getByRole('button', { name: 'Center' }).click();
  // E54.8: the popover has no colour or surface controls; the rows take the theme's colours.
  await expect(controls.getByLabel(/colou?r|Surface|Background/)).toHaveCount(0);
  const fg = await themeColor(page, '--fm-fg');

  for (let i = 0; i < count; i += 1) {
    const sample = samples.nth(i);
    await expect(sample).toHaveText(/Victor jagt zwölf Boxkämpfer/);
    await expect(sample).toHaveCSS('font-size', '72px');
    await expect(sample).toHaveCSS('line-height', '108px');
    await expect(sample).toHaveCSS('letter-spacing', '7.2px');
    await expect(sample).toHaveCSS('text-align', 'center');
    await expect(sample).toHaveCSS('color', fg);
  }
  const list = page.getByRole('tree', { name: 'Font list' });
  await expect(list).not.toHaveAttribute('data-surface');
  await expect(list).not.toHaveClass(/surface-/);

  await page.reload();
  await expect(page.locator('[data-face="1"] .sample')).toHaveCSS('font-size', '72px');
  await expect(page.locator('[data-face="1"] .sample')).toHaveText(/Victor jagt/);
  await showPreviewOptions(page);
  await expect(controls.getByRole('textbox', { name: 'Line height', exact: true })).toHaveValue(
    '1.50',
  );

  await controls.getByRole('button', { name: 'Reset' }).click();
  await expect(page.locator('[data-face="1"] .sample')).toHaveCSS('font-size', '28px');
});

test('the font-name and custom samples show per-face text', async ({ page }) => {
  await open(page);
  const controls = page.getByRole('group', { name: 'Preview settings' });
  await selectOption(controls.getByLabel('Sample'), 'fontName');
  await expect(page.locator('[data-face="1"] .sample')).toHaveText('Inter Regular');
  await expect(page.locator('[data-face="6"] .sample')).toHaveText('JetBrains Mono Regular');
  await selectOption(controls.getByLabel('Sample'), 'custom');
  await controls.getByLabel('Type to preview').fill('Hamburgefonstiv');
  await expect(page.locator('[data-face="6"] .sample')).toHaveText('Hamburgefonstiv');
});

test('auto size scales each row by its x-height or cap height, by keyboard, and persists', async ({
  page,
}) => {
  await open(page);
  const controls = page.getByRole('group', { name: 'Preview settings' });
  const autoSize = controls.getByRole('group', { name: 'Auto size' });
  const segment = (name: string) => autoSize.getByRole('button', { name, exact: true });
  const size = (face: number) =>
    page
      .locator(`[data-face="${face}"] .sample`)
      .evaluate((el) => parseFloat(getComputedStyle(el).fontSize));

  await expect(segment('Off')).toHaveAttribute('aria-pressed', 'true');
  // Keyboard-reachable: Tab from the size field lands on it (line height comes after), and
  // Tab and Space choose a segment.
  await controls.getByLabel('Size in pixels').focus();
  await page.keyboard.press('Tab');
  await expect(segment('Off')).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(segment('x-height')).toBeFocused();
  await page.keyboard.press('Space');
  await expect(segment('x-height')).toHaveAttribute('aria-pressed', 'true');
  // Mock faces 1 and 6 have x-heights of 520 and 550 per 1000; face 7 declares none.
  await expect.poll(() => size(1)).toBeCloseTo((28 * 0.5) / 0.52, 1);
  await expect.poll(() => size(6)).toBeCloseTo((28 * 0.5) / 0.55, 1);
  expect(await size(7)).toBe(28);
  // Every scaled face has the same x-height in pixels.
  expect(Math.abs((await size(1)) * 0.52 - (await size(6)) * 0.55)).toBeLessThan(1);

  await segment('Cap height').click();
  await expect.poll(() => size(1)).toBeCloseTo((28 * 0.7) / 0.72, 1);

  await page.reload();
  await showPreviewOptions(page);
  await expect(segment('Cap height')).toHaveAttribute('aria-pressed', 'true');
  await expect.poll(() => size(1)).toBeCloseTo((28 * 0.7) / 0.72, 1);

  await segment('Off').click();
  await expect.poll(() => size(1)).toBe(28);
});

test('the pangram picker lists the API languages and shows Serbian (Cyrillic)', async ({
  page,
}) => {
  await open(page);
  const controls = page.getByRole('group', { name: 'Preview settings' });
  await selectOption(controls.getByLabel('Sample'), 'pangram');
  const language = controls.getByLabel('Language');
  await expect(language).toHaveAttribute('data-value', 'en');
  const names = await optionLabels(language);
  expect(names).toContain('Serbian (Cyrillic)');
  // A preset without a pangram is listed, but it cannot be chosen.
  await language.click();
  await expect(page.getByRole('option', { name: 'Maltese', exact: true })).toHaveAttribute(
    'aria-disabled',
    'true',
  );
  await language.press('Escape');
  await selectOption(language, { label: 'Serbian (Cyrillic)' });
  await expect(page.locator('[data-face="1"] .sample')).toHaveText(
    'Љубазни фењерџија чађавог лица хоће да ми покаже штос',
  );
  await page.reload();
  await expect(controls.getByLabel('Language')).toHaveAttribute('data-value', 'sr-Cyrl');
  await expect(page.locator('[data-face="1"] .sample')).toHaveText(/фењерџија/);
});

test('Preview options is keyboard-complete: every control by Tab and keys, Escape back', async ({
  page,
}) => {
  await open(page);
  const toggle = page
    .getByRole('group', { name: 'Preview settings' })
    .getByRole('button', { name: 'Preview options' });
  const options = page.getByRole('group', { name: 'Preview options' });
  // `open` left it open: close it with Escape from inside, then open it by keyboard.
  await options.getByLabel('Size in pixels').focus();
  await page.keyboard.press('Escape');
  await expect(options).toBeHidden();
  await expect(toggle).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(options).toBeVisible();

  // Tab walks into the popover in the board's order and reaches every control.
  const stops: string[] = [];
  for (let i = 0; i < 20; i += 1) {
    await page.keyboard.press('Tab');
    const name = await page.evaluate(() => {
      const el = document.activeElement as HTMLElement | null;
      if (!el?.closest('[data-testid="preview-options"]')) return null;
      const by = el.getAttribute('aria-labelledby');
      const label = el.id ? document.querySelector(`label[for="${el.id}"]`) : null;
      return (
        el.getAttribute('aria-label') ??
        (by ? document.getElementById(by)?.textContent : label?.textContent) ??
        el.textContent ??
        ''
      ).trim();
    });
    if (name === null) break;
    stops.push(name);
  }
  expect(stops).toEqual([
    'Reset',
    'Sample',
    'Start',
    'Center',
    'End',
    'Size slider',
    'Size in pixels',
    'Off',
    'x-height',
    'Cap height',
    'Line height slider',
    'Line height',
    'Tracking slider',
    'Tracking in em',
    'Metric lines',
    'Outlines',
  ]);

  // Keys work on what the focus is on: a slider moves, a switch toggles.
  const tracking = options.getByRole('slider', { name: 'Tracking slider' });
  await tracking.focus();
  await page.keyboard.press('ArrowRight');
  await expect(options.getByLabel('Tracking in em', { exact: true })).toHaveValue('0.01');
  const outlines = options.getByRole('switch', { name: 'Outlines' });
  await outlines.focus();
  await page.keyboard.press('Space');
  await expect(outlines).toBeChecked();
  await page.keyboard.press('Escape');
  await expect(options).toBeHidden();
  await expect(toggle).toBeFocused();
});

test('Line height fades on the wall for Word and says why; the popover keeps its height', async ({
  page,
}) => {
  await open(page);
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Grid' })
    .click();
  const chips = page.getByRole('group', { name: 'Preset' });
  await chips.getByRole('button', { name: 'Pangram' }).click();
  await showPreviewOptions(page);
  const options = page.getByRole('group', { name: 'Preview options' });
  const why = options.getByText('No effect while the preview is a single line.');
  await expect(why).toHaveCount(0);
  const slider = options.getByRole('slider', { name: 'Line height slider' });
  await expect(slider).not.toHaveAttribute('aria-disabled');
  const tall = (await options.boundingBox())!.height;

  await chips.getByRole('button', { name: 'Word' }).click();
  await showPreviewOptions(page);
  await expect(why).toBeVisible();
  await expect(slider).toHaveAttribute('aria-disabled', 'true');
  await expect(slider).toBeVisible();
  const field = options.getByRole('textbox', { name: 'Line height', exact: true });
  await expect(field).toHaveAttribute('aria-disabled', 'true');
  await expect(field).toHaveAttribute(
    'aria-describedby',
    (await slider.getAttribute('aria-describedby'))!,
  );
  // The reason adds one quiet line; nothing is hidden, so the height grows by that line only.
  const short = (await options.boundingBox())!.height;
  expect(Math.abs(short - tall)).toBeLessThan(30);
});
