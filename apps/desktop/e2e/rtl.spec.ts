import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';

// E19.3: a right-to-left UI language mirrors the chrome; the preview text keeps
// its own direction. No RTL catalog ships yet, so the check runs the CLDR bidi
// pseudo-locale `ar-XB` (English messages, RTL chrome) as the OS language.

const font = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: font }),
  );
  // `?mockHebrew=1`: a Latin + Hebrew face (9), "Frank רוחל", in a folder named in Hebrew.
  await page.goto('/?mockHebrew=1#/');
  await expect(page.locator('[data-face="9"]')).toHaveAttribute('data-state', 'ready');
}

/** The left and right edges of an element on the page. */
async function edges(locator: Locator) {
  const box = await locator.boundingBox();
  if (!box) throw new Error('not visible');
  return { left: box.x, right: box.x + box.width };
}

const columns = (page: Page) => ({
  sidebar: page.getByRole('navigation', { name: 'Library' }),
  list: page.getByRole('main', { name: 'Fonts' }),
  inspector: page.getByRole('complementary', { name: 'Inspector' }),
});

/** The Hebrew face's sample after typing `text` as the list's custom sample text. */
async function sample(page: Page, text: string) {
  const controls = page.getByRole('group', { name: 'Preview settings' });
  await controls.getByLabel('Sample').selectOption('custom');
  await controls.getByLabel('Custom text').fill(text);
  const shown = page.locator('[data-face="9"] .sample');
  await expect(shown).toHaveText(text);
  return shown;
}

/** Selects the Hebrew face's family, so the inspector shows it. */
async function selectHebrew(page: Page) {
  await page.getByRole('treeitem', { name: /^Frank רוחל/ }).click();
  await expect(columns(page).inspector.locator('h2')).toContainText('Frank רוחל');
}

const HEBREW = 'שלום עולם';
const LATIN = 'Hamburgefonstiv';

test.describe('in left-to-right chrome', () => {
  test('sidebar, list, inspector run left to right; a Hebrew sample runs right to left', async ({
    page,
  }) => {
    await open(page);
    await expect(page.locator('html')).toHaveAttribute('dir', 'ltr');
    const { sidebar, list, inspector } = columns(page);
    await expect(inspector).toBeVisible();
    await expect(sidebar).toHaveCSS('direction', 'ltr');
    expect((await edges(sidebar)).right).toBeLessThanOrEqual((await edges(list)).left + 1);
    expect((await edges(list)).right).toBeLessThanOrEqual((await edges(inspector)).left + 1);

    // The sample's direction follows its text, not the chrome.
    const hebrew = await sample(page, HEBREW);
    await expect(hebrew).toHaveCSS('direction', 'rtl');
    // A right-to-left line starts at the right of its row.
    const row = await edges(page.locator('[data-face="9"]'));
    const text = await hebrew.evaluate((el) => {
      const r = document.createRange();
      r.selectNodeContents(el);
      const box = r.getBoundingClientRect();
      return { left: box.left, right: box.right };
    });
    expect(row.right - text.right).toBeLessThan(48);
    expect(text.left - row.left).toBeGreaterThan(48);
    const latin = await sample(page, LATIN);
    await expect(latin).toHaveCSS('direction', 'ltr');
  });
});

