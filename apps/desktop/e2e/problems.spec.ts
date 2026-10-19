import { expect, test, type Page } from '@playwright/test';

// The browser-mode mock lists five problems: face 6 clashes with a system font,
// face 7 is Type 1, plus a broken file, a duplicate copy and a sidecar conflict.

const sidebar = (page: Page) => page.getByRole('navigation', { name: 'Library' });
const table = (page: Page) => page.getByRole('table', { name: 'Problems' });
const filter = (page: Page, name: RegExp) =>
  page.getByRole('group', { name: 'Show problems' }).getByRole('button', { name });

test('the sidebar and the status bar count the problems', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByTestId('problem-count')).toHaveText('5 problems');
  await expect(sidebar(page).getByTestId('sidebar-problems')).toHaveText(/Problems\s*5/);
  const groups = sidebar(page).locator('[data-group]');
  await expect(groups).toHaveText([
    /Duplicates\s*1/,
    /Broken files\s*1/,
    /Legacy formats\s*1/,
    /Sidecar conflicts\s*1/,
    /Activation conflicts\s*1/,
  ]);
});

test('one view lists every kind of problem and filters by kind', async ({ page }) => {
  await page.goto('/');
  await page.getByTestId('problem-count').click();
  await expect(page.getByRole('heading', { name: 'Problems', level: 1 })).toBeVisible();
  await expect(filter(page, /^All/)).toHaveAttribute('aria-pressed', 'true');

  const rows = table(page).getByRole('row');
  await expect(rows).toHaveCount(6);
  const kinds = table(page).locator('tbody tr td:first-child');
  await expect(kinds).toHaveText([
    'Activation conflicts',
    'Legacy formats',
    'Broken files',
    'Duplicates',
    'Sidecar conflicts',
  ]);
  await expect(rows.nth(1).getByRole('cell')).toHaveText([
    'Activation conflicts',
    '/Users/demo/Fonts/JetBrainsMono-Regular.ttf',
    '/System/Library/Fonts/JetBrainsMono-Regular.ttf',
    'JetBrainsMono-Regular has the same PostScript name as a system font.',
  ]);

  await filter(page, /^Legacy formats/).click();
  await expect(page).toHaveURL(/#\/problems\?kind=legacy$/);
  await expect(filter(page, /^Legacy formats/)).toHaveAttribute('aria-pressed', 'true');
  await expect(rows).toHaveCount(2);
  await expect(rows.nth(1).getByRole('cell').nth(1)).toHaveText(
    '/Users/demo/Fonts/legacy/LEGGR___.PFB',
  );

  await filter(page, /^Folders and index/).click();
  await expect(page.getByText('No problems of this kind.')).toBeVisible();

  await filter(page, /^All/).click();
  await expect(rows).toHaveCount(6);

  await page.keyboard.press('Escape');
  await expect(page.getByRole('heading', { name: 'All fonts' })).toBeVisible();
});

test('a sidebar group opens the view filtered to it', async ({ page }) => {
  await page.goto('/');
  await sidebar(page).locator('[data-group="duplicates"]').click();
  await expect(filter(page, /^Duplicates/)).toHaveAttribute('aria-pressed', 'true');
  const rows = table(page).getByRole('row');
  await expect(rows).toHaveCount(2);
  await expect(rows.nth(1).getByRole('cell')).toHaveText([
    'Duplicates',
    '/Users/demo/Downloads/Quarto-Regular.otf',
    '/Users/demo/Fonts/Quarto-Regular.otf',
    'The same file is also at /Users/demo/Fonts/Quarto-Regular.otf.',
  ]);
});
