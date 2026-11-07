import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { axesRow, hero, openDeveloper, showSection } from './family-page';

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

// MockApi face 1 (Inter Regular): wght 100–900, default 400; smcp off by default.
async function open(page: Page) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/#/family/1?face=1');
}

async function moveWghtAndTurnOnSmcp(page: Page, wght: string) {
  const axes = axesRow(page);
  await axes.getByRole('textbox', { name: 'Weight (wght) value' }).fill(wght);
  await axes.getByRole('textbox', { name: 'Weight (wght) value' }).press('Enter');
  const features = await showSection(page, 'Features');
  await features.getByRole('switch', { name: 'Small capitals (smcp)' }).click();
}

const developer = (page: Page) => page.locator('#fm-family-developer');
const snippet = (page: Page) => developer(page).getByTestId('css-snippet');

test('the Technical details part ends the rail and reads normal while nothing changed', async ({
  page,
}) => {
  await open(page);
  const toggle = page
    .getByTestId('family-rail')
    .locator('> *')
    .last()
    .locator('xpath=preceding-sibling::button[1]');
  await expect(toggle).toHaveText('Technical details');
  await expect(toggle).toHaveAttribute('aria-expanded', 'false');
  await openDeveloper(page);
  await expect(toggle).toHaveAttribute('aria-expanded', 'true');
  await expect(snippet(page)).toHaveText(
    'font-variation-settings: normal;\nfont-feature-settings: normal;',
  );
});

test('the copied snippet is what the selected row applies', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await open(page);
  const sample = hero(page);
  await moveWghtAndTurnOnSmcp(page, '650');
  await expect(sample).toHaveCSS('font-variation-settings', '"wght" 650');
  await expect(sample).toHaveCSS('font-feature-settings', /"smcp"/);

  await openDeveloper(page);
  await expect(snippet(page)).toHaveText(
    'font-variation-settings: "wght" 650;\nfont-feature-settings: "smcp" 1;',
  );
  await developer(page).getByRole('button', { name: 'Copy the CSS snippet' }).click();
  await expect(developer(page).getByTestId('css-status')).toHaveText('Copied');
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

test('the snippet follows the sliders and switches while it is open', async ({ page }) => {
  await open(page);
  await openDeveloper(page);
  await expect(snippet(page)).toContainText('font-variation-settings: normal;');
  await moveWghtAndTurnOnSmcp(page, '700');
  await expect(snippet(page)).toHaveText(
    'font-variation-settings: "wght" 700;\nfont-feature-settings: "smcp" 1;',
  );
  // The Reset buttons bring both back to normal.
  await axesRow(page).getByRole('button', { name: 'Reset' }).click();
  await expect(snippet(page)).toHaveText(
    'font-variation-settings: normal;\nfont-feature-settings: "smcp" 1;',
  );
});

test('the Developer expander works from the keyboard and Copy is reachable', async ({ page }) => {
  await open(page);
  const toggle = page.getByRole('button', { name: 'Technical details', exact: true });
  await toggle.focus();
  await page.keyboard.press('Enter');
  await expect(toggle).toHaveAttribute('aria-expanded', 'true');
  await expect(snippet(page)).toBeVisible();

  // Tab: the Copy button; Enter copies.
  await page.evaluate(() => {
    navigator.clipboard.writeText = () => Promise.reject(new Error('refused'));
  });
  const copy = developer(page).getByRole('button', { name: 'Copy the CSS snippet' });
  await page.keyboard.press('Tab');
  await expect(copy).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(developer(page).getByTestId('css-status')).toHaveText('Could not copy');

  await toggle.focus();
  await page.keyboard.press('Enter');
  await expect(toggle).toHaveAttribute('aria-expanded', 'false');
  await expect(snippet(page)).toHaveCount(0);
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
        await open(page);
        await moveWghtAndTurnOnSmcp(page, '650');
        await openDeveloper(page);
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
