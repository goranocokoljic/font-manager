import { expect, test, type Page } from '@playwright/test';

const tab = (page: Page, name: string) => page.getByRole('tab', { name });
const bg = (page: Page) => page.evaluate(() => getComputedStyle(document.body).backgroundColor);

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto('/');
});

test('Ctrl/Cmd+, opens Settings with its four sections; Escape closes it', async ({ page }) => {
  await page.keyboard.press('ControlOrMeta+,');
  await expect(page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible();
  for (const name of ['Sources', 'Activation', 'Appearance', 'Advanced'])
    await expect(tab(page, name)).toBeVisible();
  await expect(tab(page, 'Sources')).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('list', { name: 'Sources' })).toContainText('/Users/demo/Fonts');

  await tab(page, 'Advanced').click();
  await expect(page.getByRole('spinbutton', { name: 'Preview cache size (MB)' })).toHaveValue(
    '1024',
  );
  await expect(
    page.getByRole('spinbutton', { name: 'Rescan sources every (minutes)' }),
  ).toHaveValue('60');
  await expect(
    page.getByRole('button', { name: /^Copy logs folder: .*font-manager\/logs$/ }),
  ).toBeVisible();

  await page.keyboard.press('Escape');
  await expect(page.getByRole('heading', { name: 'All fonts' })).toBeVisible();
});

test('theme and density apply at once, without a reload', async ({ page }) => {
  await page.goto('/#/settings?section=appearance');
  const light = await bg(page);
  await page.getByRole('radio', { name: 'Dark' }).check();
  await expect.poll(() => bg(page)).not.toBe(light);
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');

  await page.getByRole('radio', { name: 'Compact' }).check();
  await expect(page.locator('html')).toHaveAttribute('data-density', 'compact');
  await expect
    .poll(() => page.evaluate(() => getComputedStyle(document.documentElement).fontSize))
    .toBe('12px');

  await page.getByRole('radio', { name: 'Same as the system' }).check();
  await expect.poll(() => bg(page)).toBe(light);
});

test('the activation choice applies to the next activation', async ({ page }) => {
  await page.goto('/#/settings?section=activation');
  await page.getByRole('radio', { name: 'Activate this font anyway' }).check();
  await page.getByRole('radio', { name: 'Permanently (also after a restart)' }).check();
  await page.getByRole('button', { name: 'Close' }).click();

  // Face 6 clashes with a system font; with "anyway" it activates.
  await page.goto('/#/?face=6');
  await page.getByRole('button', { name: 'Activate' }).click();
  await expect(page.getByTestId('activation-state')).toHaveText('Active (permanent)');
});

test('a folder can be added, paused and removed; the system source stays', async ({ page }) => {
  await page.goto('/#/settings');
  await page.getByLabel('Folder to add').fill('/Users/demo/More fonts');
  await page.getByRole('button', { name: 'Add folder' }).click();
  const list = page.getByRole('list', { name: 'Sources' });
  await expect(list).toContainText('/Users/demo/More fonts');
  await expect(page.getByLabel('Folder to add')).toHaveValue('');

  await page.getByRole('checkbox', { name: 'Pause /Users/demo/More fonts' }).check();
  await expect(page.getByRole('checkbox', { name: 'Pause /Users/demo/More fonts' })).toBeChecked();
  await page.getByRole('button', { name: 'Remove /Users/demo/More fonts' }).click();
  await expect(list).not.toContainText('/Users/demo/More fonts');
  await expect(page.getByRole('button', { name: 'Remove /System/Library/Fonts' })).toBeDisabled();
});
