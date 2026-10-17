import { expect, test, type Page } from '@playwright/test';

// Browser mode runs on the mock client: its folder picker returns
// `/Users/demo/Added 1`, `/Users/demo/Added 2`, … and its scans step slowly.
const sources = (page: Page) => page.getByRole('region', { name: 'Sources' });
const row = (page: Page, name: string) =>
  sources(page).getByRole('listitem', { name, exact: true });

test('a folder is added, paused, resumed and removed', async ({ page }) => {
  await page.goto('/');
  await sources(page).getByRole('button', { name: 'Add folder' }).click();
  const added = row(page, 'Added 1');
  await expect(added).toBeVisible();
  await expect(added.getByText('Added 1')).toHaveAttribute('title', '/Users/demo/Added 1');

  await added.getByRole('button', { name: 'Pause Added 1' }).click();
  await expect(added.getByText('Paused')).toBeVisible();
  await added.getByRole('button', { name: 'Resume Added 1' }).click();
  await expect(added.getByText('Paused')).toHaveCount(0);

  await added.getByRole('button', { name: 'Remove Added 1 from the library' }).click();
  await expect(added).toHaveCount(0);
  await expect(row(page, 'Fonts')).toBeVisible();
});

test('scan progress shows inline on the source', async ({ page }) => {
  await page.goto('/');
  const fonts = row(page, 'Fonts');
  await fonts.getByRole('button', { name: 'Rescan Fonts' }).click();
  const bar = fonts.getByRole('progressbar', { name: 'Scanning Fonts' });
  await expect(bar).toBeVisible();
  await expect(bar).toHaveAttribute('max', '7');
  await expect(bar).toHaveCount(0);
});

test('the System source is listed read-only', async ({ page }) => {
  await page.goto('/');
  const system = row(page, 'System fonts');
  await expect(system).toBeVisible();
  await expect(system.getByText('Read-only')).toBeVisible();
  await expect(system.getByRole('button')).toHaveCount(0);
});
