import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';
import { wallFixtures } from '../src/api/mockWall';

const fixture = (path: string) =>
  readFileSync(fileURLToPath(new URL(`../../../fixtures/fonts/${path}`, import.meta.url)));
// Preview rows load face files from `/fmfont/<id>`. The wall library (the default here)
// serves each face its own fixture; other faces and libraries get one fixture for all.
const font = fixture('google-repo/asapsharp/AsapSharp[wdth,wght].ttf');
const wallFiles = wallFixtures();
const fontFiles = new Map<string, Buffer>();
const contentType = (path: string) => `font/${path.split('.').pop()}`;

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
// FM_SCREENS_SESSION='{"fm.mock.settings.v1":"{\"density\":\"compact\"}"}' seeds sessionStorage (the mock's saved settings).
const session: Record<string, string> = JSON.parse(process.env.FM_SCREENS_SESSION ?? '{}');
// FM_SCREENS_CLICK='Block 1 properties' clicks the first button whose name starts with it before the shot.
const click = process.env.FM_SCREENS_CLICK;
// FM_SCREENS_OPEN=Language opens the Select (a combobox) of that name, so its menu is in the shot.
const openSelect = process.env.FM_SCREENS_OPEN;
// FM_SCREENS_PIN=3 pins the first 3 list rows with C and opens Compare; FM_SCREENS_PIN_MODE=Overlay picks a mode.
const pinRows = Number(process.env.FM_SCREENS_PIN ?? 0);
const pinMode = process.env.FM_SCREENS_PIN_MODE;
// FM_SCREENS_QUICK=3 adds the first 3 list rows to the Quick Collection with B and opens its review.
const quickRows = Number(process.env.FM_SCREENS_QUICK ?? 0);
// FM_SCREENS_TAGS=Display,Body makes those tags on the first list row with T, then opens the
// multi-tagging panel for the first two rows (one mixed tag each).
const tagNames = (process.env.FM_SCREENS_TAGS ?? '').split(',').filter(Boolean);
// FM_SCREENS_COMMAND="Check a document's fonts" runs that command palette action before the shot.
const command = process.env.FM_SCREENS_COMMAND;
// FM_SCREENS_QUERY='?mockColor=1' sets MockApi options (the page URL before the hash).
/** The list's rows, or the grid's cards when the view is the grid. */
const listRows = (page: import('@playwright/test').Page) =>
  page.locator('[role="tree"][aria-label="Font list"] [role="treeitem"], .slot[role="option"]');

// The wall library (60 families, E40.2) unless the query names one: `?mockLibrary=small`
// keeps the three sample families.
const query = withWall(process.env.FM_SCREENS_QUERY ?? '');
const wall = new URLSearchParams(query).get('mockLibrary') === 'wall';

/** `query` with `mockLibrary=wall` added when it names no library. */
function withWall(q: string): string {
  const params = new URLSearchParams(q);
  if (!params.has('mockLibrary')) params.set('mockLibrary', 'wall');
  return `?${params.toString()}`;
}
// FM_SCREENS_LOCALE=ar-XB sets the browser's (the OS's) language: `ar-XB` shows the RTL chrome.
const locale = process.env.FM_SCREENS_LOCALE;
// FM_SCREENS_ROW=1 clicks that list row (1-based) after the page loads, before the route's key
// is pressed, so a key meant for the list (ArrowRight expands the family) reaches it.
// FM_SCREENS_TWISTY=4 clicks the open/fold twisty on that grid card (1-based).
const twistyCard = Number(process.env.FM_SCREENS_TWISTY ?? 0);
const rowIndex = Number(process.env.FM_SCREENS_ROW ?? 0);
// FM_SCREENS_HOVER=2 leaves the pointer over that list row (1-based) for the shot, so hover-only
// row actions (Find similar) are in it.
const hoverRow = Number(process.env.FM_SCREENS_HOVER ?? 0);
// FM_SCREENS_COLLECTION=Empty makes a collection of that name in the sidebar and opens it
// (browser mode starts without collections).
const collectionName = process.env.FM_SCREENS_COLLECTION;
// FM_SCREENS_PREFIX=list names the files `<prefix>-<theme>-<w>x<h>.png` instead of using the
// route; only for a single-route run.
const prefix = process.env.FM_SCREENS_PREFIX;
// FM_SCREENS_VIEWPORTS=1280x800,1440x900 replaces the default viewports.
const defaultViewports = [
  { width: 1280, height: 800 },
  { width: 1000, height: 700 },
];
const viewports = (process.env.FM_SCREENS_VIEWPORTS ?? '')
  .split(',')
  .map((v) =>
    v
      .trim()
      .toLowerCase()
      .match(/^(\d+)x(\d+)$/),
  )
  .flatMap((m) => (m ? [{ width: Number(m[1]), height: Number(m[2]) }] : []));
if (!viewports.length) viewports.push(...defaultViewports);

