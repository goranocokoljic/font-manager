// E50.3: the Layout mode is a split (board 9-split). The list stays at the inline start in its
// Lines form, the layout pane follows the list's cursor, and below 1100 px the split collapses to
// the pane with a Fonts button that slides the list over it.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { selectOption } from './select';

// Layout blocks load face files from `/fmfont/<id>`; serve one fixture for all.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page, width = 1280, height = 800) {
  await page.setViewportSize({ width, height });
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/');
  await page.evaluate(() => localStorage.clear());
  await page.goto('/#/');
  await expect(tree(page)).toBeVisible();
}

const tree = (page: Page) => page.getByRole('tree', { name: 'Font list' });
const modeButton = (page: Page, name: string) =>
  page.getByRole('group', { name: 'View mode' }).getByRole('button', { name, exact: true });
const paneHead = (page: Page) => page.getByRole('toolbar', { name: 'Layout pane' });
const region = (page: Page) => page.locator('section.layout');
const shownFamily = (page: Page) => page.getByTestId('layout-face').locator('.face-name');
/** The family name of the list's cursor row (the one the keys and clicks moved to). */
const cursorFamily = (page: Page) =>
  tree(page).locator('[role="treeitem"][aria-selected="true"] [data-family-name]').first();
const divider = (page: Page) => page.getByRole('separator', { name: 'Resize the font list' });
const listWidth = (page: Page) =>
  page.getByTestId('split-list').evaluate((el) => el.getBoundingClientRect().width);

async function toLayout(page: Page) {
  await modeButton(page, 'Layout').click();
  await expect(region(page)).toHaveAttribute('data-state', 'ready');
}

test('the Layout mode is a split: the Lines list at 560 px, the pane beside it', async ({
  page,
}) => {
  await open(page);
  await toLayout(page);
  const list = tree(page);
  await expect(list).toBeVisible();
  expect(await listWidth(page)).toBe(560);
  // The list is at the inline start, the divider and then the pane after it.
  const l = (await page.getByTestId('split-list').boundingBox())!;
  const d = (await divider(page).boundingBox())!;
  const p = (await page.getByTestId('split-pane').boundingBox())!;
  expect(l.x + l.width).toBeLessThanOrEqual(d.x + 1);
  expect(d.x + d.width).toBeLessThanOrEqual(p.x + 1);
  expect(p.x + p.width).toBeCloseTo(1280, 0);
  // The same list as Lines: the 24 px chevron gutter on every row (E50.4), search narrows it.
  const gutter = list.getByTestId('row-gutter').first();
  await expect(gutter).toBeVisible();
  expect((await gutter.boundingBox())!.width).toBeCloseTo(24, 0);
  const before = await list.getByRole('treeitem').count();
  await page.getByRole('searchbox').fill('mono');
  await expect.poll(() => list.getByRole('treeitem').count()).toBeLessThan(before);
  await expect(list.getByRole('treeitem', { name: /^JetBrains Mono/ })).toBeVisible();
});

test('the divider drags the list between 400 and 760 px and the width is remembered', async ({
  page,
}) => {
  await open(page);
  await toLayout(page);
  const box = (await divider(page).boundingBox())!;
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + 80, y, { steps: 5 });
  await page.mouse.up();
  expect(await listWidth(page)).toBe(640);
  await page.reload();
  await expect(region(page)).toHaveAttribute('data-state', 'ready');
  expect(await listWidth(page)).toBe(640);
  await divider(page).focus();
  await page.keyboard.press('End');
  expect(await listWidth(page)).toBe(760);
  await page.keyboard.press('Home');
  expect(await listWidth(page)).toBe(400);
});

test('the pane shows the cursor face: a click, ↓ and the pane buttons change it; text stays', async ({
  page,
}) => {
  await open(page);
  await toLayout(page);
  const list = tree(page);
  const mono = list.getByRole('treeitem', { name: /^JetBrains Mono/ });
  await mono.click();
  await expect(shownFamily(page)).toHaveText('JetBrains Mono');
  await expect(region(page)).toHaveAttribute('data-state', 'ready');

  // An edited block stays when the font changes.
  const first = region(page).getByTestId('layout-block').first();
  await first.dblclick();
  const box = region(page).getByRole('textbox', { name: 'Block 1 text' });
  await box.fill('Kept heading');
  await box.blur();
  await expect(first).toHaveText('Kept heading');

  // The change is a crossfade: the copy of the old page lies over the pane, aria-hidden.
  await page.evaluate(() => {
    const w = window as unknown as { fades: number };
    w.fades = 0;
    const host = document.querySelector('section.layout')!;
    new MutationObserver((records) => {
      for (const r of records)
        for (const n of r.addedNodes)
          if (n instanceof HTMLElement && n.getAttribute('aria-hidden') === 'true') w.fades++;
    }).observe(host, { childList: true });
  });
  await mono.click();
  await page.keyboard.press('ArrowDown');
  await expect(shownFamily(page)).toHaveText((await cursorFamily(page).textContent())!.trim());
  await expect(shownFamily(page)).not.toHaveText('JetBrains Mono');
  await expect
    .poll(() => page.evaluate(() => (window as unknown as { fades: number }).fades))
    .toBeGreaterThan(0);
  expect(
    await page.evaluate(() =>
      getComputedStyle(document.documentElement).getPropertyValue('--fm-motion-fast').trim(),
    ),
  ).toBe('120ms');
  await expect(first).toHaveText('Kept heading');

  // The pane's own buttons move the list's cursor too.
  const afterDown = (await shownFamily(page).textContent())!;
  await paneHead(page).getByRole('button', { name: 'Previous font' }).click();
  await expect(shownFamily(page)).toHaveText('JetBrains Mono');
  await paneHead(page).getByRole('button', { name: 'Next font' }).click();
  await expect(shownFamily(page)).toHaveText(afterDown);
  await expect(cursorFamily(page)).toHaveText(afterDown);
  await expect(first).toHaveText('Kept heading');
});

