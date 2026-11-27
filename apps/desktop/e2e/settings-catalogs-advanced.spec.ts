// E57.5: Settings › Online catalogs and Advanced as on board 15 — catalog cards with switches,
// the folder for installed fonts with its tree, number rows with their units, the logs folder
// and run setup again; every control by keyboard alone.
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light' });
});

const card = (page: Page, id: string) => page.locator(`[data-catalog="${id}"]`).first();

test('Online catalogs works by keyboard: the switches, How it works, Change… and fetch-missing', async ({
  page,
}) => {
  await page.goto('/#/settings?section=catalogs');
  await expect(page.getByRole('heading', { name: 'Catalogs', level: 3 })).toBeVisible();
  const google = page.getByRole('switch', { name: 'Google Fonts' });
  await expect(google).toHaveAccessibleDescription(
    'Turning it on connects to the internet to fetch its list of fonts.',
  );
  await expect(card(page, 'googleFonts').getByTestId('catalog-card-count')).toHaveText('off');
  // The cards are drawn like source cards: the mark is the name's first letter.
  await expect(card(page, 'googleFonts').locator('.mark')).toHaveText('G');

  await page.getByRole('heading', { name: 'Online catalogs', level: 2 }).focus();
  await page.keyboard.press('Tab');
  await expect(google).toBeFocused();
  await page.keyboard.press('Space');
  await expect(google).toHaveAttribute('aria-checked', 'true');
  await expect(card(page, 'googleFonts').getByTestId('catalog-card-count')).toHaveText(
    '4 families',
  );
  await expect(google).toHaveAccessibleDescription(
    'Connected. Previews are fetched while you browse it.',
  );
  await page.keyboard.press('Tab');
  await expect(page.getByRole('switch', { name: 'Fontshare' })).toBeFocused();

  // Adobe Fonts: no switch; "How it works" opens the existing note.
  await expect(card(page, 'adobeFonts').getByRole('switch')).toHaveCount(0);
  await page.keyboard.press('Tab');
  const how = page.getByRole('button', { name: 'How it works' });
  await expect(how).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(how).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByRole('region', { name: 'Adobe Fonts' })).toBeVisible();
  await page.keyboard.press('Tab'); // the note's link
  await page.keyboard.press('Tab');

  // The folder for installed fonts: not chosen yet, then Change… picks one.
  const change = page.getByRole('button', { name: 'Change…' });
  await expect(change).toBeFocused();
  await expect(change).toHaveAccessibleDescription(/^Folder for installed fonts Install puts/);
  const row = page.getByTestId('settings-download-folder');
  await expect(row).toContainText('Not chosen yet. The first Install asks for it.');
  await page.keyboard.press('Enter');
  const path = page.getByTestId('settings-download-folder-path');
  await expect(path).toBeVisible();
  const folder = (await path.textContent()) ?? '';
  await expect(path).toHaveCSS('font-family', /mono|Menlo|Consolas/i);
  // The tree: the folder, then one line per catalog that is on (Google Fonts only).
  await expect(page.getByTestId('settings-catalog-paths').getByRole('listitem')).toHaveText([
    folder,
    /^Google Fonts[\\/]<Family>$/,
  ]);

  // Download fonts a document needs: now possible, by keyboard.
  await page.keyboard.press('Tab');
  const fetch = page.getByRole('switch', { name: 'Download fonts a document needs' });
  await expect(fetch).toBeFocused();
  await expect(fetch).toBeEnabled();
  await page.keyboard.press('Space');
  await expect(fetch).toHaveAttribute('aria-checked', 'true');
});

test('Advanced works by keyboard: number rows with units, Copy path, Run setup again', async ({
  page,
  context,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.goto('/#/settings?section=advanced');
  await expect(page.getByRole('heading', { name: 'Performance', level: 3 })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Troubleshooting', level: 3 })).toBeVisible();

  const cache = page.getByRole('textbox', { name: 'Preview cache size (MB)' });
  await expect(cache).toHaveAccessibleDescription('Thumbnails and previews kept on disk.');
  await page.getByRole('heading', { name: 'Advanced', level: 2 }).focus();
  // The − and + buttons take no Tab stop: the field is the first.
  await page.keyboard.press('Tab');
  await expect(cache).toBeFocused();
  await cache.fill('2048');
  await page.keyboard.press('Enter');
  await page.reload();
  await expect(page.getByRole('textbox', { name: 'Preview cache size (MB)' })).toHaveValue('2048');
  await expect(page.locator('.unit')).toHaveText(['MB', 'minutes']);

  // The logs folder: its path in the mono face and "Copy path"; no "Open" button.
  const logs = page.getByTestId('settings-logs-folder');
  await expect(logs.locator('.value')).toHaveText(/font-manager\/logs$/);
  await expect(logs.getByRole('button')).toHaveText(['Copy path']);
  await logs.getByRole('button', { name: /^Copy path of the logs folder: / }).focus();
  await page.keyboard.press('Enter');
  await expect(logs.getByRole('status')).toHaveText('Copied');

  await page.keyboard.press('Tab');
  const runSetup = page.getByRole('button', { name: 'Run setup again' });
  await expect(runSetup).toBeFocused();
  await expect(runSetup).toHaveAccessibleDescription(
    'Opens every step with your current choices. A step you leave as it is changes nothing.',
  );
});

for (const scheme of ['light', 'dark'] as const) {
  test(`Online catalogs with a catalog on, a folder and the note passes axe (${scheme})`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto('/#/settings?section=catalogs');
    await page.getByRole('switch', { name: 'Google Fonts' }).click();
    await page.getByTestId('settings-download-folder-change').click();
    await page.getByRole('button', { name: 'How it works' }).click();
    await expect(page.getByTestId('settings-catalog-paths')).toBeVisible();
    const results = await new AxeBuilder({ page }).include('[role="tabpanel"]').analyze();
    expect(results.violations.map((v) => v.id)).toEqual([]);
  });
}
