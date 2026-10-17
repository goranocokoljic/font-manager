import { expect, test } from '@playwright/test';

// Screenshots for UI reviews (page.screenshot creates the folder): FM_SCREENS_DIR=<dir> FM_SCREENS_ROUTES=/,/other pnpm test:e2e --grep @screens
// A route may end in `@<key>` (e.g. `/?face=1@ControlOrMeta+k`): the key is pressed before the shot.
const dir = process.env.FM_SCREENS_DIR;
const routes = (process.env.FM_SCREENS_ROUTES ?? '/')
  .split(',')
  .map((r) => r.trim())
  .filter(Boolean);
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
          await page.setViewportSize(vp);
          await page.emulateMedia({ colorScheme: scheme });
          await page.goto(`/#${path}`);
          await expect(page.locator('#app')).not.toBeEmpty();
          if (press) await page.keyboard.press(press);
          await page.screenshot({ path: `${dir}/${name}`, fullPage: true, animations: 'disabled' });
        });
      }
    }
  }
});
