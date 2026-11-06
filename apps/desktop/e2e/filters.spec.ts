import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { openFilters } from './filters';

// Preview rows load face files from `/fmfont/<id>`; serve one fixture for all.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page, query = '?mockLibrary=wall') {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(`/${query}#/`);
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
}

const bar = (page: Page) => page.locator('header.toolbar').getByRole('region', { name: 'Filters' });
const opener = (page: Page) => bar(page).getByRole('button', { name: /^Filters/ });
const line = (page: Page) => page.getByTestId('count-line');
const facet = (drawer: ReturnType<Page['locator']>, group: string, name: RegExp) =>
  drawer.getByRole('group', { name: group }).getByRole('button', { name });

/** Chooses Sans serif and a Serbian language from the drawer, and closes it. */
async function chooseTwo(page: Page) {
  const drawer = await openFilters(page);
  await facet(drawer, 'Class', /^Sans serif/).click();
  await drawer.getByRole('button', { name: 'Serbian (Cyrillic)' }).click();
  await drawer.getByRole('button', { name: 'Close' }).click();
  await expect(drawer).toBeHidden();
}

test('the top bar shows Filters with an accent count badge that opens the drawer', async ({
  page,
}) => {
  await open(page);
  await expect(opener(page)).toHaveAccessibleName('Filters');
  await expect(opener(page).locator('.ui-badge')).toHaveCount(0);
  await chooseTwo(page);

  await expect(opener(page)).toHaveAccessibleName('Filters 2');
  const badge = opener(page).locator('.ui-badge');
  await expect(badge).toHaveText('2');
  const colors = await badge.evaluate((el) => {
    const probe = document.createElement('span');
    document.body.append(probe);
    const resolve = (v: string) => {
      probe.style.color = v;
      return getComputedStyle(probe).color;
    };
    const want = [resolve('var(--fm-accent)'), resolve('var(--fm-on-accent)')];
    probe.remove();
    const style = getComputedStyle(el);
    return { fill: style.backgroundColor, text: style.color, want };
  });
  expect([colors.fill, colors.text]).toEqual(colors.want);

  await opener(page).click();
  await expect(page.getByRole('dialog', { name: 'Filter options' })).toBeVisible();
});

test('active filters are chips right of the button, and a chip removes its filter', async ({
  page,
}) => {
  await open(page);
  await chooseTwo(page);
  const sans = bar(page).getByRole('button', { name: 'Remove filter Sans serif' });
  const serbian = bar(page).getByRole('button', { name: 'Remove filter Serbian (Cyrillic)' });
  await expect(sans).toBeVisible();
  await expect(serbian).toBeVisible();
  // The chips sit after the button, in reading order.
  const left = async (l: typeof sans) => (await l.boundingBox())!.x;
  expect(await left(sans)).toBeGreaterThan(await left(opener(page)));
  expect(await left(serbian)).toBeGreaterThan(await left(sans));

  // The keyboard stays in the bar when the chip under it goes away.
  await sans.focus();
  await page.keyboard.press('Enter');
  await expect(sans).toHaveCount(0);
  await expect(opener(page)).toBeFocused();
  await expect(opener(page)).toHaveAccessibleName('Filters 1');
  await expect(line(page).getByRole('status')).not.toContainText('Sans serif');

  // A language chip has its own way out.
  await expect(line(page).getByRole('status')).toContainText('Serbian (Cyrillic)');
  await serbian.click();
  await expect(serbian).toHaveCount(0);
  await expect(opener(page)).toHaveAccessibleName('Filters');
  await expect(opener(page)).toBeFocused();
  await expect(line(page).getByRole('status')).not.toContainText('Serbian (Cyrillic)');
  await expect(line(page).getByRole('status')).toHaveText('60 families');
});

