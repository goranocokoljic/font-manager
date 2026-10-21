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
// FM_SCREENS_COMPARE=JetBrains then opens "Compare with…" on that glyph and picks the first font found.
const compareWith = process.env.FM_SCREENS_COMPARE;
// FM_SCREENS_COVERAGE=Polish then opens that language in the Glyphs tab's coverage list and scrolls to it.
const coverageOf = process.env.FM_SCREENS_COVERAGE;
// FM_SCREENS_STORAGE='{"fm.views.v1":"..."}' seeds localStorage (view preferences) before the page loads.
const storage: Record<string, string> = JSON.parse(process.env.FM_SCREENS_STORAGE ?? '{}');
// FM_SCREENS_CLICK='Block 1 properties' clicks the first button whose name starts with it before the shot.
const click = process.env.FM_SCREENS_CLICK;
// FM_SCREENS_PIN=3 pins the first 3 list rows with C and opens Compare; FM_SCREENS_PIN_MODE=Overlay picks a mode.
const pinRows = Number(process.env.FM_SCREENS_PIN ?? 0);
const pinMode = process.env.FM_SCREENS_PIN_MODE;
// FM_SCREENS_QUICK=3 adds the first 3 list rows to the Quick Collection with B and opens its review.
const quickRows = Number(process.env.FM_SCREENS_QUICK ?? 0);
// FM_SCREENS_TAGS=Display,Body makes those tags on the first list row with T, then opens the
// multi-tagging panel for the first two rows (one mixed tag each).
const tagNames = (process.env.FM_SCREENS_TAGS ?? '').split(',').filter(Boolean);
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
              if (compareWith) {
                await page.getByRole('button', { name: 'Compare with…' }).click();
                const dialog = page.getByRole('dialog');
                await dialog.getByRole('combobox', { name: 'Compare with font' }).fill(compareWith);
                await expect(dialog.getByRole('option').first()).toContainText(compareWith);
                await page.keyboard.press('Enter');
                await expect(dialog.getByRole('list', { name: 'Legend' })).toContainText(
                  compareWith,
                );
              }
            }
            if (coverageOf) {
              const coverage = page.getByRole('region', { name: 'Coverage' });
              const row = coverage.getByRole('button', { name: new RegExp(`^${coverageOf}:`) });
              await row.click();
              await expect(row).toHaveAttribute('aria-expanded', 'true');
              await row.scrollIntoViewIfNeeded();
            }
          }
          if (pinRows) {
            const rows = page.getByRole('tree', { name: 'Font list' }).getByRole('treeitem');
            await rows.first().click();
            for (let i = 0; i < pinRows; i++) {
              await page.keyboard.press('c');
              await expect(page.getByTestId('toast')).toContainText(`(${i + 1} of 4)`);
              await page.keyboard.press('ArrowDown');
            }
            await page.getByTestId('toast').getByRole('button', { name: 'Dismiss' }).click();
            await page.getByRole('button', { name: /^Compare \d\/4$/ }).click();
            if (pinMode) await page.getByRole('radio', { name: pinMode }).check();
          }
          if (quickRows) {
            const rows = page.getByRole('tree', { name: 'Font list' }).getByRole('treeitem');
            await rows.first().click();
            for (let i = 0; i < quickRows; i++) {
              await page.keyboard.press('b');
              await expect(page.getByTestId('quick-count')).not.toHaveText(String(i));
              await page.keyboard.press('ArrowDown');
            }
            await page.getByTestId('toast').getByRole('button', { name: 'Dismiss' }).click();
            await page.getByTestId('quick-collection').click();
          }
          if (tagNames.length) {
            const rows = page.getByRole('tree', { name: 'Font list' }).getByRole('treeitem');
            await rows.first().click();
            const panel = page.getByRole('dialog', { name: /^Tags of / });
            for (const name of tagNames) {
              await page.keyboard.press('t');
              await panel.getByRole('combobox').fill(name);
              await page.keyboard.press('Enter');
              await expect(panel.locator(`[data-tag="${name}"]`)).toBeVisible();
              await page.keyboard.press('Escape');
            }
            await page.keyboard.press('Shift+ArrowDown');
            await page.keyboard.press('t');
            await expect(panel).toBeVisible();
          }
          if (click) {
            await page
              .getByRole('button', { name: new RegExp(`^${click}`) })
              .first()
              .click();
          }
          await page.screenshot({ path: `${dir}/${name}`, fullPage: true, animations: 'disabled' });
        });
      }
    }
  }
});
