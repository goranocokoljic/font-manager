import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { wallFixtures } from '../src/api/mockWall';
import { showSection } from './family-page';
import { openFilters } from './filters';

// 'An R like this one' (E49.9): a Letters card's letters, and the glyph card of the Family
// page's Characters section, offer 'Fonts with an R like this'. It sorts the wall by how close
// each font's drawing of the letter is to that face's (the API ranks; the mock stands in with
// its own distance), the count line reads 'Sorted by R like <family>' with a removable chip,
// and removing the chip brings back the sort that was on before. Filters stay on throughout.
const files = wallFixtures();

async function openWall(page: Page): Promise<Locator> {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.route('**/fmfont/*', (route) => {
    const id = Number(new URL(route.request().url()).pathname.split('/').pop());
    const path = files.get(id) ?? files.values().next().value!;
    return route.fulfill({
      path: fileURLToPath(new URL(`../../../fixtures/fonts/${path}`, import.meta.url)),
    });
  });
  await page.goto('/?mockLibrary=wall#/');
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Grid' })
    .click();
  const wall = page.getByRole('listbox', { name: 'Font grid' });
  await expect(wall).toBeVisible();
  return wall;
}

const controls = (page: Page) => page.getByRole('group', { name: 'Preview settings' });
const field = (page: Page) => controls(page).getByRole('textbox', { name: 'Type to preview' });
const preset = (page: Page, name: string) =>
  controls(page).getByRole('group', { name: 'Preset' }).getByRole('button', { name });
const line = (page: Page) => page.getByTestId('count-line');
const likeChip = (page: Page) => line(page).getByTestId('like-chip');

/** The family names on the wall, in order. */
const names = (wall: Locator) =>
  wall
    .locator('[data-family-name]')
    .evaluateAll((els) => els.map((e) => e.textContent!.trim()).slice(0, 8));

/** Sets the count line's sort to name, descending: a sort that is not the default. */
async function sortByNameDescending(page: Page): Promise<void> {
  await line(page).getByRole('button', { name: 'change sort order' }).click();
  const menu = page.getByRole('group', { name: 'Sort order' });
  await menu.getByRole('checkbox', { name: 'Descending' }).check();
  await page.keyboard.press('Escape');
  await expect(line(page)).toContainText('Sorted by name, descending');
}

test('a card letter sorts the wall by that glyph; removing the chip restores the sort', async ({
  page,
}) => {
  const wall = await openWall(page);
  await preset(page, 'Letters').click();
  await field(page).fill('Rg');
  // A filter and a sort of its own first: both must hold.
  const drawer = await openFilters(page);
  await drawer.getByRole('group', { name: 'Format' }).getByRole('button', { name: /^TTF/ }).click();
  await drawer.getByRole('button', { name: 'Close' }).click();
  const formatChip = page
    .getByRole('region', { name: 'Filters' })
    .getByRole('button', { name: 'Remove filter TTF' });
  await expect(formatChip).toBeVisible();
  await sortByNameDescending(page);
  await expect.poll(async () => (await names(wall)).length).toBeGreaterThan(3);
  const before = await names(wall);

  // The fourth card's R: its family goes to the front.
  const card = wall.getByRole('option').nth(3);
  const family = (await card.locator('[data-family-name]').textContent())!.trim();
  const key = (await card.getAttribute('data-key'))!;
  const likes = wall.locator(`.corner[data-for="${key}"]`).getByTestId('card-likes');
  await expect(likes).toHaveCSS('opacity', '0');
  await card.hover();
  await expect(likes).toHaveCSS('opacity', '1');
  await expect(likes.getByTestId('card-like')).toHaveText(['R', 'g']);
  // Beside the card and hidden from assistive technology, like the compare icon (an option
  // holds no button); the keyboard has L. The title names the action.
  const r = likes.getByTestId('card-like').first();
  await expect(r).toHaveAttribute('title', 'Fonts with an R like this');
  await r.click();

  await expect(likeChip(page)).toBeVisible();
  await expect(line(page)).toContainText(`Sorted by R like ${family}`);
  await expect(likeChip(page).getByTestId('like-glyph')).toHaveText('R');
  await expect.poll(async () => (await names(wall))[0]).toBe(family);
  expect(await names(wall)).not.toEqual(before);
  await expect(formatChip).toBeVisible();

  await likeChip(page).click();
  await expect(likeChip(page)).toHaveCount(0);
  await expect(line(page)).toContainText('Sorted by name, descending');
  await expect.poll(() => names(wall)).toEqual(before);
  // The chip is gone; the focus is on the sort menu's button, not lost to the page.
  await expect(line(page).getByRole('button', { name: 'change sort order' })).toBeFocused();
  await expect(formatChip).toBeVisible();
});

test('L on the cursor card sorts by its first letter', async ({ page }) => {
  const wall = await openWall(page);
  await preset(page, 'Letters').click();
  await field(page).fill('aR');
  const card = wall.getByRole('option').nth(2);
  const family = (await card.locator('[data-family-name]').textContent())!.trim();
  await card.click();
  await page.keyboard.press('l');
  await expect(line(page)).toContainText(`Sorted by a like ${family}`);
  await expect.poll(async () => (await names(wall))[0]).toBe(family);
  await expect(wall).toBeFocused();
});

test("the Characters glyph card's 'Fonts with an R like this' goes back to the wall sorted", async ({
  page,
}) => {
  const wall = await openWall(page);
  const first = wall.getByRole('option').nth(1);
  const family = (await first.locator('[data-family-name]').textContent())!.trim();
  await first.dblclick();
  await expect(page).toHaveURL(/#\/family\//);
  const panel = await showSection(page, 'Characters');
  await panel.locator('[data-code-point="82"]').click();
  await panel.getByRole('button', { name: 'Fonts with an R like this' }).click();

  await expect(page).not.toHaveURL(/family/);
  await expect(wall).toBeVisible();
  await expect(preset(page, 'Letters')).toHaveAttribute('aria-pressed', 'true');
  await expect(
    controls(page).getByRole('group', { name: 'Letters' }).getByRole('button'),
  ).toHaveText(['R']);
  await expect(line(page)).toContainText(`Sorted by R like ${family}`);
  await expect.poll(async () => (await names(wall))[0]).toBe(family);
});

// Review screenshots of the likeness sort at both review sizes (FM_SCREENS_DIR, as screens.spec).
test('@screens the wall sorted by an R like this one', async ({ page }) => {
  const dir = process.env.FM_SCREENS_DIR;
  test.skip(!dir, 'FM_SCREENS_DIR is not set');
  const wall = await openWall(page);
  await preset(page, 'Letters').click();
  await field(page).fill('ROag');
  const card = wall.getByRole('option').nth(3);
  await card.click();
  await page.keyboard.press('l');
  await expect(likeChip(page)).toBeVisible();
  for (const [w, h] of [
    [1280, 800],
    [1000, 700],
  ] as const) {
    await page.setViewportSize({ width: w, height: h });
    await wall.getByRole('option').nth(1).hover();
    await page.screenshot({ path: `${dir}/letters-like-${w}x${h}.png` });
  }
});
