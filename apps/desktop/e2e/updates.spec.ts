import { expect, test } from '@playwright/test';

// In-app updates (E18.2) on the MockApi: `?mockUpdate=1` offers a patch, `=2` a major
// update, `=4` an unsigned package, `=5` is a Flatpak copy.
const about = (mock: number) => `/?mockUpdate=${mock}#/settings?section=about`;

test('nothing is checked until the user clicks; a major update asks twice', async ({ page }) => {
  await page.goto(about(2));
  const updates = page.getByRole('region', { name: 'Updates' });
  await expect(updates.getByRole('checkbox', { name: /Check automatically/ })).not.toBeChecked();
  await expect(page.getByRole('dialog')).toHaveCount(0);

  await updates.getByRole('button', { name: 'Check for updates' }).click();
  const dialog = page.getByRole('dialog', { name: 'Font Fu 1.0.0 is available' });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByLabel('Release notes')).toContainText('The first stable release.');
  await expect(dialog).toContainText('You have version 0.1.0.');

  await dialog.getByRole('button', { name: 'Install and restart' }).click();
  const second = page.getByRole('dialog', { name: 'Install a major update?' });
  await expect(second).toContainText('Version 1.0.0 is a major update from 0.1.0');
  await expect(second.getByRole('button', { name: 'Install version 1.0.0' })).toBeFocused();
  // A click on the text keeps focus in the dialog; Tab and Shift+Tab stay inside.
  await second.getByText('It can change how').click();
  await page.keyboard.press('Tab');
  await expect(second.getByRole('button', { name: 'Cancel' })).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await expect(second.getByRole('button', { name: 'Install version 1.0.0' })).toBeFocused();

  await second.getByRole('button', { name: 'Install version 1.0.0' }).click();
  await expect(second.getByRole('status')).toContainText('Font Fu restarts');
});

test('an unsigned package is refused with a message', async ({ page }) => {
  await page.goto(about(4));
  await page.getByRole('button', { name: 'Check for updates' }).click();
  const dialog = page.getByRole('dialog', { name: 'Font Fu 0.1.1 is available' });
  await dialog.getByRole('button', { name: 'Install and restart' }).click();
  await expect(dialog.getByRole('alert')).toContainText("not signed with Font Fu's release key");
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page.getByRole('region', { name: 'Updates' })).toContainText(
    'Version 0.1.1 is available.',
  );
});

test('a Flatpak copy shows no update section, and the platform notes say why', async ({ page }) => {
  await page.goto(about(5));
  await expect(page.getByRole('list', { name: 'Platform notes' })).toContainText(
    'Flatpak installs the updates of this copy',
  );
  await expect(page.getByRole('button', { name: 'Check for updates' })).toHaveCount(0);
});

// E55.9 "Install updates when I quit".
test('"Install updates when I quit" needs the automatic check and is saved', async ({ page }) => {
  await page.goto(about(1));
  const updates = page.getByRole('region', { name: 'Updates' });
  const onQuit = updates.getByRole('checkbox', { name: 'Install updates when I quit' });
  await expect(onQuit).not.toBeChecked();
  await expect(onQuit).toBeDisabled();
  await updates.getByRole('checkbox', { name: /Check automatically/ }).check();
  await expect(onQuit).toBeEnabled();
  await onQuit.check();
  await expect(onQuit).toBeChecked();
  await expect(page.getByRole('dialog')).toHaveCount(0);
});

test('the start after an install at quit says "Updated to" once, with what is new', async ({
  page,
}) => {
  await page.goto('/?mockUpdateNotice=1');
  const toast = page.getByText('Updated to 0.1.1.');
  await expect(toast).toBeVisible();
  await expect(page.getByRole('button', { name: 'What’s new' })).toBeVisible();
});

test('the start after a failed install says so and offers the update dialog', async ({ page }) => {
  await page.goto('/?mockUpdate=1&mockUpdateNotice=2');
  await expect(page.getByText('could not install version 0.1.1')).toBeVisible();
  await page.getByRole('button', { name: 'Update…' }).click();
  await expect(page.getByRole('dialog', { name: 'Font Fu 0.1.1 is available' })).toBeVisible();
});
