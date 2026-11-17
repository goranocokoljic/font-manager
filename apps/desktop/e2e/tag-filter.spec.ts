import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { openFilters } from './filters';

// E54.3: a tag in the sidebar filters the list. The wall's tags display, body, serbian and
// pitch are each on one font of a different family. It is the search store's tag filter, the
// one the Filters popover and its chips set.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page) {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/?mockLibrary=wall#/');
  await expect(rows(page).first()).toBeVisible();
}

const rows = (page: Page) =>
  page.getByRole('tree', { name: 'Font list' }).locator('[role="treeitem"][aria-level="1"]');
const group = (page: Page) => page.getByRole('region', { name: 'Tags' });
const item = (page: Page, name: string) => group(page).locator(`li[data-tag="${name}"]`);
const tag = (page: Page, name: string) => item(page, name).getByRole('button');
const chip = (page: Page, name: string) =>
  page
    .getByRole('region', { name: 'Filters' })
    .getByRole('button', { name: `Remove filter ${name}`, exact: true });

test('a click filters, a second tag combines, the chip releases it, a drop never toggles', async ({
  page,
}) => {
  await open(page);
  const all = await rows(page).count();
  expect(all).toBeGreaterThan(2);
  await expect(tag(page, 'display')).toHaveAttribute('aria-pressed', 'false');
  // The face count stays in the accessible name.
  await expect(tag(page, 'display')).toHaveAccessibleName('display 1 font');

  // A click: one family, the tag pressed, a chip above the list, the popover's tag pressed.
  await tag(page, 'display').click();
  await expect(tag(page, 'display')).toHaveAttribute('aria-pressed', 'true');
  await expect(rows(page)).toHaveCount(1);
  await expect(chip(page, 'display')).toBeVisible();
  const drawer = await openFilters(page);
  const tagsFacet = drawer.getByRole('group', { name: 'Tag', exact: true });
  await expect(tagsFacet.getByRole('button', { name: /^display,/ })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await page.keyboard.press('Escape');
  await expect(drawer).toBeHidden();

  // A second click clears it.
  await tag(page, 'display').click();
  await expect(tag(page, 'display')).toHaveAttribute('aria-pressed', 'false');
  await expect(chip(page, 'display')).toHaveCount(0);
  await expect(rows(page)).toHaveCount(all);

  // Display again; its family is dropped on body, so it carries both.
  await tag(page, 'display').click();
  await expect(rows(page)).toHaveCount(1);
  await rows(page).first().dragTo(item(page, 'body'));
  await expect(page.getByTestId('toast')).toContainText('body');
  await expect(tag(page, 'body')).toHaveAttribute('aria-pressed', 'false');

  // A second tag combines as the popover's do: every chosen tag, so the family stays alone.
  await tag(page, 'body').click();
  await expect(tag(page, 'body')).toHaveAttribute('aria-pressed', 'true');
  await expect(tag(page, 'display')).toHaveAttribute('aria-pressed', 'true');
  await expect(rows(page)).toHaveCount(1);
  await expect(chip(page, 'body')).toBeVisible();
  await openFilters(page);
  await expect(tagsFacet.getByRole('button', { name: /^body,/ })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await page.keyboard.press('Escape');

  // A drop on a pressed tag tags the fonts and leaves the filter on.
  await rows(page).first().dragTo(item(page, 'display'));
  await expect(page.getByTestId('toast')).toContainText('display');
  await expect(tag(page, 'display')).toHaveAttribute('aria-pressed', 'true');
  await expect(rows(page)).toHaveCount(1);

  // Removing the chip releases the tag in the sidebar; body alone gives its two families.
  await chip(page, 'display').click();
  await expect(tag(page, 'display')).toHaveAttribute('aria-pressed', 'false');
  await expect(tag(page, 'body')).toHaveAttribute('aria-pressed', 'true');
  await expect(rows(page)).toHaveCount(2);
});

test('the keyboard reaches the tags with Tab and the arrows and toggles them', async ({ page }) => {
  await open(page);
  const names = await group(page)
    .locator('li[data-tag]')
    .evaluateAll((els) => els.map((el) => (el as HTMLElement).dataset.tag!));
  expect(names.length).toBeGreaterThan(1);
  // Tab reaches every tag.
  await tag(page, names[0]!).focus();
  await page.keyboard.press('Tab');
  await expect(tag(page, names[1]!)).toBeFocused();
  // The arrow keys move between them.
  await page.keyboard.press('ArrowLeft');
  await expect(tag(page, names[0]!)).toBeFocused();
  await page.keyboard.press('ArrowRight');
  await expect(tag(page, names[1]!)).toBeFocused();
  await page.keyboard.press('Home');
  await expect(tag(page, names[0]!)).toBeFocused();

  // Space and Enter toggle the filter.
  await page.keyboard.press('Space');
  await expect(tag(page, names[0]!)).toHaveAttribute('aria-pressed', 'true');
  await expect(rows(page)).toHaveCount(1);
  await page.keyboard.press('Enter');
  await expect(tag(page, names[0]!)).toHaveAttribute('aria-pressed', 'false');
  await expect(tag(page, names[0]!)).toBeFocused();
});

test('the tag filter holds in a library view, a collection and a foundry', async ({ page }) => {
  await open(page);
  const nav = page.getByRole('navigation', { name: 'Library' });
  await tag(page, 'display').click();
  await expect(rows(page)).toHaveCount(1);

  // Favorites holds the display font: the view keeps the tag on top of it.
  await nav.getByRole('button', { name: /^Favorites/ }).click();
  await expect(tag(page, 'display')).toHaveAttribute('aria-pressed', 'true');
  await expect(rows(page)).toHaveCount(1);

  // So does Client brand, a collection with the first twelve fonts.
  await nav.getByRole('listitem', { name: 'Client brand', exact: true }).click();
  await expect(tag(page, 'display')).toHaveAttribute('aria-pressed', 'true');
  await expect(rows(page)).toHaveCount(1);

  // Editorial does not hold it: nothing is left, and the tag stays pressed.
  await nav.getByRole('listitem', { name: 'Editorial', exact: true }).click();
  await expect(tag(page, 'display')).toHaveAttribute('aria-pressed', 'true');
  await expect(rows(page)).toHaveCount(0);

  // Back to All fonts, then a foundry on top: the two combine.
  await nav.getByRole('button', { name: /^All fonts/ }).click();
  await expect(rows(page)).toHaveCount(1);
  const family = await rows(page).first().getAttribute('aria-label');
  expect(family).toBeTruthy();
  await nav.getByRole('button', { name: 'Foundries', exact: true }).click();
  await nav.locator('button[data-foundry]').first().click();
  await expect(tag(page, 'display')).toHaveAttribute('aria-pressed', 'true');
  await expect(chip(page, 'display')).toBeVisible();
});
