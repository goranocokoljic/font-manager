import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';

// Comparing two copies (E56.13) on the browser mock with `?mockDuplicates=1` and
// `?mockCopyCompare=1`: Inter as the sample's file (Inter/Inter[wght].ttf), an older
// file with other metrics and no `locl` (Old/), Version 0.9 without Cyrillic (Older/)
// and the same version as a WOFF2 (Web/); Legacy Grotesk twice, the second copy
// unreadable. The user works in English and Serbian (Cyrillic).

const MY_LANGUAGES = ['en', 'sr-Cyrl'];

const open = async (page: Page) => {
  await page.addInitScript((languages) => {
    sessionStorage.setItem('fm.mock.settings.v1', JSON.stringify({ myLanguages: languages }));
  }, MY_LANGUAGES);
  await page.goto('/?mockDuplicates=1&mockCopyCompare=1#/duplicates?section=sets');
  await expect(setPanel(page).getByRole('heading', { level: 2 })).toHaveText('Inter-Regular');
};
const nav = (page: Page) => page.getByRole('navigation', { name: 'Duplicates' });
const setPanel = (page: Page) => page.getByTestId('duplicate-set');
const comparison = (page: Page) => page.getByTestId('copy-comparison');
const half = (page: Page, side: 'a' | 'b') =>
  comparison(page).locator(`[data-testid="compare-half"][data-side="${side}"]`);

/** Picks the copy whose option name matches `file` as the first or the second copy. */
async function pick(page: Page, which: 'First copy' | 'Second copy', file: RegExp) {
  await comparison(page).getByRole('combobox', { name: which }).click();
  await page.getByRole('option', { name: file }).click();
}

/** Opens the comparison of the sample's Inter with the copy whose option matches `file`. */
async function compareWith(page: Page, file: RegExp) {
  await setPanel(page)
    .getByRole('button', { name: /^Compare two copies/ })
    .click();
  await expect(comparison(page)).toBeVisible();
  await pick(page, 'First copy', /Inter\/Inter\[wght\]\.ttf/);
  await pick(page, 'Second copy', file);
  await expect(half(page, 'b')).toHaveAttribute('aria-label', expect.stringMatching(file));
}

