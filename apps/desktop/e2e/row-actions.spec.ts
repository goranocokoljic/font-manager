import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';

// E54.9: the row actions of a list row, a wall card and a table row are 32 px targets around
// a 20 px glyph and do not change the row; every one shows the app's own tooltip (its name and
// its key) on hover and on keyboard focus, never a native `title`; a press and Esc hide it.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function serve(page: Page): Promise<void> {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
}

const tree = (page: Page) => page.getByRole('tree', { name: 'Font list' });

/** The tooltip of an icon button: the `role="tooltip"` its aria-describedby names. */
async function tooltipOf(page: Page, button: Locator): Promise<Locator> {
  const ids = ((await button.getAttribute('aria-describedby')) ?? '').split(' ');
  for (const id of ids) {
    const el = page.locator(`[id="${id}"][role="tooltip"]`);
    if ((await el.count()) === 1) return el;
  }
  throw new Error('the button has no tooltip');
}

type Box = { x: number; y: number; width: number; height: number };
const overlaps = (a: Box, b: Box) =>
  a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;

/** Hovers a row action and checks its size, its tooltip's text and key, and where it is. */
async function hoverAndRead(
  page: Page,
  button: Locator,
  text: string | RegExp,
  key: string,
): Promise<void> {
  await button.hover();
  const box = (await button.boundingBox())!;
  expect(box.width, 'a 32 px target').toBeGreaterThanOrEqual(32);
  expect(box.height, 'a 32 px target').toBeGreaterThanOrEqual(32);
  const glyph = (await button.locator('svg').boundingBox())!;
  expect(Math.round(glyph.width), 'a 20 px glyph').toBe(20);
  expect(Math.round(glyph.height), 'a 20 px glyph').toBe(20);
  expect(await button.getAttribute('title'), 'no native tooltip').toBeNull();

  const tip = await tooltipOf(page, button);
  await expect(tip).toBeVisible();
  await expect(tip.locator('.text')).toHaveText(text);
  await expect(tip.locator('kbd')).toHaveText(key);
  const tipBox = (await tip.boundingBox())!;
  expect(overlaps(tipBox, box), 'the tooltip never covers its button').toBe(false);
  // A press hides it. The pointer is let go away from the button, so nothing is clicked.
  await page.mouse.down();
  await expect(tip).toBeHidden();
  await page.mouse.move(0, 0);
  await page.mouse.up();
}

/** A row's box and the box of its first line of text. */
async function geometry(row: Locator, text: Locator) {
  return { row: await row.boundingBox(), text: await text.boundingBox() };
}

test('list rows: Activate, Compare and Similar are 32 px, with their own tooltips', async ({
  page,
}) => {
  await serve(page);
  await page.goto('/?mockFaces=3&mockStyles=4#/');
  await page.evaluate(() => localStorage.clear());
  const row = tree(page).getByRole('treeitem', { name: /^Sample 2,/ });
  await expect(row).toBeVisible();
  // The row's height and its name do not move when its actions come up.
  await page.mouse.move(0, 0);
  const name = row.locator('[data-family-name]').first();
  const atRest = await geometry(row, name);
  await row.hover();
  await expect(row.getByTestId('row-similar')).toBeVisible();
  expect(await geometry(row, name)).toEqual(atRest);

  await hoverAndRead(
    page,
    row.getByTestId('row-activate'),
    /^Switch Sample 2 .* on or off for this session$/,
    'A',
  );
  await row.hover();
  await hoverAndRead(page, row.getByTestId('row-compare'), /^Add Sample 2 to Compare$/, 'C');
  await row.hover();
  await hoverAndRead(
    page,
    row.getByTestId('row-similar'),
    /^Find the fonts similar to Sample 2 /,
    'S',
  );
});

