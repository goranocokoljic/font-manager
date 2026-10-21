import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// E13.2: named comparisons. Compare loads face files from `/fmfont/<id>`; serve one fixture.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page, query = '') {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(`/?mockFaces=3${query}#/`);
  await page.evaluate(() => localStorage.clear());
  const tree = page.getByRole('tree', { name: 'Font list' });
  await expect(tree.getByRole('treeitem')).toHaveCount(6);
  await tree.getByRole('treeitem').first().click();
  return tree;
}

const toast = (page: Page) => page.getByTestId('toast');
const sheet = (page: Page) => page.getByRole('dialog', { name: 'Compare' });
const compareButton = (page: Page) => page.getByRole('button', { name: /^Compare \d\/4$/ });
const faceNames = (page: Page) => sheet(page).locator('.face .name');

async function pinAndNext(page: Page, expected: RegExp) {
  await page.keyboard.press('c');
  await expect(toast(page)).toContainText(expected);
  await page.keyboard.press('ArrowDown');
}

test('the sheet saves the comparison under a name; the list and the palette reopen it', async ({
  page,
}) => {
  await open(page);
  await pinAndNext(page, /^Inter .*\(1 of 4\)/);
  await pinAndNext(page, /^JetBrains Mono .*\(2 of 4\)/);
  await compareButton(page).click();
  const dialog = sheet(page);
  await dialog.getByRole('radio', { name: 'Stacked' }).check();

  const name = dialog.getByRole('textbox', { name: 'Comparison name' });
  const save = dialog.getByRole('button', { name: 'Save comparison' });
  await expect(save).toBeDisabled();
  await name.fill('Body pairing');
  await save.click();
  await expect(toast(page)).toHaveText(/Comparison “Body pairing” saved\./);
  await expect(dialog.getByTestId('compare-current')).toHaveText('Body pairing');
  const saved = dialog.getByRole('list', { name: 'Saved' });
  await expect(saved.getByRole('button', { name: 'Open comparison Body pairing' })).toBeVisible();

  // Change the pins and the mode, then reopen the saved one from the sheet's list.
  await dialog.getByRole('button', { name: /^Remove JetBrains Mono/ }).click();
  await dialog.getByRole('radio', { name: 'Side by side' }).check();
  await saved.getByRole('button', { name: 'Open comparison Body pairing' }).click();
  await expect(toast(page)).toHaveText(/Opened “Body pairing”\./);
  await expect(faceNames(page)).toHaveText([/^Inter /, /^JetBrains Mono /]);
  await expect(dialog.getByRole('radio', { name: 'Stacked' })).toBeChecked();

  // From the command palette, with the sheet closed and the pins changed.
  await dialog.getByRole('button', { name: /^Remove Inter/ }).click();
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('open comparison body');
  await page.keyboard.press('Enter');
  await expect(dialog).toBeVisible();
  await expect(faceNames(page)).toHaveText([/^Inter /, /^JetBrains Mono /]);
  await expect(dialog.getByRole('radio', { name: 'Stacked' })).toBeChecked();
});

test('a saved comparison opens with the fonts left and names the missing one', async ({ page }) => {
  await open(page, '&mockComparisons=1');
  // Nothing is pinned, but a saved comparison can be opened from the sheet.
  await expect(compareButton(page)).toBeEnabled();
  await compareButton(page).click();
  const dialog = sheet(page);
  await dialog.getByRole('button', { name: 'Open comparison Headline shortlist' }).click();

  await expect(faceNames(page)).toHaveText([/^Inter /, /^JetBrains Mono /]);
  await expect(dialog.getByTestId('compare-missing')).toHaveText(
    'Not in the library now: Retired Sans Bold.',
  );
  await expect(dialog.getByRole('radio', { name: 'Stacked' })).toBeChecked();
  // The saved sample text, not the list's.
  await expect(dialog.locator('.sample').first()).toHaveText('Quarterly results');
  await expect(dialog.getByRole('textbox', { name: 'Comparison name' })).toHaveValue(
    'Headline shortlist',
  );
});