test('+N hands the keyboard back to the button when the drawer closes and +N is gone', async ({
  page,
}) => {
  await open(page);
  await page.setViewportSize({ width: 1000, height: 700 });
  const drawer = await openFilters(page);
  const formats = drawer.getByRole('group', { name: 'Format' }).getByRole('button');
  for (let i = 0, n = await formats.count(); i < n; i++) await formats.nth(i).click();
  for (const name of ['English', 'Polish', 'Greek', 'Serbian (Cyrillic)']) {
    await drawer.getByRole('button', { name, exact: true }).click();
  }
  await drawer.getByRole('button', { name: 'Close' }).click();
  await expect(drawer).toBeHidden();

  const more = bar(page).getByRole('button', { name: /^\+\d+ more filter/ });
  await more.focus();
  await page.keyboard.press('Enter');
  await expect(drawer).toBeVisible();
  // Turn off all but one: every chip fits, and +N is hidden.
  const on = drawer.locator('button[aria-pressed="true"]');
  while ((await on.count()) > 1) {
    const before = await on.count();
    await on.first().click();
    await expect(on).toHaveCount(before - 1);
  }
  await expect(more).toBeHidden();
  await page.keyboard.press('Escape');
  await expect(drawer).toBeHidden();
  await expect(opener(page)).toBeFocused();
});

test('chips that do not fit collapse into +N on one row', async ({ page }) => {
  await open(page);
  const drawer = await openFilters(page);
  const formats = drawer.getByRole('group', { name: 'Format' }).getByRole('button');
  const chosen = await formats.count();
  expect(chosen).toBeGreaterThan(1);
  for (let i = 0; i < chosen; i++) await formats.nth(i).click();
  for (const name of ['English', 'Polish', 'Greek', 'Serbian (Cyrillic)']) {
    await drawer.getByRole('button', { name, exact: true }).click();
  }
  const total = chosen + 4;
  await drawer.getByRole('button', { name: 'Close' }).click();
  await expect(opener(page)).toHaveAccessibleName(`Filters ${total}`);

  // A narrow window: not all of them fit.
  await page.setViewportSize({ width: 1000, height: 700 });
  const shown = bar(page).getByRole('button', { name: /^Remove filter/ });
  const more = bar(page).getByRole('button', { name: /^\+\d+ more filter/ });
  await expect(more).toBeVisible();

  // The bar takes the room a moment after the window shrinks: until that happens, it is
  // narrower than the chips measured for it.
  await expect(async () => {
    const visible = await shown.count();
    await expect(more).toHaveText(`+${total - visible}`, { timeout: 500 });
    expect(visible).toBeLessThan(total);

    // One row: every chip on the line of the button, and none past the end of the bar.
    const barBox = (await bar(page).boundingBox())!;
    const moreBox = (await more.boundingBox())!;
    expect(moreBox.x + moreBox.width).toBeLessThanOrEqual(barBox.x + barBox.width + 1);
    for (let i = 0; i < visible; i++) {
      const box = (await shown.nth(i).boundingBox())!;
      expect(Math.abs(box.y + box.height / 2 - (moreBox.y + moreBox.height / 2))).toBeLessThan(2);
    }
  }).toPass();

  // +N opens the drawer, where all of them are.
  await more.click();
  await expect(page.getByRole('dialog', { name: 'Filter options' })).toBeVisible();

  // Room again: the chips come back and +N goes.
  await page.keyboard.press('Escape');
  await page.setViewportSize({ width: 3000, height: 700 });
  await expect(shown).toHaveCount(total);
  await expect(more).toBeHidden();
});

for (const mode of ['List', 'Table', 'Grid', 'Waterfall', 'Layout']) {
  test(`no chip wall above the ${mode} view, and every facet is in the drawer`, async ({
    page,
  }) => {
    await open(page);
    await page
      .getByRole('group', { name: 'View mode' })
      .getByRole('button', { name: mode })
      .click();
    // The top bar holds the button; the view below it has no filters of its own.
    await expect(opener(page)).toBeVisible();
    await expect(page.locator('.browse').getByRole('region', { name: 'Filters' })).toHaveCount(0);
    await expect(page.locator('.browse button[aria-pressed][aria-label*="famil"]')).toHaveCount(0);

    const drawer = await openFilters(page);
    // Parity: every facet the wall used to show, by its group.
    for (const group of [
      'Activation',
      'Class',
      'Languages',
      'Slant',
      'Variable',
      'Color',
      'Format',
      'Usage',
      'Open license',
      'Embedding',
      'Source',
      'Health',
      'Favorite',
      'Features',
    ]) {
      await expect(drawer.getByRole('group', { name: group, exact: true })).toBeVisible();
    }
  });
}

