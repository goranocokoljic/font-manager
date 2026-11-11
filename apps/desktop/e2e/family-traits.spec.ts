import { expect, test, type Page } from '@playwright/test';
import { openSection, showSection } from './family-page';
import { openFilters } from './filters';

// The trait tags of the Characters section (E49.10): the face's detected letter shapes under
// the glyph grid, each with 'find fonts with this', which goes back to the wall in the Letters
// preset with that letter typed and that value chosen as a chip. The wall library of the mock
// gives its families fixed trait detections; the small library's faces have none.

const controls = (page: Page) => page.getByRole('group', { name: 'Preview settings' });
const filterBar = (page: Page) => page.getByRole('region', { name: 'Filters' });
const traitChips = (page: Page) => filterBar(page).getByTestId('trait-chip');
const tags = (page: Page) => page.getByTestId('trait-tag');

test('a tag goes back to the wall with its letter typed and its value as a chip', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto('/?mockLibrary=wall#/');
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Grid' })
    .click();
  const wall = page.getByRole('listbox', { name: 'Font grid' });
  await expect(wall).toBeVisible();

  // Another filter on the wall first: it must still be there afterwards.
  const drawer = await openFilters(page);
  await drawer.getByRole('group', { name: 'Format' }).getByRole('button', { name: /^TTF/ }).click();
  await drawer.getByRole('button', { name: 'Close' }).click();
  const formatChip = filterBar(page).getByRole('button', { name: 'Remove filter TTF' });
  await expect(formatChip).toBeVisible();

  await wall.getByRole('option').first().dblclick();
  await expect(page).toHaveURL(/#\/family\//);
  const panel = await showSection(page, 'Characters');
  await expect(panel.getByRole('heading', { name: 'Letter shapes' })).toBeVisible();
  // Plain text, in catalog order: the O tag before the g tag; no repeated aspect.
  const words = await tags(page).locator('.words').allInnerTexts();
  const o = words.findIndex((w) => /^O · /.test(w));
  const g = words.findIndex((w) => /^g · (single|double)-storey$/.test(w));
  expect(o).toBeGreaterThanOrEqual(0);
  expect(g).toBeGreaterThan(o);
  await expect(tags(page).first().locator('svg path')).toHaveAttribute('d', /./);

  const tag = tags(page).nth(g);
  const label = words[g]!.replace(' · ', ' ');
  const find = tag.getByRole('button', { name: `Find fonts with ${label}` });
  // 'find fonts with this' shows on hover or focus.
  await expect(find).toHaveCSS('opacity', '0');
  await tag.hover();
  await expect(find).toHaveCSS('opacity', '1');
  // Focus shows it too, for the keyboard.
  await page.mouse.move(0, 0);
  await expect(find).toHaveCSS('opacity', '0');
  await find.focus();
  await expect(find).toHaveCSS('opacity', '1');
  await find.press('Enter');

  await expect(page).not.toHaveURL(/family/);
  await expect(wall).toBeVisible();
  await expect(
    controls(page).getByRole('group', { name: 'Preset' }).getByRole('button', { name: 'Letters' }),
  ).toHaveAttribute('aria-pressed', 'true');
  await expect(
    controls(page).getByRole('group', { name: 'Letters' }).getByRole('button'),
  ).toHaveText(['g']);
  await expect(traitChips(page)).toHaveCount(1);
  await expect(traitChips(page)).toHaveAttribute('aria-label', /^Remove filter g storeys /);
  await expect(formatChip).toBeVisible();
});

test('a family with no detected traits shows no tags and no heading', async ({ page }) => {
  const panel = await openSection(page, 1, 'Characters');
  await expect(panel.getByTestId('family-characters')).toBeVisible();
  await expect(tags(page)).toHaveCount(0);
  await expect(panel.getByText('Letter shapes')).toHaveCount(0);
});
