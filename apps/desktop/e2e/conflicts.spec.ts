import { expect, test, type Page } from '@playwright/test';
import { openSection, sectionPanel } from './family-page';

const info = (page: Page) => sectionPanel(page, 'Info');
const question = (page: Page) =>
  info(page).getByRole('alertdialog', { name: 'Font name already in use' });

// Face 6 (JetBrains Mono) shares its PostScript name with a system font in the mock.
test('activating a clashing face asks keep / replace for this session / cancel', async ({
  page,
}) => {
  const panel = await openSection(page, 6, 'Info');
  await panel.getByRole('button', { name: 'Activate', exact: true }).click();
  await expect(question(page)).toContainText(
    'A system font already uses the PostScript name JetBrainsMono-Regular',
  );
  await expect(question(page).getByRole('button', { name: 'Keep existing' })).toBeFocused();

  await question(page).getByRole('button', { name: 'Cancel' }).click();
  await expect(question(page)).toBeHidden();
  await expect(panel.getByTestId('activation-state')).toHaveText('Not active');

  await panel.getByRole('button', { name: 'Activate', exact: true }).click();
  await question(page).getByRole('button', { name: 'Keep existing' }).click();
  await expect(panel.getByRole('alert')).toContainText('Not activated');
  await expect(panel.getByTestId('activation-state')).toHaveText('Not active');

  await panel.getByRole('button', { name: 'Activate', exact: true }).click();
  await question(page).getByRole('button', { name: 'Replace for this session' }).click();
  await expect(panel.getByTestId('activation-state')).toHaveText('Active for this session');
});

test('the Conflicts view lists every clash and its winner', async ({ page }) => {
  await page.goto('/#/conflicts');
  await expect(page.getByRole('heading', { name: 'Font conflicts', level: 1 })).toBeVisible();
  await expect(page.getByText('No conflicts.')).toBeVisible();

  await openSection(page, 6, 'Info');
  await info(page).getByRole('button', { name: 'Activate', exact: true }).click();
  await question(page).getByRole('button', { name: 'Replace for this session' }).click();
  await expect(info(page).getByTestId('activation-state')).toHaveText('Active for this session');

  await page.keyboard.press('Escape');
  await page.getByRole('link', { name: 'Conflicts', exact: true }).click();
  const table = page.getByRole('table', { name: 'Font conflicts' });
  const row = table.getByRole('row').nth(1);
  await expect(row.getByRole('cell')).toHaveText([
    'JetBrainsMono-Regular',
    'JetBrains Mono Regular',
    'Installed font: /System/Library/Fonts/JetBrainsMono-Regular.ttf',
    'JetBrains Mono Regular',
  ]);
  await expect(table.getByRole('row')).toHaveCount(2);

  await page.keyboard.press('Escape');
  await expect(page.getByRole('heading', { name: 'All fonts' })).toBeVisible();
});

test('A on a clashing style in the list asks the same question under the count line', async ({
  page,
}) => {
  // The face comes from the URL and a search hides its row, so A acts on the one face.
  await page.goto('/#/?face=6');
  const onWall = page.getByRole('alertdialog', { name: 'Font name already in use' });
  await page.getByTestId('selected-row').waitFor();
  await page.getByRole('searchbox').fill('Inter');
  const tree = page.getByRole('tree', { name: 'Font list' });
  await expect(tree.getByRole('treeitem')).toHaveCount(1);
  await expect(page.getByTestId('selected-row')).toHaveCount(0);
  await tree.focus();
  await page.keyboard.press('a');
  await expect(onWall).toContainText('JetBrainsMono-Regular');
  await expect(onWall.getByRole('button', { name: 'Keep existing' })).toBeFocused();
  await onWall.getByRole('button', { name: 'Keep existing' }).click();
  await expect(onWall).toBeHidden();
  await expect(page.getByRole('alert')).toContainText('Not activated');
});
