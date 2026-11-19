import { expect, test } from '@playwright/test';

// In-app updates (E18.2) on the MockApi: `?mockUpdate=1` offers a patch, `=2` a major
// update, `=4` an unsigned package, `=5` is a Flatpak copy.
const about = (mock: number) => `/?mockUpdate=${mock}#/settings?section=about`;

test('nothing is checked until the user clicks; a major update asks twice', async ({ page }) => {
  await page.goto(about(2));
  const updates = page.getByRole('region', { name: 'Updates' });
  await expect(updates.getByRole('radio', { name: 'Off' })).toBeChecked();
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

// E55.10: one control of three levels over the two settings; keyboard only.
test('the update level is one control of three choices, saved and keyboard-complete', async ({
  page,
}) => {
  await page.goto(about(1));
  const updates = page.getByRole('region', { name: 'Updates' });
  const level = updates.getByRole('group', { name: 'Automatic updates' });
  const off = level.getByRole('radio', { name: 'Off' });
  const notify = level.getByRole('radio', { name: 'Tell me when there is an update' });
  const onQuit = level.getByRole('radio', {
    name: 'Download updates and install them when I quit',
  });
  await expect(level.getByRole('radio')).toHaveCount(3);
  await expect(off).toBeChecked();
  await expect(updates).toContainText('Font Fu has not checked for updates yet.');
  await expect(onQuit).toHaveAccessibleDescription(/A major version always asks you first\./);
  await expect(onQuit).toHaveAccessibleDescription(
    /Fonts activated for the session end when you quit, as they always do\./,
  );

  // Arrow keys move between the levels; each one is saved at once.
  await off.focus();
  await page.keyboard.press('ArrowDown');
  await expect(notify).toBeChecked();
  await expect(notify).toBeFocused();
  // "Tell me": the automatic check runs quietly and the last check shows what it found.
  await expect(updates).toContainText('version 0.1.1 was available');
  await expect(page.getByRole('dialog', { name: /is available/ })).toHaveCount(0);
  await page.keyboard.press('ArrowDown');
  await expect(onQuit).toBeChecked();
  await page.reload();
  await expect(onQuit).toBeChecked();
  await onQuit.focus();
  await page.keyboard.press('ArrowUp');
  await page.keyboard.press('ArrowUp');
  await expect(off).toBeChecked();
  await page.reload();
  await expect(off).toBeChecked();
});

test('while an update waits for the quit, "Install now" opens the update dialog', async ({
  page,
}) => {
  await page.goto('/?mockUpdateWaiting=1#/settings?section=about');
  const updates = page.getByRole('region', { name: 'Updates' });
  await expect(
    updates.getByRole('radio', { name: 'Download updates and install them when I quit' }),
  ).toBeChecked();
  await expect(updates).toContainText('version 0.1.1 was available');
  await expect(updates).toContainText('0.1.1 will be installed when you quit.');
  const now = updates.getByRole('button', { name: 'Install now' });
  await now.focus();
  await page.keyboard.press('Enter');
  const dialog = page.getByRole('dialog', { name: 'Font Fu 0.1.1 is available' });
  await expect(dialog).toBeVisible();
  // "Tell me" instead: nothing waits any more.
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  await updates.getByRole('radio', { name: 'Tell me when there is an update' }).check();
  await expect(updates).not.toContainText('will be installed when you quit');
  await expect(updates.getByRole('button', { name: 'Install now' })).toHaveCount(0);
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
