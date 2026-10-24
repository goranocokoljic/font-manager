import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';

// "Find similar" (E23.3) on the mock client: from the list row (its action and the S
// key), the inspector header and the command palette. `mockFaces=3&mockStyles=4` adds
// the families Sample 1–3 with four styles each; `mockSimilar=1` finds nothing,
// `mockSimilar=2` answers with an error.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page, query = '?mockFaces=3&mockStyles=4', route = '/') {
  await page.route('**/fmfont/*', (r) =>
    r.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(`/${query}#${route}`);
  const tree = page.getByRole('tree', { name: 'Font list' });
  await expect(tree.getByRole('treeitem').first()).toBeVisible();
  return tree;
}

const panel = (page: Page, name: string | RegExp = /^Similar to /) =>
  page.getByRole('region', { name });
const results = (page: Page) => panel(page).getByRole('list', { name: 'Similar fonts' });
const info = (page: Page) => page.getByRole('tabpanel', { name: 'Info' });

/** The face name of a result button, from its label "<name>, <n>% similar". */
async function nameOf(result: Locator): Promise<string> {
  const label = (await result.getAttribute('aria-label')) ?? '';
  return label.replace(/, (\d+% similar|No score)$/, '');
}

/** Every result shows its own font as live text, a percent and its reasons. */
async function expectResults(page: Page): Promise<void> {
  const buttons = results(page).getByRole('button');
  await expect(buttons.first()).toBeVisible();
  const count = await buttons.count();
  expect(count).toBeGreaterThan(1);
  for (let i = 0; i < count; i++) {
    const b = buttons.nth(i);
    await expect(b).toHaveAccessibleName(/, \d+% similar$/);
    await expect(b.getByTestId('similar-score')).toHaveText(/^\d+% similar$/);
    await expect(b).toHaveAccessibleDescription(/^Why: \S/);
    const face = await b.getAttribute('data-face');
    await expect(b.locator('.sample')).toHaveCSS('font-family', new RegExp(`fm-face-${face}\\b`));
  }
  await expect(buttons.first().locator('.row')).toHaveAttribute('data-state', 'ready');
}

test('the row action opens "Similar to" and a click selects the result in the list and inspector', async ({
  page,
}) => {
  const tree = await open(page);
  const inter = tree.getByRole('treeitem', { name: 'Inter, 1 style' });
  await inter.hover();
  const action = inter.getByRole('button', { name: 'Find the fonts similar to Inter Regular' });
  await expect(action).toBeVisible();
  await expect(action).toHaveAttribute('aria-keyshortcuts', 'S');
  await action.click();
  await expect(panel(page, 'Similar to Inter Regular')).toBeVisible();
  await expectResults(page);

  // A style of an extra family: its family expands and the style row is selected.
  const sample = results(page)
    .getByRole('button', { name: /^Sample \d \w+/ })
    .first();
  const name = await nameOf(sample);
  await sample.click();
  await expect(sample).toHaveAttribute('aria-current', 'true');
  await expect(info(page).getByRole('heading', { name })).toBeVisible();
  await expect(tree.locator('[role="treeitem"][aria-selected="true"]')).toHaveAccessibleName(
    new RegExp(`^${name}, `),
  );
});

test('S on the selected row opens the panel; arrows and Enter select; Escape closes', async ({
  page,
}) => {
  const tree = await open(page);
  await tree.getByRole('treeitem', { name: 'Inter, 1 style' }).click();
  await page.keyboard.press('ArrowDown'); // JetBrains Mono
  await expect(info(page).getByRole('heading', { name: 'JetBrains Mono Regular' })).toBeVisible();
  await page.keyboard.press('s');
  await expect(panel(page, 'Similar to JetBrains Mono Regular')).toBeVisible();
  await expectResults(page);

  const buttons = results(page).getByRole('button');
  await expect(buttons.first()).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(buttons.nth(1)).toBeFocused();
  const name = await nameOf(buttons.nth(1));
  await page.keyboard.press('Enter');
  await expect(info(page).getByRole('heading', { name })).toBeVisible();
  await expect(tree.locator('[role="treeitem"][aria-selected="true"]')).toHaveAccessibleName(
    new RegExp(`^${name}, `),
  );
  // Tab reaches every result; Shift+Tab back to the close button.
  await page.keyboard.press('Tab');
  await expect(buttons.nth(2)).toBeFocused();
  await buttons.first().focus();
  await page.keyboard.press('Shift+Tab');
  await expect(panel(page).getByRole('button', { name: 'Close the similar fonts' })).toBeFocused();

  await page.keyboard.press('Escape');
  await expect(panel(page)).toBeHidden();
  await expect(tree).toBeFocused();
});

test('the inspector header opens the panel for the shown face', async ({ page }) => {
  await open(page, '?mockFaces=3&mockStyles=4', '/?face=6');
  await expect(info(page).getByRole('heading', { name: 'JetBrains Mono Regular' })).toBeVisible();
  const button = info(page).getByRole('button', { name: 'Find similar' });
  await expect(button).toHaveAttribute('aria-keyshortcuts', 'S');
  await button.focus();
  await page.keyboard.press('Enter');
  await expect(panel(page, 'Similar to JetBrains Mono Regular')).toBeVisible();
  await expectResults(page);
  await panel(page).getByRole('button', { name: 'Close the similar fonts' }).click();
  await expect(panel(page)).toBeHidden();
});

test('the command palette lists the action with its key and runs it', async ({ page }) => {
  await open(page, '?mockFaces=3&mockStyles=4', '/?face=1');
  await expect(info(page).getByRole('heading', { name: 'Inter Regular' })).toBeVisible();
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('similar');
  const option = page.getByRole('option', { name: /Find similar fonts/ });
  await expect(option).toBeVisible();
  await expect(option).toContainText('S');
  await page.keyboard.press('Enter');
  await expect(panel(page, 'Similar to Inter Regular')).toBeVisible();
  await expectResults(page);
});

test('no similar face shows a plain message', async ({ page }) => {
  await open(page, '?mockSimilar=1', '/?face=1');
  await expect(info(page).getByRole('heading', { name: 'Inter Regular' })).toBeVisible();
  await page.getByRole('main').focus();
  await page.keyboard.press('s');
  await expect(panel(page).getByRole('status')).toHaveText(
    'No other font in the library looks like Inter Regular.',
  );
  await expect(results(page)).toHaveCount(0);
});

test('an error from the core shows a plain message', async ({ page }) => {
  await open(page, '?mockSimilar=2', '/?face=1');
  await expect(info(page).getByRole('heading', { name: 'Inter Regular' })).toBeVisible();
  await info(page).getByRole('button', { name: 'Find similar' }).click();
  await expect(panel(page).getByRole('alert')).toHaveText(
    'Font Manager could not find similar fonts. Finding similar fonts is not available yet.',
  );
  await expect(results(page)).toHaveCount(0);
});
