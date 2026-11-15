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
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'off');

  await page.keyboard.press('a');
  await expect(toast(page)).toContainText('Activated 8 fonts for this session.');
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'session');
  await expect(dot(page, 'Sample 2')).toHaveAttribute('data-activation', 'session');
  await expect(dot(page, 'Sample 3')).toHaveAttribute('data-activation', 'off');
  await expect(tree.getByRole('treeitem', { name: /^Sample 1,/ })).toHaveAccessibleName(
    'Sample 1, 4 styles, active',
  );
  await expect(activeCount(page)).toHaveText('8 fonts active');

  await toast(page).getByRole('button', { name: 'Undo' }).click();
  await expect(toast(page)).toHaveText(/Undone\./);
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'off');
  await expect(activeCount(page)).toHaveText('0 fonts active');
});

test('a family with an active style shows a session dot; Shift+A makes all permanent', async ({
  page,
}) => {
  const tree = await open(page);
  const family = tree.getByRole('treeitem', { name: /^Sample 1,/ });
  await family.click();
  await page.keyboard.press('ArrowRight'); // expand
  const style = tree.getByRole('treeitem', { level: 2 }).first();
  await style.click();
  await page.keyboard.press('a');
  await expect(style.locator('[data-activation]')).toHaveAttribute('data-activation', 'session');
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'session');
  // One style is not a bulk action: its state shows in the count line, not a toast.
  await expect(toast(page)).toHaveCount(0);
  await expect(page.getByTestId('selected-row-state')).toHaveText('active for this session');

  await family.click();
  await page.keyboard.press('Shift+A');
  await expect(toast(page)).toContainText('Activated 4 fonts permanently.');
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'permanent');
  await page.keyboard.press('Shift+A');
  await expect(toast(page)).toContainText('Deactivated 4 fonts.');
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'off');
});

test('grid cards and table rows show the same dot', async ({ page }) => {
  const tree = await open(page);
  await tree.getByRole('treeitem', { name: /^Sample 2,/ }).click();
  await page.keyboard.press('a');
  await expect(dot(page, 'Sample 2')).toHaveAttribute('data-activation', 'session');

  await page.keyboard.press('1');
  const grid = page.getByRole('listbox', { name: 'Font grid' });
  const card = grid.getByRole('option', { name: /^Sample 2,/ });
  await expect(card.locator('[data-activation]')).toHaveAttribute('data-activation', 'session');
  await expect(
    grid.getByRole('option', { name: /^Sample 1,/ }).locator('[data-activation]'),
  ).toHaveAttribute('data-activation', 'off');

  await page.keyboard.press('3');
  const table = page.getByRole('treegrid', { name: 'Font table' });
  await expect(
    table.getByRole('row', { name: /^Sample 2,/ }).locator('[data-activation]'),
  ).toHaveAttribute('data-activation', 'session');
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
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'off');

  await toast(page).getByRole('button', { name: 'Undo' }).click();
  await expect(activeCount(page)).toHaveText('8 fonts active');
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'permanent');
  await expect(dot(page, 'Sample 3')).toHaveAttribute('data-activation', 'session');
  // The modes came back too: Shift+A on the permanent family turns it off.
  await tree.getByRole('treeitem', { name: /^Sample 1,/ }).click();
  await page.keyboard.press('Shift+A');
  await expect(toast(page)).toContainText('Deactivated 4 fonts.');
});

test('"Deactivate everything not in the current set" keeps the set picked in the sidebar', async ({
  page,
}) => {
  const tree = await open(page);
  // Inter is in the sample set "Client brand"; Sample 1 is not.
  await tree.getByRole('treeitem', { name: /^Inter,/ }).click();
  await page.keyboard.press('a');
  await tree.getByRole('treeitem', { name: /^Sample 1,/ }).click();
  await page.keyboard.press('a');
  await expect(activeCount(page)).toHaveText('5 fonts active');

  const palette = page.getByRole('dialog', { name: 'Command palette' });
  const action = palette.getByRole('option', {
    name: /Deactivate everything not in the current set/,
  });
  // No current set yet: listed but dimmed.
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('not in the current set');
  await expect(action).toHaveAttribute('aria-disabled', 'true');
  await page.keyboard.press('Escape');

  const sets = page.getByRole('region', { name: 'Project sets' });
  const set = sets.getByRole('button', { name: /^Client brand/ });
  await set.click();
  await expect(set).toHaveAttribute('aria-pressed', 'true');

  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('not in the current set');
  await expect(action).not.toHaveAttribute('aria-disabled', 'true');
  await page.keyboard.press('Enter');
  await expect(toast(page)).toContainText('Deactivated 4 fonts.');
  await expect(activeCount(page)).toHaveText('1 font active');
  await expect(dot(page, 'Inter')).toHaveAttribute('data-activation', 'session');
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'off');

  await toast(page).getByRole('button', { name: 'Undo' }).click();
  await expect(activeCount(page)).toHaveText('5 fonts active');
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'session');
});

