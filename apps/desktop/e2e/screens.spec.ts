import { createHash } from 'node:crypto';
import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';
import { wallFixtures } from '../src/api/mockWall';
import { compareRows } from './compare-pins';

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
// FM_SCREENS_TAB=Characters opens that section of the family page (a /family/1 route) before the shot.
const tab = process.env.FM_SCREENS_TAB;
// FM_SCREENS_GLYPH=U+0061 then searches the Characters section for it, opening its detail.
const glyph = process.env.FM_SCREENS_GLYPH;
// FM_SCREENS_COMPARE=JetBrains then opens "Compare with…" on that glyph and picks the first font found.
const compareWith = process.env.FM_SCREENS_COMPARE;
// FM_SCREENS_COVERAGE=Polish then opens that language in the Characters section's coverage list and scrolls to it.
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
// FM_SCREENS_SELECT=1,4,8 Ctrl/Cmd-clicks those list rows or grid cards (1-based): a
// multi-selection, with the selection bar (E44.1).
const selectRows = (process.env.FM_SCREENS_SELECT ?? '')
  .split(',')
  .map(Number)
  .filter((n) => n > 0);
// FM_SCREENS_HOVER=2 leaves the pointer over that list row (1-based) for the shot, so hover-only
// row actions (Find similar) are in it.
const hoverRow = Number(process.env.FM_SCREENS_HOVER ?? 0);
// FM_SCREENS_DRAG=Editorial takes the first selected card (or the first card) with the pointer
// and holds it over that sidebar collection for the shot: the ghost, the faded cards, the
// ring and "31 → 34" (E44.2).
const dragTo = process.env.FM_SCREENS_DRAG;
// FM_SCREENS_COLLECTION=Empty makes a collection of that name in the sidebar and opens it
// (browser mode starts without collections).
const collectionName = process.env.FM_SCREENS_COLLECTION;
// FM_SCREENS_PREFIX=list names the files `<prefix>-<theme>-<w>x<h>.png` instead of using the
// route; only for a single-route run.
const prefix = process.env.FM_SCREENS_PREFIX;
// FM_SCREENS_THEMES=all (or paper,moss) shoots every scene under each of those built-in themes
// (E47.2, the review pass), named `<slug>-<theme id>-<scheme>-<w>x<h>.png`; unset, the saved
// theme (Paper) and the names without a theme id.
const themesDir = new URL('../../../data/themes/', import.meta.url);
/** The built-in themes, Paper (the default) first, from data/themes. */
function builtInThemes(): { id: string; light: { accent: string }; dark: { accent: string } }[] {
  return readdirSync(fileURLToPath(themesDir))
    .filter((f) => f.endsWith('.json') && f !== 'theme.schema.json')
    .map((f) => JSON.parse(readFileSync(fileURLToPath(new URL(f, themesDir)), 'utf8')))
    .sort(
      (a, b) => Number(b.id === 'paper') - Number(a.id === 'paper') || a.id.localeCompare(b.id),
    );
}
/** The themes a value of FM_SCREENS_THEMES names; unknown ids are an error, not a skip. */
function screensThemes(value: string | undefined) {
  const all = builtInThemes();
  const ids = (value ?? '')
    .split(',')
    .map((v) => v.trim())
    .filter(Boolean);
  if (ids.includes('all')) return all;
  return ids.map((id) => {
    const theme = all.find((t) => t.id === id);
    if (!theme) throw new Error(`FM_SCREENS_THEMES: no built-in theme ${id}`);
    return theme;
  });
}
const shotThemes = screensThemes(process.env.FM_SCREENS_THEMES);
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

  test('the theme loop knows every built-in theme, Paper first (E47.2)', () => {
    expect(builtInThemes().map((t) => t.id)).toEqual(['paper', 'graphite', 'indigo', 'moss']);
    expect(screensThemes('all').map((t) => t.id)).toEqual(['paper', 'graphite', 'indigo', 'moss']);
    expect(screensThemes('moss, paper').map((t) => t.id)).toEqual(['moss', 'paper']);
    expect(screensThemes(undefined)).toEqual([]);
    expect(() => screensThemes('nope')).toThrow(/nope/);
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
    for (const theme of shotThemes.length ? shotThemes : [undefined]) {
      for (const scheme of ['light', 'dark'] as const) {
        for (const vp of viewports) {
          const [path, press] = route.split('@');
          const slug =
            prefix ?? (route.replace(/[^a-z0-9]+/gi, '-').replace(/^-|-$/g, '') || 'home');
          const themed = theme ? `${slug}-${theme.id}` : slug;
          const name = `${themed}-${scheme}-${vp.width}x${vp.height}.png`;
          test(name, async ({ page }) => {
            await serveFonts(page);
            await page.addInitScript((entries) => {
              for (const [k, v] of Object.entries(entries)) localStorage.setItem(k, v);
            }, storage);
            // The theme is a setting: it goes into the mock's saved settings, over any seeded ones.
            const seeded = { ...session };
            if (theme) {
              const key = 'fm.mock.settings.v1';
              seeded[key] = JSON.stringify({
                ...JSON.parse(seeded[key] ?? '{}'),
                themeId: theme.id,
              });
            }
            await page.addInitScript((entries) => {
              for (const [k, v] of Object.entries(entries)) sessionStorage.setItem(k, v);
            }, seeded);
            await page.setViewportSize(vp);
            await page.emulateMedia({ colorScheme: scheme });
            await page.goto(`/${query}#${path}`);
            await expect(page.locator('#app')).not.toBeEmpty();
            if (theme) {
              // The scene is under the theme: its accent for this scheme is on :root.
              await expect
                .poll(() =>
                  page.evaluate(() =>
                    getComputedStyle(document.documentElement)
                      .getPropertyValue('--fm-accent')
                      .trim(),
                  ),
                )
                .toBe(theme[scheme].accent);
            }
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
            for (const i of selectRows) {
              await listRows(page)
                .nth(i - 1)
                .click({ modifiers: ['ControlOrMeta'] });
            }
            if (selectRows.length) {
              await expect(page.getByRole('toolbar', { name: 'Selection' })).toBeVisible();
              await page.waitForTimeout(300); // the bar rises (200 ms)
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
              await page
                .getByRole('group', { name: 'Section' })
                .getByRole('button', { name: tab, exact: true })
                .click();
              await expect(
                page.locator(`[data-testid="family-panel"][data-section="${tab.toLowerCase()}"]`),
              ).toBeVisible();
              await page.waitForTimeout(400); // the old section fades out, the new one loads
              if (glyph) {
                await page.getByRole('searchbox', { name: 'Search glyphs' }).fill(glyph);
                await expect(page.getByRole('region', { name: 'Selected glyph' })).toBeVisible();
                if (compareWith) {
                  await page.getByRole('button', { name: 'Compare with…' }).click();
                  const dialog = page.getByRole('dialog');
                  await dialog
                    .getByRole('combobox', { name: 'Compare with font' })
                    .fill(compareWith);
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
              await compareRows(page, page.getByRole('tree', { name: 'Font list' }), pinRows);
              await page.getByRole('button', { name: /^Compare \d\/4$/ }).click();
              const sheet = page.getByRole('dialog', { name: 'Compare' });
              await expect(sheet).toBeVisible();
              // The sheet slides up (E48.1): wait until it rests on the bottom edge.
              await expect
                .poll(() => sheet.evaluate((d) => Math.round(d.getBoundingClientRect().bottom)))
                .toBe(page.viewportSize()!.height);
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
            if (dragTo) {
              const from = (await listRows(page).first().boundingBox())!;
              const to = (await page
                .getByRole('region', { name: 'Collections' })
                .getByRole('listitem', { name: dragTo, exact: true })
                .boundingBox())!;
              // Taken at its top edge and held on the row's lower edge, the ghost hangs below the
              // row, as on the board, and the row's "31 → 34" stays in view.
              await page.mouse.move(from.x + 24, from.y + 2);
              await page.mouse.down();
              await page.mouse.move(from.x + 44, from.y + 22, { steps: 4 });
              await page.mouse.move(to.x + 40, to.y + to.height - 2, { steps: 8 });
              await page.mouse.move(to.x + 41, to.y + to.height - 2, { steps: 2 });
              await expect(
                page.getByTestId('collection-count').filter({ hasText: '→' }),
              ).toBeVisible();
              await page.waitForTimeout(300); // the ghost lifts, the count crossfades
            }
            await page.screenshot({
              path: `${dir}/${name}`,
              fullPage: true,
              animations: 'disabled',
            });
          });
        }
      }
    }
  }
});
