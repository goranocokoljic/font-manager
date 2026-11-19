import { expect, test } from '@playwright/test';

// An update found by the shell's automatic check is shown quietly (E55.8). On the MockApi,
// `?mockAutoUpdate=1` starts with "Check automatically" on, so the mock runs the shell's
// quiet check at start; `?mockUpdate=1` offers a patch, `=2` a major update.
const quiet = (mock = 1) => `/?mockAutoUpdate=1&mockUpdate=${mock}#/`;

test('the notice shows the version, never the dialog, and can be dismissed', async ({ page }) => {
  await page.goto(quiet());
  const notice = page.getByRole('region', { name: 'Update available' });
  await expect(notice).toContainText('Font Fu 0.1.1 is available.');
  await expect(notice.getByRole('button', { name: "See what's new" })).toBeVisible();
  const dot = page.getByRole('button', { name: "Font Fu 0.1.1 is available: see what's new" });
  await expect(dot).toBeVisible();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  // The notice does not take the focus from the page.
  await expect(notice.getByRole('button', { name: "See what's new" })).not.toBeFocused();

  await notice.getByRole('button', { name: 'Dismiss' }).click();
  await expect(notice).toHaveCount(0);
  await expect(dot).toBeVisible();
  await expect(page.getByRole('dialog')).toHaveCount(0);

  // Settings → About still says so.
  await page.evaluate(() => (location.hash = '#/settings?section=about'));
  await expect(page.getByRole('region', { name: 'Updates' })).toContainText(
    'Version 0.1.1 is available.',
  );
  await expect(notice).toHaveCount(0);

  // The next start: the shell checks again, and the notice is back.
  await page.reload();
  await expect(page.getByRole('region', { name: 'Update available' })).toContainText(
    'Font Fu 0.1.1 is available.',
  );
});

test("See what's new opens the update dialog", async ({ page }) => {
  await page.goto(quiet());
  const notice = page.getByRole('region', { name: 'Update available' });
  await notice.getByRole('button', { name: "See what's new" }).click();
  const dialog = page.getByRole('dialog', { name: 'Font Fu 0.1.1 is available' });
  await expect(dialog).toBeVisible();
  await expect(notice).toHaveCount(0);
  await dialog.getByRole('button', { name: 'Later' }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(notice).toHaveCount(0);
  await expect(
    page.getByRole('button', { name: "Font Fu 0.1.1 is available: see what's new" }),
  ).toBeVisible();
});

test('the dot on Settings opens the same dialog; a major update asks twice', async ({ page }) => {
  await page.goto(quiet(2));
  const dot = page.getByRole('button', { name: "Font Fu 1.0.0 is available: see what's new" });
  await dot.click();
  const dialog = page.getByRole('dialog', { name: 'Font Fu 1.0.0 is available' });
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Install and restart' }).click();
  const second = page.getByRole('dialog', { name: 'Install a major update?' });
  await expect(second.getByRole('button', { name: 'Install version 1.0.0' })).toBeFocused();
  await second.getByRole('button', { name: 'Cancel' }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(dot).toBeFocused();
});
