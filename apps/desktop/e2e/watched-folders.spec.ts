import { expect, test, type Page } from '@playwright/test';

// E51.2: the folder watcher's library change on the MockApi. `__fmMockWatcherAdd` copies new
// families into the watched folder "Fonts" and sends the watcher's event, as fm-core does.

type Watcher = {
  __fmMockWatcherAdd: (count?: number) => number[];
  __fmMockWatcherRemove: (faces: number[]) => void;
};
const add = (page: Page, count = 1) =>
  page.evaluate((n) => (window as unknown as Watcher).__fmMockWatcherAdd(n), count);
const remove = (page: Page, faces: number[]) =>
  page.evaluate((ids) => (window as unknown as Watcher).__fmMockWatcherRemove(ids), faces);

const viewButton = (page: Page, name: string) =>
  page.getByRole('group', { name: 'View mode' }).getByRole('button', { name });
const toast = (page: Page) => page.getByTestId('toast');

async function openGrid(page: Page) {
  await page.goto('/#/');
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
  await viewButton(page, 'Grid').click();
  const wall = page.getByRole('listbox', { name: 'Font grid' });
  await expect(wall).toBeVisible();
  return wall;
}

test('a family the watcher adds appears on the wall, the toast shows, Show filters', async ({
  page,
}) => {
  const wall = await openGrid(page);
  const cards = wall.getByRole('option');
  await expect(cards).toHaveCount(3);
  await cards.nth(0).click();
  await expect(wall.locator('[role="option"][aria-selected="true"]')).toHaveCount(1);

  const faces = await add(page, 2);
  await expect(cards.filter({ hasText: 'Arrival 1' })).toHaveCount(1);
  await expect(cards).toHaveCount(5);
  // The list updated in place: the selection is still there.
  await expect(cards.nth(0)).toHaveAttribute('aria-selected', 'true');
  await expect(toast(page)).toContainText('2 fonts added from Fonts');

  await toast(page).getByRole('button', { name: 'Show' }).click();
  await expect(page.getByTestId('only-chip')).toHaveText('New in Fonts · 2');
  await expect(cards).toHaveCount(2);
  await expect(cards.filter({ hasText: 'Arrival 2' })).toHaveCount(1);

  // The chip's × takes the filter off.
  await page.getByTestId('only-chip').click();
  await expect(cards).toHaveCount(5);

  // A removal updates the list without a toast.
  await expect(toast(page)).toHaveCount(0);
  await remove(page, faces);
  await expect(cards).toHaveCount(3);
  await page.waitForTimeout(200);
  await expect(toast(page)).toHaveCount(0);
});

test('Lines and Table list the new family too', async ({ page }) => {
  await page.goto('/#/');
  const tree = page.getByRole('tree', { name: 'Font list' });
  await viewButton(page, 'List').click();
  await expect(tree).toBeVisible();
  await add(page, 1);
  await expect(tree.getByRole('treeitem').filter({ hasText: 'Arrival 1' })).toHaveCount(1);
  await viewButton(page, 'Table').click();
  await expect(page.getByRole('row', { name: /^Arrival 1,/ })).toHaveCount(1);
});
