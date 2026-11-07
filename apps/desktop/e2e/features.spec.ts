import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { hero, openSection, showSection, sectionPanel } from './family-page';

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

// MockApi face 1 (Inter Regular): ccmp (automatic, behind "1 more"), kern and liga
// (on by default), onum, smcp, ss01.
async function openFeatures(page: Page) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  const panel = await openSection(page, 1, 'Features');
  await expect(panel.getByRole('switch').first()).toBeVisible();
  return panel;
}

test('the Features tab lists every feature by name and tag at the browser default', async ({
  page,
}) => {
  const panel = await openFeatures(page);
  for (const [label, on] of [
    ['Kerning (kern)', true],
    ['Standard ligatures (liga)', true],
    ['Oldstyle figures (onum)', false],
    ['Small capitals (smcp)', false],
    ['Stylistic set 1 (ss01)', false],
  ] as const) {
    const s = panel.getByRole('switch', { name: label, exact: true });
    await expect(s).toHaveAttribute('aria-checked', String(on));
  }
  // Applied by the shaper: behind "1 more", listed without a switch.
  await expect(panel.getByRole('switch', { name: /ccmp/ })).toHaveCount(0);
  const more = panel.getByRole('button', { name: '1 more' });
  await expect(more).toHaveAttribute('aria-expanded', 'false');
  const rare = panel.getByRole('list', { name: 'Rare features' });
  await expect(rare).toHaveCount(0);
  await more.click();
  await expect(rare).toContainText('Glyph composition and decomposition');
  await expect(rare).toContainText('Applied automatically');
});

test('each row shows its sample off and on side by side; ss01 shows the preview text', async ({
  page,
}) => {
  const panel = await openFeatures(page);
  const row = panel.locator('li[data-tag="liga"]');
  await expect(row.locator('.tag')).toHaveText('liga');
  await expect(row.locator('.tag')).toHaveCSS('font-family', /mono/i);
  const off = row.locator('.sample.off');
  const on = row.locator('.sample.on');
  await expect(off).toHaveText('fi fl ffi ffl');
  await expect(off).toHaveCSS('font-feature-settings', '"liga" 0');
  await expect(on).toHaveCSS('font-feature-settings', '"liga"');
  // Side by side: the same line, the off sample first.
  const [a, b] = [await off.boundingBox(), await on.boundingBox()];
  expect(Math.abs(a!.y - b!.y)).toBeLessThan(2);
  expect(b!.x).toBeGreaterThan(a!.x + a!.width - 1);
  // Muted: the off sample is not the colour of the on sample.
  const color = (el: Element) => getComputedStyle(el).color;
  expect(await off.evaluate(color)).not.toBe(await on.evaluate(color));
  await expect(panel.locator('li[data-tag="onum"] .sample.on')).toHaveText('0123456789');
  await expect(panel.locator('li[data-tag="ss01"] .sample.on')).toHaveText(
    (await hero(page).textContent())!.trim(),
  );
});

test('a switched-on feature reaches the hero, the Styles rows, the Text section and the rail', async ({
  page,
}) => {
  const panel = await openFeatures(page);
  await panel.getByRole('switch', { name: 'Small capitals (smcp)' }).click();
  await expect(hero(page)).toHaveCSS('font-feature-settings', '"smcp"');
  await expect(page.getByTestId('family-rail-sample')).toHaveCSS('font-feature-settings', '"smcp"');
  const styles = await showSection(page, 'Styles');
  const rows = styles.getByTestId('family-style-sample');
  await expect(rows.first()).toBeVisible();
  for (const sample of await rows.all())
    await expect(sample).toHaveCSS('font-feature-settings', '"smcp"');
  const text = await showSection(page, 'Text');
  const passages = text.locator(
    '[data-testid="family-text-passage"], [data-testid="family-text-ink-passage"]',
  );
  await expect(passages.first()).toBeVisible();
  for (const p of await passages.all())
    await expect(p).toHaveCSS('font-feature-settings', '"smcp"');
});

