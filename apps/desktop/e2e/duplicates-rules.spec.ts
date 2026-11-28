import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';

// Languages and the keeper policy (E56.14) on the browser mock with `?mockDuplicates=1`:
// three duplicate sets, each with a copy outside the system folders, so the mock's preview
// gives each a keeper. The mock's saved settings live in sessionStorage, so a reload keeps
// what was saved and drops what was not.

const open = async (page: Page) => {
  await page.goto('/?mockDuplicates=1#/duplicates?section=rules');
  await expect(page.getByTestId('duplicates-rules')).toBeVisible();
};
const rules = (page: Page) => page.getByRole('list', { name: 'Rules, most important first' });
/** The rule names in order (the rule rows only, not a rule's own short list). */
const order = (page: Page) => rules(page).locator(':scope > li .rule-name').allTextContents();
const preview = (page: Page) => page.getByTestId('dup-policy-preview');

test('the rules page shows the languages but does not edit them; its link opens Settings › Languages', async ({
  page,
}) => {
  await open(page);
  // E58.2: the editor moved to Settings › Languages.
  await expect(page.getByRole('combobox', { name: 'Add a language' })).toHaveCount(0);
  await expect(page.getByTestId('dup-guard')).toContainText(
    'Set your languages under Settings › Languages',
  );
  await page.getByTestId('dup-rules-change-languages').click();
  await expect(page).toHaveURL(/#\/settings\?section=languages/);
  await page.getByRole('combobox', { name: 'Add a language' }).click();
  await page.keyboard.type('Serb');
  await page.getByRole('option', { name: 'Serbian (Cyrillic)', exact: true }).click();
  await page.getByRole('button', { name: 'Open the rules' }).click();
  await expect(page.getByTestId('dup-rules-languages')).toHaveText('Serbian (Cyrillic)');
  await expect(page.getByTestId('dup-rules-languages').getByRole('button')).toHaveCount(0);
  await expect(page.getByTestId('dup-languages')).toHaveText('Serbian (Cyrillic)');
  await expect(page.getByTestId('dup-guard')).not.toContainText('Set your languages under');
});

test('the guard stands above the rules, always on and not a row', async ({ page }) => {
  await open(page);
  const guard = page.getByTestId('dup-guard');
  await expect(guard).toContainText('Language guard');
  await expect(guard).toContainText('Always on');
  await expect(guard.getByRole('switch')).toHaveCount(0);
  await expect(guard.getByRole('button')).toHaveCount(0);
  await expect(rules(page)).not.toContainText('guard');
  const [g, r] = await Promise.all([guard.boundingBox(), rules(page).boundingBox()]);
  expect(g!.y + g!.height).toBeLessThanOrEqual(r!.y);
});

test('reorders a rule by keyboard; the preview follows and nothing saves until Save', async ({
  page,
}) => {
  await open(page);
  await expect(preview(page)).toHaveText(
    'With these rules, 3 sets get a keeper, 0 sets are “keep both”, and 0 recommendations change.',
  );
  expect((await order(page)).slice(0, 2)).toEqual([
    'Covers more of my languages',
    'Newest version',
  ]);
  const save = page.getByTestId('dup-policy-save');
  await expect(save).toBeDisabled();

  const grip = page.getByRole('button', { name: 'Move Covers more of my languages' });
  await grip.focus();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await expect(grip).toBeFocused();
  expect((await order(page)).slice(0, 3)).toEqual([
    'Newest version',
    'Format, in my order',
    'Covers more of my languages',
  ]);
  await expect(
    page.getByText('Covers more of my languages moved to place 3 of 10.'),
  ).toBeAttached();
  await expect(page.getByTestId('dup-policy-unsaved')).toBeVisible();
  await expect(save).toBeEnabled();
  await expect(preview(page)).toContainText('3 sets get a keeper');

  // Not saved: a reload shows the saved order.
  await page.reload();
  expect((await order(page))[0]).toBe('Covers more of my languages');

  // Turned off and moved, then saved: a reload keeps it.
  await page.getByRole('switch', { name: 'Use the rule “Newest version”' }).click();
  await page.getByRole('button', { name: 'Move Most glyphs' }).focus();
  await page.keyboard.press('Home');
  await page.getByRole('switch', { name: 'Use the rule “Most glyphs”' }).click();
  await save.click();
  await expect(page.getByTestId('dup-policy-unsaved')).toHaveCount(0);
  await page.reload();
  expect((await order(page)).slice(0, 4)).toEqual([
    'Most glyphs',
    'Covers more of my languages',
    'Format, in my order',
    'In the library folder',
  ]);

  // Back to the default loads it, unsaved, until Save.
  await page.getByRole('button', { name: 'Back to the default' }).click();
  expect((await order(page)).slice(0, 4)).toEqual([
    'Covers more of my languages',
    'Newest version',
    'Format, in my order',
    'In the library folder',
  ]);
  await expect(page.getByTestId('dup-policy-unsaved')).toBeVisible();
  await save.click();
  await expect(save).toBeDisabled();
});

test('the format order opens as its own short list', async ({ page }) => {
  await open(page);
  const toggle = rules(page).getByRole('button', { name: '5 formats' });
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-expanded', 'true');
  const formats = page.getByRole('list', { name: 'Formats, best first' });
  await page.getByRole('button', { name: 'Move WOFF2', exact: true }).focus();
  await page.keyboard.press('Home');
  await expect(formats.locator('li .rule-name').first()).toHaveText('WOFF2');
  // The rules did not move.
  expect((await order(page)).slice(0, 3)).toContain('Format, in my order');
});

test('the rules page passes axe', async ({ page }) => {
  await open(page);
  await rules(page).getByRole('button', { name: '5 formats' }).click();
  const result = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
    .analyze();
  expect(result.violations.map((v) => `${v.id}: ${v.nodes[0]?.target}`)).toEqual([]);
});
