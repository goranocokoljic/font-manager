import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// Pairings (E23.4) on the mock client: the Compare sheet lists the fonts the rules
// pair with the first pin. `mockFaces=3` adds Sample 1–3; Sample 1 and 3 copy Inter
// (same designer, same x-height), Sample 2 copies JetBrains Mono.
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
  await page.goto('/?mockFaces=3#/');
  await page.evaluate(() => localStorage.clear());
  const tree = page.getByRole('tree', { name: 'Font list' });
  await expect(tree.getByRole('treeitem')).toHaveCount(6);
  await tree.getByRole('treeitem').first().click();
  return tree;
}

const sheet = (page: Page) => page.getByRole('dialog', { name: 'Compare' });
const panel = (page: Page) => sheet(page).getByRole('region', { name: /^Pairings for / });
const suggestions = (page: Page) =>
  panel(page).getByRole('list', { name: 'Suggested pairings' }).getByRole('listitem');
const pinned = (page: Page) =>
  sheet(page).getByRole('list', { name: 'Side by side' }).getByRole('listitem');

test('the Pairings panel suggests fonts for the first pin; Add to compare pins one by keyboard', async ({
  page,
}) => {
  await open(page);
  await page.keyboard.press('c');
  await page.getByRole('button', { name: 'Compare 1/4' }).click();

  // A panel inside the sheet, not a dialog of its own.
  await expect(panel(page)).toBeVisible();
  await expect(panel(page)).toHaveAccessibleName('Pairings for Inter Regular');
  await expect(page.getByRole('dialog')).toHaveCount(1);

  // Sample 1 and Sample 3, each with its match and reasons; never Inter itself.
  await expect(suggestions(page)).toHaveCount(2);
  await expect(suggestions(page).nth(0)).toContainText('Sample 1 Regular');
  await expect(suggestions(page).nth(1)).toContainText('Sample 3 Regular');
  for (const item of await suggestions(page).all()) {
    await expect(item).toContainText('30% match');
    await expect(item).toContainText('Why: same designer, similar x-height');
  }

  // Tab reaches the labelled button; Enter pins the suggestion.
  const add = panel(page).getByRole('button', { name: 'Add Sample 1 Regular to Compare' });
  await expect(add).toHaveAccessibleDescription('Why: same designer, similar x-height');
  for (let i = 0; i < 20; i++) {
    if (await add.evaluate((el) => el === document.activeElement)) break;
    await page.keyboard.press('Tab');
  }
  await expect(add).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('toast')).toContainText(
    'Sample 1 Regular pinned to Compare (2 of 4)',
  );
  await expect(pinned(page)).toHaveCount(2);
  await expect(pinned(page).nth(1)).toContainText('Sample 1 Regular');

  // The button stays where the focus is, now marked as pinned.
  const done = panel(page).getByRole('button', { name: 'Sample 1 Regular is pinned' });
  await expect(done).toBeFocused();
  await expect(done).toHaveAttribute('aria-disabled', 'true');
  await expect(done).toHaveText('Pinned');
  await page.keyboard.press('Enter');
  await expect(pinned(page)).toHaveCount(2);
});

test('the panel says when nothing pairs with the first pin', async ({ page }) => {
  const tree = await open(page);
  // Legacy Grotesk (third row) declares no x-height and shares no designer.
  await tree.getByRole('treeitem').nth(2).click();
  await page.keyboard.press('c');
  await page.getByRole('button', { name: 'Compare 1/4' }).click();
  await expect(panel(page)).toHaveAccessibleName(/^Pairings for Legacy Grotesk /);
  await expect(panel(page).getByRole('status')).toHaveText(
    /^No font in the library pairs with Legacy Grotesk .* by the rules\.$/,
  );
  // Removing the only pin closes the sheet.
  await sheet(page)
    .getByRole('button', { name: /^Remove Legacy Grotesk/ })
    .click();
  await expect(sheet(page)).toBeHidden();
});
