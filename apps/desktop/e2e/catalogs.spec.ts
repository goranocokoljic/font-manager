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
