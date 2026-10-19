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

// "Keep this one" (E8.3): the browser-mode mock has one exact duplicate group,
// Quarto-Regular in the library folder (the recommended keeper) and in Downloads.
const KEPT = '/Users/demo/Fonts/Quarto-Regular.otf';
const COPY = '/Users/demo/Downloads/Quarto-Regular.otf';
const resolve = (page: Page) => page.getByRole('region', { name: 'Resolve duplicates' });
const dialog = (page: Page) => page.getByRole('dialog', { name: /to the trash\?/ });
const toast = (page: Page) => page.getByTestId('undo-toast');

test('keep this one shows what will move, asks once, and undoes from the toast', async ({
  page,
}) => {
  await page.goto('/#/problems?kind=duplicates');
  await resolve(page)
    .getByRole('button', { name: `Keep this one: ${KEPT}` })
    .click();

  // The dry run: what will move, nothing moved yet.
  await expect(dialog(page)).toBeVisible();
  await expect(dialog(page).getByRole('heading')).toHaveText('Move 1 copy to the trash?');
  await expect(dialog(page).getByRole('list', { name: 'Will move to the trash' })).toHaveText(COPY);
  await page.keyboard.press('Escape');
  await expect(dialog(page)).toBeHidden();
  await expect(filter(page, /^Duplicates/)).toHaveText(/Duplicates\s*1/);

  await resolve(page)
    .getByRole('button', { name: `Keep this one: ${KEPT}` })
    .click();
  await dialog(page).getByRole('button', { name: 'Move to trash' }).click();
  await expect(toast(page)).toContainText('Moved 1 copy to the trash.');
  await expect(filter(page, /^Duplicates/)).toHaveText(/Duplicates\s*0/);
  await expect(resolve(page)).toBeHidden();

  await toast(page).getByRole('button', { name: 'Undo' }).click();
  await expect(toast(page)).toBeHidden();
  await expect(filter(page, /^Duplicates/)).toHaveText(/Duplicates\s*1/);
  const history = page.getByRole('table', { name: 'History' });
  await expect(history.locator('tbody tr td').nth(2)).toHaveText('Put back 1 copy.');
});

test('the undo toast goes after 10 s and the history still undoes', async ({ page }) => {
  await page.clock.install();
  await page.goto('/#/problems?kind=duplicates');
  await resolve(page)
    .getByRole('button', { name: `Keep this one: ${COPY}` })
    .click();
  await expect(dialog(page).getByRole('list', { name: 'Will move to the trash' })).toHaveText(KEPT);
  await dialog(page).getByRole('button', { name: 'Move to trash' }).click();
  await expect(toast(page)).toBeVisible();

  await page.clock.runFor(9_000);
  await expect(toast(page)).toBeVisible();
  await page.clock.runFor(1_000);
  await expect(toast(page)).toBeHidden();

  const history = page.getByRole('table', { name: 'History' });
  await expect(history.locator('tbody tr')).toHaveCount(1);
  await expect(history.locator('tbody tr td').nth(1)).toHaveText(COPY);
  await history.getByRole('button', { name: /^Undo/ }).click();
  await expect(history.locator('tbody tr td').nth(2)).toHaveText('Put back 1 copy.');
  await expect(history.getByRole('button', { name: /^Undo/ })).toHaveCount(0);
  await expect(filter(page, /^Duplicates/)).toHaveText(/Duplicates\s*1/);
});