test('a table row and a wall card: the compare action is 32 px with its tooltip', async ({
  page,
}) => {
  await serve(page);
  await page.goto('/?mockFaces=3&mockStyles=4#/');
  await page.evaluate(() => localStorage.clear());
  await expect(tree(page).getByRole('treeitem').first()).toBeVisible();

  await page.keyboard.press('3');
  const table = page.getByRole('treegrid', { name: 'Font table' });
  await expect(table).toBeVisible();
  const tableRow = table.getByRole('row').nth(1);
  await page.mouse.move(0, 0);
  const atRest = await tableRow.boundingBox();
  await tableRow.hover();
  expect(await tableRow.boundingBox(), 'the table row keeps its height').toEqual(atRest);
  await hoverAndRead(page, tableRow.getByTestId('row-compare'), /^Add .+ to Compare$/, 'C');

  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Grid' })
    .click();
  const wall = page.getByRole('listbox', { name: 'Font grid' });
  await expect(wall).toBeVisible();
  const card = wall.getByRole('option').first();
  await card.hover();
  const compare = page.getByTestId('card-compare').first();
  await hoverAndRead(page, compare, /^Add .+ to Compare$/, 'C');
});

test('a keyboard focus shows the tooltip at once; Esc hides it', async ({ page }) => {
  await serve(page);
  await page.goto('/?mockFaces=3&mockStyles=4#/family/1');
  await expect(page.getByTestId('family-meta')).toBeVisible();
  const similar = page.getByRole('button', { name: 'Find similar' });
  await expect(similar).toBeVisible();
  // Focus by key: Tab from the button before it lands on Similar.
  await page.getByTestId('family-compare').focus();
  await page.keyboard.press('Tab');
  await expect(similar).toBeFocused();
  const tip = await tooltipOf(page, similar);
  await expect(tip).toBeVisible({ timeout: 200 });
  await expect(tip.locator('.text')).toHaveText('Find similar');
  await expect(tip.locator('kbd')).toHaveText('S');
  // Similar's new icon: two waves, no lens.
  const markup = await similar.locator('svg').innerHTML();
  expect(markup).not.toContain('<circle');
  await page.keyboard.press('Escape');
  await expect(tip).toBeHidden();
});

test('the command palette and the shortcut sheet show Similar’s icon', async ({ page }) => {
  await serve(page);
  await page.goto('/?mockFaces=3#/');
  await expect(tree(page).getByRole('treeitem').first()).toBeVisible();
  await tree(page).getByRole('treeitem').first().click();
  await page.keyboard.press('ControlOrMeta+k');
  const palette = page.getByRole('dialog', { name: 'Command palette' });
  await expect(palette).toBeVisible();
  await page.keyboard.type('similar');
  const option = palette.getByRole('option').filter({ hasText: 'Find similar' }).first();
  await expect(option.locator('.icon svg')).toHaveCount(1);
  await page.keyboard.press('Escape');

  await page.keyboard.press('?');
  const sheet = page.getByRole('dialog', { name: 'Keyboard shortcuts' });
  await expect(sheet).toBeVisible();
  const row = sheet.locator('.row').filter({ hasText: 'Find similar' });
  await expect(row.locator('dt svg')).toHaveCount(1);
});

// Screens for the review (E54.9): a hovered row with a tooltip up. Only with FM_SCREENS_DIR.
test('@screens row actions and their tooltip', async ({ page }) => {
  const dir = process.env.FM_SCREENS_DIR;
  test.skip(!dir, 'FM_SCREENS_DIR is not set');
  await serve(page);
  await page.goto('/?mockFaces=3&mockStyles=4#/');
  const row = tree(page).getByRole('treeitem').nth(1);
  await expect(row).toBeVisible();
  for (const [w, h] of [
    [1280, 800],
    [1000, 700],
  ] as const) {
    await page.setViewportSize({ width: w, height: h });
    await page.mouse.move(0, 0);
    await row.hover();
    const similar = row.getByTestId('row-similar');
    await similar.hover();
    await expect(await tooltipOf(page, similar)).toBeVisible();
    await page.screenshot({ path: `${dir}/row-actions-${w}x${h}.png` });
  }
});
