import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// E54.5: the catalogs use the library's toolbar. Switching between the library and a catalog
// moves or resizes no shared control (the E50.2 rule); a mode with no effect in a catalog is
// off with its reason (E51.4); the Adobe Fonts note keeps the bar's height and title style.

const font = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

const sidebar = (page: Page) => page.getByRole('navigation', { name: 'Library' });
const toolbar = (page: Page) => page.locator('header.toolbar');
const viewSwitch = (page: Page) => toolbar(page).getByRole('group', { name: 'View mode' });

async function open(page: Page, width: number) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: font }),
  );
  await page.setViewportSize({ width, height: 800 });
  await page.goto('/?mockCatalog=1');
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
}

async function showLibrary(page: Page) {
  await sidebar(page).getByRole('button', { name: 'All fonts' }).click();
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
}

async function showGoogleFonts(page: Page) {
  await page.getByTestId('catalog-googleFonts').click();
  await expect(page.getByTestId('catalog-list').getByRole('listitem')).toHaveCount(4);
}

/** The shared controls of the bar, each with its box rounded to whole pixels. */
async function sharedBoxes(page: Page): Promise<Record<string, string>> {
  // Let the view switch's slide settle.
  await page.waitForTimeout(400);
  const bar = toolbar(page);
  const parts = {
    bar,
    sidebar: bar.getByRole('button', { name: 'Sidebar' }),
    title: bar.locator('h1.title'),
    search: bar.getByRole('searchbox'),
    viewSwitch: viewSwitch(page),
    compare: bar.getByRole('button', { name: /^Compare \d+\/\d+$/ }),
    size: bar.getByTestId('size-slider'),
    ...Object.fromEntries(
      ['List', 'Grid', 'Table', 'Waterfall', 'Layout'].map((m) => [
        `mode ${m}`,
        viewSwitch(page).getByRole('button', { name: m, exact: true }),
      ]),
    ),
  };
  const out: Record<string, string> = {};
  for (const [name, locator] of Object.entries(parts)) {
    await expect(locator, name).toHaveCount(1);
    const b = (await locator.boundingBox())!;
    out[name] =
      `@${Math.round(b.x)},${Math.round(b.y)} ${Math.round(b.width)}x${Math.round(b.height)}`;
  }
  return out;
}

for (const width of [1280, 1024]) {
  test(`library and catalog share every control at the same place and size (${width} px)`, async ({
    page,
  }) => {
    await open(page, width);
    const library = await sharedBoxes(page);
    await showGoogleFonts(page);
    const catalog = await sharedBoxes(page);
    expect(catalog).toEqual(library);
    await page.getByTestId('catalog-fontshare').click();
    expect(await sharedBoxes(page), 'Fontshare').toEqual(library);
    await showLibrary(page);
    expect(await sharedBoxes(page), 'back in the library').toEqual(library);
  });
}

test("the catalog's bar has the library's children; no text button for the view mode", async ({
  page,
}) => {
  await open(page, 1280);
  await showGoogleFonts(page);
  const kinds = await toolbar(page).evaluate((el) =>
    [...el.children].map((c) => {
      const name = c.getAttribute('aria-label') ?? c.getAttribute('role') ?? '';
      return `${c.tagName}.${[...c.classList]
        .filter((k) => k !== 'off')
        .sort()
        .join('.')}[${name}]`;
    }),
  );
  expect(kinds).toEqual([
    expect.stringMatching(/^BUTTON\..*\[Sidebar\]$/),
    expect.stringMatching(/^H1\.title\[\]$/),
    expect.stringMatching(/^INPUT\.search.*\[Search Google Fonts\]$/),
    expect.stringMatching(/^DIV\..*fill.*\[\]$/),
    expect.stringMatching(/^DIV\.segmented.*\[View mode\]$/),
    expect.stringMatching(/^BUTTON\..*\[Compare \d+\/\d+\]$/),
    expect.stringMatching(/^DIV\.size\[\]$/),
  ]);
  // Icons only in the switch: no segment shows its name as text.
  const texts = await viewSwitch(page)
    .getByRole('button')
    .evaluateAll((bs) => bs.map((b) => b.textContent?.trim()));
  expect(texts).toEqual(['', '', '', '', '']);
  // The size slider is in the bar, once.
  await expect(page.getByTestId('size-slider')).toHaveCount(1);
  // Refresh and Check for updates are icon buttons with a name and a tooltip.
  for (const [testId, name] of [
    ['catalog-refresh', 'Refresh'],
    ['catalog-check-updates', 'Check for updates'],
  ] as const) {
    const b = page.getByTestId(testId);
    await expect(b).toHaveAccessibleName(name);
    await expect(b).toHaveText('');
    await expect(b).toHaveAttribute('title', /\S/);
    await expect(b.locator('svg')).toHaveCount(1);
  }
});

