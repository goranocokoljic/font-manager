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

// MockApi face 1 (Inter Regular): wght 100–900, default 400; smcp off by default.
async function open(page: Page, { showInspector = false } = {}) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/#/?face=1');
  // Narrow windows hide the inspector until asked.
  if (showInspector) await page.getByRole('button', { name: 'Inspector' }).click();
}

async function moveWghtAndTurnOnSmcp(page: Page, wght: string) {
  await page.getByRole('tab', { name: 'Axes' }).click();
  const axes = page.getByRole('tabpanel', { name: 'Axes' });
  await axes.getByRole('textbox', { name: 'Weight (wght) value' }).fill(wght);
  await axes.getByRole('textbox', { name: 'Weight (wght) value' }).press('Enter');
  await page.getByRole('tab', { name: 'Features' }).click();
  await page
    .getByRole('tabpanel', { name: 'Features' })
    .getByRole('switch', { name: 'Small capitals (smcp)' })
    .click();
}

const developer = (page: Page) => page.getByRole('tabpanel', { name: 'Developer' });
const snippet = (page: Page) => developer(page).getByTestId('css-snippet');

test('the Developer tab comes after Glyphs and reads normal while nothing changed', async ({
  page,
}) => {
  await open(page);
  const tabs = page.getByRole('tablist', { name: 'Inspector tabs' }).getByRole('tab');
  await expect(tabs).toHaveText(['Info', 'Axes', 'Features', 'Glyphs', 'Developer']);
  await page.getByRole('tab', { name: 'Developer' }).click();
  await expect(snippet(page)).toHaveText(
    'font-variation-settings: normal;\nfont-feature-settings: normal;',
  );
});

test('the copied snippet is what the selected row applies', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await open(page);
  const sample = page.locator('[data-face="1"] .sample').first();
  await moveWghtAndTurnOnSmcp(page, '650');
  await expect(sample).toHaveCSS('font-variation-settings', '"wght" 650');
  await expect(sample).toHaveCSS('font-feature-settings', /"smcp"/);

  await page.getByRole('tab', { name: 'Developer' }).click();
  await expect(snippet(page)).toHaveText(
    'font-variation-settings: "wght" 650;\nfont-feature-settings: "smcp" 1;',
  );
  await developer(page).getByRole('button', { name: 'Copy the CSS snippet' }).click();
  await expect(developer(page).getByRole('status')).toHaveText('Copied');
  // The Windows clipboard gives the line break back as CRLF.
  const copied = (await page.evaluate(() => navigator.clipboard.readText())).replace(/\r\n/g, '\n');
  expect(copied).toBe('font-variation-settings: "wght" 650;\nfont-feature-settings: "smcp" 1;');

  // Each declaration, as the browser reads it, is what the row applies.
  const [applied, parsed] = await sample.evaluate((el, css) => {
    const probe = document.createElement('span');
    probe.style.cssText = css;
    const row = (el as HTMLElement).style;
    return [
      [row.fontVariationSettings, row.fontFeatureSettings],
      [probe.style.fontVariationSettings, probe.style.fontFeatureSettings],
    ];
  }, copied);
  expect(parsed).toEqual(applied);
});

test('the snippet follows the sliders and switches without reopening the tab', async ({ page }) => {
  await open(page);
  await page.getByRole('tab', { name: 'Developer' }).click();
  await expect(snippet(page)).toContainText('font-variation-settings: normal;');
  await moveWghtAndTurnOnSmcp(page, '700');
  await page.getByRole('tab', { name: 'Developer' }).click();
  await expect(snippet(page)).toHaveText(
    'font-variation-settings: "wght" 700;\nfont-feature-settings: "smcp" 1;',
  );
  // The Reset buttons bring both back to normal.
  await page.getByRole('tab', { name: 'Axes' }).click();
  await page.getByRole('tabpanel', { name: 'Axes' }).getByRole('button', { name: 'Reset' }).click();
  await page.getByRole('tab', { name: 'Developer' }).click();
  await expect(snippet(page)).toHaveText(
    'font-variation-settings: normal;\nfont-feature-settings: "smcp" 1;',
  );
});

test('the Developer tab follows the tabs pattern and Copy is reachable by keyboard', async ({
  page,
}) => {
  await open(page);
  const info = page.getByRole('tab', { name: 'Info' });
  const dev = page.getByRole('tab', { name: 'Developer' });
  await info.focus();
  await page.keyboard.press('End');
  await expect(dev).toBeFocused();
  await expect(dev).toHaveAttribute('aria-selected', 'true');
  await expect(dev).toHaveAttribute('tabindex', '0');
  await expect(info).toHaveAttribute('tabindex', '-1');
  await expect(developer(page)).toBeVisible();
  // It wraps: ArrowRight from the last tab is Info, ArrowLeft back is Developer.
  await page.keyboard.press('ArrowRight');
  await expect(info).toBeFocused();
  await page.keyboard.press('ArrowLeft');
  await expect(dev).toBeFocused();
  await page.keyboard.press('Home');
  await expect(info).toBeFocused();
  await page.keyboard.press('End');

  // Tab: the panel, then the Copy button; Enter copies.
  await page.evaluate(() => {
    navigator.clipboard.writeText = () => Promise.reject(new Error('refused'));
  });
  const copy = developer(page).getByRole('button', { name: 'Copy the CSS snippet' });
  await page.keyboard.press('Tab');
  await expect(developer(page)).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(copy).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(developer(page).getByRole('status')).toHaveText('Could not copy');
});

// Screenshots of the Developer tab for UI reviews: FM_SCREENS_DIR=<dir> pnpm test:e2e --grep @screens
test.describe('@screens developer tab', () => {
  const dir = process.env.FM_SCREENS_DIR;
  test.skip(!dir, 'FM_SCREENS_DIR is not set');
  for (const scheme of ['light', 'dark'] as const) {
    for (const vp of [
      { width: 1280, height: 800 },
      { width: 1000, height: 700 },
    ]) {
      test(`developer-${scheme}-${vp.width}x${vp.height}`, async ({ page }) => {
        await page.setViewportSize(vp);
        await page.emulateMedia({ colorScheme: scheme });
        await open(page, { showInspector: vp.width < 1280 });
        await moveWghtAndTurnOnSmcp(page, '650');
        await page.getByRole('tab', { name: 'Developer' }).click();
        await expect(snippet(page)).toContainText('"smcp" 1');
        await page.screenshot({
          path: `${dir}/developer-${scheme}-${vp.width}x${vp.height}.png`,
          fullPage: true,
          animations: 'disabled',
        });
      });
    }
  }
});