test('the pane head holds the face, the preset, the width, prev/next and Open in Studio', async ({
  page,
}) => {
  await open(page);
  await toLayout(page);
  const head = paneHead(page);
  await expect(head.getByTestId('layout-face')).toContainText(/\S+ · \d{3}/);
  await expect(head.getByRole('combobox', { name: 'Preset' })).toBeVisible();
  await expect(head.getByTestId('layout-width')).toContainText(/Width \d+ px/);
  await expect(head.getByRole('button', { name: 'Previous font' })).toBeVisible();
  await expect(head.getByRole('button', { name: 'Next font' })).toBeVisible();
  await expect(head.getByRole('button', { name: 'Open in Studio' })).toBeEnabled();
});

test('switch to Layout, ↓ three times, change the preset: the font is kept', async ({ page }) => {
  await open(page);
  const list = tree(page);
  await list.getByRole('treeitem').first().click();
  await toLayout(page);
  await list.focus();
  for (let i = 0; i < 3; i++) await page.keyboard.press('ArrowDown');
  const family = (await cursorFamily(page).textContent())!.trim();
  await expect(shownFamily(page)).toHaveText(family);
  await selectOption(page.getByRole('combobox', { name: 'Preset' }), 'Quote');
  await expect(page.getByRole('region', { name: 'Quote layout' })).toHaveAttribute(
    'data-state',
    'ready',
  );
  await expect(shownFamily(page)).toHaveText(family);
  await expect(cursorFamily(page)).toHaveText(family);
});

test('below 1100 px the split is the pane; Fonts slides the list over it', async ({ page }) => {
  await open(page, 1000, 700);
  await toLayout(page);
  await expect(tree(page)).toHaveCount(0);
  await expect(divider(page)).toHaveCount(0);
  const fonts = paneHead(page).getByRole('button', { name: 'Fonts', exact: true });
  await expect(fonts).toHaveAttribute('aria-expanded', 'false');
  const pane = (await page.getByTestId('split-pane').boundingBox())!;

  await fonts.click();
  await expect(tree(page)).toBeVisible();
  await expect(tree(page)).toBeFocused();
  // Over the pane: the pane does not move.
  await expect
    .poll(async () => Math.round((await page.getByTestId('split-list').boundingBox())!.x))
    .toBe(Math.round(pane.x));
  expect((await page.getByTestId('split-pane').boundingBox())!.x).toBe(pane.x);

  // ↓ in the slid-in list changes the pane behind it.
  const before = (await shownFamily(page).textContent())!;
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await expect(shownFamily(page)).not.toHaveText(before);

  // The list covers the pane's body, not its head: the Fonts button stays visible and clickable.
  const head = (await paneHead(page).boundingBox())!;
  const over = (await page.getByTestId('split-list').boundingBox())!;
  expect(over.y).toBeGreaterThanOrEqual(head.y + head.height - 1);
  // The covered body takes no focus: Tab from the list does not land under it.
  await expect(page.getByTestId('split-pane-content')).toHaveJSProperty('inert', true);
  await tree(page).focus();
  await page.keyboard.press('Tab');
  expect(
    await page.evaluate(
      () => !!document.activeElement?.closest('[data-testid="split-pane-content"]'),
    ),
  ).toBe(false);

  await tree(page).focus();
  await page.keyboard.press('Escape');
  await expect(tree(page)).toHaveCount(0);
  await expect(fonts).toBeFocused();
  await expect(page.getByTestId('split-pane-content')).toHaveJSProperty('inert', false);

  // A click on Fonts closes it as it opened it.
  await fonts.click();
  await expect(tree(page)).toBeVisible();
  await fonts.click();
  await expect(tree(page)).toHaveCount(0);
  await expect(fonts).toHaveAttribute('aria-expanded', 'false');
});

test('selection works in the split: Ctrl+click and Shift+click raise the selection bar', async ({
  page,
}) => {
  await open(page);
  await toLayout(page);
  const rows = tree(page).getByRole('treeitem');
  await rows.nth(0).click();
  await rows.nth(2).click({ modifiers: ['ControlOrMeta'] });
  await expect(rows.nth(0)).toHaveAttribute('aria-selected', 'true');
  await expect(rows.nth(2)).toHaveAttribute('aria-selected', 'true');
  const bar = page.getByTestId('selection-bar');
  await expect(bar).toBeVisible();
  await expect(bar.getByTestId('selection-bar-name')).toHaveText('2 families');
  await rows.nth(0).click();
  await rows.nth(2).click({ modifiers: ['Shift'] });
  await expect(bar.getByTestId('selection-bar-name')).toHaveText('3 families');
});
