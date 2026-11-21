import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';

// The Duplicates module (E56.11) on the browser mock with `?mockDuplicates=1`: JetBrains
// Mono twice as the same file (an identical set), Inter in three versions (another
// version) and Legacy Grotesk twice in one version (the same version, Type 1).

const open = async (page: Page, section = '', query = '?mockDuplicates=1') => {
  await page.goto(`/${query}#/duplicates${section ? `?section=${section}` : ''}`);
  await expect(page.getByRole('navigation', { name: 'Duplicates' })).toBeVisible();
};
const nav = (page: Page) => page.getByRole('navigation', { name: 'Duplicates' });
const queue = (page: Page) => page.getByRole('listbox', { name: 'Sets to decide' });
const setPanel = (page: Page) => page.getByTestId('duplicate-set');
const copies = (page: Page) => setPanel(page).getByTestId('duplicate-copy');

/** WCAG contrast of two computed `rgb(…)` colors. */
function contrast(a: string, b: string): number {
  const lum = (c: string) => {
    const [r, g, bl] = c
      .match(/[\d.]+/g)!
      .slice(0, 3)
      .map(Number);
    const lin = (v: number) => {
      const x = v / 255;
      return x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
    };
    return 0.2126 * lin(r!) + 0.7152 * lin(g!) + 0.0722 * lin(bl!);
  };
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (hi! + 0.05) / (lo! + 0.05);
}

