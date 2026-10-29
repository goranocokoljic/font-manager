import { expect, test, type Page } from '@playwright/test';

// E32.2 on the MockApi: the picker is the mock in `src/api/` ("Brand brochure.indd",
// `?mockDocument=1` a text file, `=2` a damaged document); `?mockActive=6` starts with
// JetBrains Mono active. The document names Futura (missing), Inter and JetBrains Mono.
const panel = (page: Page) => page.getByRole('region', { name: 'Document fonts' });
const row = (page: Page, name: string) =>
  panel(page).getByRole('button', { name: new RegExp(`^${name}: `) });

async function checkDocument(page: Page): Promise<void> {
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type("Check a document's fonts");
  await page.keyboard.press('Enter');
}

test.beforeEach(async ({ page }) => {
  await page.goto('/');
  await page.evaluate(() => localStorage.clear());
});

test('the palette opens a document and lists active, in-library and missing fonts; Activate turns the library rows Active', async ({
  page,
}) => {
  await page.goto('/?mockActive=6#/');
  await expect(page.getByTestId('active-count')).toHaveText('1 font active');
  await checkDocument(page);

  await expect(panel(page)).toBeVisible();
  await expect(panel(page)).toContainText('Brand brochure.indd');
  await expect(row(page, 'Futura-Medium')).toHaveAccessibleName('Futura-Medium: Missing');
  await expect(row(page, 'Inter-Regular')).toHaveAccessibleName('Inter-Regular: In library');
  await expect(row(page, 'JetBrainsMono-Regular')).toHaveAccessibleName(
    'JetBrainsMono-Regular: Active',
  );
  await expect(panel(page)).toContainText('Font Manager does not download them');
  // Not a modal: the font list stays in the page and usable.
  await expect(page.getByRole('dialog')).toHaveCount(0);

  // Keyboard: the first row has the focus; Down, then Tab to the Activate button.
  await expect(row(page, 'Futura-Medium')).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(row(page, 'Inter-Regular')).toBeFocused();
  await page.keyboard.press('End');
  await expect(row(page, 'JetBrainsMono-Regular')).toBeFocused();
  await page.keyboard.press('Tab');
  const activate = panel(page).getByRole('button', { name: 'Activate 1 font' });
  await expect(activate).toBeFocused();
  await page.keyboard.press('Enter');

  await expect(row(page, 'Inter-Regular')).toHaveAccessibleName('Inter-Regular: Active');
  await expect(row(page, 'Futura-Medium')).toHaveAccessibleName('Futura-Medium: Missing');
  await expect(activate).toBeHidden();
  await expect(panel(page)).toContainText('library has is active');
  await expect(page.getByTestId('active-count')).toHaveText('2 fonts active');
  await expect(page.getByTestId('toast')).toContainText('Activated 1 font for this session');

  // Escape closes the panel.
  await row(page, 'Inter-Regular').focus();
  await page.keyboard.press('Escape');
  await expect(panel(page)).toBeHidden();
});

test('the close button is labelled and closes the panel', async ({ page }) => {
  await page.goto('/#/');
  await checkDocument(page);
  await expect(panel(page)).toBeVisible();
  await panel(page).getByRole('button', { name: 'Close the document fonts' }).click();
  await expect(panel(page)).toBeHidden();
});

for (const [n, reason] of [
  ['1', 'Font Manager reads the fonts of .indd, .ai, .psd, .sketch, .fig, .docx and .pptx'],
  ['2', "This document's fonts could not be read."],
] as const)
  test(`a file that cannot be checked (${n}) is a toast with the reason and no panel`, async ({
    page,
  }) => {
    await page.goto(`/?mockDocument=${n}#/`);
    await checkDocument(page);
    await expect(page.getByTestId('toast')).toContainText(reason);
    await expect(panel(page)).toHaveCount(0);
  });
