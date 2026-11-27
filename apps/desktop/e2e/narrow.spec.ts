import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { compareRows } from './compare-pins';
import { NARROW_VIEWS, SMALL_VIEWPORTS } from './narrow';

// E58.6: a larger interface is a smaller window. At 125 and 150 per cent the smallest window
// (1000 × 700) has the room of 800 × 560 and of 667 × 467 CSS px. Every view stays usable
// there: no horizontal scroll bar on the page, no control cut off, the toolbar wraps, and a
// wide table scrolls inside its own box. The panes fold by themselves when there is no room.

const font = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page, route: string) {
  await page.route('**/fmfont/*', (r) =>
    r.fulfill({ status: 200, contentType: 'font/ttf', body: font }),
  );
  await page.goto(`/#${route}`);
  await expect(page.locator('#app')).not.toBeEmpty();
}

/** How far the page itself scrolls sideways, in CSS px (0: no horizontal scroll bar). */
function pageOverflow(page: Page): Promise<number> {
  return page.evaluate(() => {
    const root = document.scrollingElement ?? document.documentElement;
    return Math.max(root.scrollWidth - root.clientWidth, document.body.scrollWidth - innerWidth);
  });
}

/**
 * The controls inside `scope` that the user cannot reach: outside the window, or clipped by
 * a box that hides its overflow. A control inside a box that scrolls (a wide table) is
 * reachable by scrolling that box.
 */
function cutOffControls(page: Page, scope = 'body'): Promise<string[]> {
  return page.evaluate((scope) => {
    const CONTROLS =
      'button, a[href], input:not([type=hidden]), select, textarea, [role=button], [role=tab], [role=combobox], [role=switch], [role=radio], [role=checkbox], [role=slider]';
    const out: string[] = [];
    const describe = (el: Element) =>
      (el.getAttribute('aria-label') ?? el.textContent ?? el.tagName).trim().slice(0, 60) ||
      el.outerHTML.slice(0, 80);
    for (const root of document.querySelectorAll(scope)) {
      for (const el of root.querySelectorAll(CONTROLS)) {
        const r = el.getBoundingClientRect();
        // Screen-reader-only and not rendered controls take no room on screen.
        if (r.width <= 1 || r.height <= 1) continue;
        const style = getComputedStyle(el);
        if (style.visibility === 'hidden' || style.display === 'none') continue;
        let reason = '';
        let scrolls = false;
        for (let a = el.parentElement; a && a !== document.body; a = a.parentElement) {
          const s = getComputedStyle(a);
          if (s.overflowX === 'auto' || s.overflowX === 'scroll') {
            scrolls = true;
            break;
          }
          if (s.overflowX === 'hidden' || s.overflowX === 'clip') {
            const b = a.getBoundingClientRect();
            if (r.left < b.left - 1 || r.right > b.right + 1) {
              reason = 'clipped';
              break;
            }
          }
        }
        if (!reason && !scrolls && (r.left < -1 || r.right > innerWidth + 1)) reason = 'outside';
        if (reason) out.push(`${reason}: ${describe(el)}`);
      }
    }
    return out;
  }, scope);
}

/** The toolbars of the page: the view's top bar and every `role=toolbar`. */
const TOOLBARS = '[data-top-bar], [role="toolbar"]';

const box = async (l: Locator) => (await l.boundingBox())!;

