import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { openFilters } from './filters';

// E58.2: Settings › Languages on the browser mock with `?mockLanguages=more` (more languages
// than the filter drawer's ten). The mock keeps its settings in sessionStorage, so a reload
// shows what was saved. Every change saves at once: there is no Save button.

const OPEN = '/?mockLanguages=more#/settings?section=languages';

const mine = (page: Page) => page.getByRole('list', { name: 'Languages I work in' });
const names = (page: Page) => mine(page).locator('.name');

async function add(page: Page, name: string): Promise<void> {
  await page.getByRole('combobox', { name: 'Add a language' }).click();
  await page.keyboard.type(name.slice(0, 4));
  await page.getByRole('option', { name, exact: true }).click();
}

test('the section stands between Appearance and Online catalogs', async ({ page }) => {
  await page.goto(OPEN);
  const tabs = await page.getByRole('tab').allTextContents();
  const at = tabs.map((t) => t.trim()).indexOf('Languages');
  expect(tabs.map((t) => t.trim()).slice(at - 1, at + 2)).toEqual([
    'Appearance',
    'Languages',
    'Online catalogs',
  ]);
  await expect(page.getByRole('heading', { name: 'Languages', level: 2 })).toBeVisible();
});

test('adds, reorders by keyboard and removes languages; each change is saved at once', async ({
  page,
}) => {
  await page.goto(OPEN);
  // None set: the line says who leads now (the system's language, English here).
  await expect(page.getByTestId('my-languages-empty')).toHaveText(
    'None yet. Until you add one, the language of your system comes first: English.',
  );

  await add(page, 'Serbian (Latin)');
  await add(page, 'Greek');
  await add(page, 'Polish');
  await expect(names(page)).toHaveText(['Serbian (Latin)', 'Greek', 'Polish']);
  await expect(mine(page).locator('li').first()).toContainText('ČĆĐŠŽčćđšž');

  // Keyboard: from the grip, Home moves Polish to the top; the focus stays on its grip.
  const grip = page.getByRole('button', { name: 'Move Polish', exact: true });
  await grip.focus();
  await page.keyboard.press('Home');
  await expect(names(page)).toHaveText(['Polish', 'Serbian (Latin)', 'Greek']);
  await expect(grip).toBeFocused();
  await expect(page.getByText('Polish moved to place 1 of 3.')).toBeAttached();
  await page.keyboard.press('ArrowDown');
  await expect(names(page)).toHaveText(['Serbian (Latin)', 'Polish', 'Greek']);

  // Removing by the x, reached from the keyboard.
  await page.getByRole('button', { name: 'Remove Greek' }).focus();
  await page.keyboard.press('Enter');
  await expect(names(page)).toHaveText(['Serbian (Latin)', 'Polish']);

  // Saved through the API at once: a reload keeps all of it.
  await page.reload();
  await expect(names(page)).toHaveText(['Serbian (Latin)', 'Polish']);
  // "Then the rest" follows the API: places 3 to 10 after the user's two.
  const first = page.getByRole('list', { name: 'The rest of the first ten' }).locator('li');
  await expect(first).toHaveCount(8);
  await expect(first.first()).toHaveText('3 English');
  await expect(first.last()).toContainText('10 ');
  await expect(page.getByTestId('languages-behind')).toContainText('Behind “Show more”');

  // The filter drawer reads the same order.
  await page.goto('/?mockLanguages=more#/');
  await expect(page.locator('[data-face="1"]')).toBeVisible();
  const drawer = await openFilters(page);
  await expect(
    drawer.getByRole('group', { name: 'Languages' }).locator('button.chip:not(.more)').first(),
  ).toHaveText('Serbian (Latin)');
});

test('the way from Duplicates: the foot of its navigation and back to the rules', async ({
  page,
}) => {
  await page.goto('/?mockDuplicates=1&mockLanguages=more#/duplicates');
  const nav = page.getByRole('navigation', { name: 'Duplicates' });
  await nav.getByRole('link', { name: 'Change languages' }).click();
  await expect(page).toHaveURL(/#\/settings\?section=languages/);
  await expect(page.getByRole('tab', { name: 'Languages' })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await add(page, 'Serbian (Cyrillic)');
  const open = page.getByRole('button', { name: 'Open the rules' });
  await expect(page.getByText('Duplicates uses this list too')).toBeVisible();
  await open.click();
  await expect(page).toHaveURL(/#\/duplicates\?section=rules/);
  await expect(page.getByTestId('dup-languages')).toHaveText('Serbian (Cyrillic)');
  await expect(page.getByTestId('dup-rules-languages')).toHaveText('Serbian (Cyrillic)');
});

for (const scheme of ['light', 'dark'] as const) {
  test(`passes the accessibility and contrast checks, empty and set (${scheme})`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto(OPEN);
    await expect(page.getByTestId('my-languages-empty')).toBeVisible();
    const check = async () =>
      (await new AxeBuilder({ page }).include('[role="tabpanel"]').analyze()).violations.map(
        (v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`,
      );
    expect(await check(), 'empty').toEqual([]);
    await add(page, 'Serbian (Latin)');
    await add(page, 'Greek');
    await expect(names(page)).toHaveCount(2);
    expect(await check(), 'set').toEqual([]);
  });
}
