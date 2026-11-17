import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { sectionPanel, showSection } from './family-page';

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
const info = (page: Page) => sectionPanel(page, 'Info');

/** The face name of a result button, from its label "<name>, <n>% similar". */
async function nameOf(result: Locator): Promise<string> {
  const label = (await result.getAttribute('aria-label')) ?? '';
  return label.replace(/, (\d+% similar|No score)(, \+\d+ more styles?)?$/, '');
}

/** Every result shows its own font as live text, a percent and its reasons. */
async function expectResults(page: Page): Promise<void> {
  const buttons = results(page).getByRole('button');
  await expect(buttons.first()).toBeVisible();
  const count = await buttons.count();
  expect(count).toBeGreaterThan(1);
  for (let i = 0; i < count; i++) {
    const b = buttons.nth(i);
    // A family row names how many more of its styles it stands for (review 25).
    await expect(b).toHaveAccessibleName(/, \d+% similar(, \+\d+ more styles?)?$/);
    await expect(b.getByTestId('similar-score')).toHaveText(/^\d+% similar$/);
    await expect(b).toHaveAccessibleDescription(/^(Why: \S.*)?$/); // only what differs from the other results
    const face = await b.getAttribute('data-face');
    await expect(b.locator('.sample')).toHaveCSS('font-family', new RegExp(`fm-face-${face}\\b`));
  }
  await expect(buttons.first().locator('.row')).toHaveAttribute('data-state', 'ready');
}

