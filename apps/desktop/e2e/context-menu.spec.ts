import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { faceHash } from './family-page';

// E54.1: a right-click anywhere never shows the webview's own menu. One window listener cancels
// every `contextmenu` event; the app's own menus (Characters grid, Foundries sidebar) still open.
// A browser cannot show whether its native menu appeared, so each check reads the event's
// `defaultPrevented` from a window listener added after the app's, which sees the final state.

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page, hash = '#/', query = '') {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto(`/${query}${hash}`);
  await page.evaluate(() => {
    const w = window as unknown as { fmMenus: boolean[] };
    w.fmMenus = [];
    window.addEventListener('contextmenu', (e) => w.fmMenus.push(e.defaultPrevented));
  });
}

/** The `defaultPrevented` of every context menu event since the last call. */
const takeMenus = (page: Page) =>
  page.evaluate(() => {
    const w = window as unknown as { fmMenus: boolean[] };
    return w.fmMenus.splice(0);
  });

/** Right-clicks `target` and asserts the webview's menu was cancelled and no app menu opened. */
async function rightClickDoesNothing(page: Page, target: Locator) {
  await expect(target).toBeVisible();
  await target.click({ button: 'right' });
  expect(await takeMenus(page)).toEqual([true]);
  await expect(page.getByRole('menu')).toHaveCount(0);
}

const setMode = (page: Page, name: string) =>
  page.getByRole('group', { name: 'View mode' }).getByRole('button', { name, exact: true }).click();

test('a right-click on the list, the sidebar and the toolbar does nothing', async ({ page }) => {
  await open(page);
  const tree = page.getByRole('tree', { name: 'Font list' });
  await rightClickDoesNothing(page, tree.getByRole('treeitem').first());
  await rightClickDoesNothing(page, page.getByRole('navigation', { name: 'Library' }));
  await rightClickDoesNothing(page, page.locator('header.toolbar'));
});

test("a right-click in a text field shows the app's text menu, never the webview's", async ({
  page,
}) => {
  await open(page);
  // E54.2: Cut, Copy, Paste and Select all, the app's own.
  await page.getByRole('searchbox', { name: 'Search fonts' }).click({ button: 'right' });
  expect(await takeMenus(page)).toEqual([true]);
  await expect(page.getByRole('menu', { name: 'Edit text' })).toBeVisible();
});

test('the Menu key and Shift+F10 (a keyboard context menu) are cancelled too', async ({ page }) => {
  await open(page);
  const field = page.getByRole('searchbox', { name: 'Search fonts' });
  await field.focus();
  // Chromium fires no event for the keys in a headless run: dispatch what the webview sends.
  await field.dispatchEvent('contextmenu', { bubbles: true, cancelable: true, button: 0 });
  expect(await takeMenus(page)).toEqual([true]);
});

test('a right-click on a card and a table row does nothing', async ({ page }) => {
  await open(page);
  await setMode(page, 'Grid');
  await rightClickDoesNothing(page, page.getByRole('option').first());
  await setMode(page, 'Table');
  const grid = page.getByRole('treegrid', { name: 'Font table' });
  await rightClickDoesNothing(page, grid.locator('.spacer [role="row"]').first());
});

test('a right-click on the family page does nothing', async ({ page }) => {
  await open(page, faceHash(1));
  await rightClickDoesNothing(page, page.getByTestId('family-specimen'));
});

test('the Characters grid still opens its own menu', async ({ page }) => {
  await open(page, faceHash(1));
  await page
    .getByRole('group', { name: 'Section' })
    .getByRole('button', { name: 'Characters', exact: true })
    .click();
  const cell = page.getByRole('gridcell', { name: 'U+0061 a', exact: true });
  await cell.click({ button: 'right' });
  await expect(page.getByRole('menu', { name: 'Actions for U+0061' })).toBeVisible();
  expect(await takeMenus(page)).toEqual([true]);
});

test('the Foundries sidebar still opens its own menu', async ({ page }) => {
  await open(page, '#/', '?mockLibrary=wall');
  const nav = page.getByRole('navigation', { name: 'Library' });
  await nav.getByRole('button', { name: 'Foundries', exact: true }).click();
  const row = page.locator('#fm-foundries-list button[data-foundry="Font Diner"]');
  await row.scrollIntoViewIfNeeded();
  await page.evaluate(
    () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r(null)))),
  );
  await row.click({ button: 'right' });
  await expect(page.getByRole('menu', { name: 'Foundry Font Diner' })).toBeVisible();
  expect(await takeMenus(page)).toEqual([true]);
});
