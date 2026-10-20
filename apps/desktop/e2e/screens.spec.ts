import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';

// Preview rows load face files from `/fmfont/<id>`; serve one fixture for all.
const font = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

// Screenshots for UI reviews (page.screenshot creates the folder): FM_SCREENS_DIR=<dir> FM_SCREENS_ROUTES=/,/other pnpm test:e2e --grep @screens
// A route may end in `@<key>` (e.g. `/?face=1@ControlOrMeta+k`): the key is pressed before the shot.
const dir = process.env.FM_SCREENS_DIR;
const routes = (process.env.FM_SCREENS_ROUTES ?? '/')
  .split(',')
  .map((r) => r.trim())
  .filter(Boolean);
// FM_SCREENS_TAB=Glyphs opens that inspector tab before the shot.
const tab = process.env.FM_SCREENS_TAB;
// FM_SCREENS_GLYPH=U+0061 then searches the Glyphs tab for it, opening its detail.
const glyph = process.env.FM_SCREENS_GLYPH;
// FM_SCREENS_STORAGE='{"fm.views.v1":"..."}' seeds localStorage (view preferences) before the page loads.
const storage: Record<string, string> = JSON.parse(process.env.FM_SCREENS_STORAGE ?? '{}');
const viewports = [
  { width: 1280, height: 800 },
  { width: 1000, height: 700 },
];

test.describe('@screens', () => {
  test.skip(!dir, 'FM_SCREENS_DIR is not set');

  for (const route of routes) {
    for (const scheme of ['light', 'dark'] as const) {
      for (const vp of viewports) {
        const [path, press] = route.split('@');
        const name = `${route.replace(/[^a-z0-9]+/gi, '-').replace(/^-|-$/g, '') || 'home'}-${scheme}-${vp.width}x${vp.height}.png`;
        test(name, async ({ page }) => {
          await page.route('**/fmfont/*', (r) =>
            r.fulfill({ status: 200, contentType: 'font/ttf', body: font }),
          );
          await page.addInitScript((entries) => {
            for (const [k, v] of Object.entries(entries)) localStorage.setItem(k, v);
          }, storage);
          await page.setViewportSize(vp);
          await page.emulateMedia({ colorScheme: scheme });
          await page.goto(`/#${path}`);
          await expect(page.locator('#app')).not.toBeEmpty();
          if (press) await page.keyboard.press(press);
          if (tab) {
            // Narrow windows hide the inspector until asked.
            const target = page.getByRole('tab', { name: tab });
            if (!(await target.isVisible()))
              await page.getByRole('button', { name: 'Inspector' }).click();
            await target.click();
            await expect(page.getByRole('tabpanel', { name: tab })).toBeVisible();
            if (glyph) {
              await page.getByRole('searchbox', { name: 'Search glyphs' }).fill(glyph);
              await expect(page.getByRole('region', { name: 'Selected glyph' })).toBeVisible();
            }
          }
          await page.screenshot({ path: `${dir}/${name}`, fullPage: true, animations: 'disabled' });
        });
      }
    }
  }
});
