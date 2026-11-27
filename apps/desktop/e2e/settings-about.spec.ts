// E57.7: Settings › About as on board 15 — the quiet panel with the logotype and the version,
// the state of updates as the title of one row, the levels as cards, the platform notes.
import { expect, test } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light' });
});

test('About opens with the logotype, one line and the version from app_info', async ({ page }) => {
  await page.goto('/#/settings?section=about');
  const hero = page.getByTestId('app-info');
  await expect(hero.getByRole('img', { name: 'Font Fu' })).toBeVisible();
  await expect(hero).toContainText(
    'A free, open-source font manager for Windows, macOS and Linux.',
  );
  await expect(hero).toContainText(/API \d+\.\d+\.\d+ ·/);
  // The logotype stands at the size of the board's wordmark.
  const logo = await hero.getByRole('img', { name: 'Font Fu' }).boundingBox();
  expect(logo?.height).toBe(72);
  // The platform notes sit in a quiet panel under their heading.
  await expect(page.getByRole('heading', { name: 'Platform notes' })).toBeVisible();
  await expect(page.getByRole('list', { name: 'Platform notes' }).locator('li')).not.toHaveCount(0);
});

test('the state of updates is the row title, announced as a status, by keyboard alone', async ({
  page,
}) => {
  await page.goto('/#/settings?section=about');
  const updates = page.getByRole('region', { name: 'Updates' });
  const state = updates.getByRole('status');
  await expect(state).toHaveText('Font Fu has not checked for updates yet.');
  const check = updates.getByRole('button', { name: 'Check for updates' });
  await page.getByRole('heading', { name: 'About', level: 2 }).focus();
  await page.keyboard.press('Tab');
  await expect(check).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(state).toHaveText('Font Fu is up to date.');
  await expect(updates).toContainText('Font Fu was up to date.');
  // One region, one status: the line under the title is not announced again.
  await expect(updates.getByRole('status')).toHaveCount(1);
  // Next stop: the chosen level card, in one column with its line.
  await page.keyboard.press('Tab');
  const off = updates.getByRole('radio', { name: 'Off' });
  await expect(off).toBeFocused();
  await expect(off).toHaveAccessibleDescription('Font Fu checks only when you ask.');
  await expect(
    updates.getByRole('radio', { name: 'Tell me when there is an update' }),
  ).toHaveAccessibleDescription(
    'Checks at start and once a day. Nothing installs until you confirm.',
  );
});

test('a failed check is said on the status row', async ({ page }) => {
  await page.goto('/?mockUpdate=3#/settings?section=about');
  const updates = page.getByRole('region', { name: 'Updates' });
  await updates.getByRole('button', { name: 'Check for updates' }).click();
  await expect(updates.getByRole('status')).toHaveText('The last check failed.');
  const check = updates.getByRole('button', { name: 'Check for updates' });
  await expect(updates.getByRole('alert')).toContainText('could not reach its release server');
  await expect(check).toBeEnabled();
});

test('a found version shows "Show version x" on the row, and it opens the dialog', async ({
  page,
}) => {
  await page.goto('/?mockUpdate=1#/settings?section=about');
  const updates = page.getByRole('region', { name: 'Updates' });
  await updates.getByRole('button', { name: 'Check for updates' }).click();
  const dialog = page.getByRole('dialog', { name: 'Font Fu 0.1.1 is available' });
  await dialog.getByRole('button', { name: 'Later' }).click();
  await expect(updates.getByRole('status')).toHaveText('Version 0.1.1 is available.');
  await updates.getByRole('button', { name: 'Show version 0.1.1' }).click();
  await expect(dialog).toBeVisible();
});
