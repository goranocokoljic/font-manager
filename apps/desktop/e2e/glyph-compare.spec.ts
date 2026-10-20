import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';

// A fixture with Cyrillic, so the webview draws Ж from a real font.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-static/robotoslab/RobotoSlab-ExtraBold.ttf',
      import.meta.url,
    ),
  ),
);

/** MockApi face 1 (Inter Regular), the Glyphs tab, Ж selected; resolves to the Compare button. */
async function openDetail(page: Page) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/#/?face=1');
  await page.getByRole('tab', { name: 'Glyphs' }).click();
  const panel = page.getByRole('tabpanel', { name: 'Glyphs' });
  await panel.getByRole('searchbox', { name: 'Search glyphs' }).fill('U+0416');
  const detail = panel.getByRole('region', { name: 'Selected glyph' });
  await expect(detail.getByLabel('Code point', { exact: true })).toHaveText('U+0416');
  return detail.getByRole('button', { name: 'Compare with…' });
}

test('keyboard only: pick a font by name, change the size, swap the font, close', async ({
  page,
}) => {
  const button = await openDetail(page);
  await button.focus();
  await page.keyboard.press('Enter');
  const dialog = page.getByRole('dialog', { name: 'Compare U+0416' });
  await expect(dialog).toBeVisible();
  const picker = dialog.getByRole('combobox', { name: 'Compare with font' });
  await expect(picker).toBeFocused();

  // Search by name; Enter picks the first match.
  await page.keyboard.type('JetBrains');
  await expect(dialog.getByRole('option').first()).toContainText('JetBrains Mono');
  await page.keyboard.press('Enter');
  const legend = dialog.getByRole('list', { name: 'Legend' });
  await expect(legend).toContainText('Inter Regular');
  await expect(legend).toContainText('JetBrains Mono Regular');
  await expect(legend).toContainText('x-height 550, cap height 730');
  const glyphs = dialog.locator('text.glyph');
  await expect(glyphs).toHaveCount(2);
  for (const g of await glyphs.all()) {
    await expect(g).toHaveText('Ж');
    await expect(g).toHaveCSS('fill-opacity', '0.5');
  }
  await expect(glyphs.nth(1)).toHaveCSS('font-family', /fm-face-6/);
  const fills = await glyphs.evaluateAll((els) => els.map((e) => getComputedStyle(e).fill));
  expect(fills[0]).not.toBe(fills[1]);
  const ys = await glyphs.evaluateAll((els) => els.map((e) => e.getAttribute('y')));
  expect(ys[0]).toBe(ys[1]);

  // Tab reaches the style picker, then the size: arrow keys change it.
  await page.keyboard.press('Tab');
  await expect(dialog.getByRole('combobox', { name: 'Style' })).toBeFocused();
  await page.keyboard.press('Tab');
  const size = dialog.getByRole('slider', { name: 'Size' });
  await expect(size).toBeFocused();
  await page.keyboard.press('ArrowRight');
  await expect(dialog.getByRole('spinbutton', { name: 'Size in pixels' })).toHaveValue('208');
  await expect(glyphs.first()).toHaveAttribute('font-size', '208');

  // Swap the font without closing: Legacy Grotesk has no Cyrillic.
  await picker.focus();
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.type('Legacy');
  await expect(dialog.getByRole('option').first()).toContainText('Legacy Grotesk');
  await page.keyboard.press('Enter');
  await expect(dialog.getByRole('status')).toHaveText(
    'Legacy Grotesk Regular has no glyph for U+0416.',
  );
  await expect(glyphs).toHaveCount(1);
  await expect(glyphs.first()).toHaveAttribute('font-size', '208');

  // Tab stays in the dialog; Escape closes it and returns to the button.
  for (let i = 0; i < 8; i++) {
    await page.keyboard.press('Tab');
    expect(await dialog.evaluate((d) => d.contains(document.activeElement))).toBe(true);
  }
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  await expect(page.getByRole('button', { name: 'Compare with…' })).toBeFocused();
  // The glyph detail stays open.
  await expect(page.getByRole('region', { name: 'Selected glyph' })).toBeVisible();
});

