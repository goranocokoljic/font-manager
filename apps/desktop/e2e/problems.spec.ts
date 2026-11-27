import { expect, test, type Page } from '@playwright/test';

// The browser-mode mock lists five problems: face 6 clashes with a system font,
// face 7 is Type 1, plus a broken file, a duplicate copy and a sidecar conflict. The
// duplicate copy is the Duplicates module's (E56.15), so the view counts four.
// E57.9 (board 16): the shell of Settings, the groups on the left, the rows on the right.

const sidebar = (page: Page) => page.getByRole('navigation', { name: 'Library' });
const nav = (page: Page) => page.getByRole('tablist');
const tab = (page: Page, name: RegExp) => nav(page).getByRole('tab', { name });
const heading = (page: Page) => page.getByTestId('problems-heading');
const group = (page: Page, id: string) => page.locator(`[data-group="${id}"]`);

test('the foot of the sidebar counts the problems, once', async ({ page }) => {
  await page.goto('/');
  await expect(sidebar(page).getByTestId('problem-count')).toHaveText('4 problems');
  await expect(page.getByTestId('problem-count')).toHaveCount(1);
});

test('the groups are a tab list the keyboard walks, each opening its rows', async ({ page }) => {
  await page.goto('/');
  await page.getByTestId('problem-count').click();
  await expect(page.getByRole('heading', { name: 'Problems', level: 1 })).toBeVisible();
  await expect(tab(page, /^All/)).toBeFocused();
  await expect(tab(page, /^All/)).toHaveAttribute('aria-selected', 'true');
  await expect(heading(page)).toHaveText('4 things need a look');
  await expect(page.getByTestId('problems-group').locator('h3')).toHaveText([
    'Broken files · 1',
    'Legacy formats · 1',
    'Sync conflicts · 1',
    'Activation conflicts · 1',
  ]);
  const activation = group(page, 'activation').getByTestId('problem-row');
  await expect(activation.locator('.name')).toHaveText('JetBrainsMono-Regular.ttf');
  await expect(activation.locator('.kind')).toHaveText('same name as a system font');
  await expect(activation.locator('.message')).toHaveText(
    'JetBrainsMono-Regular has the same PostScript name as a system font.',
  );
  await expect(activation.locator('.path')).toHaveText(
    '/Users/demo/Fonts/JetBrainsMono-Regular.ttf',
  );

  // Arrows move between the groups and open them; the URL follows.
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await expect(tab(page, /^Legacy formats/)).toBeFocused();
  await expect(page).toHaveURL(/#\/problems\?kind=legacy$/);
  await expect(heading(page)).toHaveText('1 thing needs a look');
  await expect(group(page, 'legacy').locator('.path')).toHaveText(
    '/Users/demo/Fonts/legacy/LEGGR___.PFB',
  );
  await expect(page.getByTestId('problems-group')).toHaveCount(1);

  // A group with nothing: muted, still opens.
  await page.keyboard.press('End');
  await expect(tab(page, /^Folders and index/)).toBeFocused();
  await expect(heading(page)).toHaveText('No problems');
  await expect(page.getByText('Nothing of this kind right now.')).toBeVisible();

  // Enter on a group moves into the panel; Tab reaches the row's action, which then shows.
  await page.keyboard.press('Home');
  await page.keyboard.press('Enter');
  await expect(heading(page)).toBeFocused();
  await page.keyboard.press('Tab');
  const copy = page.getByRole('button', { name: 'Copy path: Quarto-Bold.otf' });
  await expect(copy).toBeFocused();
  await expect(copy).toHaveCSS('opacity', '1');
  // Tab goes on to the next group's row.
  await page.keyboard.press('Tab');
  await expect(page.getByRole('button', { name: 'Copy path: LEGGR___.PFB' })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('heading', { name: 'All fonts' })).toBeVisible();
});

test('a row shows its one action on hover only', async ({ page }) => {
  await page.goto('/#/problems');
  const row = group(page, 'activation').getByTestId('problem-row');
  const decide = row.getByRole('button', { name: 'Decide: JetBrainsMono-Regular.ttf' });
  await expect(decide).toHaveCSS('opacity', '0');
  await row.hover();
  await expect(decide).toHaveCSS('opacity', '1');
  await expect(decide).toHaveText('Decide…');
  await decide.click();
  await expect(page).toHaveURL(/#\/conflicts$/);
});

test('a link to a kind opens the view filtered to it', async ({ page }) => {
  await page.goto('/#/problems?kind=legacy');
  await expect(tab(page, /^Legacy formats/)).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByTestId('problems-group')).toHaveCount(1);
  await expect(group(page, 'legacy').getByTestId('problem-row')).toHaveCount(1);
});

// E56.15: duplicates are the Duplicates module's. Problems shows one line for them, which
// is not in the total, and the old link to the duplicates kind opens the module.
test('duplicates are one line that opens the module, out of the total', async ({ page }) => {
  await page.goto('/?mockDuplicates=1#/problems');
  await expect(tab(page, /^Duplicates/)).toHaveCount(0);
  const line = page.getByTestId('problems-duplicates');
  await expect(line).toContainText(
    /^\d+ fonts have more than one copy\. Copies are not problems; they are cleaned up in Duplicates\./,
  );
  await line.getByRole('link', { name: 'Open Duplicates' }).click();
  await expect(page).toHaveURL(/#\/duplicates/);

  await page.goto('/#/problems?kind=duplicates');
  await expect(page).toHaveURL(/#\/duplicates/);
});

test('a sync conflict shows both values and is resolved by keyboard', async ({ page }) => {
  await page.goto('/#/problems?kind=sidecar');
  const conflict = page.getByRole('group', { name: 'Inter: Rating' });
  await expect(conflict).toContainText('This computer');
  await expect(conflict).toContainText('4 stars');
  await expect(conflict).toContainText('Other computer');
  await expect(conflict).toContainText('2 stars');
  const useTheirs = conflict.getByRole('button', {
    name: 'Use theirs: Rating of Inter on the other computer, 2 stars',
  });
  await conflict.getByRole('button', { name: /^Keep mine: Rating of Inter/ }).focus();
  await page.keyboard.press('Tab');
  await expect(useTheirs).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(conflict).toBeHidden();
  // The last row of the group: the focus goes to the group's heading.
  await expect(page.locator('#problems-group-sidecar')).toBeFocused();
  await expect(tab(page, /^Sync conflicts/)).toHaveText('Sync conflicts');
  await expect(tab(page, /^All/)).toHaveText(/All\s*3/);
});

test('the empty state is the neutral "All good." panel, the navigation still there', async ({
  page,
}) => {
  await page.goto('/?mockProblems=0&mockDuplicates=1#/problems');
  await expect(heading(page)).toHaveText('No problems');
  await expect(page.getByText('Your library is in good shape.')).toBeVisible();
  const good = page.getByTestId('problems-empty');
  await expect(good).toContainText('All good.');
  await expect(nav(page).getByRole('tab')).toHaveCount(6);
  await expect(page.getByTestId('problems-group')).toHaveCount(0);
  await expect(page.getByTestId('problems-duplicates')).toBeVisible();
  // Neutral, never green: the panel's colours are the surface and the text's.
  const colors = await good.evaluate((el) => {
    const css = getComputedStyle(el);
    const probe = document.createElement('div');
    probe.style.color = 'var(--fm-success)';
    probe.style.backgroundColor = 'var(--fm-surface)';
    document.body.append(probe);
    const p = getComputedStyle(probe);
    const out = { bg: css.backgroundColor, surface: p.backgroundColor, success: p.color };
    const title = getComputedStyle(el.querySelector('.good-title')!).color;
    probe.remove();
    return { ...out, title };
  });
  expect(colors.bg).toBe(colors.surface);
  expect(colors.title).not.toBe(colors.success);

  await page.goto('/?mockProblems=0#/problems');
  await expect(page.getByTestId('problems-empty')).toBeVisible();
  await expect(page.getByTestId('problems-duplicates')).toHaveCount(0);
});

test('the view reads in German', async ({ page }) => {
  await page.addInitScript(() => {
    sessionStorage.setItem('fm.mock.settings.v1', JSON.stringify({ language: 'de' }));
  });
  await page.goto('/#/problems');
  await expect(heading(page)).toHaveText('4 Sachen brauchen einen Blick');
  await expect(tab(page, /^Sync-Konflikte/)).toBeVisible();
  await expect(page.getByRole('button', { name: 'Pfad kopieren: Quarto-Bold.otf' })).toHaveText(
    'Pfad kopieren',
  );
});