test('a language one copy lacks comes first, its glyphs are drawn and the sample shows it', async ({
  page,
}) => {
  await open(page);
  await compareWith(page, /Older\/Inter-Regular\.ttf/);
  const c = comparison(page);
  await expect(c.getByRole('heading', { level: 3 })).toHaveText(
    'Compare Inter/Inter[wght].ttf with Older/Inter-Regular.ttf',
  );
  // Each half: a specimen and what it covers of my languages.
  await expect(half(page, 'a').getByTestId('compare-specimen')).toBeVisible();
  await expect(half(page, 'a').locator('[data-language="sr-Cyrl"]')).toHaveClass(/has/);
  await expect(half(page, 'b').locator('[data-language="sr-Cyrl"]')).toHaveClass(/lacks/);
  await expect(half(page, 'b').locator('[data-language="sr-Cyrl"]')).toContainText(
    'Serbian (Cyrillic) missing',
  );
  await expect(half(page, 'b').locator('[data-language="en"]')).toHaveClass(/has/);
  // The languages come before the glyphs, and say who loses what.
  const languages = c.getByTestId('compare-languages');
  await expect(languages.locator('[data-kind="lost"]')).toHaveText(
    'Only Inter/Inter[wght].ttf covers Serbian (Cyrillic) in full.',
  );
  const order = await c
    .getByTestId('compare-differences')
    .locator(':scope > [data-testid]')
    .evaluateAll((els) => els.map((e) => e.getAttribute('data-testid')));
  expect(order.indexOf('compare-languages')).toBeLessThan(order.indexOf('compare-glyphs'));
  // The glyphs the second copy lacks are shown as glyphs, in the first copy's face.
  const removed = c.getByTestId('compare-glyphs').locator('[data-kind="removed"]');
  await expect(removed).toContainText('30 glyphs only in Inter/Inter[wght].ttf:');
  await expect(removed.getByRole('img', { name: 'а (U+0430)' })).toHaveText('а');
  await expect(removed).toContainText('+20 more');
  // Neither a metrics change nor a container-only answer for this pair.
  await expect(c.getByTestId('compare-reflow')).toHaveCount(0);
  await expect(c.getByTestId('compare-container-only')).toHaveCount(0);

  // The sample switches to the language's sample: the copy without Cyrillic shows tofu.
  const samples = c.getByTestId('compare-samples');
  await expect(samples.getByRole('button', { name: 'Your sample' })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await samples.getByRole('button', { name: 'Serbian (Cyrillic)' }).click();
  await expect(samples.getByRole('button', { name: 'Serbian (Cyrillic)' })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  const specimenA = half(page, 'a').getByTestId('compare-specimen');
  const specimenB = half(page, 'b').getByTestId('compare-specimen');
  await expect(specimenA).toHaveAttribute('lang', 'sr-Cyrl');
  await expect(specimenA).toContainText('Љубазни');
  await expect(specimenB.getByRole('img', { name: /missing glyph/ }).first()).toBeVisible();
  await expect(specimenA.getByRole('img', { name: /missing glyph/ })).toHaveCount(0);

  // "Keep this one" acts on the draft as in the queue: the others leave.
  await half(page, 'a')
    .getByRole('button', { name: 'Keep this one: Inter/Inter[wght].ttf' })
    .click();
  await expect(half(page, 'a')).toHaveAttribute('data-choice', 'keep');
  await expect(half(page, 'b')).toHaveAttribute('data-choice', 'trash');
  await expect(half(page, 'b').getByTestId('compare-choice')).toHaveText('In the plan: Trash');
});

test('changed metrics say the text may reflow, and locl on one side only is named', async ({
  page,
}) => {
  await open(page);
  await compareWith(page, /Old\/Inter-Regular\.ttf/);
  const c = comparison(page);
  const metrics = c.getByTestId('compare-metrics');
  await expect(metrics).toContainText('The vertical metrics differ: the line spacing changes.');
  await expect(metrics).toContainText('14 glyphs have another width.');
  await expect(c.getByTestId('compare-reflow')).toHaveText(
    'Text set in one copy may reflow in the other.',
  );
  // The same languages, but only the first copy has locl forms for Serbian (Cyrillic).
  await expect(c.getByTestId('compare-languages').locator('[data-kind="loclLost"]')).toHaveText(
    'Only Inter/Inter[wght].ttf has localized forms (locl) for Serbian (Cyrillic).',
  );
  await expect(c.getByTestId('compare-languages').locator('[data-kind="lost"]')).toHaveCount(0);
  await expect(half(page, 'a').locator('[data-language="sr-Cyrl"]')).toContainText('locl');
  await expect(half(page, 'b').locator('[data-language="sr-Cyrl"]')).not.toContainText('locl');
  await expect(c.getByTestId('compare-glyphs')).toHaveCount(0);

  // Hide and Keep both act on the same plan as the queue.
  await half(page, 'b').getByRole('button', { name: 'Hide: Old/Inter-Regular.ttf' }).click();
  await expect(half(page, 'b')).toHaveAttribute('data-choice', 'hide');
  await expect(nav(page).getByTestId('dup-count-plan')).toHaveText('0');
  await c.getByRole('button', { name: /^Keep both/ }).click();
  await expect(nav(page).getByTestId('dup-count-plan')).toHaveText('1');
  await expect(setPanel(page).getByRole('heading', { level: 2 })).toHaveText('LegacyGrotesk');
  await expect(comparison(page)).toHaveCount(0);
});

test('the same font in another format differs only in the container', async ({ page }) => {
  await open(page);
  await compareWith(page, /Web\/Inter-Regular\.woff2/);
  const c = comparison(page);
  await expect(c.getByTestId('compare-container-only')).toHaveText(
    'Only the container differs: the TrueType (TTF) and the WOFF2 file hold the same font.',
  );
  await expect(c.getByTestId('compare-languages')).toHaveCount(0);
  await expect(c.getByTestId('compare-glyphs')).toHaveCount(0);
  await expect(c.getByTestId('compare-metrics')).toHaveCount(0);
  await expect(half(page, 'b')).toContainText('WOFF2');
});

test('an unreadable copy shows the API message in its half, and the keyboard does it all', async ({
  page,
}) => {
  await open(page);
  // To the Legacy Grotesk set, then C opens the comparison with the focus on its heading.
  await page.getByRole('listbox', { name: 'Sets to decide' }).getByRole('option').nth(1).click();
  await expect(setPanel(page).getByRole('heading', { level: 2 })).toHaveText('LegacyGrotesk');
  await page.keyboard.press('c');
  const c = comparison(page);
  await expect(c.getByRole('heading', { level: 3 })).toBeFocused();
  const broken = c.getByTestId('compare-broken');
  await expect(broken).toHaveCount(1);
  await expect(broken).toContainText('This copy could not be read.');
  await expect(broken).toContainText(
    "The file is truncated: the 'glyf' table ends before its declared length.",
  );
  await expect(c.getByTestId('compare-not-compared')).toBeVisible();
  await expect(c.getByTestId('compare-specimen')).toHaveCount(1);

  // Escape closes the comparison and gives the focus back; the module stays open.
  await page.keyboard.press('Escape');
  await expect(comparison(page)).toHaveCount(0);
  await expect(setPanel(page).getByRole('button', { name: /^Compare two copies/ })).toBeFocused();
  await expect(page).toHaveURL(/#\/duplicates/);

  // Every control is reached by Tab; N answers "not duplicates" from the comparison.
  await page.keyboard.press('c');
  await expect(c.getByRole('heading', { level: 3 })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(c.getByRole('combobox', { name: 'First copy' })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(c.getByRole('combobox', { name: 'Second copy' })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(c.getByRole('button', { name: 'Close the comparison' })).toBeFocused();
  await page.keyboard.press('n');
  await expect(nav(page).getByTestId('dup-count-plan')).toHaveText('1');
  // Legacy Grotesk left the queue: Inter is the one set still to decide.
  await expect(page.getByRole('heading', { name: '1 set to decide' })).toBeVisible();
  await expect(setPanel(page).getByRole('heading', { level: 2 })).toHaveText('Inter-Regular');
});

test('the comparison reads in German', async ({ page }) => {
  await page.addInitScript(() => {
    sessionStorage.setItem(
      'fm.mock.settings.v1',
      JSON.stringify({ myLanguages: ['en', 'sr-Cyrl'], language: 'de' }),
    );
  });
  await page.goto('/?mockDuplicates=1&mockCopyCompare=1#/duplicates?section=sets');
  await expect(setPanel(page).getByRole('heading', { level: 2 })).toHaveText('Inter-Regular');
  await setPanel(page)
    .getByRole('button', { name: /^Zwei Kopien vergleichen/ })
    .click();
  const c = comparison(page);
  await expect(c.getByRole('button', { name: 'Vergleich schließen' })).toBeVisible();
  await expect(c.getByRole('button', { name: /^Beide behalten/ })).toBeVisible();
  await expect(
    c.getByTestId('compare-samples').getByRole('button', { name: 'Ihr Muster' }),
  ).toBeVisible();
});

for (const colorScheme of ['light', 'dark'] as const) {
  test(`the comparison passes a WCAG 2.2 AA scan (${colorScheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme, reducedMotion: 'reduce' });
    await open(page);
    await compareWith(page, /Older\/Inter-Regular\.ttf/);
    await comparison(page)
      .getByTestId('compare-samples')
      .getByRole('button', { name: 'Serbian (Cyrillic)' })
      .click();
    const result = await new AxeBuilder({ page })
      .include('[data-testid="copy-comparison"]')
      .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
      .analyze();
    expect(result.violations.map((v) => `${v.id}: ${v.nodes[0]?.target}`)).toEqual([]);
  });
}
