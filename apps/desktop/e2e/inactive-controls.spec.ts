import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// E51.4: the size slider and the Language picker are off (dimmed, aria-disabled, never
// hidden) wherever they have no effect, and their tooltip says why.

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto('/#/');
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
}

const slider = (page: Page) => page.locator('header.toolbar').getByLabel('Size slider');
const controls = (page: Page) => page.getByRole('group', { name: 'Preview settings' });
const language = (page: Page) => controls(page).getByRole('combobox', { name: 'Language' });
const chip = (page: Page, name: string) =>
  controls(page).getByRole('group', { name: 'Preset' }).getByRole('button', { name });

async function setMode(page: Page, name: string) {
  const button = page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name, exact: true });
  await button.click();
  await expect(button).toHaveAttribute('aria-pressed', 'true');
}

const opacity = (page: Page, testId: string) =>
  page
    .getByTestId(testId)
    .first()
    .evaluate((el) => getComputedStyle(el).opacity);

test('the size slider is off in Waterfall and Table, on in List and Grid', async ({ page }) => {
  await open(page);
  for (const mode of ['List', 'Grid']) {
    await setMode(page, mode);
    await expect(slider(page)).toBeVisible();
    await expect(slider(page)).not.toHaveAttribute('aria-disabled', 'true');
    expect(await opacity(page, 'size-slider')).toBe('1');
  }
  await setMode(page, 'List');
  const before = await slider(page).inputValue();
  for (const [mode, why] of [
    ['Waterfall', 'Size is fixed in Waterfall — switch to List or Grid to change it'],
    ['Table', "Size is set by the strip in Table — use the strip's size slider"],
  ] as const) {
    await setMode(page, mode);
    // Never hidden: the toolbar keeps its shape (E50.2).
    await expect(slider(page)).toBeVisible();
    await expect(slider(page)).toHaveAttribute('aria-disabled', 'true');
    await expect(slider(page)).toHaveAttribute('title', why);
    await expect(slider(page)).toHaveAccessibleDescription(why);
    expect(await opacity(page, 'size-slider')).toBe('0.4');
    // A press neither moves it nor focuses it.
    await slider(page).click({ force: true });
    await expect(slider(page)).not.toBeFocused();
    await expect(slider(page)).toHaveValue(before);
  }
  // Reachable by keyboard, and the keys do not move it.
  await slider(page).focus();
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('End');
  await expect(slider(page)).toHaveValue(before);
  // Back in List the stored size applies again; nothing was reset.
  await setMode(page, 'List');
  await expect(slider(page)).not.toHaveAttribute('aria-disabled', 'true');
  await expect(slider(page)).toHaveValue(before);
});

test('the Language picker is on only for the Pangram preset', async ({ page }) => {
  await open(page);
  await chip(page, 'Pangram').click();
  await expect(language(page)).not.toHaveAttribute('aria-disabled', 'true');
  await language(page).click();
  await page.getByRole('option', { name: 'German', exact: true }).click();
  await expect(language(page)).toHaveAttribute('data-value', 'de');

  for (const preset of ['Word', 'Alphabet', 'Numerals', 'Paragraph', 'Letters']) {
    await chip(page, preset).click();
    await expect(language(page)).toBeVisible();
    await expect(language(page)).toHaveAttribute('aria-disabled', 'true');
    await expect(language(page)).toHaveAttribute(
      'title',
      'Language applies to the Pangram preset — switch to Pangram',
    );
    expect(await opacity(page, 'language-picker')).toBe('0.4');
  }
  // Off, a press opens nothing and the stored language stays.
  await language(page).click({ force: true });
  await expect(language(page)).toHaveAttribute('aria-expanded', 'false');
  await expect(language(page)).not.toBeFocused();
  await language(page).focus();
  await page.keyboard.press('ArrowDown');
  await expect(language(page)).toHaveAttribute('aria-expanded', 'false');
  await expect(language(page)).toHaveAttribute('data-value', 'de');

  await chip(page, 'Pangram').click();
  await expect(language(page)).not.toHaveAttribute('aria-disabled', 'true');
  await expect(language(page)).toHaveAttribute('data-value', 'de');
  expect(await opacity(page, 'language-picker')).toBe('1');
});

test('Paragraph turns the size slider off in List, and the rows take the body size', async ({
  page,
}) => {
  await open(page);
  await chip(page, 'Paragraph').click();
  await expect(slider(page)).toHaveAttribute('aria-disabled', 'true');
  await expect(slider(page)).toHaveAttribute(
    'title',
    'Paragraph is set at the body size — change it in Preview options',
  );
  await expect
    .poll(() =>
      page
        .locator('[data-face="1"] .sample')
        .evaluate((el) => parseFloat(getComputedStyle(el).fontSize)),
    )
    .toBe(17);
  await chip(page, 'Word').click();
  await expect(slider(page)).not.toHaveAttribute('aria-disabled', 'true');
});
