import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { axesRow, sectionPanel, showSection } from './family-page';

// E41.5: the library has no inspector. One row selected shows in the count line, Enter or a
// click on the name opens the family page, Space opens Quick look, and every part the inspector
// had lives on the family page.

const font = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

const MODES = [
  { mode: 'list', view: 'tree', name: 'Font list', item: 'treeitem' },
  { mode: 'table', view: 'treegrid', name: 'Font table', item: 'row' },
  { mode: 'grid', view: 'listbox', name: 'Font grid', item: 'option' },
  { mode: 'waterfall', view: 'tree', name: 'Font waterfall', item: 'treeitem' },
] as const;

async function open(page: Page, mode: string, query = '') {
  await page.route('**/fmfont/*', (r) =>
    r.fulfill({ status: 200, contentType: 'font/ttf', body: font }),
  );
  await page.addInitScript(
    (v) => localStorage.setItem('fm.views.v1', v),
    JSON.stringify({ all: { mode } }),
  );
  await page.goto(`/${query}#/`);
}

/** Inter's row or card in the view. */
function inter(page: Page, m: (typeof MODES)[number]): Locator {
  return page
    .getByRole(m.view, { name: m.name })
    .locator('[data-key]')
    .filter({ hasText: 'Inter' })
    .first();
}

/** The name in that row or card; a waterfall row has one line per size under its name. */
const nameOf = (row: Locator) => row.locator('[data-family-name]').first();