test('the row action opens "Similar to" and a click opens the result on its family page', async ({
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
  await showSection(page, 'Info');
  await expect(info(page).getByRole('heading', { name })).toBeVisible();
  // Back: the results are still there, the pick marked.
  await page.keyboard.press('Escape');
  await expect(panel(page)).toBeVisible();
  await expect(sample).toHaveAttribute('aria-current', 'true');
  // The panel replaces the list; closing it shows the picked face selected.
  await panel(page).getByRole('button', { name: 'Close the similar fonts' }).click();
  await expect(tree.locator('[role="treeitem"][aria-selected="true"]')).toHaveAccessibleName(
    // A one-style family is selected as its family row, not as a style row.
    new RegExp(`^(${name}|${name.replace(/ \S+$/, '')}), `),
  );
});

test('S on the selected row opens the panel; arrows and Enter select; Escape closes', async ({
  page,
}) => {
  const tree = await open(page);
  await tree.getByRole('treeitem', { name: 'Inter, 1 style' }).click();
  await page.keyboard.press('ArrowDown'); // JetBrains Mono
  await expect(page.getByTestId('selected-row')).toContainText('JetBrains Mono');
  await page.keyboard.press('s');
  await expect(panel(page, 'Similar to JetBrains Mono Regular')).toBeVisible();
  await expectResults(page);

  const buttons = results(page).getByRole('button');
  await expect(buttons.first()).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(buttons.nth(1)).toBeFocused();
  const name = await nameOf(buttons.nth(1));
  // Tab reaches every result; Shift+Tab back to the close button.
  await page.keyboard.press('Tab');
  await expect(buttons.nth(2)).toBeFocused();
  await buttons.nth(1).focus();
  await page.keyboard.press('Enter');
  await showSection(page, 'Info');
  await expect(info(page).getByRole('heading', { name })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(buttons.nth(1)).toHaveAttribute('aria-current', 'true');
  await buttons.first().focus();
  await page.keyboard.press('Shift+Tab');
  await expect(panel(page).getByRole('button', { name: 'Close the similar fonts' })).toBeFocused();

  await page.keyboard.press('Escape');
  await expect(panel(page)).toBeHidden();
  await expect(tree).toBeFocused();
  await expect(tree.locator('[role="treeitem"][aria-selected="true"]')).toHaveAccessibleName(
    // A one-style family is selected as its family row, not as a style row.
    new RegExp(`^(${name}|${name.replace(/ \S+$/, '')}), `),
  );
});

test('the family page opens the panel for the shown face', async ({ page }) => {
  await page.route('**/fmfont/*', (r) =>
    r.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/?mockFaces=3&mockStyles=4#/family/3?face=6');
  await showSection(page, 'Info');
  await expect(info(page).getByRole('heading', { name: 'JetBrains Mono Regular' })).toBeVisible();
  const button = page.getByRole('button', { name: 'Find similar' });
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
  await expect(page.getByTestId('selected-row')).toContainText('Inter');
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('similar');
  const option = page.getByRole('option', { name: /Find similar fonts/ });
  await expect(option).toBeVisible();
  await expect(option).toContainText('S');
  await page.keyboard.press('Enter');
  await expect(panel(page, 'Similar to Inter Regular')).toBeVisible();
  await expectResults(page);
});

/** Saves the panel at both review viewports when FM_SCREENS_DIR is set. */
async function shoot(page: Page, name: string): Promise<void> {
  if (!process.env.FM_SCREENS_DIR) return;
  for (const [w, h] of [
    [1280, 800],
    [1000, 700],
  ] as const) {
    await page.setViewportSize({ width: w, height: h });
    await page.screenshot({ path: `${process.env.FM_SCREENS_DIR}/${name}-${w}x${h}.png` });
  }
}

test('@screens the header says what the panel is; the reference is its own block (E54.10)', async ({
  page,
}) => {
  await open(page, '?mockFaces=3&mockStyles=4', '/?face=1');
  await page.getByRole('main').focus();
  await page.keyboard.press('s');
  const region = panel(page, 'Similar to Inter Regular');
  await expect(region).toBeVisible();
  await expectResults(page);

  // Header: the icon, the title at the view titles' size and weight, the count, the way back.
  const head = region.locator('header');
  await expect(head.getByTestId('similar-icon')).toBeVisible();
  const title = head.getByRole('heading', { name: 'Similar to Inter Regular' });
  const viewTitle = page.locator('h1.title').first();
  for (const prop of ['font-size', 'font-weight'])
    await expect(title).toHaveCSS(
      prop,
      await viewTitle.evaluate((el, p) => getComputedStyle(el).getPropertyValue(p), prop),
    );
  const rows = await results(page).getByRole('button').count();
  await expect(head.getByTestId('similar-count')).toHaveText(`${rows} fonts`);
  const back = head.getByRole('button', { name: /^Back to library/ });
  await expect(back).toBeVisible();
  await expect(back.locator('kbd')).toHaveText('Esc');
  await expect(back).toHaveAttribute('aria-keyshortcuts', 'Escape');
  const close = head.getByRole('button', { name: 'Close the similar fonts' });
  const [b, c] = [await back.boundingBox(), await close.boundingBox()];
  expect(b && c && c.x - (b.x + b.width)).toBeLessThan(16); // side by side

  // The reference: a labelled block above the list, not one of its rows.
  const reference = region.getByRole('region', { name: 'Reference' });
  await expect(reference).toBeVisible();
  await expect(reference).toContainText('Inter Regular');
  await expect(results(page).getByRole('button', { name: /^Inter Regular,/ })).toHaveCount(0);
  const [r, l] = [await reference.boundingBox(), await results(page).boundingBox()];
  expect(r && l && r.y + r.height).toBeLessThanOrEqual(l?.y ?? 0);

  // Each bar has a name and a value.
  const bar = results(page).getByRole('button').first().getByTestId('similar-bar');
  await expect(bar).toHaveAttribute('role', 'meter');
  await expect(bar).toHaveAttribute('aria-label', /^Similarity: \d+%$/);
  await expect(bar).toHaveAttribute('aria-valuenow', /^\d+$/);
  // No bare "Fallback" badge in the panel.
  await expect(region.getByTestId('fallback-hint')).toHaveCount(0);

  await shoot(page, 'similar');

  await back.click();
  await expect(panel(page)).toBeHidden();
});

test('@screens nothing close enough says so in plain words and offers the way back', async ({
  page,
}) => {
  const tree = await open(page, '?mockSimilar=1', '/?face=1');
  await expect(page.getByTestId('selected-row')).toContainText('Inter');
  await page.getByRole('main').focus();
  await page.keyboard.press('s');
  const status = panel(page).getByTestId('similar-empty').getByRole('status');
  await expect(status.getByRole('heading', { name: 'Nothing is close enough' })).toBeVisible();
  await expect(status).toContainText('No font in the library looks close enough to this one.');
  await expect(results(page)).toHaveCount(0);
  await shoot(page, 'similar-empty');
  await panel(page)
    .getByTestId('similar-empty')
    .getByRole('button', { name: /^Back to library/ })
    .click();
  await expect(panel(page)).toBeHidden();
  await expect(tree).toBeVisible();
});

test('a font that cannot be compared says why', async ({ page }) => {
  await open(page, '?mockSimilar=3', '/?face=1');
  await page.getByRole('main').focus();
  await page.keyboard.press('s');
  const status = panel(page).getByTestId('similar-empty').getByRole('status');
  await expect(status.getByRole('heading', { name: 'This font cannot be compared' })).toBeVisible();
  await expect(status).toContainText('it does not draw the Latin letters');
});

test('an error from the core shows a plain message', async ({ page }) => {
  await page.goto('/?mockSimilar=2#/family/1?face=1');
  await page.getByRole('button', { name: 'Find similar' }).click();
  await expect(panel(page).getByRole('alert')).toHaveText(
    'Font Fu could not find similar fonts. Finding similar fonts is not available yet.',
  );
  await expect(results(page)).toHaveCount(0);
});