test('Table and Layout are off in a catalog, with the reason, and do nothing', async ({ page }) => {
  await open(page, 1280);
  await showGoogleFonts(page);
  for (const name of ['Table', 'Layout']) {
    const seg = viewSwitch(page).getByRole('button', { name, exact: true });
    await expect(seg).toBeVisible();
    await expect(seg).toHaveAttribute('aria-disabled', 'true');
    await expect(seg).toHaveAccessibleDescription(/a catalog has List, Grid and Waterfall/);
    await expect(seg).toHaveAttribute('title', /a catalog has List, Grid and Waterfall/);
    // A real press: Playwright waits for an aria-disabled button to be enabled, so dispatch it.
    await seg.dispatchEvent('click');
    await expect(seg).toHaveAttribute('aria-pressed', 'false');
    // Still on the Tab path, so its reason can be read.
    await seg.focus();
    await expect(seg).toBeFocused();
    await page.keyboard.press('Enter');
    await expect(seg).toHaveAttribute('aria-pressed', 'false');
  }
  await expect(viewSwitch(page).getByRole('button', { name: 'List', exact: true })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  const grid = viewSwitch(page).getByRole('button', { name: 'Grid', exact: true });
  await grid.click();
  await expect(grid).toHaveAttribute('aria-pressed', 'true');
  await expect(grid).not.toHaveAttribute('aria-disabled', 'true');
});

test('the Adobe Fonts note keeps the bar height and title style, with no idle control', async ({
  page,
}) => {
  await open(page, 1280);
  const style = (sel: ReturnType<typeof toolbar>) =>
    sel.locator('h1.title').evaluate((h) => {
      const s = getComputedStyle(h);
      return [s.fontFamily, s.fontSize, s.fontWeight, s.lineHeight, s.color].join(' | ');
    });
  const libraryBar = (await toolbar(page).boundingBox())!;
  const libraryTitle = await style(toolbar(page));
  await sidebar(page)
    .getByRole('region', { name: 'Catalogs' })
    .getByRole('button', { name: 'Adobe Fonts' })
    .click();
  const note = page.getByRole('region', { name: 'Adobe Fonts' });
  const bar = note.locator('header.toolbar');
  await expect(bar.getByRole('heading', { name: 'Adobe Fonts', level: 1 })).toBeVisible();
  const box = (await bar.boundingBox())!;
  expect(box.height).toBe(libraryBar.height);
  expect(box.y).toBe(libraryBar.y);
  expect(await style(bar)).toBe(libraryTitle);
  // Only the sidebar toggle, which works here.
  await expect(bar.getByRole('button')).toHaveCount(1);
  await expect(bar.getByRole('button', { name: 'Sidebar' })).toBeVisible();
  await expect(bar.getByRole('searchbox')).toHaveCount(0);
  await expect(bar.getByRole('group')).toHaveCount(0);
  await expect(bar.getByTestId('size-slider')).toHaveCount(0);
});

test.describe('the longest locale (de-DE)', () => {
  test.use({ locale: 'de-DE' });

  for (const width of [1280, 1024]) {
    test(`the refreshed date pushes no control out of the bar (${width} px)`, async ({ page }) => {
      await page.route('**/fmfont/*', (route) =>
        route.fulfill({ status: 200, contentType: 'font/ttf', body: font }),
      );
      await page.setViewportSize({ width, height: 800 });
      await page.goto('/?mockCatalog=1');
      await page.getByTestId('catalog-googleFonts').click();
      await expect(page.getByTestId('catalog-list').getByRole('listitem')).toHaveCount(4);
      await expect(page.getByTestId('catalog-refreshed')).toHaveText(/^Aktualisiert /);
      const bar = (await toolbar(page).boundingBox())!;
      const controls = [
        page.getByTestId('catalog-refresh'),
        page.getByTestId('catalog-check-updates'),
        page.getByTestId('catalog-search'),
        toolbar(page).locator('.segmented'),
        page.getByTestId('size-slider'),
        page.getByTestId('catalog-refreshed'),
      ];
      for (const c of controls) {
        const b = (await c.boundingBox())!;
        expect(b.x).toBeGreaterThanOrEqual(bar.x);
        expect(b.x + b.width).toBeLessThanOrEqual(bar.x + bar.width + 0.5);
        expect(b.y).toBeGreaterThanOrEqual(bar.y);
        expect(b.y + b.height).toBeLessThanOrEqual(bar.y + bar.height + 0.5);
      }
      // The date never overlaps the buttons after it.
      const date = (await page.getByTestId('catalog-refreshed').boundingBox())!;
      const refresh = (await page.getByTestId('catalog-refresh').boundingBox())!;
      expect(date.x + date.width).toBeLessThanOrEqual(refresh.x + 0.5);
    });
  }

  // The tight case: a column just wide enough to keep the bar on one row, so the catalog's row
  // is narrower than the count, the date and both buttons. The date gives way, nothing spills.
  test('in a one-row bar too narrow for the date, it shortens and no control overlaps', async ({
    page,
  }) => {
    await page.route('**/fmfont/*', (route) =>
      route.fulfill({ status: 200, contentType: 'font/ttf', body: font }),
    );
    await page.setViewportSize({ width: 1220, height: 800 });
    await page.goto('/?mockCatalog=1');
    await page.getByTestId('catalog-googleFonts').click();
    await expect(page.getByTestId('catalog-list').getByRole('listitem')).toHaveCount(4);
    await expect(page.getByTestId('catalog-refreshed')).toHaveText(/^Aktualisiert /);
    const bar = toolbar(page);
    const box = async (l: ReturnType<typeof toolbar>) => (await l.boundingBox())!;
    const sidebarButton = await box(bar.getByRole('button').first());
    const fill = await box(bar.locator('.fill'));
    const switcher = await box(bar.locator('.segmented'));
    // One row: the catalog's row sits beside the view switch, not under it.
    expect(Math.abs(fill.y + fill.height / 2 - (switcher.y + switcher.height / 2))).toBeLessThan(4);
    expect(Math.abs(fill.y - sidebarButton.y)).toBeLessThan(8);
    // Squeeze the catalog's row below what its content needs (count + date + two buttons is
    // about 280 px in German): a fixed width, so the case does not hang on one OS's text widths.
    await page.addStyleTag({ content: 'header.toolbar .fill { flex: 0 0 200px !important; }' });
    const squeezed = await box(bar.locator('.fill'));
    expect(Math.round(squeezed.width)).toBe(200);
    // The date is shortened, its full text in the tooltip.
    const date = page.getByTestId('catalog-refreshed');
    const clipped = await date.evaluate((el) => el.scrollWidth > el.clientWidth);
    expect(clipped, 'the date is ellipsized').toBe(true);
    await expect(date).toHaveAttribute('title', /^Aktualisiert /);
    // Every control of the row stays in the row, before the view switch, in order.
    const dateBox = await box(date);
    const refresh = await box(page.getByTestId('catalog-refresh'));
    const check = await box(page.getByTestId('catalog-check-updates'));
    expect(dateBox.x + dateBox.width).toBeLessThanOrEqual(refresh.x + 0.5);
    expect(refresh.x + refresh.width).toBeLessThanOrEqual(check.x + 0.5);
    const after = await box(bar.locator('.segmented'));
    expect(check.x + check.width).toBeLessThanOrEqual(squeezed.x + squeezed.width + 0.5);
    expect(squeezed.x + squeezed.width).toBeLessThanOrEqual(after.x + 0.5);
    // The count is still whole: the date gave way first.
    const count = page.locator('.browser .count');
    expect(await count.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
  });
});
