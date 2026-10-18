import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// Preview rows load face files from `/fmfont/<id>`; serve one fixture for all.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function openTable(page: Page, query = '') {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(`/${query}#/`);
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Table' })
    .click();
  const grid = page.getByRole('treegrid', { name: 'Font table' });
  await expect(grid.locator('.spacer [role="row"]').first()).toBeVisible();
  return grid;
}

const names = (grid: ReturnType<Page['getByRole']>) =>
  grid.locator('.spacer [role="row"] [data-column="name"] .text').allTextContents();
const headerNames = (grid: ReturnType<Page['getByRole']>) =>
  grid.getByRole('columnheader').evaluateAll((hs) => hs.map((h) => h.getAttribute('data-column')));

test("table mode shows the spec's columns for every family", async ({ page }) => {
  const grid = await openTable(page);
  await expect(grid.getByRole('columnheader')).toHaveText([
    /Activation/,
    /Name/,
    /Styles/,
    /Style/,
    /Weight/,
    /Format/,
    /Version/,
    /Manufacturer/,
    /Location/,
    /License/,
    /Date added/,
    /Tags/,
  ]);
  const inter = grid.getByRole('row', { name: 'Inter, 1 style' });
  await expect(inter.locator('[data-column="weightRange"]')).toHaveText('100–900');
  await expect(inter.locator('[data-column="format"]')).toHaveText(/TT\s*Variable/);
  await expect(inter.locator('[data-column="manufacturer"]')).toHaveText('rsms');
  await expect(inter.locator('[data-column="location"]')).toHaveText('/Users/demo/Fonts');
  await expect(inter.locator('[data-column="license"]')).toHaveText('OFL');
  await expect(inter.locator('[data-column="weightLine"] .mark')).toHaveCount(1);
  await expect(inter.locator('[data-column="activation"]')).toHaveText('Inactive');
});

test('columns sort, reorder and hide, and the setup survives a reload', async ({ page }) => {
  const grid = await openTable(page);
  expect(await names(grid)).toEqual(['Inter', 'JetBrains Mono', 'Legacy Grotesk']);

  const manufacturer = grid.getByRole('columnheader', { name: /Manufacturer/ });
  await manufacturer.getByRole('button').click();
  await expect(manufacturer).toHaveAttribute('aria-sort', 'ascending');
  await expect.poll(() => names(grid)).toEqual(['JetBrains Mono', 'Legacy Grotesk', 'Inter']);
  await manufacturer.getByRole('button').click();
  await expect(manufacturer).toHaveAttribute('aria-sort', 'descending');
  await expect.poll(() => names(grid)).toEqual(['Inter', 'Legacy Grotesk', 'JetBrains Mono']);

  // Reorder by dragging the Tags header onto Name (a window wide enough to show both).
  await page.setViewportSize({ width: 2400, height: 800 });
  await grid
    .getByRole('columnheader', { name: /Tags/ })
    .dragTo(grid.getByRole('columnheader', { name: /^Name/ }));
  expect((await headerNames(grid)).slice(0, 3)).toEqual(['activation', 'tags', 'name']);

  // Hide and move with the keyboard, through the Columns menu.
  await page.getByRole('button', { name: 'Columns' }).click();
  const menu = page.getByRole('group', { name: 'Table columns' });
  await menu.getByRole('checkbox', { name: 'Location', exact: true }).press('Space');
  await expect(grid.getByRole('columnheader', { name: /Location/ })).toHaveCount(0);
  await menu.getByRole('button', { name: 'Move Version up' }).press('Enter');
  await page.keyboard.press('Escape');
  await expect(menu).toHaveCount(0);
  const before = await headerNames(grid);
  expect(before.indexOf('version')).toBe(before.indexOf('format') - 1);

  await page.reload();
  const again = page.getByRole('treegrid', { name: 'Font table' });
  await expect(again.locator('.spacer [role="row"]').first()).toBeVisible();
  expect(await headerNames(again)).toEqual(before);
  await expect(again.getByRole('columnheader', { name: /Manufacturer/ })).toHaveAttribute(
    'aria-sort',
    'descending',
  );
  expect(await names(again)).toEqual(['Inter', 'Legacy Grotesk', 'JetBrains Mono']);

  // The list view keeps its own (default) order.
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'List' })
    .click();
  await expect(
    page.getByRole('tree', { name: 'Font list' }).getByRole('treeitem').first(),
  ).toHaveAttribute('aria-label', 'Inter, 1 style, not active');
});

test('the preview strip shows the selected family with its own size slider', async ({ page }) => {
  const grid = await openTable(page);
  const strip = page.getByRole('region', { name: 'Preview strip' });
  await expect(strip).toContainText('Select a family to preview it here.');

  await grid.getByRole('row', { name: 'JetBrains Mono, 1 style' }).click();
  const sample = strip.locator('[data-face="6"] .sample');
  await expect(sample).toBeVisible();
  await expect(sample).toHaveCSS('font-size', '28px'); // the toolbar size

  const slider = strip.getByRole('slider', { name: 'Strip size' });
  await slider.fill('96');
  await expect(sample).toHaveCSS('font-size', '96px');
  await expect(strip.locator('output')).toHaveText('96 px');
  // The toolbar size did not change.
  await expect(page.getByLabel('Size in pixels')).toHaveValue('28');

  // Keyboard: the cursor moves the strip to the next family.
  await grid.press('ArrowDown');
  await expect(strip.locator('[data-face="7"] .sample')).toHaveCSS('font-size', '96px');

  await strip.getByRole('button', { name: 'Use toolbar size' }).click();
  await expect(strip.locator('[data-face="7"] .sample')).toHaveCSS('font-size', '28px');
});

test('the table expands families to styles and keeps the list keyboard model', async ({ page }) => {
  const grid = await openTable(page, '?mockFaces=3&mockStyles=4');
  await grid.focus();
  await grid.press('End');
  await grid.press('ArrowRight');
  await expect(grid.locator('[role="row"][aria-level="2"]')).toHaveCount(4);
  await grid.press('ArrowDown');
  await grid.press('Shift+ArrowDown');
  await expect(grid.locator('[role="row"][aria-selected="true"]')).toHaveCount(2);
  await expect(page.getByText('2 selected')).toBeVisible();
});
