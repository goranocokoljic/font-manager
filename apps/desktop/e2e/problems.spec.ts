import { expect, test, type Page } from '@playwright/test';

// The browser-mode mock lists five problems: face 6 clashes with a system font,
// face 7 is Type 1, plus a broken file, a duplicate copy and a sidecar conflict. The
// duplicate copy is the Duplicates module's (E56.15), so the view counts four.

const sidebar = (page: Page) => page.getByRole('navigation', { name: 'Library' });
const table = (page: Page) => page.getByRole('table', { name: 'Problems' });
const filter = (page: Page, name: RegExp) =>
  page.getByRole('group', { name: 'Show problems' }).getByRole('button', { name });

test('the foot of the sidebar counts the problems, once', async ({ page }) => {
  await page.goto('/');
  await expect(sidebar(page).getByTestId('problem-count')).toHaveText('4 problems');
  await expect(page.getByTestId('problem-count')).toHaveCount(1);
});

test('one view lists every kind of problem and filters by kind', async ({ page }) => {
  await page.goto('/');
  await page.getByTestId('problem-count').click();
  await expect(page.getByRole('heading', { name: 'Problems', level: 1 })).toBeVisible();
  await expect(filter(page, /^All/)).toHaveAttribute('aria-pressed', 'true');

  const rows = table(page).getByRole('row');
  await expect(rows).toHaveCount(5);
  const kinds = table(page).locator('tbody tr td:first-child');
  await expect(kinds).toHaveText([
    'Activation conflicts',
    'Legacy formats',
    'Broken files',
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
  await expect(rows).toHaveCount(5);

  await page.keyboard.press('Escape');
  await expect(page.getByRole('heading', { name: 'All fonts' })).toBeVisible();
});

test('a link to a kind opens the view filtered to it', async ({ page }) => {
  await page.goto('/#/problems?kind=legacy');
  await expect(filter(page, /^Legacy formats/)).toHaveAttribute('aria-pressed', 'true');
  await expect(table(page).getByRole('row')).toHaveCount(2);
});

// E56.15: duplicates are the Duplicates module's. Problems shows one line for them, which
// is not in the total, and the old link to the duplicates kind opens the module.
test('duplicates are one line that opens the module, out of the total', async ({ page }) => {
  await page.goto('/?mockDuplicates=1#/problems');
  await expect(filter(page, /^Duplicates/)).toHaveCount(0);
  const line = page.getByTestId('problems-duplicates');
  await expect(line).toContainText(/^\d+ fonts have more than one copy\./);
  await line.getByRole('link', { name: 'Open Duplicates' }).click();
  await expect(page).toHaveURL(/#\/duplicates/);

  await page.goto('/#/problems?kind=duplicates');
  await expect(page).toHaveURL(/#\/duplicates/);
});

test('a sidecar conflict shows both values and is resolved by keyboard', async ({ page }) => {
  await page.goto('/#/problems?kind=sidecar');
  const conflict = page.getByRole('group', { name: 'Inter: Rating' });
  await expect(conflict).toContainText('4 stars');
  await expect(conflict).toContainText('2 stars');
  const useTheirs = conflict.getByRole('button', {
    name: 'Use theirs: Rating of Inter on the other computer, 2 stars',
  });
  await conflict.getByRole('button', { name: /^Keep mine: Rating of Inter/ }).focus();
  await page.keyboard.press('Tab');
  await expect(useTheirs).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(conflict).toBeHidden();
  await expect(filter(page, /^Sidecar/)).toHaveText(/Sidecar\D*0/);
});
