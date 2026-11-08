import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';

// Dragging families onto collections, tags and the Studio (E44.2, board `4-drag.png`). The
// wall's sidebar has Editorial (31 fonts), the smart Variable, and the tags display, body,
// serbian and pitch.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function openWall(page: Page): Promise<Locator> {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
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

const cards = (wall: Locator) => wall.getByRole('option');
const collections = (page: Page) => page.getByRole('region', { name: 'Collections' });
const collection = (page: Page, name: string) =>
  collections(page).getByRole('listitem', { name, exact: true });
const countOf = (row: Locator) => row.getByTestId('collection-count');
const tag = (page: Page, name: string) =>
  page.getByRole('region', { name: 'Tags' }).locator(`li[data-tag="${name}"]`);
const bar = (page: Page) => page.getByRole('toolbar', { name: 'Selection' });
const ghost = (page: Page) => page.getByTestId('drag-ghost');

/** The centre of `el` in the window. */
async function centre(el: Locator): Promise<{ x: number; y: number }> {
  const box = (await el.boundingBox())!;
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

/** Takes `from` with the pointer and moves it over `to`, without letting go. */
async function dragOver(page: Page, from: Locator, to: Locator): Promise<void> {
  const a = await centre(from);
  await page.mouse.move(a.x, a.y);
  await page.mouse.down();
  await page.mouse.move(a.x + 20, a.y + 20, { steps: 4 });
  const b = await centre(to);
  await page.mouse.move(b.x, b.y, { steps: 6 });
  await page.mouse.move(b.x + 1, b.y, { steps: 2 });
}

/** Selects the first `n` cards of the wall. */
async function selectFirst(wall: Locator, n: number): Promise<void> {
  await cards(wall).nth(0).click();
  if (n > 1)
    await cards(wall)
      .nth(n - 1)
      .click({ modifiers: ['Shift'] });
}

test.describe('dragging fonts to a target', () => {
  test('the ghost lifts and follows, the origin fades, Editorial shows 31 → n and takes the drop', async ({
    page,
  }) => {
    const wall = await openWall(page);
    await selectFirst(wall, 3);
    const editorial = collection(page, 'Editorial');
    await expect(countOf(editorial)).toHaveText('31');

    const from = cards(wall).nth(0);
    const start = await centre(from);
    await page.mouse.move(start.x, start.y);
    await page.mouse.down();
    await page.mouse.move(start.x + 40, start.y + 30, { steps: 4 });

    // The ghost: the card at 0.9 scale, tilted −3°, with a shadow and the count badge.
    await expect(ghost(page)).toBeVisible();
    await expect(ghost(page)).toHaveAttribute('data-count', '3');
    await expect(page.getByTestId('drag-ghost-badge')).toHaveText('3');
    await expect(ghost(page)).toContainText('+ 2 more');
    const lift = page.getByTestId('drag-ghost-lift');
    await expect
      .poll(() => lift.evaluate((el) => getComputedStyle(el).transform))
      .toMatch(/^matrix\(0\.89\d*, -0\.047\d*, 0\.047\d*, 0\.89\d*, 0, 0\)$/);
    await expect.poll(() => lift.evaluate((el) => getComputedStyle(el).boxShadow)).not.toBe('none');
    // It follows the pointer: its box moves with it.
    const before = (await ghost(page).boundingBox())!;
    await page.mouse.move(start.x + 140, start.y + 90, { steps: 4 });
    await expect
      .poll(async () => Math.round((await ghost(page).boundingBox())!.x - before.x))
      .toBe(100);
    // The three dragged cards stay in place at 35 %; the others do not fade.
    for (const i of [0, 1, 2]) await expect(cards(wall).nth(i)).toHaveCSS('opacity', '0.35');
    await expect(cards(wall).nth(3)).toHaveCSS('opacity', '1');

    // Over Editorial: the accent ring and "31 → n".
    const target = await centre(editorial);
    await page.mouse.move(target.x, target.y, { steps: 6 });
    await page.mouse.move(target.x + 1, target.y, { steps: 2 });
    await expect(editorial).toHaveClass(/drop-inside/);
    await expect(editorial).toHaveCSS('outline-style', 'solid');
    await expect(countOf(editorial)).toHaveText(/^31 → \d+$/);
    const after = (await countOf(editorial).textContent())!.split(' → ')[1]!;
    expect(Number(after)).toBeGreaterThan(31);

    // The drop: the ghost flies into the row, the drag ends, the count becomes n.
    await page.mouse.up();
    await expect(ghost(page)).toHaveCount(0);
    await expect(page.getByTestId('drag-ghost-flight')).toHaveCount(0);
    await expect(countOf(editorial)).toHaveText(after);
    await expect(editorial).not.toHaveClass(/drop-inside/);
    await expect(cards(wall).nth(0)).toHaveCSS('opacity', '1');
  });

  test('the drop flies the ghost into the row', async ({ page }) => {
    const wall = await openWall(page);
    await selectFirst(wall, 1);
    const editorial = collection(page, 'Editorial');
    await dragOver(page, cards(wall).nth(0), editorial);
    await expect(countOf(editorial)).toHaveText(/→/);
    await page.mouse.up();
    // The flight is on screen for the move duration, then lands.
    const flight = page.getByTestId('drag-ghost-flight');
    await expect(flight).toBeAttached();
    await expect(flight).toHaveCount(0);
  });

  test('under reduced motion the ghost follows without the tilt and the drop is a cut', async ({
    page,
  }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    const wall = await openWall(page);
    await selectFirst(wall, 1);
    const editorial = collection(page, 'Editorial');
    await dragOver(page, cards(wall).nth(0), editorial);
    await expect
      .poll(() =>
        page.getByTestId('drag-ghost-lift').evaluate((el) => getComputedStyle(el).transform),
      )
      .toBe('matrix(0.9, 0, 0, 0.9, 0, 0)');
    const flights: number[] = [];
    await page.exposeFunction('sawFlight', () => flights.push(1));
    await page.evaluate(() =>
      new MutationObserver(() => {
        if (document.querySelector('[data-testid="drag-ghost-flight"]'))
          (window as unknown as { sawFlight: () => void }).sawFlight();
      }).observe(document.body, { childList: true, subtree: true }),
    );
    await page.mouse.up();
    await expect(countOf(editorial)).not.toHaveText(/→|^31$/);
    expect(flights).toHaveLength(0);
  });

  test('a tag shows its count now and after, and the drop tags the fonts', async ({ page }) => {
    const wall = await openWall(page);
    await selectFirst(wall, 2);
    const display = tag(page, 'display');
    await dragOver(page, cards(wall).nth(1), display);
    await expect(display).toHaveClass(/drop/);
    const count = display.getByTestId('tag-drop-count');
    await expect(count).toHaveText(/^1 → \d+$/);
    const after = (await count.textContent())!.split(' → ')[1]!;
    await page.mouse.up();
    await expect(page.getByTestId('toast')).toContainText('display');
    await expect(display.getByLabel(`${after} fonts`, { exact: true })).toBeAttached();
    await expect(count).toHaveCount(0);
  });

  test('the Studio button takes a drop and opens the Studio with the fonts', async ({ page }) => {
    const wall = await openWall(page);
    await selectFirst(wall, 2);
    const studio = bar(page).getByTestId('bar-studio');
    await dragOver(page, cards(wall).nth(0), studio);
    await expect(studio).toHaveClass(/drop/);
    await expect(studio).toHaveCSS('outline-style', 'solid');
    await page.mouse.up();
    await expect(page).toHaveURL(/#\/studio\?face=\d+&headline=\d+/);
  });

  test('a smart collection takes no drop and says why', async ({ page }) => {
    const wall = await openWall(page);
    await selectFirst(wall, 1);
    const variable = collection(page, 'Variable');
    const was = (await countOf(variable).textContent())!;
    await dragOver(page, cards(wall).nth(0), variable);
    const why = variable.getByRole('tooltip');
    await expect(why).toHaveText(
      'A smart collection fills itself from its rules, so fonts cannot be dropped on it.',
    );
    await expect(variable).not.toHaveClass(/drop-inside/);
    await expect(countOf(variable)).toHaveText(was);
    await page.mouse.up();
    await expect(why).toHaveCount(0);
    await expect(countOf(variable)).toHaveText(was);
    await expect(page.getByTestId('toast')).toHaveCount(0);
  });

  test('Esc cancels a drag: the ghost goes, the cards come back, nothing is dropped', async ({
    page,
  }) => {
    const wall = await openWall(page);
    await selectFirst(wall, 2);
    const editorial = collection(page, 'Editorial');
    await dragOver(page, cards(wall).nth(0), editorial);
    await expect(countOf(editorial)).toHaveText(/^31 → /);
    await page.keyboard.press('Escape');
    await expect(ghost(page)).toHaveCount(0);
    await expect(cards(wall).nth(0)).toHaveCSS('opacity', '1');
    await expect(countOf(editorial)).toHaveText('31');
    await page.mouse.up();
    await expect(countOf(editorial)).toHaveText('31');
    await expect(page.getByTestId('drag-ghost-flight')).toHaveCount(0);
    // Esc ended the drag, not the selection.
    await expect(bar(page)).toBeVisible();
  });
});

test.describe('the keyboard reaches every drop', () => {
  test('the selection bar adds to a collection and tags with the keys alone', async ({ page }) => {
    const wall = await openWall(page);
    await selectFirst(wall, 1);
    const editorial = collection(page, 'Editorial');
    const menu = bar(page).getByRole('combobox', { name: 'Add to collection' });
    await menu.focus();
    await page.keyboard.press('Enter');
    const option = page.getByRole('option', { name: 'Editorial' });
    await expect(option).toBeVisible();
    // The smart collections are not offered: they take no fonts.
    await expect(page.getByRole('option', { name: 'Variable' })).toHaveCount(0);
    await option.focus();
    await page.keyboard.press('Enter');
    await expect(page.getByTestId('toast')).toContainText('Added to Editorial');
    await expect(countOf(editorial)).not.toHaveText('31');

    await bar(page).getByTestId('bar-tag').focus();
    await page.keyboard.press('Enter');
    const panel = page.getByRole('dialog', { name: /^Tags of / });
    await expect(panel).toBeVisible();
    await panel.getByRole('combobox', { name: 'Filter tags or name a new one' }).fill('pitch');
    await page.keyboard.press('Space');
    await page.keyboard.press('Enter');
    await expect(page.getByTestId('toast')).toContainText('Added 1 tag on 1 font');
    await expect(tag(page, 'pitch').getByLabel('2 fonts', { exact: true })).toBeAttached();
  });
});
