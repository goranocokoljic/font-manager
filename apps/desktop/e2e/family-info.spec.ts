import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { openDeveloper, openSection } from './family-page';

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

// MockApi face 1 (Inter Regular): 1000 units/em, ascender 900, descender -250, x 520, cap 720.
async function open(page: Page) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  return openSection(page, 1, 'Info');
}

test('Info shows Design, Licence, Files and Metrics in a two-column grid', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  const panel = await open(page);
  const groups = panel.getByTestId('family-info').locator('> section');
  await expect(groups.locator('h3')).toHaveText(['Design', 'Licence', 'Files', 'Metrics']);
  const box = async (i: number) => (await groups.nth(i).boundingBox())!;
  // Design and Licence side by side, Files and Metrics under them.
  expect(Math.abs((await box(0)).y - (await box(1)).y)).toBeLessThan(1);
  expect((await box(1)).x).toBeGreaterThan((await box(0)).x);
  expect((await box(2)).y).toBeGreaterThan((await box(0)).y);
  expect(Math.abs((await box(2)).x - (await box(0)).x)).toBeLessThan(1);

  const design = panel.getByRole('region', { name: 'Design' });
  await expect(design.getByTestId('info-designer')).toHaveText('Rasmus Andersson');
  await expect(design.getByTestId('info-axes')).toHaveText('Weight 100–900');
  await expect(design.getByTestId('info-named-styles')).toHaveText('4');
  await expect(design.getByTestId('info-description')).toHaveText(
    'A typeface carefully crafted and designed for computer screens.',
  );
  const licence = panel.getByRole('region', { name: 'Licence' });
  await expect(licence.getByTestId('info-licence')).toHaveText('SIL Open Font License');
  await expect(licence.getByTestId('info-may')).not.toHaveText('—');
  await expect(licence.getByTestId('info-may-not')).toBeVisible();
  const files = panel.getByRole('region', { name: 'Files' });
  await expect(files.getByTestId('info-format')).toHaveText('TrueType (TTF) · variable · 121 kB');
  await expect(files.getByTestId('health-badge')).toHaveText('OK');
  const metrics = panel.getByRole('region', { name: 'Metrics' });
  await expect(metrics.getByTestId('info-x')).toHaveText('520 (0.52)');
});

for (const width of [1366, 1920, 2560]) {
  test(`the grid stays two columns on a ${width} px window`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    const panel = await open(page);
    const groups = panel.getByTestId('family-info').locator('> section');
    const box = async (i: number) => (await groups.nth(i).boundingBox())!;
    // Licence beside Design; Files and Metrics on the second row, under them.
    expect(Math.abs((await box(1)).y - (await box(0)).y)).toBeLessThan(1);
    expect((await box(2)).y).toBeGreaterThan((await box(0)).y + (await box(0)).height - 1);
    expect(Math.abs((await box(2)).x - (await box(0)).x)).toBeLessThan(1);
    expect(Math.abs((await box(3)).x - (await box(1)).x)).toBeLessThan(1);
    expect(Math.abs((await box(3)).y - (await box(2)).y)).toBeLessThan(1);
  });
}

test('the grid is one column when the window is narrow', async ({ page }) => {
  await page.setViewportSize({ width: 640, height: 900 });
  const panel = await open(page);
  const groups = panel.getByTestId('family-info').locator('> section');
  const xs = await Promise.all(
    [0, 1, 2, 3].map(async (i) => (await groups.nth(i).boundingBox())!.x),
  );
  expect(new Set(xs.map(Math.round)).size).toBe(1);
});

test('the record link opens the licence form with the focus in it', async ({ page }) => {
  const panel = await open(page);
  const link = panel.getByRole('button', { name: 'Add your purchase or licence note' });
  await link.press('Enter');
  await expect(panel.getByLabel('License type')).toBeFocused();
});

test('Metrics draw Hxg at 96 px on the face, dashed at cap and x-height, a solid baseline', async ({
  page,
}) => {
  const panel = await open(page);
  const svg = panel.getByTestId('metrics-specimen');
  const text = svg.locator('text');
  await expect(text).toHaveText('Hxg');
  await expect(text).toHaveCSS('font-size', '96px');
  await expect(text).toHaveCSS('font-family', /^"?fm-face/);
  const y = async (which: string) =>
    Number(await svg.locator(`line[data-line="${which}"]`).getAttribute('y1'));
  // 96 px over 1000 units: baseline 900 down, cap 720 and x 520 above it.
  expect(await y('baseline')).toBeCloseTo(86.4);
  expect(await y('cap')).toBeCloseTo(17.28);
  expect(await y('x')).toBeCloseTo(36.48);
  await expect(svg.locator('line[data-line="cap"]')).toHaveCSS('stroke-dasharray', /\d/);
  await expect(svg.locator('line[data-line="x"]')).toHaveCSS('stroke-dasharray', /\d/);
  await expect(svg.locator('line[data-line="baseline"]')).toHaveCSS('stroke-dasharray', 'none');
});

test('hash, name IDs, vendor and tables are in Technical details at the end of the rail', async ({
  page,
}) => {
  const panel = await open(page);
  // None of them shows in the Info section.
  await expect(panel).not.toContainText('Inter-Regular');
  await expect(panel).not.toContainText('RSMS');
  await expect(panel).not.toContainText('0'.repeat(63) + '1');
  const rail = page.getByTestId('family-rail');
  const toggle = rail.getByRole('button', { name: 'Technical details', exact: true });
  await expect(toggle).toHaveAttribute('aria-expanded', 'false');
  // The expander is the rail's last control.
  await expect(rail.locator('button').last()).toHaveText('Technical details');
  const technical = await openDeveloper(page);
  await expect(toggle).toHaveAttribute('aria-expanded', 'true');
  await expect(technical.getByTestId('technical-hash')).toHaveText('0'.repeat(63) + '1');
  await expect(technical.getByTestId('technical-vendor')).toHaveText('RSMS');
  await expect(technical.getByTestId('technical-tables').locator('li')).toHaveText([
    'GDEF',
    'GPOS',
    'GSUB',
    'OS/2',
    'cmap',
    'fvar',
    'glyf',
    'head',
    'name',
    'post',
  ]);
  await expect(technical).toContainText('Inter-Regular');
  await expect(technical.getByTestId('css-snippet')).toBeVisible();
});

test.describe('@screens', () => {
  const dir = process.env.FM_SCREENS_DIR;
  test.skip(!dir, 'FM_SCREENS_DIR is not set');
  for (const [width, height] of [
    [1280, 800],
    [1000, 700],
  ] as const) {
    test(`the Info section and Technical details at ${width}x${height}`, async ({ page }) => {
      await page.setViewportSize({ width, height });
      const panel = await open(page);
      await panel.getByRole('region', { name: 'Design' }).scrollIntoViewIfNeeded();
      await page.waitForTimeout(400); // the old section fades out
      await page.screenshot({ path: `${dir}/family-info-${width}x${height}.png` });
      const technical = await openDeveloper(page);
      await technical.getByTestId('technical-tables').scrollIntoViewIfNeeded();
      await page.screenshot({ path: `${dir}/family-technical-${width}x${height}.png` });
    });
  }
});
