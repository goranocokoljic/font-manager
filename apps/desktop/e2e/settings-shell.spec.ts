// E57.3: the Settings shell of board 15 — the navigation by keyboard, the section kept in the
// URL over a reload, and the x icon button in the panel's top corner.
import { expect, test, type Page } from '@playwright/test';

const tab = (page: Page, name: string) => page.getByRole('tab', { name });

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light' });
});

test('the navigation works by keyboard, and a new section focuses its heading', async ({
  page,
}) => {
  await page.goto('/#/settings');
  await expect(tab(page, 'Sources')).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(tab(page, 'Activation')).toBeFocused();
  await expect(tab(page, 'Activation')).toHaveAttribute('aria-selected', 'true');
  await expect(page).toHaveURL(/section=activation/);
  await page.keyboard.press('End');
  await expect(tab(page, 'About')).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(tab(page, 'Sources')).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await expect(tab(page, 'About')).toBeFocused();
  // Enter on a tab, or a click, moves focus to the section's heading.
  await page.keyboard.press('Home');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await expect(tab(page, 'Appearance')).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Appearance', level: 2 })).toBeFocused();
  await tab(page, 'Advanced').click();
  await expect(page.getByRole('heading', { name: 'Advanced', level: 2 })).toBeFocused();
  await expect(page.getByRole('tabpanel', { name: 'Advanced' })).toBeVisible();
});

test('the open section is in the URL and survives a reload', async ({ page }) => {
  await page.goto('/#/settings');
  await tab(page, 'Local API').click();
  await expect(page).toHaveURL(/#\/settings\?section=localApi/);
  await page.reload();
  await expect(tab(page, 'Local API')).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('heading', { name: 'Local API', level: 2 })).toBeVisible();
});

test('the x in the panel corner closes Settings, Esc too; no top bar is left', async ({ page }) => {
  await page.goto('/#/settings?section=activation');
  await expect(page.locator('[data-top-bar]')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Close', exact: true })).toHaveCount(0);
  const close = page.getByRole('button', { name: 'Close settings' });
  // In the panel's top corner, at the end of the line: right of the panel's content.
  const box = (await close.boundingBox())!;
  const nav = (await page.getByRole('navigation', { name: 'Settings' }).boundingBox())!;
  const width = page.viewportSize()!.width;
  expect(box.x).toBeGreaterThan(nav.x + nav.width);
  expect(width - (box.x + box.width)).toBeLessThan(32);
  expect(box.y).toBeLessThan(32);
  // Its tooltip names it and says Esc.
  await close.hover();
  await expect(page.getByRole('tooltip')).toContainText('Close settings');
  await expect(page.getByRole('tooltip')).toContainText('Esc');
  await close.click();
  await expect(page).not.toHaveURL(/settings/);

  await page.goto('/#/settings?section=about');
  await expect(tab(page, 'About')).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(page).not.toHaveURL(/settings/);

  // Esc still closes after a click on plain text in the panel (focus is then on the page).
  await page.goto('/#/settings?section=about');
  await page.getByTestId('app-info').click();
  await expect(tab(page, 'About')).not.toBeFocused();
  await page.keyboard.press('Escape');
  await expect(page).not.toHaveURL(/settings/);
});

test('the shell: a 232 px navigation, the panel at most 760 px wide (Sources 920)', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto('/#/settings?section=activation');
  const nav = (await page.getByRole('navigation', { name: 'Settings' }).boundingBox())!;
  expect(nav.width).toBe(232);
  const heading = page.getByRole('heading', { name: 'Activation', level: 2 });
  const section = heading.locator('xpath=..');
  expect((await section.boundingBox())!.width).toBeLessThanOrEqual(760);
  await tab(page, 'Sources').click();
  const sources = page
    .getByRole('heading', { name: 'Sources', level: 2 })
    .locator('xpath=../../..');
  expect((await sources.boundingBox())!.width).toBeLessThanOrEqual(920);
  expect((await sources.boundingBox())!.width).toBeGreaterThan(760);
});
