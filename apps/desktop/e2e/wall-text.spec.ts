import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { wallFixtures } from '../src/api/mockWall';

// Paragraph mode (E43.1, boards 3-wall-text.png and 1-wall-paragraph.png): your own long
// text, or the Paragraph preset, turns the wall into reading rows. The wall library has
// 60 families; the ones without Latin Extended-A cannot render Serbian.
const files = wallFixtures();
const SERBIAN =
  'Ljubazni fenjerdžija čađavog lica hoće da mi pokaže štos, pa onda i drugi štos, i još jedan.';

async function openWall(page: Page): Promise<Locator> {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.route('**/fmfont/*', (route) => {
    const id = Number(new URL(route.request().url()).pathname.split('/').pop());
    const path = files.get(id);
    if (!path) return route.fulfill({ status: 404 });
    // Paths are relative to fixtures/fonts.
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

const row = (page: Page) => page.getByRole('group', { name: 'Preview settings' });
const field = (page: Page) => row(page).getByRole('textbox', { name: 'Type to preview' });
const chip = (page: Page, name: string) =>
  row(page).getByRole('group', { name: 'Preset' }).getByRole('button', { name });
const countLine = (page: Page) => page.getByTestId('count-line');

test.describe('paragraph mode', () => {
  test('long text of your own becomes two-column reading rows at body size', async ({ page }) => {
    const wall = await openWall(page);
    await field(page).fill(SERBIAN);
    await expect(wall).toHaveAttribute('data-layout', 'rows');
    await expect(chip(page, 'Paragraph')).toHaveAttribute('aria-pressed', 'true');

    const cards = wall.getByRole('option');
    await expect(cards.nth(1)).toBeVisible();
    // Two columns: the first two rows side by side, the third under the first.
    const [a, b, c] = await Promise.all([0, 1, 2].map((i) => cards.nth(i).boundingBox()));
    expect(b!.y).toBe(a!.y);
    expect(b!.x).toBeGreaterThan(a!.x + a!.width);
    expect(c!.x).toBe(a!.x);
    expect(c!.y).toBeGreaterThan(a!.y);

    const first = cards.first();
    const card = first.locator('.card');
    await expect(card).toHaveCSS('padding', '18px');
    const meta = first.locator('.meta');
    await expect(meta).toHaveCSS('font-size', '12px');
    const text = first.getByTestId('card-live-text');
    await expect(text).toHaveCSS('font-size', '17px');
    await expect(text).toHaveCSS('line-height', `${17 * 1.45}px`);
    // The name and its meta on top, the text under it.
    const metaBox = await meta.boundingBox();
    const textBox = await text.boundingBox();
    expect(metaBox!.y + metaBox!.height).toBeLessThanOrEqual(textBox!.y);
    await expect(countLine(page).getByTestId('body-size')).toHaveText(
      'Body size 17 px · line height 1.45',
    );
  });

  test('the Preview options size applies to the rows', async ({ page }) => {
    const wall = await openWall(page);
    await field(page).fill(SERBIAN);
    await expect(wall).toHaveAttribute('data-layout', 'rows');
    await row(page).getByRole('button', { name: 'Preview options' }).click();
    const size = row(page).getByLabel('Size in pixels');
    await expect(size).toHaveValue('17');
    await size.fill('21');
    await size.press('Enter');
    await expect(wall.getByTestId('card-live-text').first()).toHaveCSS('font-size', '21px');
    await expect(countLine(page).getByTestId('body-size')).toContainText('Body size 21 px');
  });

  test('the count line says how many families render text past Basic Latin', async ({ page }) => {
    await openWall(page);
    await field(page).fill(SERBIAN);
    const count = countLine(page).getByTestId('renders-text');
    await expect(count).toHaveText(/^\d+ of 60 families$/);
    const covered = Number((await count.textContent())!.split(' ')[0]);
    expect(covered).toBeGreaterThan(0);
    expect(covered).toBeLessThan(60);
    await expect(countLine(page).getByRole('status')).toHaveText(
      `${covered} of 60 families render your text completely`,
    );
    // Plain English text: every family renders it, the line counts families.
    await field(page).fill('The quick brown fox jumps over the lazy dog.');
    await expect(countLine(page).getByTestId('renders-text')).toHaveCount(0);
    await expect(countLine(page).getByRole('status')).toHaveText('60 families');
  });

  test('the switch crossfades, and switching back restores the wall and its scroll', async ({
    page,
  }) => {
    const wall = await openWall(page);
    await wall.evaluate((el) => {
      el.scrollTop = 437;
      el.dispatchEvent(new Event('scroll'));
    });
    await page.waitForTimeout(200);
    const left = await wall.evaluate((el) => el.scrollTop);
    expect(left).toBeGreaterThan(0);

    // Watch the page: both lists are on screen at once while they crossfade, each fading.
    const grids = page.locator('[role="listbox"][aria-label="Font grid"]');
    await page.evaluate(() => {
      const seen = { max: 0, fading: [] as string[] };
      (window as unknown as { seen: typeof seen }).seen = seen;
      new MutationObserver(() => {
        const els = [...document.querySelectorAll('[role="listbox"][aria-label="Font grid"]')];
        if (els.length > seen.max) {
          seen.max = els.length;
          seen.fading = els.map((el) => getComputedStyle(el).transitionProperty);
        }
      }).observe(document.body, { childList: true, subtree: true });
    });
    await chip(page, 'Paragraph').click();
    await expect(page.locator('[role="listbox"][data-layout="rows"]')).toBeVisible();
    await expect(grids).toHaveCount(1);
    const seen = await page.evaluate(
      () => (window as unknown as { seen: { max: number; fading: string[] } }).seen,
    );
    expect(seen.max).toBe(2);
    expect(seen.fading).toEqual(['opacity', 'opacity']);

    await chip(page, 'Word').click();
    await expect(page.locator('[role="listbox"][data-layout="wall"]')).toBeVisible();
    await expect(grids).toHaveCount(1);
    expect(await grids.evaluate((el) => el.scrollTop)).toBe(left);
  });
});
