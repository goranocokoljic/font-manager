import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { optionLabels, selectOption } from './select';

// E54.6: the pangram's language in the catalog views. The catalog view keeps its own
// Language choice apart from the library's; every card and row shows that pangram; a family
// whose preview file lacks letters of it shows them as tofu (E43.4), as library rows do.
// MockApi: Lora's preview maps Latin and Latin-1 only, so Serbian (Latin)'s č ć đ š ž are
// missing; Inter, Lato and Space Mono map Latin Extended-A too.

const font = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);
const SERBIAN = 'Ljubazni fenjerdžija čađavog lica hoće da mi pokaže štos';
const ENGLISH = 'The quick brown fox jumps over the lazy dog';

async function open(page: Page) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: font }),
  );
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto('/?mockCatalog=1');
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
}

async function showGoogleFonts(page: Page) {
  await page.getByTestId('catalog-googleFonts').click();
  await expect(page.getByTestId('catalog-list').getByRole('listitem')).toHaveCount(4);
}

const language = (page: Page) =>
  page.getByRole('group', { name: 'Preview settings' }).getByLabel('Language');
const card = (page: Page, family: string) =>
  page.locator(`[data-testid="catalog-family"][data-family="${family}"]`);
const sampleOf = (page: Page, family: string) => card(page, family).locator('.sample').first();
/** A sample's text with each tofu box read as the character it stands for. */
const readText = (page: Page, family: string) =>
  sampleOf(page, family).evaluate((el) =>
    [...el.childNodes]
      .map((n) =>
        n instanceof HTMLElement && n.classList.contains('fm-tofu')
          ? (n.dataset.char ?? '')
          : (n.textContent ?? ''),
      )
      .join(''),
  );

test('choosing Serbian (Latin) in a catalog shows its pangram, with tofu where a font lacks letters', async ({
  page,
}) => {
  await open(page);
  await showGoogleFonts(page);

  // A language without a pangram is listed, marked and cannot be chosen, as in the library.
  const names = await optionLabels(language(page));
  expect(names).toContain('Serbian (Latin)');
  await language(page).click();
  await expect(page.getByRole('option', { name: 'Maltese', exact: true })).toHaveAttribute(
    'aria-disabled',
    'true',
  );
  await language(page).press('Escape');

  await selectOption(language(page), { label: 'Serbian (Latin)' });
  await expect(language(page)).toHaveAttribute('data-value', 'sr-Latn');

  // Every row shows the pangram (a tofu box stands for its character); the fonts that map
  // it show it whole.
  for (const family of ['Inter', 'Lato', 'Space Mono']) {
    await expect(sampleOf(page, family)).toHaveText(SERBIAN);
  }
  await expect(sampleOf(page, 'Lora').locator('.fm-tofu').first()).toBeVisible();
  expect(await readText(page, 'Lora')).toBe(SERBIAN);
  for (const family of ['Inter', 'Lato', 'Space Mono']) {
    await expect(card(page, family).locator('.fm-tofu')).toHaveCount(0);
  }
  // Lora's file lacks č ć đ š ž: each is a box with its name, never a fallback font's glyph.
  const lora = sampleOf(page, 'Lora');
  await expect(lora.locator('.fm-tofu')).toHaveCount(6);
  const boxes = await lora
    .locator('.fm-tofu')
    .evaluateAll((els) => [...new Set(els.map((e) => e.getAttribute('data-char')))]);
  expect(boxes).toEqual(['ž', 'č', 'đ', 'ć', 'š']);
  await expect(lora.getByRole('img', { name: 'missing glyph đ' }).first()).toBeVisible();
  await expect(card(page, 'Lora').getByTestId('missing-glyphs')).toHaveText(/5 glyphs missing/);
  await expect(card(page, 'Lora').getByTestId('fallback-hint')).toHaveCount(0);

  // The cards of the grid show the start of the pangram, with the same tofu.
  await page
    .locator('header.toolbar')
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Grid', exact: true })
    .click();
  await expect(sampleOf(page, 'Inter')).toHaveText(/^Ljubazni fenjerdžija/);
  await expect(sampleOf(page, 'Lora').locator('.fm-tofu[data-char="ž"]')).toHaveCount(1);
  await expect(card(page, 'Inter').locator('.fm-tofu')).toHaveCount(0);

  // Remembered for the catalog view, apart from the library's.
  await page
    .getByRole('navigation', { name: 'Library' })
    .getByRole('button', { name: 'All fonts' })
    .click();
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
  await expect(language(page)).toHaveAttribute('data-value', 'en');
  await expect(page.locator('[data-face="1"] .sample')).toHaveText(ENGLISH);

  // Fontshare is a catalog view too: the same choice.
  await page.getByTestId('catalog-fontshare').click();
  await expect(language(page)).toHaveAttribute('data-value', 'sr-Latn');

  await page.reload();
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
  await expect(language(page)).toHaveAttribute('data-value', 'en');
  await showGoogleFonts(page);
  await expect(language(page)).toHaveAttribute('data-value', 'sr-Latn');
  await expect(sampleOf(page, 'Lato')).toHaveText(/fenjerdžija/);
});
