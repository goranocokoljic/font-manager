import { expect, test, type Page } from '@playwright/test';

const palette = (page: Page) => page.getByRole('dialog', { name: 'Command palette' });
const query = (page: Page) => page.getByRole('combobox', { name: 'Search commands' });
const options = (page: Page) => palette(page).getByRole('option');
const inspector = (page: Page) => page.getByRole('complementary', { name: 'Inspector' });

test.beforeEach(async ({ page }) => {
  await page.goto('/');
  await page.evaluate(() => localStorage.clear());
});

test('Ctrl/Cmd+K opens the palette with every action, fuzzy search narrows it', async ({
  page,
}) => {
  await page.goto('/#/?face=1');
  await expect(page.getByTestId('activation-state')).toHaveText('Not active');
  await page.keyboard.press('ControlOrMeta+k');
  await expect(palette(page)).toBeVisible();
  await expect(query(page)).toBeFocused();
  for (const title of [
    'Show all commands',
    'Show or hide the inspector',
    'Go to the sidebar',
    'Go to the font list',
    'Go to the inspector',
    'Activate or deactivate for this session',
    'Activate or deactivate permanently',
  ])
    await expect(options(page).filter({ hasText: title })).toHaveCount(1);

  await page.keyboard.type('gtfl');
  await expect(options(page).first()).toContainText('Go to the font list');
  await page.keyboard.press('Enter');
  await expect(palette(page)).toBeHidden();
  await expect(page.getByRole('main')).toBeFocused();

  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.press('Escape');
  await expect(palette(page)).toBeHidden();
  await expect(page.getByRole('main')).toBeFocused();
});

test('keyboard only: activate, go to columns and hide the inspector', async ({ page }) => {
  await page.goto('/#/?face=1');
  const state = page.getByTestId('activation-state');
  await expect(state).toHaveText('Not active');

  // A and Shift+A from the keyboard model.
  await page.keyboard.press('a');
  await expect(state).toHaveText('Active for this session');
  await page.keyboard.press('Shift+A');
  await expect(state).toHaveText('Active (permanent)');
  await page.keyboard.press('Shift+A');
  await expect(state).toHaveText('Not active');

  // The same actions through the palette.
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('session');
  await page.keyboard.press('Enter');
  await expect(state).toHaveText('Active for this session');

  // Tab moves from a column into its controls; columns follow in order:
  // sidebar, its divider, then the list.
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('sidebar');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('navigation', { name: 'Library' })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(page.getByRole('button', { name: 'Add folder' })).toBeFocused();
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('font list');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('main')).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await expect(page.getByRole('separator', { name: 'Resize sidebar' })).toBeFocused();

  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('hide insp');
  await page.keyboard.press('Enter');
  await expect(inspector(page)).toBeHidden();

  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('go insp');
  await page.keyboard.press('Enter');
  await expect(inspector(page)).toBeVisible();
  await expect(inspector(page)).toBeFocused();
});

test('typing in the palette does not trigger single-key shortcuts', async ({ page }) => {
  await page.goto('/#/?face=1');
  await expect(page.getByTestId('activation-state')).toHaveText('Not active');
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('a');
  await expect(query(page)).toHaveValue('a');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('activation-state')).toHaveText('Not active');
});

test('the status bar hint opens the palette', async ({ page }) => {
  await page.getByRole('button', { name: /Commands/ }).click();
  await expect(palette(page)).toBeVisible();
  await page.mouse.click(5, 5); // the backdrop
  await expect(palette(page)).toBeHidden();
});