for (const vp of SMALL_VIEWPORTS) {
  test.describe(`at ${vp.width} × ${vp.height}`, () => {
    test.use({ viewport: vp });

    for (const view of NARROW_VIEWS) {
      test(`the ${view.name}: no page scroll, no control cut off, every toolbar control in reach`, async ({
        page,
      }) => {
        await open(page, view.route);
        await expect(view.ready(page).first()).toBeVisible();
        await page.waitForTimeout(300); // panels finish their fade
        expect(await pageOverflow(page)).toBeLessThanOrEqual(0);
        // The library, the family page and the Studio have a top bar; the shells of Settings,
        // Duplicates and Problems have none (their close button is checked with the page).
        if (view.toolbar) {
          expect(await page.locator(TOOLBARS).count()).toBeGreaterThan(0);
          expect(await cutOffControls(page, TOOLBARS)).toEqual([]);
        }
        expect(await cutOffControls(page)).toEqual([]);
      });
    }

    for (const shell of ['Settings', 'Duplicates', 'Problems'] as const) {
      test(`${shell} keeps its navigation beside the panel`, async ({ page }) => {
        await open(page, `/${shell.toLowerCase()}`);
        const nav = page.getByRole('navigation', { name: shell });
        await expect(nav).toBeVisible();
        expect(await nav.locator('a, button').count()).toBeGreaterThan(1);
        const n = await box(nav);
        expect(n.width).toBeGreaterThan(150);
        expect(n.x + n.width).toBeLessThan(vp.width / 2);
      });
    }

    test('a wide table scrolls inside its own box, not the page', async ({ page }) => {
      await open(page, '/');
      await page
        .getByRole('group', { name: 'View mode' })
        .getByRole('button', { name: 'Table' })
        .click();
      const grid = page.getByRole('treegrid', { name: 'Font table' });
      await expect(grid.locator('[role="row"]').nth(1)).toBeVisible();
      expect(await pageOverflow(page)).toBeLessThanOrEqual(0);
      const g = await box(grid);
      expect(g.x).toBeGreaterThanOrEqual(0);
      expect(g.x + g.width).toBeLessThanOrEqual(vp.width);
      expect(await cutOffControls(page, TOOLBARS)).toEqual([]);
    });

    test("Compare's faces stack one above the other", async ({ page }) => {
      await open(page, '/');
      const tree = page.getByRole('tree', { name: 'Font list' });
      await tree.getByRole('treeitem').first().click();
      await compareRows(page, tree, 2);
      await page.getByRole('button', { name: /^Compare \d\/4$/ }).click();
      const sheet = page.getByRole('dialog', { name: 'Compare' });
      await sheet
        .getByRole('radiogroup', { name: 'Mode' })
        .getByRole('radio', { name: 'Grid' })
        .check();
      const faces = sheet.getByRole('list', { name: 'Grid' }).getByRole('listitem');
      await expect(faces).toHaveCount(2);
      const [a, b] = [await box(faces.nth(0)), await box(faces.nth(1))];
      expect(b.y).toBeGreaterThanOrEqual(a.y + a.height - 1);
      expect(Math.abs(a.x - b.x)).toBeLessThanOrEqual(1);
      expect(await pageOverflow(page)).toBeLessThanOrEqual(0);
    });

    test("the Studio's panels stack, the roles above the page", async ({ page }) => {
      await open(page, '/studio?face=1');
      const panels = page.getByRole('region', { name: /^Panel \d: / });
      await expect(panels).toHaveCount(1);
      await page.getByRole('button', { name: 'Compare 2 pairings' }).click();
      await expect(panels).toHaveCount(2);
      const [a, b] = [await box(panels.nth(0)), await box(panels.nth(1))];
      // Each as wide as the window, the second under the first.
      expect(Math.abs(a.x - b.x)).toBeLessThanOrEqual(1);
      expect(a.width).toBeGreaterThan(vp.width - 2);
      expect(b.y).toBeGreaterThanOrEqual(a.y + a.height - 1);
      // In a panel the roles rail is above the page.
      const rail = await box(panels.nth(0).locator('.rail'));
      const paper = await box(panels.nth(0).locator('.layout').first());
      expect(paper.y).toBeGreaterThanOrEqual(rail.y + rail.height - 1);
      expect(await pageOverflow(page)).toBeLessThanOrEqual(0);
      expect(await cutOffControls(page, TOOLBARS)).toEqual([]);
    });
  });
}

test.describe('the panes fold by themselves', () => {
  const sidebar = (page: Page) => page.getByRole('navigation', { name: 'Library' });
  const toggle = (page: Page) => page.getByRole('button', { name: 'Sidebar' });

  test.beforeEach(async ({ page }) => {
    await open(page, '/');
    await page.evaluate(() => localStorage.clear());
    await page.reload();
    await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
  });

  test('the sidebar folds without room and comes back with it', async ({ page }) => {
    await expect(sidebar(page)).toBeVisible();
    await page.setViewportSize({ width: 800, height: 560 });
    await expect(sidebar(page)).toBeVisible();
    await page.setViewportSize({ width: 667, height: 467 });
    await expect(sidebar(page)).toBeHidden();
    await expect(toggle(page)).toHaveAttribute('aria-pressed', 'false');
    // The list takes the window.
    const list = await box(page.getByRole('main'));
    expect(list.x).toBe(0);
    expect(list.width).toBe(667);
    await page.setViewportSize({ width: 1000, height: 700 });
    await expect(sidebar(page)).toBeVisible();
    await expect(toggle(page)).toHaveAttribute('aria-pressed', 'true');
  });

  test('a fold by itself is not saved: with room again the sidebar is back', async ({ page }) => {
    await page.setViewportSize({ width: 667, height: 467 });
    await expect(sidebar(page)).toBeHidden();
    await page.reload();
    await expect(sidebar(page)).toBeHidden();
    await page.setViewportSize({ width: 1280, height: 800 });
    await expect(sidebar(page)).toBeVisible();
  });

  test('a sidebar folded by hand stays folded when the room comes back', async ({ page }) => {
    await page.keyboard.press('[');
    await expect(sidebar(page)).toBeHidden();
    await page.setViewportSize({ width: 667, height: 467 });
    await page.setViewportSize({ width: 1280, height: 800 });
    await expect(sidebar(page)).toBeHidden();
    await page.reload();
    await expect(sidebar(page)).toBeHidden();
  });

  test('the toggle shows a folded sidebar for a look, and the choice stays', async ({ page }) => {
    await page.setViewportSize({ width: 667, height: 467 });
    await expect(sidebar(page)).toBeHidden();
    await toggle(page).click();
    await expect(sidebar(page)).toBeVisible();
    await expect(toggle(page)).toHaveAttribute('aria-pressed', 'true');
    await toggle(page).click();
    await expect(sidebar(page)).toBeHidden();
    await page.setViewportSize({ width: 1000, height: 700 });
    await expect(sidebar(page)).toBeVisible();
  });
});
