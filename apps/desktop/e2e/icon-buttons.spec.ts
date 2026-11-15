import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';

// E50.6: one icon size everywhere. At 1280 × 800 every icon button on the screens the story
// names — the rows, the cards, the compare sheet, the family page, the selection bar and the
// catalogs — is a box of at least 28 × 28. `size="sm"` (24 px) is only for a sidebar row and
// a tag chip, so those are left out. Row actions come up on hover or focus and stay while a
// menu of the row is open.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function serve(page: Page): Promise<void> {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
}

/** Every icon button laid out inside `scope` (hidden-at-rest row actions too): its box. */
async function iconButtonBoxes(scope: Locator) {
  return scope.evaluate((root) =>
    [...root.querySelectorAll<HTMLElement>('.ui-icon-button:not([data-size="sm"])')]
      .filter((b) => b.getClientRects().length > 0)
      .map((b) => {
        const r = b.getBoundingClientRect();
        return {
          name: b.getAttribute('aria-label') ?? b.textContent ?? '',
          width: Math.round(r.width * 100) / 100,
          height: Math.round(r.height * 100) / 100,
        };
      }),
  );
}

/** Fails on any icon button in `scope` smaller than 28 × 28; at least `min` must be there. */
async function expectTargets(scope: Locator, where: string, min: number): Promise<void> {
  const boxes = await iconButtonBoxes(scope);
  expect(boxes.length, `${where}: icon buttons found`).toBeGreaterThanOrEqual(min);
  const small = boxes.filter((b) => b.width < 28 || b.height < 28);
  expect(small, `${where}: icon buttons under 28 × 28`).toEqual([]);
}

const tree = (page: Page) => page.getByRole('tree', { name: 'Font list' });
const body = (page: Page) => page.locator('body');

test('every icon button is at least 28 × 28: rows, cards, compare, family page, selection bar', async ({
  page,
}) => {
  await serve(page);
  await page.goto('/?mockFaces=3#/');
  await page.evaluate(() => localStorage.clear());
  await expect(tree(page).getByRole('treeitem')).toHaveCount(6);

  // Lines: every row's activate, compare and similar icons (three per row), and the toolbar.
  await expect(tree(page).getByTestId('row-activate').first()).toBeAttached();
  await expectTargets(tree(page), 'list rows', 6 * 3);
  await expectTargets(body(page), 'the library screen', 6 * 3 + 2);

  // The compare sheet.
  await tree(page).getByRole('treeitem').first().click();
  await page.keyboard.press('c');
  const sheet = page.getByRole('dialog', { name: 'Compare' });
  await expect(sheet).toBeVisible();
  await expectTargets(sheet, 'the compare sheet', 3);
  await page.keyboard.press('Escape');
  await expect(sheet).toBeHidden();

  // The Table: the compare icon of every row.
  await page.keyboard.press('3');
  const table = page.getByRole('treegrid', { name: 'Font table' });
  await expect(table).toBeVisible();
  await expectTargets(table, 'table rows', 6);

  // The family page.
  await page.goto('/?mockFaces=3&mockStyles=4#/family/1');
  await expect(page.getByTestId('family-meta')).toBeVisible();
  await expectTargets(body(page), 'the family page', 5);
});

test('wall cards and the selection bar keep 28 × 28 icon buttons', async ({ page }) => {
  await serve(page);
  await page.goto('/?mockLibrary=wall#/');
  await page.evaluate(() => localStorage.clear());
  await expect(tree(page)).toBeVisible();
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Grid' })
    .click();
  const wall = page.getByRole('listbox', { name: 'Font grid' });
  await expect(wall).toBeVisible();
  const cards = wall.getByRole('option');
  // The cards' compare icons sit in the box after each card, inside the wall's viewport.
  await expectTargets(wall.locator('xpath=..'), 'wall cards', 4);

  await cards.nth(0).click();
  await cards.nth(2).click({ modifiers: ['ControlOrMeta'] });
  const bar = page.getByRole('toolbar', { name: 'Selection' });
  await expect(bar).toBeVisible();
  await expectTargets(bar, 'the selection bar', 1);
});

