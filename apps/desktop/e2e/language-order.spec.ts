import { expect, test, type Page } from '@playwright/test';
import { openFilters } from './filters';
import { optionLabels, selectOption, showPreviewOptions } from './select';

// E58.3: every list of languages shows the order `list_languages` answers (ADR 0028). On the
// browser mock with `?mockLanguages=more` (16 languages, more than the drawer's ten), the
// user sets two languages; they lead the filter drawer and the sample menu, and a chosen
// language past the first ten stays in the drawer when it folds again. The mock keeps its
// settings in sessionStorage, so they survive the page loads.

const MORE = '/?mockLanguages=more';

/** Sets the user's languages under Settings › Languages (E58.2), in this order. */
async function setMyLanguages(page: Page, names: string[]): Promise<void> {
  await page.goto(`${MORE}#/settings?section=languages`);
  for (const name of names) {
    const picker = page.getByRole('combobox', { name: 'Add a language' });
    await picker.click();
    await page.keyboard.type(name.slice(0, 4));
    await page.getByRole('option', { name, exact: true }).click();
  }
  await expect(page.getByTestId('my-languages').locator('.name')).toHaveText(names);
}

const languageChips = (page: Page) =>
  page
    .getByRole('dialog', { name: 'Filter options' })
    .getByRole('group', { name: 'Languages' })
    .locator('button.chip:not(.more)');

test('two languages of mine lead the filter drawer and the sample menu', async ({ page }) => {
  await setMyLanguages(page, ['Polish', 'Greek']);
  await page.goto(`${MORE}#/`);
  await expect(page.locator('[data-face="1"]')).toBeVisible();

  // The filter drawer: the first ten of the answer, the user's two first.
  await openFilters(page);
  const chips = languageChips(page);
  await expect(chips).toHaveCount(10);
  await expect(chips.nth(0)).toHaveText('Polish');
  await expect(chips.nth(1)).toHaveText('Greek');
  await expect(chips.filter({ hasText: 'Turkish' })).toHaveCount(0);

  // A language past the first ten, chosen, stays when the list folds again.
  const group = page.getByRole('dialog', { name: 'Filter options' }).getByRole('group', {
    name: 'Languages',
  });
  await group.getByRole('button', { name: '6 more…' }).click();
  await expect(chips).toHaveCount(16);
  await chips.filter({ hasText: 'Turkish' }).click();
  await expect(chips.filter({ hasText: 'Turkish' })).toHaveAttribute('aria-pressed', 'true');
  await group.getByRole('button', { name: 'Fewer' }).click();
  await expect(chips).toHaveCount(11);
  await expect(chips.nth(10)).toHaveText('Turkish');
  await expect(chips.nth(0)).toHaveText('Polish');
  await page.keyboard.press('Escape');

  // The sample menu: the same order, with a hairline after the user's languages.
  await showPreviewOptions(page);
  const controls = page.getByRole('group', { name: 'Preview settings' });
  await selectOption(controls.getByLabel('Sample'), 'pangram');
  const language = controls.getByLabel('Language');
  const names = await optionLabels(language);
  expect(names.slice(0, 2)).toEqual(['Polish', 'Greek']);
  expect(names).toHaveLength(16);
  await language.click();
  const breaks = page.locator('.ui-popover [role="option"].break');
  await expect(breaks).toHaveCount(1);
  await expect(breaks).toHaveText(names[2]!);
});

test.describe('@screens', () => {
  const dir = process.env.FM_SCREENS_DIR;
  test.skip(!dir, 'FM_SCREENS_DIR is not set');
  for (const [width, height] of [
    [1280, 800],
    [1000, 700],
  ] as const) {
    test(`my languages lead the drawer and the sample menu at ${width}x${height}`, async ({
      page,
    }) => {
      await page.setViewportSize({ width, height });
      await setMyLanguages(page, ['Polish', 'Greek']);
      await page.goto(`${MORE}#/`);
      await expect(page.locator('[data-face="1"]')).toBeVisible();
      await openFilters(page);
      await page.waitForTimeout(400);
      await page.screenshot({ path: `${dir}/language-drawer-${width}x${height}.png` });
      await page.keyboard.press('Escape');
      await showPreviewOptions(page);
      const controls = page.getByRole('group', { name: 'Preview settings' });
      await selectOption(controls.getByLabel('Sample'), 'pangram');
      await controls.getByLabel('Language').click();
      await expect(page.locator('.ui-popover [role="option"].break')).toHaveCount(1);
      await page.waitForTimeout(400);
      await page.screenshot({ path: `${dir}/language-menu-${width}x${height}.png` });
    });
  }
});
