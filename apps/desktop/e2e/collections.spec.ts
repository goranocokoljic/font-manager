import { expect, test, type Page } from '@playwright/test';

// Browser mode runs on the mock client; collections start empty.
const sidebar = (page: Page) => page.getByRole('region', { name: 'Collections' });
const row = (page: Page, name: string) =>
  sidebar(page).getByRole('listitem', { name, exact: true });
/** Opens a collection; its row buttons show. */
const open = (page: Page, name: string) =>
  row(page, name).getByRole('button', { name, exact: true }).click();

async function make(page: Page, name: string) {
  await sidebar(page).getByRole('button', { name: 'New', exact: true }).click();
  const input = sidebar(page).getByRole('textbox', { name: 'New collection name' });
  await input.fill(name);
  await input.press('Enter');
  await expect(row(page, name)).toBeVisible();
}

test('collections are made, renamed, nested, reordered and deleted', async ({ page }) => {
  await page.goto('/');
  await make(page, 'Clients');
  await make(page, 'Brand');

  await open(page, 'Brand');
  await row(page, 'Brand').getByRole('button', { name: 'Rename Brand' }).click();
  const rename = sidebar(page).getByRole('textbox', { name: 'New name for Brand' });
  await rename.fill('Marque');
  await rename.press('Enter');
  await expect(row(page, 'Marque')).toBeVisible();

  // Drag Marque into the middle of Clients: it nests.
  await row(page, 'Marque').dragTo(row(page, 'Clients'));
  await expect(row(page, 'Marque')).toHaveAttribute('data-depth', '1');

  await open(page, 'Marque');
  await row(page, 'Marque').getByRole('button', { name: 'Move Marque out of its parent' }).click();
  await expect(row(page, 'Marque')).toHaveAttribute('data-depth', '0');
  await row(page, 'Marque').getByRole('button', { name: 'Move Marque up' }).click();
  const order = sidebar(page).locator('li[data-collection]');
  await expect(order).toHaveText([/Marque/, /Clients/]);

  await open(page, 'Clients');
  // Nest again from the keyboard: In puts Clients inside Marque, the collection above it.
  const into = row(page, 'Clients').getByRole('button', {
    name: 'Move Clients into the collection above',
  });
  await into.focus();
  await page.keyboard.press('Enter');
  await expect(row(page, 'Clients')).toHaveAttribute('data-depth', '1');
  await open(page, 'Clients');
  await row(page, 'Clients').getByRole('button', { name: 'Delete Clients' }).click();
  await expect(row(page, 'Clients')).toHaveCount(0);
});

test('fonts dragged from the list land in a collection, and the sidebar counts them', async ({
  page,
}) => {
  await page.goto('/');
  await make(page, 'Mono');
  const list = page.getByRole('tree', { name: 'Font list' });
  const family = list.getByRole('treeitem', { name: /JetBrains Mono/ });
  await family.dragTo(row(page, 'Mono'));
  await expect(row(page, 'Mono').getByLabel('1 font', { exact: true })).toBeVisible();

  // Open it: the list shows what it holds, under its name.
  await open(page, 'Mono');
  await expect(page.getByRole('heading', { name: 'Mono' })).toBeVisible();
  await expect(list.getByRole('treeitem')).toHaveCount(1);
  await sidebar(page).getByRole('button', { name: 'All fonts' }).click();
  await expect(page.getByRole('heading', { name: 'All fonts' })).toBeVisible();
});

test('the palette adds the selected fonts to a collection', async ({ page }) => {
  await page.goto('/');
  await make(page, 'Picks');
  const list = page.getByRole('tree', { name: 'Font list' });
  await list.getByRole('treeitem', { name: /Inter/ }).first().click();
  await page.keyboard.press('Control+k');
  await page.getByRole('combobox', { name: 'Search commands' }).fill('add selected picks');
  await page.keyboard.press('Enter');
  await expect(row(page, 'Picks').getByLabel(/^[1-9]\d* fonts?$/)).toBeVisible();
});

test('"Export license inventory…" saves a CSV of the library or of one collection', async ({
  page,
}) => {
  await page.goto('/');
  const toast = page.getByTestId('toast');
  const palette = async (query: string) => {
    await page.keyboard.press('ControlOrMeta+k');
    await page.getByRole('combobox', { name: 'Search commands' }).fill(query);
    await page.keyboard.press('Enter');
  };

  // Under "All fonts" the palette exports the whole library.
  await palette('export license inventory');
  await expect(toast).toContainText(/Saved the license inventory: [1-9]\d* families\./);
  const all = Number(/(\d+) families/.exec((await toast.textContent()) ?? '')?.[1]);

  // A collection's own button exports its families only: none yet.
  await make(page, 'Picks');
  await open(page, 'Picks');
  await row(page, 'Picks')
    .getByRole('button', { name: 'Export the license inventory of Picks' })
    .click();
  await expect(toast).toContainText('Saved the license inventory: 0 families.');

  // With one family in it, the palette exports the open collection.
  const list = page.getByRole('tree', { name: 'Font list' });
  await page.getByRole('button', { name: 'All fonts', exact: true }).click();
  await list.getByRole('treeitem', { name: /Inter/ }).first().click();
  await palette('add selected picks');
  await expect(row(page, 'Picks').getByLabel(/^[1-9]\d* fonts?$/)).toBeVisible();
  await open(page, 'Picks');
  await palette('export license inventory');
  await expect(toast).toContainText('Saved the license inventory: 1 family.');
  expect(all).toBeGreaterThan(1);
});