test('the toast is announced and stays while the pointer is on its Undo button', async ({
  page,
}) => {
  await page.clock.install();
  const tree = await open(page);
  await tree.getByRole('treeitem', { name: /^Sample 1,/ }).click();
  await page.keyboard.press('a');
  const region = page.getByRole('status').filter({ has: toast(page) });
  await expect(region).toHaveAttribute('aria-live', 'polite');
  // E48.6: the toast is click-through; only its Undo button takes the pointer.
  await toast(page).getByRole('button', { name: 'Undo' }).hover();
  await page.clock.runFor(30_000);
  await expect(toast(page)).toBeVisible();
  await page.mouse.move(5, 5);
  await page.clock.runFor(10_000);
  await expect(toast(page)).toHaveCount(0);
});

// E50.5: the dot is a 24 px button that toggles session activation; a permanent dot opens its menu.
test('a click on the dot toggles the session; a permanent dot opens its menu', async ({ page }) => {
  await open(page);
  const d = dot(page, 'Sample 1');
  await expect(d).toHaveAttribute(
    'aria-label',
    'Sample 1: inactive. Click to activate it for this session.',
  );
  const box = await d.boundingBox();
  expect(box?.width).toBe(24);
  expect(box?.height).toBe(24);
  const inner = await d.locator('.dot').boundingBox();
  expect(inner?.width).toBe(10);

  await d.click();
  await expect(toast(page)).toContainText('Activated 4 fonts for this session.');
  await expect(d).toHaveAttribute('data-activation', 'session');
  await expect(d).toHaveAttribute(
    'aria-label',
    'Sample 1: active for this session. Click to deactivate it.',
  );
  await d.click();
  await expect(toast(page)).toContainText('Deactivated 4 fonts.');
  await expect(d).toHaveAttribute('data-activation', 'off');

  // Permanent: the click opens the menu; the removal is its item.
  await page.getByRole('treeitem', { name: /^Sample 1,/ }).click();
  await page.keyboard.press('Shift+A');
  await expect(d).toHaveAttribute('data-activation', 'permanent');
  await expect(d.locator('.dot')).toHaveCSS('outline-style', 'solid');
  await d.click();
  const menu = page.getByRole('menu', { name: 'Activation of Sample 1' });
  await expect(menu).toBeVisible();
  await expect(d).toHaveAttribute('data-activation', 'permanent');
  await menu.getByRole('menuitem', { name: 'Deactivate' }).click();
  await expect(toast(page)).toContainText('Deactivated 4 fonts.');
  await expect(d).toHaveAttribute('data-activation', 'off');
});

test('the same dot in Lines, the Wall, the Table and the Family page hero', async ({ page }) => {
  const tree = await open(page);
  await expect(tree.getByTestId('activation-dot').first()).toBeVisible();
  await page.keyboard.press('1');
  const grid = page.getByRole('listbox', { name: 'Font grid' });
  const card = grid.getByRole('option', { name: /^Sample 2,/ });
  await card.getByTestId('activation-dot').click();
  await expect(card.getByTestId('activation-dot')).toHaveAttribute('data-activation', 'session');
  await page.keyboard.press('3');
  const table = page.getByRole('treegrid', { name: 'Font table' });
  const row = table.getByRole('row', { name: /^Sample 2,/ });
  await expect(row.getByTestId('activation-dot')).toHaveAttribute('data-activation', 'session');
  await row.getByTestId('activation-dot').click();
  await expect(row.getByTestId('activation-dot')).toHaveAttribute('data-activation', 'off');

  await page.goto('/?mockFaces=3&mockStyles=4#/family/1');
  const hero = page.getByTestId('family-meta').getByTestId('activation-dot');
  await expect(hero).toHaveAttribute('data-activation', 'off');
  await hero.click();
  await expect(hero).toHaveAttribute('data-activation', 'session');
});

// Screens for the review (E50.5): the three dot states in Lines, the Wall, the Table and the
// Family page hero, at both review sizes. Runs only with FM_SCREENS_DIR set.
test('@screens the three dot states in every view', async ({ page }) => {
  const dir = process.env.FM_SCREENS_DIR;
  test.skip(!dir, 'FM_SCREENS_DIR is not set');
  const tree = await open(page);
  await tree.getByRole('treeitem', { name: /^Sample 1,/ }).click();
  await page.keyboard.press('Shift+A');
  await expect(dot(page, 'Sample 1')).toHaveAttribute('data-activation', 'permanent');
  await dot(page, 'Sample 2').click();
  await expect(dot(page, 'Sample 2')).toHaveAttribute('data-activation', 'session');
  await page.mouse.move(0, 0);
  const sizes = [
    [1280, 800],
    [1000, 700],
  ] as const;
  const shoot = async (name: string) => {
    for (const [w, h] of sizes) {
      await page.setViewportSize({ width: w, height: h });
      await page.screenshot({ path: `${dir}/dots-${name}-${w}x${h}.png` });
    }
  };
  await shoot('lines');
  await page.keyboard.press('1');
  await shoot('wall');
  await page.keyboard.press('3');
  await shoot('table');
  await page.keyboard.press('2');
  await dot(page, 'Sample 1').click();
  await expect(page.getByRole('menu', { name: 'Activation of Sample 1' })).toBeVisible();
  await page.screenshot({ path: `${dir}/dots-menu-1000x700.png` });
  await page.keyboard.press('Escape');
  await page.getByRole('treeitem', { name: /^Sample 1,/ }).dblclick();
  await expect(page.getByTestId('family-meta').getByTestId('activation-dot')).toHaveAttribute(
    'data-activation',
    'permanent',
  );
  await shoot('family');
});