test("toggling smcp changes the hero's computed font-feature-settings; Reset restores it", async ({
  page,
}) => {
  const panel = await openFeatures(page);
  const sample = hero(page);
  await expect(sample).toHaveCSS('font-feature-settings', 'normal');

  const smcp = panel.getByRole('switch', { name: 'Small capitals (smcp)' });
  await smcp.click();
  await expect(smcp).toHaveAttribute('aria-checked', 'true');
  await expect(sample).toHaveCSS('font-feature-settings', /"smcp"/);

  await panel.getByRole('switch', { name: 'Kerning (kern)' }).click();
  await expect(sample).toHaveCSS('font-feature-settings', /"kern" 0/);

  await panel.getByRole('button', { name: 'Reset' }).click();
  await expect(smcp).toHaveAttribute('aria-checked', 'false');
  await expect(sample).toHaveCSS('font-feature-settings', 'normal');
});

test('"Copy CSS for N switched on" copies font-feature-settings as the preview applies it', async ({
  page,
  context,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  const panel = await openFeatures(page);
  const sample = hero(page);
  const copy = panel.getByTestId('features-copy');
  const status = panel.getByRole('status').filter({ hasText: /Copied|Could not copy/ });
  const clipboard = () => page.evaluate(() => navigator.clipboard.readText());

  // Nothing switched: nothing to copy.
  await expect(copy).toHaveText('Copy CSS for 0 switched on');
  await expect(copy).toBeDisabled();

  // smcp is off by default, liga on: both written; onum switched twice is not.
  await panel.getByRole('switch', { name: 'Small capitals (smcp)' }).click();
  await panel.getByRole('switch', { name: 'Standard ligatures (liga)' }).click();
  await panel.getByRole('switch', { name: 'Oldstyle figures (onum)' }).click();
  await panel.getByRole('switch', { name: 'Oldstyle figures (onum)' }).click();
  await expect(sample).toHaveCSS('font-feature-settings', /"smcp"/);
  await expect(copy).toHaveText('Copy CSS for 2 switched on');
  await copy.click();
  await expect(status).toHaveText('Copied');
  await expect.poll(clipboard).toBe('font-feature-settings: "liga" 0, "smcp" 1;');
  // The copied value, as the browser reads it, is what the row applies.
  const copied = (await clipboard()).replace(/^font-feature-settings: (.*);$/, '$1');
  const [applied, parsed] = await sample.evaluate((el, value) => {
    const probe = document.createElement('span');
    probe.style.fontFeatureSettings = value;
    return [(el as HTMLElement).style.fontFeatureSettings, probe.style.fontFeatureSettings];
  }, copied);
  expect(parsed).toBe(applied);
});

test('"Copy CSS" for features is keyboard-reachable and says when the clipboard refuses', async ({
  page,
}) => {
  const panel = await openFeatures(page);
  await page.evaluate(() => {
    navigator.clipboard.writeText = () => Promise.reject(new Error('refused'));
  });
  const copy = panel.getByTestId('features-copy');
  await panel.getByRole('switch', { name: 'Small capitals (smcp)' }).click();
  await panel.getByRole('switch').first().focus();
  await page.keyboard.press('Shift+Tab');
  await expect(panel.getByRole('button', { name: 'Reset' })).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await expect(copy).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(panel.getByRole('status').filter({ hasText: 'Could not copy' })).toBeVisible();
});

test('the preview strip follows the switches of its face', async ({ page }) => {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/#/');
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Table' })
    .click();
  const grid = page.getByRole('treegrid', { name: 'Font table' });
  await grid.getByRole('row', { name: /^Inter,/ }).click();
  const strip = page.getByRole('region', { name: 'Preview strip' });
  const sample = strip.locator('[data-face="1"] .sample');
  await expect(sample).toHaveCSS('font-feature-settings', 'normal');
  await page.keyboard.press('o');
  const panel = await showSection(page, 'Features');
  await panel.getByRole('switch', { name: 'Oldstyle figures (onum)' }).click();
  await page.getByRole('button', { name: 'Library' }).click();
  await expect(sample).toHaveCSS('font-feature-settings', /"onum"/);
});

test('every switch is reachable and operable from the keyboard', async ({ page }) => {
  const panel = await openFeatures(page);
  const switches = panel.getByRole('switch');
  await expect(switches).toHaveCount(5);
  await switches.first().focus();
  for (let i = 1; i < 5; i++) {
    await page.keyboard.press('Tab');
    await expect(switches.nth(i)).toBeFocused();
  }
  // The last one is ss01; Space and Enter switch it.
  await page.keyboard.press('Space');
  await expect(switches.nth(4)).toHaveAttribute('aria-checked', 'true');
  await page.keyboard.press('Enter');
  await expect(switches.nth(4)).toHaveAttribute('aria-checked', 'false');
});

test('the keyboard moves between the sections and leaves the arrows to the family', async ({
  page,
}) => {
  await page.goto('/#/family/1?face=1');
  const sections = page.getByRole('group', { name: 'Section' });
  const styles = sections.getByRole('button', { name: 'Styles' });
  await styles.focus();
  // Styles, Text, Characters, Features.
  for (let i = 0; i < 3; i++) await page.keyboard.press('Tab');
  const features = sections.getByRole('button', { name: 'Features' });
  await expect(features).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(features).toHaveAttribute('aria-pressed', 'true');
  await expect(sectionPanel(page, 'Features')).toBeVisible();
  await page.keyboard.press('Shift+Tab');
  await page.keyboard.press('Shift+Tab');
  await page.keyboard.press('Shift+Tab');
  await page.keyboard.press('Enter');
  await expect(styles).toHaveAttribute('aria-pressed', 'true');
});

test('the switches are kept per face while the app runs', async ({ page }) => {
  const panel = await openFeatures(page);
  const smcp = panel.getByRole('switch', { name: 'Small capitals (smcp)' });
  await smcp.click();
  // Another face starts at its defaults; coming back keeps the switch.
  await page.evaluate(() => (location.hash = '#/family/3?face=6'));
  await expect(panel.getByRole('switch')).toHaveCount(2);
  await expect(panel.getByRole('switch', { name: 'Kerning (kern)' })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await page.evaluate(() => (location.hash = '#/family/1?face=1'));
  await expect(smcp).toHaveAttribute('aria-checked', 'true');
  await expect(hero(page)).toHaveCSS('font-feature-settings', /"smcp"/);
});

for (const colorScheme of ['light', 'dark'] as const) {
  test(`switch outlines are at least 3:1 in the ${colorScheme} theme`, async ({ page }) => {
    await page.emulateMedia({ colorScheme });
    const panel = await openFeatures(page);
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
    const off = panel.getByRole('switch', { name: 'Small capitals (smcp)' });
    const on = panel.getByRole('switch', { name: 'Kerning (kern)' });
    expect(await off.evaluate(ratio)).toBeGreaterThanOrEqual(3);
    expect(await on.evaluate(ratio)).toBeGreaterThanOrEqual(3);
  });
}

// Screenshots of the Features section for UI reviews: FM_SCREENS_DIR=<dir> pnpm test:e2e --grep @screens
test.describe('@screens features tab', () => {
  const dir = process.env.FM_SCREENS_DIR;
  test.skip(!dir, 'FM_SCREENS_DIR is not set');
  for (const scheme of ['light', 'dark'] as const) {
    for (const vp of [
      { width: 1280, height: 800 },
      { width: 1000, height: 700 },
    ]) {
      test(`features-${scheme}-${vp.width}x${vp.height}`, async ({ page }) => {
        await page.setViewportSize(vp);
        await page.emulateMedia({ colorScheme: scheme });
        const panel = await openFeatures(page);
        await panel.getByRole('switch', { name: 'Small capitals (smcp)' }).click();
        await expect(hero(page)).toHaveCSS('font-feature-settings', /"smcp"/);
        await page.screenshot({
          path: `${dir}/features-${scheme}-${vp.width}x${vp.height}.png`,
          fullPage: true,
          animations: 'disabled',
        });
      });
    }
  }
});
