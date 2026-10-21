import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// Browser mode runs on the mock client; the Quick Collection starts empty.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/#/');
}

const list = (page: Page) => page.getByRole('tree', { name: 'Font list' });
const family = (page: Page, name: string) =>
  list(page).getByRole('treeitem', { name: new RegExp(`^${name}`) });
const count = (page: Page) => page.getByTestId('quick-count');
const review = (page: Page) => page.getByRole('region', { name: 'Quick Collection' }).last();
const reviewRows = (page: Page) =>
  page.getByRole('list', { name: 'Fonts in the Quick Collection' }).getByRole('listitem');

/** Selects Inter and JetBrains Mono and adds both with B. */
async function addTwo(page: Page) {
  await family(page, 'Inter').click();
  await family(page, 'JetBrains Mono').click({ modifiers: ['ControlOrMeta'] });
  await page.keyboard.press('b');
  await expect(count(page)).toHaveText('2');
}

test('B adds the selected fonts to the Quick Collection, marks them, and removes them again', async ({
  page,
}) => {
  await open(page);
  await expect(count(page)).toHaveText('0');
  const inter = family(page, 'Inter');
  await inter.click();
  await page.keyboard.press('b');
  await expect(count(page)).toHaveText('1');
  await expect(page.getByTestId('toast')).toContainText('Added 1 font to Quick Collection');
  await expect(inter).toHaveAttribute('aria-label', /, in Quick Collection$/);
  await expect(inter.getByTestId('quick-mark')).toBeVisible();
  await expect(family(page, 'JetBrains Mono').getByTestId('quick-mark')).toHaveCount(0);

  // The same key again takes it out.
  await page.keyboard.press('b');
  await expect(count(page)).toHaveText('0');
  await expect(inter.getByTestId('quick-mark')).toHaveCount(0);

  // The command palette runs the same action.
  await page.keyboard.press('ControlOrMeta+k');
  await page.getByRole('combobox', { name: 'Search commands' }).fill('quick collection add');
  await page.keyboard.press('Enter');
  await expect(count(page)).toHaveText('1');
});

test('the review lists the fonts with the preview settings; each drops by key or button', async ({
  page,
}) => {
  await open(page);
  await page
    .getByRole('group', { name: 'Preview settings' })
    .getByLabel('Size in pixels')
    .fill('40');
  await addTwo(page);
  await page.getByTestId('quick-collection').click();
  await expect(page.getByRole('heading', { name: 'Quick Collection' })).toBeVisible();
  await expect(page.getByTestId('quick-collection')).toHaveAttribute('aria-current', 'true');
  await expect(reviewRows(page)).toHaveCount(2);
  await expect(reviewRows(page).first().locator('.sample')).toHaveCSS('font-size', '40px');

  // The button drops one.
  await review(page)
    .getByRole('button', { name: /^Drop JetBrains Mono/ })
    .click();
  await expect(reviewRows(page)).toHaveCount(1);
  await expect(count(page)).toHaveText('1');

  // Delete drops the focused one, from the keyboard.
  await review(page)
    .getByRole('button', { name: /^Drop Inter/ })
    .focus();
  await page.keyboard.press('Delete');
  await expect(reviewRows(page)).toHaveCount(0);
  await expect(review(page)).toContainText('The Quick Collection is empty.');
  await expect(count(page)).toHaveText('0');

  // Back to the list.
  await review(page).getByRole('button', { name: 'Back to fonts' }).click();
  await expect(list(page)).toBeVisible();
});

test('"Save as collection" keeps exactly the fonts left; "Tag all" tags them', async ({ page }) => {
  await open(page);
  await addTwo(page);
  await page.getByTestId('quick-collection').click();
  await review(page)
    .getByRole('button', { name: /^Drop JetBrains Mono/ })
    .click();
  await expect(reviewRows(page)).toHaveCount(1);

  // Every control is reached from the keyboard, by its label.
  await review(page).getByLabel('Collection name').focus();
  await page.keyboard.type('Keepers');
  await page.keyboard.press('Tab');
  await expect(review(page).getByRole('button', { name: 'Save as collection' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('toast')).toContainText(
    'Saved 1 font as the collection “Keepers”.',
  );

  await review(page).getByLabel('Tag').fill('shortlist');
  await review(page).getByRole('button', { name: 'Tag all' }).click();
  await expect(page.getByTestId('toast')).toContainText('Tagged 1 font “shortlist”.');

  // The collection holds Inter only.
  const collections = page.getByRole('region', { name: 'Collections' });
  const keepers = collections.getByRole('listitem', { name: 'Keepers', exact: true });
  await expect(keepers.getByLabel('1 font', { exact: true })).toBeVisible();
  await keepers.getByRole('button', { name: 'Keepers', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Keepers' })).toBeVisible();
  await expect(list(page).getByRole('treeitem')).toHaveCount(1);
  await expect(family(page, 'Inter')).toBeVisible();
});
