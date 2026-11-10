import { expect, type Locator, type Page } from '@playwright/test';

// E48.5: C starts a new comparison with the cursor row as its base and opens the sheet;
// a row's compare icon adds to the set. These helpers build a set the way a user does.

const toast = (page: Page) => page.getByTestId('toast');
const sheet = (page: Page) => page.getByRole('dialog', { name: 'Compare' });

/** C on the cursor row: a new comparison opens; Escape closes the sheet again. */
export async function startWithC(page: Page): Promise<void> {
  await page.keyboard.press('c');
  await expect(toast(page)).toContainText('New comparison with');
  await expect(sheet(page)).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(sheet(page)).toBeHidden();
}

/** Adds row `index` of the list `tree` with its compare icon; the toast says `expected`. */
export async function addRow(
  page: Page,
  tree: Locator,
  index: number,
  expected: RegExp | string,
): Promise<void> {
  const row = tree.getByRole('treeitem').nth(index);
  await row.hover();
  await row.getByTestId('row-compare').click();
  await expect(toast(page)).toContainText(expected);
}

/** C on the cursor row (the first), then the next rows' icons: `count` faces in the set. */
export async function compareRows(page: Page, tree: Locator, count: number): Promise<void> {
  await startWithC(page);
  for (let i = 1; i < count; i++) await addRow(page, tree, i, `(${i + 1} of 4)`);
}