test('the module is a view of its own with its navigation and the overview', async ({ page }) => {
  await open(page);
  const n = nav(page);
  await expect(n.getByRole('button', { name: 'Library' })).toBeVisible();
  for (const name of ['Overview', 'Folders', 'Sets', 'Plan', 'History', 'Hidden copies'])
    await expect(n.getByRole('button', { name: new RegExp(`^${name}`) })).toBeVisible();
  const width = await n.evaluate((el) => el.getBoundingClientRect().width);
  expect(width).toBe(232);
  await expect(n.getByTestId('dup-count-sets')).toHaveText('2');
  await expect(n.getByTestId('dup-count-plan')).toHaveText('0');
  await expect(n.getByTestId('dup-languages')).toHaveText('None set');
  await expect(n.getByRole('link', { name: 'Change languages and rules' })).toBeVisible();

  const overview = page.getByTestId('duplicates-overview');
  await expect(overview.getByRole('heading', { level: 2 })).toHaveText(
    '3 fonts have more than one copy',
  );
  // The mock plans the E8.1 keeper rule: the JetBrains copy, both older Inter files and
  // the second Legacy Grotesk file leave.
  await expect(overview).toContainText('4 extra files in 4 folders, up to');
  await expect(page.getByTestId('dup-offer-languages')).toBeVisible();
  await expect(page.getByTestId('dup-identical').getByRole('heading')).toHaveText(
    '1 identical copy in 1 folder',
  );
  await expect(overview.getByRole('button', { name: /^Another version/ })).toContainText('2 files');
  await expect(overview.getByRole('button', { name: /^Same version, another file/ })).toContainText(
    '1 file',
  );

  // Back to the library, and Escape does the same.
  await n.getByRole('button', { name: 'Library' }).click();
  await expect(page).toHaveURL(/#\/$/);
  await open(page);
  await page.keyboard.press('Escape');
  await expect(page).toHaveURL(/#\/$/);
});

test('with a language set the offer is gone and the languages stand at the foot', async ({
  page,
}) => {
  await open(page);
  await page
    .getByTestId('dup-offer-languages')
    .getByRole('link', { name: 'Set languages' })
    .click();
  const rules = page.getByTestId('duplicates-rules');
  await rules.getByRole('combobox', { name: 'Add a language' }).click();
  await page.getByRole('option', { name: 'Serbian (Cyrillic)', exact: true }).click();
  await expect(nav(page).getByTestId('dup-languages')).toHaveText('Serbian (Cyrillic)');
  await nav(page).getByRole('button', { name: 'Overview' }).click();
  await expect(
    page.getByTestId('duplicates-overview').getByRole('heading', { level: 2 }),
  ).toBeVisible();
  await expect(page.getByTestId('dup-offer-languages')).toHaveCount(0);
});

test('the identical copies are one decision that opens the plan', async ({ page }) => {
  await open(page);
  await page.getByTestId('dup-identical').getByRole('button', { name: 'Review the plan' }).click();
  const plan = page.getByTestId('duplicates-plan');
  await expect(plan).toContainText('1 identical copy is in the plan.');
  await expect(nav(page).getByTestId('dup-count-plan')).toHaveText('1');
  await expect(nav(page).getByRole('button', { name: /^Plan/ })).toHaveAttribute(
    'aria-current',
    'page',
  );
  // Identical sets are not in the queue.
  await nav(page).getByRole('button', { name: /^Sets/ }).click();
  await expect(queue(page).getByRole('option')).toHaveCount(2);
  await expect(queue(page)).not.toContainText('JetBrainsMono');
});

test('a set shows its copies with the keeper and the API reason', async ({ page }) => {
  await open(page, 'sets');
  await expect(setPanel(page).getByRole('heading', { level: 2 })).toHaveText('Inter-Regular');
  await expect(copies(page)).toHaveCount(3);
  // The mock's keeper is the newest version: the sample's file in /Inter/.
  const keeper = setPanel(page).locator('[data-recommended="true"]');
  await expect(keeper).toHaveCount(1);
  await expect(keeper).toContainText('/Users/demo/Fonts/Inter/Inter[wght].ttf');
  await expect(keeper).toContainText('Stays: it is the copy that is kept.');
  const older = copies(page).filter({ hasText: '/Users/demo/Fonts/Older/Inter-Regular.ttf' });
  await expect(older).toContainText('Version 0.9');
  await expect(older).toContainText('TrueType (TTF)');
  await expect(older.getByRole('button', { name: 'Trash' })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(setPanel(page)).toContainText('2 files to the Trash');
});

test('a decision of each kind goes into the plan and the queue moves on', async ({ page }) => {
  await open(page, 'sets');
  // Pick another keeper with its number: the third copy (Older) is kept, the others go.
  await expect(setPanel(page).getByRole('heading', { level: 2 })).toHaveText('Inter-Regular');
  await queue(page).getByRole('option').first().focus();
  await page.keyboard.press('3');
  const older = copies(page).nth(2);
  await expect(older.getByRole('button', { name: 'Keep' })).toHaveAttribute('aria-pressed', 'true');
  await expect(copies(page).nth(0).getByRole('button', { name: 'Trash' })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  // Hide the first copy instead of trashing it, with the mouse.
  await copies(page).nth(0).getByRole('button', { name: 'Hide' }).click();
  await setPanel(page)
    .getByRole('button', { name: /^Add to plan and go on/ })
    .click();
  // The queue moved on and the focus is on the row that took the set's place.
  await expect(setPanel(page).getByRole('heading', { level: 2 })).toHaveText('LegacyGrotesk');
  await expect(queue(page).getByRole('option')).toHaveCount(1);
  await expect(queue(page).getByRole('option').first()).toBeFocused();
  await expect(nav(page).getByTestId('dup-count-plan')).toHaveText('1');

  // "Not duplicates" by key.
  await page.keyboard.press('n');
  await expect(page.getByRole('heading', { name: 'No set is left to decide.' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '0 sets to decide' })).toBeFocused();

  // The plan holds both; take them back and answer "keep both" and the trash.
  await nav(page).getByRole('button', { name: /^Plan/ }).click();
  const plan = page.getByTestId('duplicates-plan');
  await expect(plan.locator('[data-set="name:Inter-Regular"]')).toContainText('2 copies leave');
  await expect(plan.locator('[data-set="name:LegacyGrotesk"]')).toContainText('Not duplicates');
  await plan.getByRole('button', { name: 'Take back LegacyGrotesk' }).click();
  await nav(page).getByRole('button', { name: /^Sets/ }).click();
  await expect(setPanel(page).getByRole('heading', { level: 2 })).toHaveText('LegacyGrotesk');
  await setPanel(page)
    .getByRole('button', { name: /^Keep all/ })
    .click();
  await nav(page).getByRole('button', { name: /^Plan/ }).click();
  await expect(plan.locator('[data-set="name:LegacyGrotesk"]')).toContainText('Keep all');

  // Nothing moved: the library still lists every copy.
  await nav(page).getByRole('button', { name: 'Overview' }).click();
  await expect(
    page.getByTestId('duplicates-overview').getByRole('heading', { level: 2 }),
  ).toHaveText('3 fonts have more than one copy');

  // The plan survives leaving the module and a reload (the mock keeps it as the core
  // keeps it in the index).
  await nav(page).getByRole('button', { name: 'Library' }).click();
  await page.reload();
  await open(page, 'plan');
  await expect(page.getByTestId('duplicates-plan').locator('[data-set]')).toHaveCount(2);
});

test('the queue filters by level and format and sorts through the API', async ({ page }) => {
  await open(page, 'sets');
  const rows = queue(page).getByRole('option');
  await expect(rows).toHaveCount(2);
  await page
    .getByRole('group', { name: 'Level' })
    .getByRole('button', { name: /^Another version/ })
    .click();
  await expect(rows).toHaveCount(1);
  await expect(rows.first()).toContainText('Inter-Regular');
  await page
    .getByRole('group', { name: 'Level' })
    .getByRole('button', { name: /^Same version/ })
    .click();
  await expect(rows.first()).toContainText('LegacyGrotesk');
  await page.getByRole('group', { name: 'Level' }).getByRole('button', { name: 'All' }).click();
  await expect(rows).toHaveCount(2);

  await page.getByRole('button', { name: 'Filters' }).click();
  await page.getByRole('combobox', { name: 'Format' }).click();
  await page.getByRole('option', { name: /Type 1/ }).click();
  await expect(rows).toHaveCount(1);
  await expect(rows.first()).toContainText('LegacyGrotesk');
  await page.getByRole('combobox', { name: 'Format' }).click();
  await page.getByRole('option', { name: 'Every format' }).click();
  await expect(rows).toHaveCount(2);

  // The mock has no index to recommend from: no set has a recommendation.
  await page.getByRole('checkbox', { name: 'Has a recommendation' }).check();
  await expect(page.getByRole('heading', { name: 'No set matches these filters.' })).toBeVisible();
  await page.getByRole('checkbox', { name: 'Has a recommendation' }).uncheck();
  await expect(rows).toHaveCount(2);

  // By copies: Inter (three) before Legacy Grotesk (two), as by name.
  await page.getByRole('combobox', { name: 'Order' }).click();
  await page.getByRole('option', { name: 'by copies' }).click();
  await expect(rows.first()).toContainText('3 copies');
  await page.getByRole('combobox', { name: 'Order' }).click();
  await page.getByRole('option', { name: 'by space freed' }).click();
  await expect(rows).toHaveCount(2);
});

test('the queue is keyboard-complete', async ({ page }) => {
  await open(page, 'sets');
  await queue(page).getByRole('option').first().focus();
  await page.keyboard.press('ArrowDown');
  await expect(setPanel(page).getByRole('heading', { level: 2 })).toHaveText('LegacyGrotesk');
  await expect(queue(page).getByRole('option').nth(1)).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await expect(setPanel(page).getByRole('heading', { level: 2 })).toHaveText('Inter-Regular');
  // 2 keeps the second copy; H hides the copy chosen by number, K keeps it again.
  await page.keyboard.press('1');
  await page.keyboard.press('h');
  await expect(copies(page).nth(0).getByRole('button', { name: 'Hide' })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await page.keyboard.press('k');
  await expect(copies(page).nth(0).getByRole('button', { name: 'Keep' })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await page.keyboard.press('t');
  await expect(copies(page).nth(0).getByRole('button', { name: 'Trash' })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  // C opens the comparison of two copies.
  await page.keyboard.press('c');
  await expect(page.getByTestId('copy-comparison')).toBeVisible();
  // B keeps all; the focus goes to the row now in the set's place.
  await page.keyboard.press('b');
  await expect(setPanel(page).getByRole('heading', { level: 2 })).toHaveText('LegacyGrotesk');
  await expect(queue(page).getByRole('option').first()).toBeFocused();
  // Enter adds the set with its choices.
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: '0 sets to decide' })).toBeFocused();
  await expect(nav(page).getByTestId('dup-count-plan')).toHaveText('2');
});

for (const colorScheme of ['light', 'dark'] as const) {
  test(`the overview and the queue pass a WCAG 2.2 AA scan (${colorScheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme, reducedMotion: 'reduce' });
    const scan = async () => {
      const result = await new AxeBuilder({ page })
        .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
        .analyze();
      expect(result.violations.map((v) => `${v.id}: ${v.nodes[0]?.target}`)).toEqual([]);
    };
    await open(page);
    await expect(page.getByTestId('dup-identical')).toBeVisible();
    await scan();
    await nav(page).getByRole('button', { name: /^Sets/ }).click();
    await expect(copies(page)).toHaveCount(3);
    await page.getByRole('button', { name: 'Filters' }).click();
    await scan();
    // The selected row is marked at 3:1 against an unselected one: a solid accent edge,
    // where an unselected row shows the list's background.
    const rows = queue(page).getByRole('option');
    await rows.nth(1).hover();
    const edge = await rows.first().evaluate((el) => getComputedStyle(el).borderInlineStartColor);
    const ground = await queue(page).evaluate((el) => {
      let node: HTMLElement | null = el;
      while (node) {
        const bg = getComputedStyle(node).backgroundColor;
        if (bg !== 'rgba(0, 0, 0, 0)') return bg;
        node = node.parentElement;
      }
      return getComputedStyle(document.body).backgroundColor;
    });
    expect(contrast(edge, ground)).toBeGreaterThanOrEqual(3);
  });
}