test('a catalog item: a 28 px download arrow, no text button', async ({ page }) => {
  await serve(page);
  await page.goto('/?mockCatalog=1');
  await page
    .getByRole('region', { name: 'Catalogs' })
    .getByRole('button', { name: 'Google Fonts', exact: true })
    .click();
  const list = page.getByRole('list', { name: 'Google Fonts families' });
  await expect(list.getByRole('listitem')).toHaveCount(4);
  await expectTargets(list, 'catalog items', 4);
  const lato = list.getByRole('listitem').filter({ hasText: 'Lato' });
  const download = lato.getByRole('button', { name: 'Download Lato…' });
  await expect(download).toHaveAttribute('title', 'Download Lato…');
  await expect(download).toHaveText('');
  await expect(lato.getByRole('button', { name: 'Install Lato' })).toHaveCount(0);
});

test('row actions show on hover and focus, and stay while the row’s menu is open', async ({
  page,
}) => {
  await serve(page);
  await page.goto('/?mockFaces=3&mockStyles=4#/');
  await page.evaluate(() => localStorage.clear());
  await expect(tree(page).getByRole('treeitem').first()).toBeVisible();
  const row = (name: string) => tree(page).getByRole('treeitem', { name: new RegExp(`^${name},`) });
  const actionsOf = (name: string) => row(name).locator('.row-actions');
  const shown = (el: Locator) => el.evaluate((e) => getComputedStyle(e).visibility === 'visible');

  // Sample 2 is not the cursor: hidden at rest, shown on hover.
  await page.mouse.move(0, 0);
  await expect.poll(() => shown(actionsOf('Sample 2'))).toBe(false);
  await row('Sample 2').hover();
  await expect.poll(() => shown(actionsOf('Sample 2'))).toBe(true);
  await page.mouse.move(0, 0);
  await expect.poll(() => shown(actionsOf('Sample 2'))).toBe(false);

  // Focus inside the row (its activation dot) shows them without the pointer.
  await row('Sample 2').locator('[data-activation]').focus();
  await expect.poll(() => shown(actionsOf('Sample 2'))).toBe(true);
  await page.locator('body').click({ position: { x: 1, y: 1 } });

  // A permanent dot's menu: the pointer leaves the row for the menu, the actions stay.
  await row('Sample 1').click();
  await page.keyboard.press('Shift+A');
  const dot = row('Sample 1').locator('[data-activation]');
  await expect(dot).toHaveAttribute('data-activation', 'permanent');
  await row('Sample 3').click();
  await dot.click();
  const menu = page.getByRole('menu', { name: 'Activation of Sample 1' });
  await expect(menu).toBeVisible();
  await page.mouse.move(0, 0);
  await expect.poll(() => shown(actionsOf('Sample 1'))).toBe(true);
  await page.keyboard.press('Escape');
  await expect(menu).toBeHidden();
  // Esc gives the focus back to the dot; a click on another row takes it from there.
  await row('Sample 3').click();
  await expect.poll(() => shown(actionsOf('Sample 1'))).toBe(false);
});

// Screens for the review (E50.6): the 28 px icons in a hovered row, on the family page and in
// a catalog item (with the ring while one installs), at both review sizes. Only with FM_SCREENS_DIR.
test('@screens icon buttons in rows, the family page and the catalogs', async ({ page }) => {
  const dir = process.env.FM_SCREENS_DIR;
  test.skip(!dir, 'FM_SCREENS_DIR is not set');
  await serve(page);
  const shoot = async (name: string) => {
    for (const [w, h] of [
      [1280, 800],
      [1000, 700],
    ] as const) {
      await page.setViewportSize({ width: w, height: h });
      await page.screenshot({ path: `${dir}/icons-${name}-${w}x${h}.png` });
    }
  };
  await page.goto('/?mockFaces=3&mockStyles=4#/');
  await expect(tree(page).getByRole('treeitem').first()).toBeVisible();
  await tree(page).getByRole('treeitem').nth(1).hover();
  await shoot('lines');
  await page.goto('/?mockFaces=3&mockStyles=4#/family/1');
  await expect(page.getByTestId('family-meta')).toBeVisible();
  await shoot('family');
  await page.goto('/?mockCatalog=1');
  await page
    .getByRole('region', { name: 'Catalogs' })
    .getByRole('button', { name: 'Google Fonts', exact: true })
    .click();
  const list = page.getByRole('list', { name: 'Google Fonts families' });
  await expect(list.getByRole('listitem')).toHaveCount(4);
  await shoot('catalog');
});
