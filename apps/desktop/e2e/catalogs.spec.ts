import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// E22.6: Adobe Fonts is listed under Online catalogs as a note and a link. The
// browser-mode mock opens the link in a new tab, standing in for the default
// browser; the site is stubbed so the test needs no network.

const sidebar = (page: Page) => page.getByRole('navigation', { name: 'Library' });
const NOTE =
  'Adobe Fonts are delivered encrypted through Creative Cloud, so no third-party app can manage them. Activate them in the Creative Cloud app.';

test.beforeEach(async ({ context }) => {
  await context.route('https://fonts.adobe.com/**', (route) =>
    route.fulfill({ contentType: 'text/html', body: '<title>Adobe Fonts</title>' }),
  );
});

test('the sidebar entry opens the note; the link opens fonts.adobe.com', async ({
  page,
  context,
}) => {
  await page.goto('/');
  const catalogs = sidebar(page).getByRole('region', { name: 'Online catalogs' });
  const entry = catalogs.getByRole('button', { name: 'Adobe Fonts' });
  await expect(entry).toBeVisible();

  await entry.focus();
  await page.keyboard.press('Enter');
  await expect(entry).toHaveAttribute('aria-current', 'true');
  const note = page.getByRole('region', { name: 'Adobe Fonts' });
  await expect(note.getByRole('heading', { name: 'Adobe Fonts', level: 1 })).toBeVisible();
  await expect(note).toContainText(NOTE);

  const link = note.getByRole('link', { name: 'Open Adobe Fonts' });
  await expect(link).toBeVisible();
  await expect(link).toHaveAttribute('href', 'https://fonts.adobe.com');
  await link.focus();
  await expect(link).toBeFocused();
  const [site] = await Promise.all([context.waitForEvent('page'), page.keyboard.press('Enter')]);
  expect(new URL(site.url()).host).toBe('fonts.adobe.com');
  await expect(page, 'the app stays where it was').toHaveURL(/localhost|127\.0\.0\.1/);
});

test('Settings -> Online catalogs shows the same note, reachable by keyboard', async ({
  page,
  context,
}) => {
  await page.goto('/#/settings?section=catalogs');
  const tab = page.getByRole('tab', { name: 'Online catalogs' });
  await expect(tab).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('heading', { name: 'Online catalogs', level: 2 })).toBeVisible();

  const entry = page.getByRole('button', { name: 'Adobe Fonts' });
  await tab.focus();
  await page.keyboard.press('Tab'); // the tab panel
  await page.keyboard.press('Tab'); // the Google Fonts switch (E22.2)
  await expect(page.getByRole('checkbox', { name: 'Google Fonts' })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(entry).toBeFocused();
  await expect(entry).toHaveAttribute('aria-expanded', 'false');
  await page.keyboard.press('Enter');
  await expect(entry).toHaveAttribute('aria-expanded', 'true');

  const note = page.getByRole('region', { name: 'Adobe Fonts' });
  await expect(note.getByRole('heading', { name: 'Adobe Fonts', level: 3 })).toBeVisible();
  await expect(note).toContainText(NOTE);
  await page.keyboard.press('Tab');
  const link = note.getByRole('link', { name: 'Open Adobe Fonts' });
  await expect(link).toBeFocused();
  const [site] = await Promise.all([context.waitForEvent('page'), page.keyboard.press('Enter')]);
  expect(new URL(site.url()).host).toBe('fonts.adobe.com');
});

// E22.2: Google Fonts in the browser-mode mock. Previews load from
// `/fmfont/catalog-<id>-<hex name>`; serve one fixture for all and count them.
const font = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function serveFonts(page: Page): Promise<string[]> {
  const asked: string[] = [];
  await page.route('**/fmfont/*', (route) => {
    asked.push(new URL(route.request().url()).pathname);
    return route.fulfill({ status: 200, contentType: 'font/ttf', body: font });
  });
  return asked;
}

const catalogRows = (page: Page) =>
  page.getByRole('list', { name: 'Google Fonts families' }).getByRole('listitem');

