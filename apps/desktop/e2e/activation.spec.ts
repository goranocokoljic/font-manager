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

async function open(page: Page, query = '?mockFaces=3&mockStyles=4') {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(`/${query}#/`);
  const tree = page.getByRole('tree', { name: 'Font list' });
  await expect(tree.getByRole('treeitem').first()).toBeVisible();
  return tree;
}

const dot = (page: Page, family: string) =>
  page.getByRole('treeitem', { name: new RegExp(`^${family},`) }).locator('[data-activation]');
const toast = (page: Page) => page.getByTestId('toast');
const activeCount = (page: Page) => page.getByText(/fonts? active$/);

test('A on selected families activates them all, shows their dots and undoes from the toast', async ({
  page,
}) => {
  const tree = await open(page);
  await tree.getByRole('treeitem', { name: /^Sample 1,/ }).click();
  await page.keyboard.press('Shift+ArrowDown');
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'inactive');

  await page.keyboard.press('a');
  await expect(toast(page)).toContainText('Activated 8 fonts for this session.');
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'active');
  await expect(dot(page, 'Sample 2')).toHaveAttribute('data-activation', 'active');
  await expect(dot(page, 'Sample 3')).toHaveAttribute('data-activation', 'inactive');
  await expect(tree.getByRole('treeitem', { name: /^Sample 1,/ })).toHaveAccessibleName(
    'Sample 1, 4 styles, active',
  );
  await expect(activeCount(page)).toHaveText('8 fonts active');

  await toast(page).getByRole('button', { name: 'Undo' }).click();
  await expect(toast(page)).toHaveText(/Undone\./);
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'inactive');
  await expect(activeCount(page)).toHaveText('0 fonts active');
});

test('a family with some active styles shows a partial dot; Shift+A makes all permanent', async ({
  page,
}) => {
  const tree = await open(page);
  const family = tree.getByRole('treeitem', { name: /^Sample 1,/ });
  await family.click();
  await page.keyboard.press('ArrowRight'); // expand
  const style = tree.getByRole('treeitem', { level: 2 }).first();
  await style.click();
  await page.keyboard.press('a');
  await expect(style.locator('[data-activation]')).toHaveAttribute('data-activation', 'active');
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'partial');
  // One style is not a bulk action: its outcome is in the inspector, not a toast.
  await expect(toast(page)).toHaveCount(0);

  await family.click();
  await page.keyboard.press('Shift+A');
  await expect(toast(page)).toContainText('Activated 4 fonts permanently.');
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'active');
  await page.keyboard.press('Shift+A');
  await expect(toast(page)).toContainText('Deactivated 4 fonts.');
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'inactive');
});

test('grid cards and table rows show the same dot', async ({ page }) => {
  const tree = await open(page);
  await tree.getByRole('treeitem', { name: /^Sample 2,/ }).click();
  await page.keyboard.press('a');
  await expect(dot(page, 'Sample 2')).toHaveAttribute('data-activation', 'active');

  await page.keyboard.press('3');
  const grid = page.getByRole('listbox', { name: 'Font grid' });
  const card = grid.getByRole('option', { name: /^Sample 2,/ });
  await expect(card.locator('[data-activation]')).toHaveAttribute('data-activation', 'active');
  await expect(
    grid.getByRole('option', { name: /^Sample 1,/ }).locator('[data-activation]'),
  ).toHaveAttribute('data-activation', 'inactive');

  await page.keyboard.press('2');
  const table = page.getByRole('treegrid', { name: 'Font table' });
  await expect(
    table.getByRole('row', { name: /^Sample 2,/ }).locator('[data-activation]'),
  ).toHaveAttribute('data-activation', 'active');
});

test('"Deactivate all" in the palette deactivates every face and the toast undoes it', async ({
  page,
}) => {
  const tree = await open(page);
  await tree.getByRole('treeitem', { name: /^Sample 1,/ }).click();
  await page.keyboard.press('Shift+A');
  await tree.getByRole('treeitem', { name: /^Sample 3,/ }).click();
  await page.keyboard.press('a');
  await expect(activeCount(page)).toHaveText('8 fonts active');

  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('deactivate all');
  const palette = page.getByRole('dialog', { name: 'Command palette' });
  await expect(palette.getByRole('option').first()).toContainText('Deactivate all');
  await page.keyboard.press('Enter');
  await expect(toast(page)).toContainText('Deactivated 8 fonts.');
  await expect(activeCount(page)).toHaveText('0 fonts active');
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'inactive');

  await toast(page).getByRole('button', { name: 'Undo' }).click();
  await expect(activeCount(page)).toHaveText('8 fonts active');
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'active');
  await expect(dot(page, 'Sample 3')).toHaveAttribute('data-activation', 'active');
  // The modes came back too: Shift+A on the permanent family turns it off.
  await tree.getByRole('treeitem', { name: /^Sample 1,/ }).click();
  await page.keyboard.press('Shift+A');
  await expect(toast(page)).toContainText('Deactivated 4 fonts.');
});

test('the toast is announced and stays while the pointer is on it', async ({ page }) => {
  await page.clock.install();
  const tree = await open(page);
  await tree.getByRole('treeitem', { name: /^Sample 1,/ }).click();
  await page.keyboard.press('a');
  const region = page.getByRole('status').filter({ has: toast(page) });
  await expect(region).toHaveAttribute('aria-live', 'polite');
  await toast(page).hover();
  await page.clock.runFor(30_000);
  await expect(toast(page)).toBeVisible();
  await page.mouse.move(5, 5);
  await page.clock.runFor(10_000);
  await expect(toast(page)).toHaveCount(0);
});
