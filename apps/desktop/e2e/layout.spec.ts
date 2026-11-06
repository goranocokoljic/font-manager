import { expect, test, type Page } from '@playwright/test';

const sidebar = (page: Page) => page.getByRole('navigation', { name: 'Library' });
const list = (page: Page) => page.getByRole('main');
const width = async (page: Page) =>
  sidebar(page).evaluate((el) => el.getBoundingClientRect().width);

async function drag(page: Page, name: string, dx: number) {
  const box = await page.getByRole('separator', { name }).boundingBox();
  if (!box) throw new Error(`no divider ${name}`);
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + dx, y, { steps: 5 });
  await page.mouse.up();
}

test.beforeEach(async ({ page }) => {
  await page.goto('/');
  await page.evaluate(() => localStorage.clear());
  await page.reload();
});

test('shows sidebar and font list side by side, with no inspector', async ({ page }) => {
  await expect(sidebar(page)).toBeVisible();
  await expect(list(page)).toBeVisible();
  await expect(page.getByRole('complementary', { name: 'Inspector' })).toHaveCount(0);
  const [s, l] = await Promise.all([sidebar(page), list(page)].map((el) => el.boundingBox()));
  expect(s!.x + s!.width).toBeLessThanOrEqual(l!.x);
  // The list runs to the window's end.
  expect(l!.x + l!.width).toBe(page.viewportSize()!.width);
  await expect(page.getByRole('separator')).toHaveCount(1);
});

test('dragging the divider resizes the sidebar', async ({ page }) => {
  const before = await width(page);
  await drag(page, 'Resize sidebar', 60);
  expect(await width(page)).toBeCloseTo(before + 60, 0);
});

test('a divider resizes with the arrow keys', async ({ page }) => {
  const before = await width(page);
  await page.getByRole('separator', { name: 'Resize sidebar' }).focus();
  await page.keyboard.press('ArrowRight');
  expect(await width(page)).toBe(before + 8);
});

test('the list takes the whole width on a narrow window, and [ hides the sidebar', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1000, height: 700 });
  const open = (await list(page).boundingBox())!;
  expect(open.x + open.width).toBe(1000);
  await page.keyboard.press('[');
  await expect(sidebar(page)).toBeHidden();
  expect((await list(page).boundingBox())!.width).toBe(1000);
  await page.keyboard.press('[');
  await expect(sidebar(page)).toBeVisible();
});

test('the toolbar button toggles the sidebar', async ({ page }) => {
  const toggle = page.getByRole('button', { name: 'Sidebar' });
  await expect(toggle).toHaveAttribute('aria-pressed', 'true');
  await toggle.click();
  await expect(sidebar(page)).toBeHidden();
  await expect(toggle).toHaveAttribute('aria-pressed', 'false');
});

test('the width and the closed sidebar are restored after a relaunch', async ({ page }) => {
  await drag(page, 'Resize sidebar', 48);
  const sidebarWidth = await width(page);
  await page.reload();
  await expect(sidebar(page)).toBeVisible();
  expect(await width(page)).toBe(sidebarWidth);

  await page.keyboard.press('[');
  await expect(sidebar(page)).toBeHidden();
  await page.reload();
  await expect(list(page)).toBeVisible();
  await expect(sidebar(page)).toBeHidden();
});