test.describe('in right-to-left chrome', () => {
  test.use({ locale: 'ar-XB' });

  test('the layout mirrors: sidebar right, inspector left, text from the right, icons flipped', async ({
    page,
  }) => {
    await open(page);
    const html = page.locator('html');
    await expect(html).toHaveAttribute('dir', 'rtl');
    await expect(html).toHaveAttribute('lang', 'ar-XB');
    const { sidebar, list, inspector } = columns(page);
    await expect(inspector).toBeVisible();
    // Mirrored: inspector | list | sidebar from left to right.
    expect((await edges(inspector)).right).toBeLessThanOrEqual((await edges(list)).left + 1);
    expect((await edges(list)).right).toBeLessThanOrEqual((await edges(sidebar)).left + 1);
    const viewport = page.viewportSize()!;
    expect((await edges(sidebar)).right).toBeGreaterThan(viewport.width - 2);

    // The list's row labels start at the right edge of the list.
    const row = page.getByRole('treeitem').first();
    const label = row.locator('.label bdi').first();
    const rowEdges = await edges(row);
    const labelEdges = await edges(label);
    expect(rowEdges.right - labelEdges.right).toBeLessThan(48);
    expect(labelEdges.left - rowEdges.left).toBeGreaterThan(48);
    await expect(row.locator('.label')).toHaveCSS('direction', 'rtl');

    // A closed family's ▸ points left (toward the row's end).
    const twisty = page.locator('.twisty').first();
    await expect(twisty).toHaveText('▸');
    await expect(twisty).toHaveCSS('transform', 'matrix(-1, 0, 0, 1, 0, 0)');

    // A key chord keeps its order: "Ctrl+K" in the status bar reads left to right.
    await expect(page.locator('footer kbd').first()).toHaveCSS('direction', 'ltr');
  });

  test('a Latin sample runs left to right in the mirrored chrome', async ({ page }) => {
    await open(page);
    const latin = await sample(page, LATIN);
    await expect(latin).toHaveCSS('direction', 'ltr');
    const hebrew = await sample(page, HEBREW);
    await expect(hebrew).toHaveCSS('direction', 'rtl');
  });

  test('names and paths are bidi-isolated in the list, inspector and status bar', async ({
    page,
  }) => {
    await open(page);
    await selectHebrew(page);
    const row = page.getByRole('treeitem', { name: /^Frank רוחל/ });
    await expect(row.locator('.label bdi')).toHaveText('Frank רוחל');
    const inspector = columns(page).inspector;
    await expect(inspector.locator('h2 bdi')).toHaveText(/^Frank רוחל /);
    await expect(inspector.getByRole('button', { name: /^Copy path/ }).locator('bdi')).toHaveText(
      '/Users/demo/Fonts/עברית/FrankRuhl-Regular.ttf',
    );
    await expect(page.getByTestId('app-info').locator('dd bdi').first()).toHaveText(
      /^Font Manager/,
    );
    // Isolated, the name's Hebrew does not pull the text after it into its run:
    // the style follows the name on the line, at the inline end of the heading.
    await expect(inspector.locator('h2 bdi')).toHaveCSS('unicode-bidi', 'isolate');
  });

  test('arrow keys follow what the user sees: divider and inspector tabs', async ({ page }) => {
    await open(page);
    // The sidebar is on the right: ArrowLeft moves the divider left, widening it.
    const divider = page.getByRole('separator', { name: 'Resize sidebar' });
    const before = Number(await divider.getAttribute('aria-valuenow'));
    await divider.focus();
    await page.keyboard.press('ArrowLeft');
    await expect(divider).toHaveAttribute('aria-valuenow', String(before + 8));
    const box = (await edges(columns(page).sidebar)).left;
    await page.keyboard.press('ArrowRight');
    await expect(divider).toHaveAttribute('aria-valuenow', String(before));
    expect((await edges(columns(page).sidebar)).left).toBeGreaterThan(box);

    // The tabs run from the right: ArrowLeft goes to the next one (Axes after Info).
    await selectHebrew(page);
    const info = page.getByRole('tab', { name: 'Info' });
    const axes = page.getByRole('tab', { name: 'Axes' });
    expect((await edges(axes)).right).toBeLessThanOrEqual((await edges(info)).left + 1);
    await info.click();
    await page.keyboard.press('ArrowLeft');
    await expect(axes).toBeFocused();
    await page.keyboard.press('ArrowRight');
    await expect(info).toBeFocused();
  });

  test('the grid runs from the right, and ArrowLeft goes to the next card', async ({ page }) => {
    await open(page);
    await page
      .getByRole('group', { name: 'View mode' })
      .getByRole('button', { name: 'Grid' })
      .click();
    const grid = page.getByRole('listbox', { name: 'Font grid' });
    const cards = grid.getByRole('option');
    await expect(cards.nth(1)).toBeVisible();
    const first = await edges(cards.nth(0));
    const second = await edges(cards.nth(1));
    expect(second.right).toBeLessThanOrEqual(first.left + 1);
    expect(first.right).toBeGreaterThan((await edges(grid)).right - 24);

    await cards.nth(0).click();
    await expect(cards.nth(0)).toHaveAttribute('aria-selected', 'true');
    await page.keyboard.press('ArrowLeft');
    await expect(cards.nth(1)).toHaveAttribute('aria-selected', 'true');
    await page.keyboard.press('ArrowRight');
    await expect(cards.nth(0)).toHaveAttribute('aria-selected', 'true');
  });
});
