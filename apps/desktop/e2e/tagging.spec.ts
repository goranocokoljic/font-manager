import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// Browser mode runs on the mock client; no face has a tag yet. `mockFaces=3&mockStyles=4`
// adds the families Sample 1–3 with 4 styles each.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/?mockFaces=3&mockStyles=4#/');
  const tree = page.getByRole('tree', { name: 'Font list' });
  await expect(tree.getByRole('treeitem').first()).toBeVisible();
  return tree;
}

const family = (page: Page, name: string) =>
  page
    .getByRole('tree', { name: 'Font list' })
    .getByRole('treeitem', { name: new RegExp(`^${name},`) });
const panel = (page: Page) => page.getByRole('dialog', { name: /^Tags of / });
const filter = (page: Page) =>
  panel(page).getByRole('combobox', { name: 'Filter tags or name a new one' });
const option = (page: Page, name: string) =>
  panel(page).locator(`[role="option"][data-tag="${name}"]`);
const group = (page: Page) => page.getByRole('region', { name: 'Tags' });
const sidebarTag = (page: Page, name: string) => group(page).locator(`li[data-tag="${name}"]`);

/** Opens the panel with T and makes `name` on the selection with Enter. */
async function make(page: Page, name: string) {
  await page.keyboard.press('t');
  await expect(panel(page)).toBeVisible();
  await filter(page).fill(name);
  await page.keyboard.press('Enter');
  await expect(option(page, name)).toHaveAttribute('data-state', 'all');
  await page.keyboard.press('Escape');
  await expect(panel(page)).toHaveCount(0);
}

test('the panel tags several fonts with several tags in one go and shows a mixed state', async ({
  page,
}) => {
  await open(page);
  // Sample 1 alone gets "Display"; Sample 2 alone gets "Body".
  await family(page, 'Sample 1').click();
  await make(page, 'Display');
  await family(page, 'Sample 2').click();
  await make(page, 'Body');
  await expect(sidebarTag(page, 'Display')).toContainText('4');
  await expect(sidebarTag(page, 'Body')).toContainText('4');

  // Both selected: each tag is on some of the 8 fonts.
  await family(page, 'Sample 1').click();
  await family(page, 'Sample 2').click({ modifiers: ['ControlOrMeta'] });
  await page.keyboard.press('t');
  await expect(panel(page).getByRole('heading')).toHaveText('Tags of 8 fonts');
  await expect(option(page, 'Display')).toHaveAttribute('data-state', 'some');
  await expect(option(page, 'Display')).toContainText('on some (4 of 8)');
  await expect(option(page, 'Body')).toHaveAttribute('data-state', 'some');

  // Put Display on all and take Body off all; one Apply sends both (stores/tagging.test.ts
  // proves it is one tagFaces and one untagFaces call).
  await option(page, 'Display').click();
  await option(page, 'Body').click();
  await option(page, 'Body').click();
  await expect(option(page, 'Display')).toHaveAttribute('data-state', 'all');
  await expect(option(page, 'Body')).toHaveAttribute('data-state', 'none');
  await panel(page).getByRole('button', { name: 'Apply 2' }).click();
  await expect(panel(page)).toHaveCount(0);
  await expect(page.getByTestId('toast')).toContainText(
    'Added 1 tag and removed 1 tag on 8 fonts.',
  );
  await expect(sidebarTag(page, 'Display')).toContainText('8');
  // A tag on no face is gone.
  await expect(sidebarTag(page, 'Body')).toHaveCount(0);

  // The palette opens the same panel.
  await page.keyboard.press('ControlOrMeta+k');
  await page.getByRole('combobox', { name: 'Search commands' }).fill('tag the selected');
  await page.keyboard.press('Enter');
  await expect(panel(page)).toBeVisible();
  await expect(option(page, 'Display')).toHaveAttribute('data-state', 'all');
});