/** Serves `/fmfont/<id>`: on the wall each face's own fixture, else one fixture for all. */
async function serveFonts(page: import('@playwright/test').Page) {
  await page.route('**/fmfont/*', (r) => {
    const id = Number(new URL(r.request().url()).pathname.split('/').pop());
    const path = wall ? wallFiles.get(id) : undefined;
    if (!path) return r.fulfill({ status: 200, contentType: 'font/ttf', body: font });
    if (!fontFiles.has(path)) fontFiles.set(path, fixture(path));
    return r.fulfill({ status: 200, contentType: contentType(path), body: fontFiles.get(path)! });
  });
}

// CI sets no FM_SCREENS_* and skips the shots below: these prove what the shots rely on.
test.describe('the screenshots default (E40.2)', () => {
  test.skip(!!process.env.FM_SCREENS_QUERY, 'FM_SCREENS_QUERY chooses another library');

  test('uses the wall and serves each face its own fixture', async ({ page }) => {
    await serveFonts(page);
    await page.goto(`/${query}#/`);
    const rows = page.getByRole('tree', { name: 'Font list' }).locator('[aria-level="1"]');
    await expect(rows.first()).toHaveAttribute('aria-label', /^Asap Sharp,/);
    await expect(rows.nth(1)).toHaveAttribute('aria-label', /^Crimson Pro,/);
    const digest = (id: number) =>
      page.evaluate(async (url) => {
        const res = await fetch(url);
        const hash = await crypto.subtle.digest('SHA-256', await res.arrayBuffer());
        const hex = [...new Uint8Array(hash)].map((b) => b.toString(16).padStart(2, '0'));
        return { type: res.headers.get('content-type'), hash: hex.join('') };
      }, `/fmfont/${id}`);
    const woff2 = [...wallFiles].find(([, path]) => path.endsWith('.woff2'))!;
    for (const [id, path] of [[1, wallFiles.get(1)!], woff2] as const) {
      expect(await digest(id)).toEqual({
        type: contentType(path),
        hash: createHash('sha256').update(fixture(path)).digest('hex'),
      });
    }
  });

  test('a query keeps its options and may name another library', () => {
    expect(withWall('')).toBe('?mockLibrary=wall');
    expect(withWall('?mockColor=1')).toBe('?mockColor=1&mockLibrary=wall');
    expect(withWall('?mockLibrary=small')).toBe('?mockLibrary=small');
  });
});

test.describe('@screens', () => {
  test.skip(!dir, 'FM_SCREENS_DIR is not set');
  if (locale) test.use({ locale });

  for (const route of routes) {
    for (const scheme of ['light', 'dark'] as const) {
      for (const vp of viewports) {
        const [path, press] = route.split('@');
        const slug = prefix ?? (route.replace(/[^a-z0-9]+/gi, '-').replace(/^-|-$/g, '') || 'home');
        const name = `${slug}-${scheme}-${vp.width}x${vp.height}.png`;
        test(name, async ({ page }) => {
          await serveFonts(page);
          await page.addInitScript((entries) => {
            for (const [k, v] of Object.entries(entries)) localStorage.setItem(k, v);
          }, storage);
          await page.addInitScript((entries) => {
            for (const [k, v] of Object.entries(entries)) sessionStorage.setItem(k, v);
          }, session);
          await page.setViewportSize(vp);
          await page.emulateMedia({ colorScheme: scheme });
          await page.goto(`/${query}#${path}`);
          await expect(page.locator('#app')).not.toBeEmpty();
          if (collectionName) {
            const sidebar = page.getByRole('region', { name: 'Collections' });
            await sidebar.getByRole('button', { name: 'New collection', exact: true }).click();
            const input = sidebar.getByRole('textbox', { name: 'New collection name' });
            await input.fill(collectionName);
            await input.press('Enter');
            const item = sidebar.getByRole('listitem', { name: collectionName, exact: true });
            await expect(item).toBeVisible();
            await item.getByRole('button', { name: collectionName, exact: true }).click();
          }
          if (rowIndex) {
            await listRows(page)
              .nth(rowIndex - 1)
              .click();
          }
          if (twistyCard) {
            await listRows(page)
              .nth(twistyCard - 1)
              .locator('.twisty')
              .click();
            await page.waitForTimeout(200);
          }
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
          if (command) {
            await page.keyboard.press('ControlOrMeta+k');
            await page.keyboard.type(command);
            await page.keyboard.press('Enter');
            await expect(page.getByRole('dialog', { name: 'Command palette' })).toBeHidden();
          }
          if (click) {
            await page
              .getByRole('button', { name: new RegExp(`^${click}`) })
              .first()
              .click();
            await page.waitForTimeout(300); // panels and popovers finish their fade
          }
          if (openSelect) {
            await page.getByRole('combobox', { name: openSelect, exact: true }).first().click();
            await expect(page.getByRole('listbox').first()).toBeVisible();
            // The menu fades in (120 ms); the shot waits for it to settle.
            await expect(page.locator('.ui-popover').first()).toHaveCSS('opacity', '1');
          }
          if (hoverRow) {
            await listRows(page)
              .nth(hoverRow - 1)
              .hover();
            await page.waitForTimeout(200); // the row's actions fade in
          }
          await page.screenshot({ path: `${dir}/${name}`, fullPage: true, animations: 'disabled' });
        });
      }
    }
  }
});
