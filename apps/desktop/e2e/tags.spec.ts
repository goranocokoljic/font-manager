import { expect, test, type Page } from '@playwright/test';

// Browser mode runs on the mock client; no face has tags, stars or a favorite yet.
const info = (page: Page) => page.getByRole('tabpanel', { name: 'Info' });
const chips = (page: Page) => page.getByRole('region', { name: 'Filters' });
const names = (page: Page) =>
  page
    .getByRole('tree', { name: 'Font list' })
    .locator('[role="treeitem"][aria-level="1"]')
    .evaluateAll((rows) => rows.map((r) => r.getAttribute('aria-label') ?? ''));

test('the Info tab tags, favorites and rates a face, and the chips filter by them', async ({
  page,
}) => {
  await page.goto('/#/?face=1');
  const panel = info(page);
  await expect(panel.getByRole('heading', { name: 'Inter Regular' })).toBeVisible();

  const input = panel.getByRole('combobox', { name: 'Add a tag' });
  await input.fill('display');
  await input.press('Enter');
  await expect(panel.getByRole('list', { name: 'Tags' }).getByRole('listitem')).toHaveText([
    /display/,
  ]);
  await expect(input).toHaveValue('');

  const favorite = panel.getByRole('button', { name: 'Favorite' });
  await favorite.click();
  await expect(favorite).toHaveAttribute('aria-pressed', 'true');
  await panel.getByRole('button', { name: '4 stars' }).click();
  await expect(panel.getByTestId('rating')).toHaveText('4 of 5');

  // Another face: completion offers the tag the library now has.
  await page.evaluate(() => (location.hash = '#/?face=6'));
  await expect(panel.getByRole('button', { name: 'Favorite' })).toHaveAttribute(
    'aria-pressed',
    'false',
  );
  await expect(page.locator('#fm-tag-names option[value="display"]')).toHaveCount(1);

  // Each one filters the list to Inter, with its count on the chip.
  for (const [group, chip] of [
    ['Tag', 'display, 1 family'],
    ['Favorite', 'Favorites, 1 family'],
    ['Rating', '4+ stars, 1 family'],
  ]) {
    const button = chips(page).getByRole('group', { name: group }).getByRole('button', {
      name: chip,
    });
    await button.click();
    await expect(button).toHaveAttribute('aria-pressed', 'true');
    await expect.poll(() => names(page)).toEqual([expect.stringMatching(/^Inter/)]);
    await button.click();
    await expect(button).toHaveAttribute('aria-pressed', 'false');
  }

  // Removing the last tag drops it from the chips.
  await page.evaluate(() => (location.hash = '#/?face=1'));
  await panel.getByRole('button', { name: 'Remove tag display' }).click();
  await expect(chips(page).getByRole('group', { name: 'Tag' })).toBeHidden();
});
