import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';

// E50.12: the Foundries group of the sidebar on the wall library (46 foundries): folded at
// first with its count, the ten largest when open, a click filters the wall, a right-click
// merges with an undo toast.

const nav = (page: Page) => page.getByRole('navigation', { name: 'Library' });
const heading = (page: Page) => nav(page).getByRole('button', { name: 'Foundries', exact: true });
const list = (page: Page) => page.locator('#fm-foundries-list');
const row = (page: Page, name: string) => list(page).locator(`button[data-foundry="${name}"]`);

async function open(page: Page) {
  await page.goto('/?mockLibrary=wall#/');
  await expect(heading(page)).toBeVisible();
}

test('sits between Collections and Tags, folded, with the count; opening is remembered', async ({
  page,
}) => {
  await open(page);
  const names = await nav(page).getByRole('heading', { level: 2 }).allInnerTexts();
  const at = (n: string) => names.findIndex((h) => h.trim().toLowerCase() === n);
  expect(at('foundries')).toBe(at('collections') + 1);
  expect(at('tags')).toBe(at('foundries') + 1);
  await expect(heading(page)).toHaveAttribute('aria-expanded', 'false');
  await expect(list(page)).toBeHidden();
  await expect(nav(page).getByTestId('section-count')).toContainText('46');

  await heading(page).click();
  await expect(list(page).locator('button[data-foundry]')).toHaveCount(10);
  await expect(row(page, 'Omnibus-Type').locator('.side-count')).toHaveText('5');
  await expect(list(page).getByTestId('all-foundries')).toHaveText('All 46 foundries…');
  await page.reload();
  await expect(heading(page)).toHaveAttribute('aria-expanded', 'true');
});

test('a click filters the wall with a chip and marks the row; a second click clears it', async ({
  page,
}) => {
  await open(page);
  await heading(page).click();
  await row(page, 'Sorkin Type').click();
  const chip = page.getByTestId('foundry-chip');
  await expect(chip).toHaveText('Foundry · Sorkin Type');
  await expect(row(page, 'Sorkin Type')).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByRole('tree', { name: 'Font list' }).getByRole('treeitem')).toHaveCount(3);
  await row(page, 'Sorkin Type').click();
  await expect(chip).toHaveCount(0);
  await expect(row(page, 'Sorkin Type')).toHaveAttribute('aria-pressed', 'false');
});

test('All N foundries… searches every foundry; Enter filters and gives the focus back', async ({
  page,
}) => {
  await open(page);
  await heading(page).click();
  await list(page).getByTestId('all-foundries').click();
  const field = page.getByRole('searchbox', { name: 'Find a foundry' });
  await expect(field).toBeFocused();
  await field.fill('git');
  await field.press('Enter');
  await expect(page.getByTestId('foundry-chip')).toHaveText('Foundry · GitHub');
  await expect(row(page, 'GitHub')).toHaveAttribute('aria-pressed', 'true');
  await expect(list(page).getByTestId('all-foundries')).toBeFocused();
});

test('right-click merges into another foundry with an undo toast', async ({ page }) => {
  await open(page);
  await heading(page).click();
  await row(page, 'Font Diner').click({ button: 'right' });
  const item = page
    .getByRole('menu', { name: 'Foundry Font Diner' })
    .getByRole('menuitem', { name: 'Merge into…' });
  await expect(item).toBeFocused();
  await item.press('Enter');
  const field = page.getByRole('searchbox', { name: 'Merge Font Diner into…' });
  await expect(field).toBeFocused();
  await field.fill('omni');
  await field.press('Enter');
  await expect(row(page, 'Font Diner')).toHaveCount(0);
  await expect(row(page, 'Omnibus-Type').locator('.side-count')).toHaveText('8');
  await expect(row(page, 'Omnibus-Type')).toBeFocused();
  const toast = page.getByTestId('toast');
  await expect(toast).toContainText('Merged “Font Diner” into “Omnibus-Type”.');
  await toast.getByRole('button', { name: 'Undo' }).click();
  await expect(row(page, 'Font Diner').locator('.side-count')).toHaveText('3');
  await expect(row(page, 'Omnibus-Type').locator('.side-count')).toHaveText('5');
});

for (const scheme of ['light', 'dark'] as const) {
  test(`the open group, the search list and the menu pass a WCAG 2.2 AA scan (${scheme})`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: scheme, reducedMotion: 'reduce' });
    await open(page);
    await heading(page).click();
    await row(page, 'Sorkin Type').click();
    const scan = async () => {
      const result = await new AxeBuilder({ page })
        .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
        .include('nav')
        .include('[data-ui-popover]')
        .analyze();
      expect(result.violations.map((v) => `${v.id}: ${v.nodes[0]?.target}`)).toEqual([]);
    };
    await list(page).getByTestId('all-foundries').click();
    await expect(page.getByRole('searchbox', { name: 'Find a foundry' })).toBeFocused();
    await scan();
    await page.keyboard.press('Escape');
    await row(page, 'Be').click({ button: 'right' });
    await expect(
      page.getByRole('menu', { name: 'Foundry Be' }).getByRole('menuitem', { name: 'Merge into…' }),
    ).toBeFocused();
    await scan();
  });
}