/** WCAG contrast ratio of the field's border against the page background. */
const borderContrast = (scope: Locator, name: string) =>
  scope.getByLabel(name, { exact: true }).evaluate((el) => {
    const rgb = (c: string) => (c.match(/\d+(\.\d+)?/g) ?? []).slice(0, 3).map(Number);
    const lum = (c: string) => {
      const [r, g, b] = rgb(c).map((v) => {
        const s = v / 255;
        return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
      });
      return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    };
    const a = lum(getComputedStyle(el).borderTopColor);
    const b = lum(getComputedStyle(document.body).backgroundColor);
    return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
  });

for (const scheme of ['light', 'dark'] as const) {
  test(`the compare fields are outlined at 3:1 or more (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await (await openDetail(page)).click();
    const dialog = page.getByRole('dialog', { name: 'Compare U+0416' });
    await dialog.getByRole('combobox', { name: 'Compare with font' }).fill('JetBrains');
    await expect(dialog.getByRole('option').first()).toContainText('JetBrains Mono');
    await page.keyboard.press('Enter');
    await expect(dialog.getByRole('combobox', { name: 'Style' })).toBeVisible();
    for (const name of ['Compare with font', 'Style', 'Size in pixels'])
      expect(await borderContrast(dialog, name)).toBeGreaterThanOrEqual(3);
  });
}

/**
 * The strongest WCAG contrast any rendered pixel of each guide line reaches against
 * the page background: the drawing's lines are rasterized at the page's pixel ratio
 * with their computed colors, so anti-aliasing is measured, not assumed.
 */
const guideContrast = (dialog: Locator) =>
  dialog.locator('svg.drawing').evaluate(async (svg) => {
    const rgb = (c: string) => (c.match(/\d+(\.\d+)?/g) ?? []).slice(0, 3).map(Number);
    const lum = ([r, g, b]: number[]) => {
      const f = (v: number) => {
        const s = v / 255;
        return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
      };
      return 0.2126 * f(r!) + 0.7152 * f(g!) + 0.0722 * f(b!);
    };
    const ratio = (a: number, b: number) => (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
    const bg = getComputedStyle(document.body).backgroundColor;
    const w = Number(svg.getAttribute('width'));
    const h = Number(svg.getAttribute('height'));
    const dpr = window.devicePixelRatio;
    const out: Record<string, number> = {};
    for (const line of svg.querySelectorAll<SVGLineElement>('line.guide')) {
      const cs = getComputedStyle(line);
      const copy = `<line x1="0" x2="${w}" y1="${line.getAttribute('y1')}" y2="${line.getAttribute('y2')}" stroke="${cs.stroke}" stroke-width="${cs.strokeWidth}" stroke-dasharray="${cs.strokeDasharray === 'none' ? '' : cs.strokeDasharray}"/>`;
      const src = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}">${copy}</svg>`;
      const img = new Image();
      img.src = `data:image/svg+xml,${encodeURIComponent(src)}`;
      await img.decode();
      const canvas = document.createElement('canvas');
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      const ctx = canvas.getContext('2d')!;
      ctx.fillStyle = bg;
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
      const data = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
      const back = lum(rgb(bg));
      let best = 1;
      for (let i = 0; i < data.length; i += 4)
        best = Math.max(best, ratio(lum([data[i]!, data[i + 1]!, data[i + 2]!]), back));
      out[line.dataset.line ?? '?'] = best;
    }
    return out;
  });

for (const scheme of ['light', 'dark'] as const) {
  test(`the metric guide lines of both fonts reach 3:1 against the background (${scheme})`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await (await openDetail(page)).click();
    const dialog = page.getByRole('dialog', { name: 'Compare U+0416' });
    await dialog.getByRole('combobox', { name: 'Compare with font' }).fill('JetBrains');
    await expect(dialog.getByRole('option').first()).toContainText('JetBrains Mono');
    await page.keyboard.press('Enter');
    await expect(dialog.locator('text.glyph')).toHaveCount(2);
    const contrast = await guideContrast(dialog);
    expect(Object.keys(contrast).sort()).toEqual([
      'capHeight-0',
      'capHeight-1',
      'xHeight-0',
      'xHeight-1',
    ]);
    for (const [line, value] of Object.entries(contrast))
      expect(value, `${line} contrast`).toBeGreaterThanOrEqual(3);
  });
}
