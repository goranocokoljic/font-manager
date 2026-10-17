import { expect, test, type Page } from '@playwright/test';

const sidebar = (page: Page) => page.getByRole('navigation', { name: 'Library' });
const list = (page: Page) => page.getByRole('main');
const inspector = (page: Page) => page.getByRole('complementary', { name: 'Inspector' });
const width = async (page: Page, which: 'Library' | 'Inspector') =>
  (which === 'Library' ? sidebar(page) : inspector(page)).evaluate(
    (el) => el.getBoundingClientRect().width,
  );

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

test('shows sidebar, font list and inspector side by side', async ({ page }) => {
  await expect(sidebar(page)).toBeVisible();
  await expect(list(page)).toBeVisible();
  await expect(inspector(page)).toBeVisible();
  const [s, l, i] = await Promise.all(
    [sidebar(page), list(page), inspector(page)].map((el) => el.boundingBox()),
  );
  expect(s!.x + s!.width).toBeLessThanOrEqual(l!.x);
  expect(l!.x + l!.width).toBeLessThanOrEqual(i!.x);
  await expect(page.getByRole('separator')).toHaveCount(2);
});

test('dragging the dividers resizes the sidebar and the inspector', async ({ page }) => {
  const sidebarBefore = await width(page, 'Library');
  const inspectorBefore = await width(page, 'Inspector');
  await drag(page, 'Resize sidebar', 60);
  await drag(page, 'Resize inspector', -40);
  expect(await width(page, 'Library')).toBeCloseTo(sidebarBefore + 60, 0);
  expect(await width(page, 'Inspector')).toBeCloseTo(inspectorBefore + 40, 0);
});

test('a divider resizes with the arrow keys', async ({ page }) => {
  const before = await width(page, 'Library');
  await page.getByRole('separator', { name: 'Resize sidebar' }).focus();
  await page.keyboard.press('ArrowRight');
  expect(await width(page, 'Library')).toBe(before + 8);
});

test('the inspector collapses under 1,100 px and reopens with the shortcut', async ({ page }) => {
  await page.setViewportSize({ width: 1099, height: 700 });
  await expect(inspector(page)).toBeHidden();
  await page.keyboard.press('ControlOrMeta+i');
  await expect(inspector(page)).toBeVisible();
  await page.keyboard.press('ControlOrMeta+i');
  await expect(inspector(page)).toBeHidden();

  await page.setViewportSize({ width: 1100, height: 700 });
  await expect(inspector(page)).toBeVisible();
});

test('the toolbar button toggles the inspector', async ({ page }) => {
  const toggle = page.getByRole('button', { name: 'Inspector' });
  await expect(toggle).toHaveAttribute('aria-pressed', 'true');
  await toggle.click();
  await expect(inspector(page)).toBeHidden();
  await expect(toggle).toHaveAttribute('aria-pressed', 'false');
});

test('widths and the closed inspector are restored after a relaunch', async ({ page }) => {
  await drag(page, 'Resize sidebar', 48);
  const sidebarWidth = await width(page, 'Library');
  await page.keyboard.press('ControlOrMeta+i');
  await expect(inspector(page)).toBeHidden();

  await page.reload();
  await expect(sidebar(page)).toBeVisible();
  expect(await width(page, 'Library')).toBe(sidebarWidth);
  await expect(inspector(page)).toBeHidden();

  await page.keyboard.press('ControlOrMeta+i');
  await page.reload();
  await expect(inspector(page)).toBeVisible();
});
