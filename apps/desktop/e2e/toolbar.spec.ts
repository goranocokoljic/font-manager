import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';

// E50.2: the toolbar never changes shape. The same controls at the same places in every view
// mode; a control of one view lives inside that view (the table's Columns ⋯, the layout preset).

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

/** The view switch in its order (E51.3): Lines · Wall · Table · Waterfall · Layout. */
const MODES = ['List', 'Grid', 'Table', 'Waterfall', 'Layout'] as const;

async function open(page: Page) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto('/#/');
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
}

const toolbar = (page: Page) => page.locator('header.toolbar');
const viewSwitch = (page: Page) => page.getByRole('group', { name: 'View mode' });
const modeButton = (page: Page, name: string) =>
  viewSwitch(page).getByRole('button', { name, exact: true });

async function setMode(page: Page, name: (typeof MODES)[number]) {
  await modeButton(page, name).click();
  await expect(modeButton(page, name)).toHaveAttribute('aria-pressed', 'true');
}

/** Each child of the toolbar: what it is and where it is, rounded to whole pixels. */
function shape(bar: Locator) {
  return bar.evaluate((el) =>
    [...el.children].map((c) => {
      const r = c.getBoundingClientRect();
      const name = c.getAttribute('aria-label') ?? c.getAttribute('role') ?? '';
      return `${c.tagName}.${[...c.classList].sort().join('.')}[${name}] @${Math.round(r.x)},${Math.round(r.y)} ${Math.round(r.width)}x${Math.round(r.height)}`;
    }),
  );
}

test('the toolbar has the same children at the same places in all five view modes', async ({
  page,
}) => {
  await open(page);
  const names = await viewSwitch(page)
    .getByRole('button')
    .evaluateAll((bs) => bs.map((b) => b.getAttribute('aria-label') ?? b.textContent?.trim()));
  expect(names).toEqual([...MODES]);

  const shapes: string[][] = [];
  for (const mode of MODES) {
    await setMode(page, mode);
    // Let the switch's slide settle before measuring.
    await page.waitForTimeout(400);
    shapes.push(await shape(toolbar(page)));
  }
  for (const s of shapes.slice(1)) expect(s).toEqual(shapes[0]);
  // Nothing of one view in the toolbar.
  await expect(toolbar(page).getByRole('button', { name: 'Columns' })).toHaveCount(0);
  await expect(toolbar(page).getByRole('combobox', { name: 'Preset' })).toHaveCount(0);
});

test('the Columns ⋯ ends the table header row; the preset is in the layout pane', async ({
  page,
}) => {
  await open(page);
  await setMode(page, 'Table');
  const header = page.getByRole('treegrid', { name: 'Font table' }).getByRole('row').first();
  const columns = page
    .getByTestId('table-columns')
    .getByRole('button', { name: 'Columns', exact: true });
  await expect(columns).toBeVisible();
  const h = (await header.boundingBox())!;
  const c = (await columns.boundingBox())!;
  // On the header row, at its end.
  expect(c.y).toBeGreaterThanOrEqual(h.y - 1);
  expect(c.y + c.height).toBeLessThanOrEqual(h.y + h.height + 1);
  // Pinned at the end of the header row: the right edge of the table's visible part (the
  // table is wider than the window, so its own end scrolls; the ⋯ does not).
  const grid = page.getByRole('treegrid', { name: 'Font table' });
  const end = await grid.evaluate((el) => el.getBoundingClientRect().left + el.clientWidth);
  expect(Math.abs(c.x + c.width - end)).toBeLessThanOrEqual(6);
  await grid.evaluate((el) => (el.scrollLeft = el.scrollWidth));
  const last = (await header.getByRole('columnheader').last().boundingBox())!;
  expect(c.x).toBeGreaterThanOrEqual(last.x + last.width - 1);
  await grid.evaluate((el) => (el.scrollLeft = 0));
  await columns.click();
  await expect(page.getByRole('group', { name: 'Table columns' })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(columns).toBeFocused();

  await setMode(page, 'Layout');
  const preset = page.getByRole('combobox', { name: 'Preset' });
  await expect(preset).toBeVisible();
  const bar = (await toolbar(page).boundingBox())!;
  expect((await preset.boundingBox())!.y).toBeGreaterThanOrEqual(bar.y + bar.height);
});

test('the view switch does not move from Table to Layout to Wall', async ({ page }) => {
  await open(page);
  const boxes = [];
  for (const mode of ['Table', 'Layout', 'Grid'] as const) {
    await setMode(page, mode);
    await page.waitForTimeout(400);
    boxes.push(await viewSwitch(page).boundingBox());
  }
  expect(boxes[0]).not.toBeNull();
  for (const b of boxes.slice(1)) expect(b).toEqual(boxes[0]);
});

test('the 1–5 keys, the palette and the shortcut sheet follow the switch order (E51.3)', async ({
  page,
}) => {
  await open(page);
  // Lines is the first button and the view a fresh window opens in.
  await expect(modeButton(page, 'List')).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('tree', { name: 'Font list' }).focus();
  for (const [i, mode] of [...MODES].entries()) {
    await page.keyboard.press(String(i + 1));
    await expect(modeButton(page, mode)).toHaveAttribute('aria-pressed', 'true');
  }

  await page.keyboard.press('ControlOrMeta+k');
  const palette = page.getByRole('dialog', { name: 'Command palette' });
  await page.keyboard.type('Show fonts as');
  // Each 'Show fonts as' action carries the key of its place in the switch.
  for (const [i, m] of ['list', 'grid', 'table', 'waterfall', 'layout'].entries())
    await expect(
      palette
        .getByRole('option')
        .filter({ hasText: `Show fonts as a ${m}` })
        .locator('kbd'),
    ).toHaveText(String(i + 1));
  await page.keyboard.press('Escape');

  await page.keyboard.press('?');
  const sheet = page.getByRole('dialog', { name: /Keyboard shortcuts/ });
  await expect(sheet).toContainText('List, Grid, Table, Waterfall, Layout');
});
