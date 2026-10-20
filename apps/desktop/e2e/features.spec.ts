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

// MockApi face 1 (Inter Regular): ccmp (automatic), kern and liga (on by
// default), onum, smcp, ss01.
async function openFeatures(page: Page, { showInspector = false } = {}) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/#/?face=1');
  // Narrow windows hide the inspector until asked.
  if (showInspector) await page.getByRole('button', { name: 'Inspector' }).click();
  await page.getByRole('tab', { name: 'Features' }).click();
  const panel = page.getByRole('tabpanel', { name: 'Features' });
  await expect(panel.getByRole('switch').first()).toBeVisible();
  return panel;
}

test('the Features tab lists every feature by name and tag at the browser default', async ({
  page,
}) => {
  const panel = await openFeatures(page);
  await expect(page.getByRole('tab', { name: 'Features' })).toHaveAttribute(
    'aria-selected',
    'true',
  );
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
  // Applied by the shaper: listed, no switch.
  await expect(panel.getByRole('switch', { name: /ccmp/ })).toHaveCount(0);
  const auto = panel.getByRole('list', { name: 'Applied automatically' });
  await expect(auto).toContainText('Glyph composition and decomposition');
  await expect(auto).toContainText('applied automatically');
});

test("toggling smcp changes the row's computed font-feature-settings; Reset restores it", async ({
  page,
}) => {
  const panel = await openFeatures(page);
  const sample = page.locator('[data-face="1"] .sample').first();
  await expect(sample).toHaveCSS('font-feature-settings', 'normal');

  const smcp = panel.getByRole('switch', { name: 'Small capitals (smcp)' });
  await smcp.click();
  await expect(smcp).toHaveAttribute('aria-checked', 'true');
  await expect(sample).toHaveCSS('font-feature-settings', /"smcp"/);
  // Another face's row is untouched.
  await expect(page.locator('[data-face="6"] .sample').first()).toHaveCSS(
    'font-feature-settings',
    'normal',
  );

  await panel.getByRole('switch', { name: 'Kerning (kern)' }).click();
  await expect(sample).toHaveCSS('font-feature-settings', /"kern" 0/);

  await panel.getByRole('button', { name: 'Reset' }).click();
  await expect(smcp).toHaveAttribute('aria-checked', 'false');
  await expect(sample).toHaveCSS('font-feature-settings', 'normal');
});

test('"Copy CSS" copies font-feature-settings for the changed features, as the preview applies them', async ({
  page,
  context,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  const panel = await openFeatures(page);
  const sample = page.locator('[data-face="1"] .sample').first();
  const copy = panel.getByRole('button', { name: 'Copy CSS for the features' });
  const status = panel.getByRole('status').filter({ hasText: /Copied|Could not copy/ });
  const clipboard = () => page.evaluate(() => navigator.clipboard.readText());

  await copy.click();
  await expect(status).toHaveText('Copied');
  expect(await clipboard()).toBe('font-feature-settings: normal;');

  // smcp is off by default, liga on: both written; onum switched twice is not.
  await panel.getByRole('switch', { name: 'Small capitals (smcp)' }).click();
  await panel.getByRole('switch', { name: 'Standard ligatures (liga)' }).click();
  await panel.getByRole('switch', { name: 'Oldstyle figures (onum)' }).click();
  await panel.getByRole('switch', { name: 'Oldstyle figures (onum)' }).click();
  await expect(sample).toHaveCSS('font-feature-settings', /"smcp"/);
  await copy.click();
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
  const copy = panel.getByRole('button', { name: 'Copy CSS for the features' });
  await panel.getByRole('switch').first().focus();
  await page.keyboard.press('Shift+Tab');
  // Reset is disabled while nothing changed, so Copy CSS is just before the switches.
  await expect(copy).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(panel.getByRole('status').filter({ hasText: 'Could not copy' })).toBeVisible();
});

test('the preview strip follows the switches of its face', async ({ page }) => {
  const panel = await openFeatures(page);
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Table' })
    .click();
  const grid = page.getByRole('treegrid', { name: 'Font table' });
  await grid.getByRole('row', { name: /^Inter,/ }).click();
  const strip = page.getByRole('region', { name: 'Preview strip' });
  const sample = strip.locator('[data-face="1"] .sample');
  await expect(sample).toHaveCSS('font-feature-settings', 'normal');
  await panel.getByRole('switch', { name: 'Oldstyle figures (onum)' }).click();
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

test('arrow keys move between the inspector tabs', async ({ page }) => {
  await page.goto('/#/?face=1');
  const info = page.getByRole('tab', { name: 'Info' });
  await info.focus();
  // Info, Axes, Features, Glyphs.
  await page.keyboard.press('ArrowRight');
  await expect(page.getByRole('tab', { name: 'Axes' })).toBeFocused();
  await page.keyboard.press('ArrowRight');
  const features = page.getByRole('tab', { name: 'Features' });
  await expect(features).toBeFocused();
  await expect(features).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('tabpanel', { name: 'Features' })).toBeVisible();
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.press('ArrowLeft');
  await expect(info).toHaveAttribute('aria-selected', 'true');
});

test('the switches are kept per face while the app runs', async ({ page }) => {
  const panel = await openFeatures(page);
  const smcp = panel.getByRole('switch', { name: 'Small capitals (smcp)' });
  await smcp.click();
  // Another face starts at its defaults; coming back keeps the switch.
  await page.evaluate(() => (location.hash = '#/?face=6'));
  await expect(panel.getByRole('switch')).toHaveCount(2);
  await expect(panel.getByRole('switch', { name: 'Kerning (kern)' })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await page.evaluate(() => (location.hash = '#/?face=1'));
  await expect(smcp).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-face="1"] .sample').first()).toHaveCSS(
    'font-feature-settings',
    /"smcp"/,
  );
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

// Screenshots of the Features tab for UI reviews: FM_SCREENS_DIR=<dir> pnpm test:e2e --grep @screens
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
        const panel = await openFeatures(page, { showInspector: vp.width < 1280 });
        await panel.getByRole('switch', { name: 'Small capitals (smcp)' }).click();
        await expect(page.locator('[data-face="1"]').first()).toHaveAttribute(
          'data-state',
          'ready',
        );
        await page.screenshot({
          path: `${dir}/features-${scheme}-${vp.width}x${vp.height}.png`,
          fullPage: true,
          animations: 'disabled',
        });
      });
    }
  }
});