test('turn Google Fonts on, browse, search and change the sample text, by keyboard', async ({
  page,
}) => {
  const asked = await serveFonts(page);
  await page.goto('/');
  const catalogs = sidebar(page).getByRole('region', { name: 'Online catalogs' });
  const entry = catalogs.getByRole('button', { name: 'Google Fonts', exact: true });
  const turnOn = catalogs.getByRole('button', { name: 'Turn on Google Fonts' });
  await expect(turnOn).toHaveAccessibleDescription('Connects to the internet');

  // Open it while off: no list, nothing fetched.
  await entry.focus();
  await page.keyboard.press('Enter');
  await expect(entry).toHaveAttribute('aria-current', 'true');
  await expect(page.getByRole('heading', { name: 'Google Fonts', level: 1 })).toBeVisible();
  await expect(page.getByTestId('catalog-off')).toBeVisible();
  await expect(page.getByTestId('catalog-refreshed')).toHaveText('Not refreshed yet');
  expect(asked.filter((p) => p.includes('catalog-'))).toEqual([]);

  // Turn it on from the sidebar with the keyboard: the list arrives.
  await page.keyboard.press('Tab');
  await expect(turnOn).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(catalogRows(page)).toHaveCount(4);
  await expect(turnOn).toHaveCount(0);
  await expect(page.getByTestId('catalog-refreshed')).toHaveText(/^Refreshed /);
  await expect(page.locator('.browser .count')).toHaveText('4 families');
  await expect
    .poll(() => asked.filter((p) => p.startsWith('/fmfont/catalog-googleFonts-')).length)
    .toBe(4);
  expect(asked).toContain('/fmfont/catalog-googleFonts-4c61746f'); // "Lato"
  // Live text in the family's own font, never an image.
  await expect(page.locator('.browser img')).toHaveCount(0);
  const lato = catalogRows(page).filter({ hasText: 'Lato' }).locator('.sample');
  await expect(lato).toHaveCSS('font-family', /fm-catalog-googleFonts-4c61746f/);

  // Search through the core.
  const search = page.getByRole('searchbox', { name: 'Search Google Fonts' });
  await search.focus();
  await page.keyboard.type('lo');
  await expect(catalogRows(page)).toHaveCount(1);
  await expect(catalogRows(page).first()).toHaveAccessibleName('Lora');
  await page.keyboard.press('Escape');
  await expect(catalogRows(page)).toHaveCount(4);

  // The same preview controls: a custom sample text shows in every row.
  const controls = page.getByRole('group', { name: 'Preview settings' });
  const sample = controls.getByRole('combobox', { name: 'Sample' });
  await sample.focus();
  await sample.selectOption('custom');
  const text = controls.getByRole('textbox', { name: 'Custom text' });
  await text.focus();
  await page.keyboard.type('Hamburgefonstiv');
  await expect(catalogRows(page).locator('.sample')).toHaveText(Array(4).fill('Hamburgefonstiv'));
  const size = controls.getByRole('spinbutton', { name: /Size/ });
  await size.fill('48');
  await size.press('Enter');
  await expect(lato).toHaveCSS('font-size', '48px');

  // List, Grid and Waterfall, each a labelled toggle reachable by keyboard.
  const modes = page.getByRole('group', { name: 'View mode' });
  const waterfall = modes.getByRole('button', { name: 'Waterfall' });
  await waterfall.focus();
  await page.keyboard.press('Enter');
  await expect(waterfall).toHaveAttribute('aria-pressed', 'true');
  await expect(catalogRows(page).first().locator('.line')).toHaveCount(6);
  await modes.getByRole('button', { name: 'Grid' }).press('Enter');
  await expect(catalogRows(page).first().locator('.item')).toHaveClass(/grid/);

  // The list itself is a labelled Tab stop; Refresh is a labelled button.
  const list = page.getByRole('list', { name: 'Google Fonts families' });
  await list.focus();
  await expect(list).toBeFocused();
  await expect(page.getByRole('button', { name: 'Refresh' })).toBeEnabled();
});

test('offline, a failed refresh keeps the cached list with a plain note', async ({ page }) => {
  await serveFonts(page);
  await page.goto('/?mockCatalog=1');
  await sidebar(page).getByRole('button', { name: 'Google Fonts', exact: true }).click();
  await expect(catalogRows(page)).toHaveCount(4);
  const refresh = page.getByRole('button', { name: 'Refresh' });
  await refresh.focus();
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('catalog-offline')).toHaveText(
    'Offline - showing the list from Oct 1, 2026',
  );
  await expect(catalogRows(page)).toHaveCount(4);
});

test('Settings -> Online catalogs turns Google Fonts on with the same switch', async ({ page }) => {
  await serveFonts(page);
  await page.goto('/#/settings?section=catalogs');
  const box = page.getByRole('checkbox', { name: 'Google Fonts' });
  await expect(box).not.toBeChecked();
  await expect(box).toHaveAccessibleDescription(/connects to the internet/);
  await box.focus();
  await page.keyboard.press('Space');
  await expect(box).toBeChecked();
  await page.goto('/#/');
  // The sidebar sees the same state: on, no "Turn on" button.
  await expect(sidebar(page).getByRole('button', { name: 'Turn on Google Fonts' })).toHaveCount(0);
});

for (const locale of ['en-US', 'de-DE']) {
  test.describe(`narrow window (${locale})`, () => {
    test.use({ locale, viewport: { width: 1100, height: 700 } });

    test('the catalog toolbar wraps so Refresh and its date stay inside the column', async ({
      page,
    }) => {
      await serveFonts(page);
      await page.goto('/?mockCatalog=1');
      await expect(page.locator('#fm-inspector')).toBeVisible();
      await page.getByTestId('catalog-googleFonts').click();
      await expect(page.getByTestId('catalog-list').getByRole('listitem')).toHaveCount(4);
      const column = (await page.locator('main.list').boundingBox())!;
      for (const testId of ['catalog-refresh', 'catalog-refreshed', 'catalog-search']) {
        const box = (await page.getByTestId(testId).boundingBox())!;
        expect(box.x, testId).toBeGreaterThanOrEqual(column.x);
        expect(box.x + box.width, testId).toBeLessThanOrEqual(column.x + column.width);
      }
      const refresh = page.getByTestId('catalog-refresh');
      await refresh.focus();
      await expect(refresh).toBeInViewport({ ratio: 1 });
    });
  });
}
