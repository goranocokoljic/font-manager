import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { wallFixtures } from '../src/api/mockWall';

// Paragraph mode (E43.1, boards 3-wall-text.png and 1-wall-paragraph.png): your own long
// text, or the Paragraph preset, turns the wall into reading rows. The wall library has
// 60 families; the ones without Latin Extended-A cannot render Serbian.
const files = wallFixtures();
const SERBIAN =
  'Ljubazni fenjerdžija čađavog lica hoće da mi pokaže štos, pa onda i drugi štos, i još jedan.';

async function openWall(page: Page, query = '?mockLibrary=wall'): Promise<Locator> {
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
  await page.goto(`/${query}#/`);
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

test.describe('renders-my-text filter (E43.3)', () => {
  const hideSwitch = (page: Page) => countLine(page).getByRole('switch');
  const filters = (page: Page) => page.getByRole('region', { name: 'Filters' });
  /** The wall's size: every card says how many there are (a virtualized list). */
  const size = (page: Page) =>
    page.getByRole('listbox', { name: 'Font grid' }).getByRole('option').first();

  test('names the detected language and hides the families with missing glyphs', async ({
    page,
  }) => {
    const wall = await openWall(page);
    await field(page).fill(SERBIAN);
    await expect(row(page).getByTestId('preview-hint')).toHaveText(
      // The mock's detection ranks by shared letters; the core's by its own data.
      /^\p{Lu}[^·]+ detected · showing fonts that render every glyph$/u,
    );
    const count = countLine(page).getByTestId('renders-text');
    await expect(count).toHaveText(/^\d+ of 60 families$/);
    const covered = Number((await count.textContent())!.split(' ')[0]);
    await expect(countLine(page).getByTestId('hide-missing')).toHaveText(
      `Hide the ${60 - covered} with missing glyphs`,
    );
    await expect(hideSwitch(page)).toHaveAttribute('aria-checked', 'false');
    await expect(size(page)).toHaveAttribute('aria-setsize', '60');
    await expect(wall.getByRole('option').first()).toHaveAttribute('aria-posinset', '1');

    await hideSwitch(page).click();
    await expect(hideSwitch(page)).toHaveAttribute('aria-checked', 'true');
    await expect(size(page)).toHaveAttribute('aria-setsize', String(covered));
    // A filter chip like any other, counted in the Filters badge.
    const chip = filters(page).getByRole('button', { name: 'Remove filter Renders your text' });
    await expect(chip).toBeVisible();
    await expect(filters(page).locator('button.opener .count')).toHaveText('1');

    // Remembered with the view.
    await page.reload();
    await expect(hideSwitch(page)).toHaveAttribute('aria-checked', 'true');
    await expect(chip).toBeVisible();

    // Removing the chip turns the switch off and brings every family back.
    await chip.click();
    await expect(hideSwitch(page)).toHaveAttribute('aria-checked', 'false');
    await expect(chip).toHaveCount(0);
    await expect(size(page)).toHaveAttribute('aria-setsize', '60');
  });

  test('toggling on 1,000 families re-renders in under 100 ms', async ({ page }) => {
    const wall = await openWall(page, '?mockLibrary=wall&mockFaces=940');
    await field(page).fill(SERBIAN);
    await expect(hideSwitch(page)).toBeVisible();
    await expect(wall.getByRole('option').first()).toHaveAttribute('aria-setsize', '1000');
    // Click to the painted frame: the search (the mock, standing in for the core) and the
    // page's work and paint. The DOM change is seen as it happens; the next frame paints it.
    const once = () =>
      page.evaluate(
        () =>
          new Promise<number>((resolve) => {
            const setsize = () =>
              document
                .querySelector('[role="listbox"][aria-label="Font grid"] [role="option"]')
                ?.getAttribute('aria-setsize');
            const before = setsize();
            const seen = new MutationObserver(() => {
              if (setsize() === before) return;
              seen.disconnect();
              requestAnimationFrame(() => resolve(performance.now() - t0));
            });
            seen.observe(document.body, { subtree: true, childList: true, attributes: true });
            const t0 = performance.now();
            document
              .querySelector<HTMLElement>('[data-testid="count-line"] [role="switch"]')!
              .click();
          }),
      );
    const first = wall.getByRole('option').first();
    // Three times on (and off between): the median, so one garbage collection is not the number.
    const runs: number[] = [];
    for (let i = 0; i < 3; i++) {
      runs.push(await once());
      await expect(first).not.toHaveAttribute('aria-setsize', '1000');
      await hideSwitch(page).click();
      await expect(first).toHaveAttribute('aria-setsize', '1000');
    }
    const ms = runs.sort((a, b) => a - b)[1]!;
    console.log(
      `renders-text switch: the wall repainted ${ms.toFixed(1)} ms after the click (median of ${runs.map((r) => r.toFixed(1)).join(', ')})`,
    );
    // The CI macOS VM runs this page 2-3x slower (see search.spec.ts): there it checks the
    // switch re-renders promptly; the 100 ms budget is held on a desktop machine.
    expect(ms).toBeLessThan(process.env.CI ? 300 : 100);
  });
});