for (const m of MODES) {
  test.describe(`in the ${m.mode} view`, () => {
    test('there is no inspector pane, toggle or shortcut', async ({ page }) => {
      await open(page, m.mode);
      await expect(inter(page, m)).toBeVisible();
      await expect(page.locator('#fm-inspector')).toHaveCount(0);
      await expect(page.getByRole('complementary', { name: 'Inspector' })).toHaveCount(0);
      await expect(page.getByRole('button', { name: 'Inspector' })).toHaveCount(0);
      await expect(page.getByRole('separator', { name: 'Resize inspector' })).toHaveCount(0);
      await page.keyboard.press(']');
      await page.keyboard.press('ControlOrMeta+i');
      await expect(page.locator('#fm-inspector')).toHaveCount(0);
      await expect(page.getByRole('main').locator('aside')).toHaveCount(0);
    });

    test('selecting a row shows its name and state in the count line', async ({ page }) => {
      await open(page, m.mode);
      const selected = page.getByTestId('selected-row');
      await expect(inter(page, m)).toBeVisible();
      await expect(selected).toHaveCount(0);
      await nameOf(inter(page, m)).click({ modifiers: ['ControlOrMeta'] });
      await expect(selected).toContainText('Inter');
      await expect(page.getByTestId('selected-row-state')).toHaveText('not active');
      await page.keyboard.press('a');
      await expect(page.getByTestId('selected-row-state')).toHaveText(/^active/);
    });

    test('Enter opens the family page, and Esc comes back to the row', async ({ page }) => {
      await open(page, m.mode);
      await inter(page, m).click({ position: { x: 4, y: 4 }, modifiers: ['ControlOrMeta'] });
      await page.keyboard.press('Enter');
      await expect(page).toHaveURL(/#\/family\/1\b/);
      await expect(page.getByRole('heading', { name: 'Inter', level: 1 })).toBeVisible();
      await page.keyboard.press('Escape');
      await expect(inter(page, m)).toBeVisible();
      await expect(page.getByTestId('selected-row')).toContainText('Inter');
    });

    test('a click on the name opens the family page', async ({ page }) => {
      await open(page, m.mode);
      await nameOf(inter(page, m)).click();
      await expect(page).toHaveURL(/#\/family\/1\b/);
      await expect(page.getByRole('heading', { name: 'Inter', level: 1 })).toBeVisible();
    });

    test('a click with a modifier on the name only selects', async ({ page }) => {
      await open(page, m.mode);
      await nameOf(inter(page, m)).click({ modifiers: ['ControlOrMeta'] });
      await expect(page.getByTestId('selected-row')).toContainText('Inter');
      await expect(page).not.toHaveURL(/#\/family\//);
    });

    test('Space opens Quick look', async ({ page }) => {
      await open(page, m.mode);
      await nameOf(inter(page, m)).click({ modifiers: ['ControlOrMeta'] });
      await expect(page.getByTestId('selected-row')).toContainText('Inter');
      await page.keyboard.press('Space');
      await expect(page.getByRole('dialog', { name: /^Quick look: Inter/ })).toBeVisible();
      await expect(page).not.toHaveURL(/#\/family\//);
    });
  });
}

test('with no row selected, A leaves alone the face the URL still names', async ({ page }) => {
  await page.goto('/?mockActive=6#/');
  const tree = page.getByRole('tree', { name: 'Font list' });
  const active = page.getByText(/^\d+ fonts? active$/);
  await expect(active).toHaveText('1 font active');
  await tree
    .getByRole('treeitem', { name: /^JetBrains Mono,/ })
    .click({ position: { x: 600, y: 4 } });
  await expect(page).toHaveURL(/face=6/);
  await page.getByRole('searchbox').fill('Inter');
  await expect(tree.getByRole('treeitem')).toHaveCount(1);
  await expect(page.getByTestId('selected-row')).toHaveCount(0);
  await expect(page).toHaveURL(/face=6/);
  await tree.focus();
  await page.keyboard.press('a');
  // The key ran first: Inter comes on after it, and the hidden face is still active.
  await tree.getByRole('treeitem', { name: /^Inter,/ }).click({ position: { x: 600, y: 4 } });
  await page.keyboard.press('a');
  await expect(active).toHaveText('2 fonts active');
});

test('the table keeps its column menu', async ({ page }) => {
  await open(page, 'table');
  await expect(page.getByRole('treegrid', { name: 'Font table' })).toBeVisible();
  await expect(page.getByRole('button', { name: /^Columns/ })).toBeVisible();
});

test('the shortcut sheet and the palette no longer offer the inspector', async ({ page }) => {
  await open(page, 'list');
  await page.keyboard.press('?');
  const sheet = page.getByRole('dialog');
  await expect(sheet).toContainText('Sidebar');
  await expect(sheet).not.toContainText('Inspector');
  await page.keyboard.press('Escape');
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('inspector');
  await expect(page.getByRole('option')).toHaveCount(0);
});

test('Enter on a style opens the page on that style', async ({ page }) => {
  await open(page, 'list', '?mockFaces=3&mockStyles=4');
  const tree = page.getByRole('tree', { name: 'Font list' });
  await tree.getByRole('treeitem', { name: /^Sample 1,/ }).click({ position: { x: 300, y: 4 } });
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown'); // the second style, not the family's first
  const style = tree.locator('[role="treeitem"][aria-selected="true"]');
  const name = (await style.getAttribute('aria-label'))!.split(',')[0]!;
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/face=\d+/);
  const info = await showSection(page, 'Info');
  await expect(info.getByRole('heading', { name })).toBeVisible();
});

test('every inspector part has its home on the family page', async ({ page }) => {
  await page.route('**/fmfont/*', (r) =>
    r.fulfill({ status: 200, contentType: 'font/ttf', body: font }),
  );
  await page.goto('/?mockColor=1#/family/8?face=8');
  const info = await showSection(page, 'Info');
  // Palettes, the Licence group and Notes are in Info with the facts (E42.8).
  await expect(info.getByRole('radiogroup', { name: 'Color palette' })).toBeVisible();
  await expect(info.getByRole('region', { name: 'Licence' })).toBeVisible();
  await expect(info.getByRole('textbox', { name: 'Notes', exact: true })).toBeVisible();
  await expect(info.getByRole('textbox', { name: 'Family notes' })).toBeVisible();
  await expect((await showSection(page, 'Characters')).getByRole('searchbox')).toBeVisible();
  await expect(
    (await showSection(page, 'Features')).getByRole('button', { name: /Copy CSS/ }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Technical details', exact: true }).click();
  await expect(page.locator('#fm-family-developer').getByTestId('css-snippet')).toBeVisible();

  await page.goto('/#/family/1?face=1');
  await expect(axesRow(page).getByRole('slider', { name: /Weight/ })).toBeVisible();
  await expect(sectionPanel(page, 'Styles')).toBeVisible();
});

test('a style in the Styles section is the one the panels describe', async ({ page }) => {
  await page.goto('/?mockFaces=3&mockStyles=4#/');
  await page
    .getByRole('tree', { name: 'Font list' })
    .getByRole('treeitem', { name: /^Sample 1,/ })
    .click({ position: { x: 300, y: 4 } });
  await page.keyboard.press('o');
  const styles = sectionPanel(page, 'Styles').getByTestId('family-style-row');
  await expect(styles.first()).toHaveAttribute('aria-selected', 'true');
  await styles.nth(1).click({ position: { x: 20, y: 20 } });
  await expect(styles.nth(1)).toHaveAttribute('aria-selected', 'true');
  await expect(styles.first()).toHaveAttribute('aria-selected', 'false');
  await expect(page).toHaveURL(/face=\d+/);
  const name = (await styles.nth(1).locator('.title').textContent())!.trim();
  const info = await showSection(page, 'Info');
  await expect(info.getByRole('heading', { name: `Sample 1 ${name}` })).toBeVisible();
});

test('Esc in a notes field saves it and stays on the page', async ({ page }) => {
  await page.goto('/#/family/1?face=1');
  const info = await showSection(page, 'Info');
  const notes = info.getByRole('textbox', { name: 'Notes', exact: true });
  await notes.fill('Saved by Esc');
  await page.keyboard.press('Escape');
  await expect(notes).not.toBeFocused();
  await expect(page).toHaveURL(/#\/family\/1/);
  await page.reload();
  await page.goto('/#/family/1?face=1');
  await expect(page).toHaveURL(/#\/family\/1/);
});