test('the panel is keyboard-driven: fuzzy filter, arrows, Space, Enter, Escape', async ({
  page,
}) => {
  await open(page);
  await family(page, 'Sample 1').click();
  await make(page, 'Brand');
  await make(page, 'Display');
  await make(page, 'Body text');

  await family(page, 'Sample 2').click();
  await page.keyboard.press('t');
  await expect(filter(page)).toBeFocused();
  // "bt" fuzzy-matches "Body text" only (b…t); Brand has no t.
  await filter(page).fill('bt');
  await expect(panel(page).getByRole('option')).toHaveCount(1);
  await expect(panel(page).getByRole('option')).toHaveAttribute('data-tag', 'Body text');
  await filter(page).fill('');
  await expect(panel(page).getByRole('option')).toHaveCount(3);

  // A to Z: Body text, Brand, Display. Arrow down to Brand, Space toggles it on.
  const active = () => panel(page).locator('[role="option"].active');
  await expect(active()).toHaveAttribute('data-tag', 'Body text');
  await page.keyboard.press('ArrowDown');
  await expect(active()).toHaveAttribute('data-tag', 'Brand');
  await expect(filter(page)).toHaveAttribute('aria-activedescendant', /fm-tag-option-1/);
  await page.keyboard.press('Space');
  await expect(option(page, 'Brand')).toHaveAttribute('data-state', 'all');
  await expect(filter(page)).toHaveValue('');
  // Arrow up wraps around to the last tag.
  await page.keyboard.press('ArrowUp');
  await page.keyboard.press('ArrowUp');
  await expect(active()).toHaveAttribute('data-tag', 'Display');

  // Filter, Space, Enter: text that still shows a tag is a filter, so Enter applies the
  // staged toggle and makes no tag from the filter text.
  await filter(page).fill('disp');
  await expect(panel(page).getByRole('option')).toHaveCount(1);
  await expect(panel(page)).not.toContainText('Enter makes the tag');
  await page.keyboard.press('Space');
  await expect(option(page, 'Display')).toHaveAttribute('data-state', 'all');
  await page.keyboard.press('Enter');
  await expect(panel(page)).toHaveCount(0);
  await expect(sidebarTag(page, 'Display')).toContainText('8');
  await expect(sidebarTag(page, 'disp')).toHaveCount(0);
  await expect(sidebarTag(page, 'Brand')).toContainText('8');

  // Enter on a name no tag has (the filter shows none) makes it on the selection at once.
  await page.keyboard.press('t');
  await filter(page).fill('Headline');
  await expect(panel(page).getByRole('option')).toHaveCount(0);
  await expect(panel(page)).toContainText('Enter makes the tag “Headline” on 4 fonts');
  await page.keyboard.press('Enter');
  await expect(option(page, 'Headline')).toHaveAttribute('data-state', 'all');
  await expect(sidebarTag(page, 'Headline')).toContainText('4');

  // Enter with nothing typed and nothing staged just closes.
  await page.keyboard.press('Enter');
  await expect(panel(page)).toHaveCount(0);

  // Escape closes without applying.
  await page.keyboard.press('t');
  await page.keyboard.press('Space');
  await expect(option(page, 'Body text')).toHaveAttribute('data-state', 'all');
  await page.keyboard.press('Escape');
  await expect(panel(page)).toHaveCount(0);
  await expect(sidebarTag(page, 'Body text')).toContainText('4');
  // Focus is back on the list, which it was taken from.
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeFocused();
});

test('the Tags group counts each tag; a list or table drop tags every dragged font', async ({
  page,
}) => {
  await open(page);
  await family(page, 'Sample 1').click();
  await make(page, 'Display');
  await expect(group(page).getByRole('heading', { name: 'Tags' })).toBeVisible();
  await expect(sidebarTag(page, 'Display').getByLabel('4 fonts', { exact: true })).toBeVisible();

  // A family row from the list: all four of its styles.
  await family(page, 'Sample 2').dragTo(sidebarTag(page, 'Display'));
  await expect(sidebarTag(page, 'Display').getByLabel('8 fonts', { exact: true })).toBeVisible();

  // Two selected family rows from the table.
  await page.keyboard.press('2');
  const grid = page.getByRole('treegrid', { name: 'Font table' });
  await expect(grid).toBeVisible();
  const row = (name: string) => grid.getByRole('row', { name: new RegExp(`^${name},`) });
  await row('Sample 3').click();
  await row('Inter').click({ modifiers: ['ControlOrMeta'] });
  await row('Sample 3').dragTo(sidebarTag(page, 'Display'));
  await expect(sidebarTag(page, 'Display').getByLabel('13 fonts', { exact: true })).toBeVisible();
});

test('a drag unfolds the Tags group, marks the tags the fonts carry, and folds it back', async ({
  page,
}) => {
  await open(page);
  await family(page, 'Sample 1').click();
  await make(page, 'Display');
  await make(page, 'Serif');
  await family(page, 'Sample 2').click();
  await make(page, 'Body');

  const fold = group(page).getByRole('button', { name: 'Tags' });
  await fold.click();
  await expect(fold).toHaveAttribute('aria-expanded', 'false');
  await expect(sidebarTag(page, 'Display')).toBeHidden();

  // Hold a drag of Sample 1 over the list: the group opens and marks Display and Serif.
  const source = family(page, 'Sample 1');
  await source.hover();
  await page.mouse.down();
  const box = (await family(page, 'Sample 3').boundingBox())!;
  await page.mouse.move(box.x + 20, box.y + 5, { steps: 5 });
  await expect(fold).toHaveAttribute('aria-expanded', 'true');
  await expect(sidebarTag(page, 'Display')).toHaveAttribute('data-carried', 'true');
  await expect(sidebarTag(page, 'Serif')).toHaveAttribute('data-carried', 'true');
  await expect(sidebarTag(page, 'Body')).not.toHaveAttribute('data-carried', 'true');

  // Drop it on Body: tagged, unmarked, and folded again.
  const body = (await sidebarTag(page, 'Body').boundingBox())!;
  await page.mouse.move(body.x + 10, body.y + body.height / 2, { steps: 5 });
  await page.mouse.up();
  await expect(fold).toHaveAttribute('aria-expanded', 'false');
  await fold.click();
  await expect(sidebarTag(page, 'Body').getByLabel('8 fonts', { exact: true })).toBeVisible();
  await expect(group(page).locator('[data-carried]')).toHaveCount(0);

  // Unfolded before the drag: it stays unfolded after.
  await family(page, 'Sample 3').dragTo(sidebarTag(page, 'Serif'));
  await expect(fold).toHaveAttribute('aria-expanded', 'true');
  await expect(sidebarTag(page, 'Serif').getByLabel('8 fonts', { exact: true })).toBeVisible();
});