test('the count line reads the count, the filters and the sort', async ({ page }) => {
  await open(page);
  const status = line(page).getByRole('status');
  await expect(status).toHaveText('60 families');
  await expect(line(page)).toContainText('Sorted by name');
  await chooseTwo(page);

  const count = (await status.locator('strong').textContent())!;
  expect(count).toMatch(/^\d+ famil(y|ies)$/);
  await expect(status).toHaveText(`${count} · Sans serif · Serbian (Cyrillic)`);
  await expect(line(page)).toContainText('Sorted by name · change');
  // The count is the families the wall shows.
  const n = Number(count.split(' ')[0]);
  await expect(
    page.locator('[role="tree"][aria-label="Font list"] [aria-level="1"]').first(),
  ).toHaveAttribute('aria-setsize', String(n));
});

const names = (page: Page) =>
  page
    .getByRole('tree', { name: 'Font list' })
    .locator('[aria-level="1"]')
    .evaluateAll((rows) => rows.map((r) => r.getAttribute('aria-label')!.split(',')[0]!));

test('change sorts the list, with filters on or off, and the line says so', async ({ page }) => {
  await open(page, '?mockFaces=2&mockStyles=4');
  const change = line(page).getByRole('button', { name: 'change sort order' });
  const menu = page.getByRole('group', { name: 'Sort order' });
  await change.click();
  await expect(menu.getByRole('radio', { name: 'Name' })).toBeFocused();
  await expect(menu.getByRole('radio', { name: 'Name' })).toBeChecked();

  await menu.getByRole('checkbox', { name: 'Descending' }).check();
  await expect(line(page)).toContainText('Sorted by name, descending');
  await expect
    .poll(() => names(page))
    .toEqual(['Sample 2', 'Sample 1', 'Legacy Grotesk', 'JetBrains Mono', 'Inter']);

  // The order holds with a filter on: search results take it too.
  await page.keyboard.press('Escape');
  await expect(menu).toBeHidden();
  await expect(change).toBeFocused();
  const drawer = await openFilters(page);
  await facet(drawer, 'Slant', /^Italic/).click();
  await expect.poll(() => names(page)).toEqual(['Sample 2', 'Sample 1']);
  await drawer.getByRole('button', { name: 'Close' }).click();

  // Other key, ascending again.
  await change.click();
  await menu.getByRole('checkbox', { name: 'Descending' }).uncheck();
  await expect(line(page)).toContainText('Sorted by name');
  await expect.poll(() => names(page)).toEqual(['Sample 1', 'Sample 2']);
  await menu.getByRole('radio', { name: 'Date added' }).check();
  await expect(line(page)).toContainText('Sorted by date added');
});

test('a text search says best match, and the menu goes back to it', async ({ page }) => {
  await open(page, '?mockFaces=2&mockStyles=4');
  await page.getByRole('searchbox', { name: 'Search fonts' }).fill('sample');
  await expect(line(page)).toContainText('Sorted by best match');
  await line(page).getByRole('button', { name: 'change sort order' }).click();
  const menu = page.getByRole('group', { name: 'Sort order' });
  await expect(menu.getByRole('radio', { name: 'Best match' })).toBeChecked();
  await expect(menu.getByRole('checkbox', { name: 'Descending' })).toBeDisabled();
  await menu.getByRole('radio', { name: 'Name' }).check();
  await expect(line(page)).toContainText('Sorted by name');
  await menu.getByRole('radio', { name: 'Best match' }).check();
  await expect(line(page)).toContainText('Sorted by best match');
});

test('a sort chosen in the table holds in the wall', async ({ page }) => {
  await open(page, '?mockFaces=2&mockStyles=4');
  const modes = page.getByRole('group', { name: 'View mode' });
  await modes.getByRole('button', { name: 'Table' }).click();
  const header = page.getByRole('columnheader', { name: /^Name/ });
  await header.getByRole('button').click();
  await header.getByRole('button').click();
  await expect(header).toHaveAttribute('aria-sort', 'descending');
  await expect(line(page)).toContainText('Sorted by name, descending');
  await modes.getByRole('button', { name: 'List' }).click();
  await expect.poll(async () => (await names(page))[0]).toBe('Sample 2');
});
