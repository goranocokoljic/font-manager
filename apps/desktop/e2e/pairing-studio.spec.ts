import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';

// Layout blocks load face files from `/fmfont/<id>`; serve one fixture for all.
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
  await page.goto('/');
  await page.evaluate(() => localStorage.clear());
  await page.goto('/#/');
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
}

const modeButton = (page: Page, name: string) =>
  page.getByRole('group', { name: 'View mode' }).getByRole('button', { name, exact: true });
const presetPicker = (page: Page) => page.getByRole('combobox', { name: 'Preset' });
const openButton = (page: Page) =>
  page.getByRole('button', { name: 'Open in Pairing Studio', exact: true });
const studio = (page: Page) => page.getByRole('region', { name: /^Pairing Studio: / });
const studioPicker = (page: Page) => studio(page).getByRole('combobox', { name: 'Layout' });
const layout = (scope: Page | Locator, name: string) =>
  scope.getByRole('region', { name: `${name} layout` });
const blocks = (region: Locator) => region.getByTestId('layout-block');
const fileAction = (page: Page, name: string) =>
  page.getByRole('toolbar', { name: 'Layout file' }).getByRole('button', { name, exact: true });

test('Layout mode opens the layout and face in Pairing Studio; Close returns to the library as it was', async ({
  page,
}) => {
  await open(page);
  const tree = page.getByRole('tree', { name: 'Font list' });
  const mono = tree.getByRole('treeitem', { name: /^JetBrains Mono/ });
  await mono.click();
  await modeButton(page, 'Layout').click();
  await presetPicker(page).selectOption('columns');
  await expect(layout(page, 'Columns')).toContainText('JetBrains Mono');
  // In Layout mode with a face shown, the button is there.
  await expect(openButton(page)).toBeEnabled();
  const libraryUrl = page.url();

  await openButton(page).click();
  await expect(page).toHaveURL(/#\/studio\?layout=columns&face=\d+$/);
  const panel = studio(page);
  await expect(panel).toBeVisible();
  await expect(panel).toHaveAccessibleName(/^Pairing Studio: JetBrains Mono /);
  await expect(panel.getByTestId('studio-font')).toContainText('JetBrains Mono');

  // The layout's blocks are live text in the face, in the same components as Layout mode.
  const columns = layout(panel, 'Columns');
  await expect(columns).toHaveAttribute('data-state', 'ready');
  await expect(blocks(columns).first()).toBeVisible();
  const family = await blocks(columns)
    .first()
    .evaluate((el) => getComputedStyle(el).fontFamily);
  expect(family).toMatch(/^"?fm-face-\d+/);

  // The picker lists the layout files; picking one shows it in the panel.
  await expect(studioPicker(page)).toHaveValue('columns');
  const names = await studioPicker(page).locator('option').allTextContents();
  expect(names).toEqual(expect.arrayContaining(['Book', 'Columns', 'Quote']));
  await studioPicker(page).selectOption('quote');
  await expect(page).toHaveURL(/layout=quote/);
  await expect(layout(panel, 'Quote')).toContainText('JetBrains Mono');

  // Close goes back to the library: same URL, same mode, same preset and selection.
  await panel.getByRole('button', { name: 'Close', exact: true }).click();
  await expect(studio(page)).toBeHidden();
  expect(page.url()).toBe(libraryUrl);
  await expect(modeButton(page, 'Layout')).toHaveAttribute('aria-pressed', 'true');
  await expect(presetPicker(page)).toHaveValue('columns');
  await expect(layout(page, 'Columns')).toContainText('JetBrains Mono');
  await modeButton(page, 'List').click();
  await expect(openButton(page)).toHaveCount(0);
  await expect(tree.getByRole('treeitem', { name: /^JetBrains Mono/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
});

test('the palette opens Pairing Studio; edits save to the layout file; Escape closes', async ({
  page,
}) => {
  await open(page);
  await page.getByRole('tree', { name: 'Font list' }).getByRole('treeitem').first().click();
  await modeButton(page, 'Layout').click();
  await expect(layout(page, 'Book')).toBeVisible();
  const libraryUrl = page.url();

  await page.keyboard.press('ControlOrMeta+k');
  await page.getByRole('combobox', { name: 'Search commands' }).fill('Pairing Studio');
  await expect(
    page.getByRole('option', { name: /Open the layout in Pairing Studio/ }),
  ).toBeVisible();
  await page.keyboard.press('Enter');
  const panel = studio(page);
  await expect(panel).toBeVisible();
  await expect(page).toHaveURL(/#\/studio\?layout=book&face=\d+$/);

  // Editing goes through the same store as Layout mode: the draft, Save, the file.
  const book = layout(panel, 'Book');
  await blocks(book).first().dblclick();
  const box = book.getByRole('textbox', { name: 'Block 1 text' });
  await box.fill('Studio heading');
  // Escape in the text box drops the edit only; the panel stays open.
  await page.keyboard.press('Escape');
  await expect(panel).toBeVisible();
  await expect(blocks(book).first()).toHaveText('Chapter One');
  await blocks(book).first().dblclick();
  await box.fill('Studio heading');
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(fileAction(page, 'Save')).toBeEnabled();
  await fileAction(page, 'Save').click();
  await expect(page.getByRole('toolbar', { name: 'Layout file' })).toContainText('Saved.');

  // A block property edit uses the core's ranges, like Layout mode.
  await panel.getByRole('button', { name: /^Block 1 properties/ }).click();
  await expect(panel.getByRole('dialog', { name: 'Block 1 properties' })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(panel.getByRole('dialog', { name: 'Block 1 properties' })).toBeHidden();
  await expect(panel).toBeVisible();

  // Escape on the panel closes it; the library's Layout mode shows the saved file.
  await page.keyboard.press('Escape');
  await expect(studio(page)).toBeHidden();
  expect(page.url()).toBe(libraryUrl);
  await expect(modeButton(page, 'Layout')).toHaveAttribute('aria-pressed', 'true');
  await expect(blocks(layout(page, 'Book')).first()).toHaveText('Studio heading');
  await expect(fileAction(page, 'Save')).toBeDisabled();
});

test('a Pairing Studio opened by its URL closes to the library with its face selected', async ({
  page,
}) => {
  await open(page);
  // The library opened in List mode, so only the studio reads the layouts folder here.
  await page.goto('/#/studio?layout=quote&face=1');
  await expect(layout(studio(page), 'Quote')).toBeVisible();
  // The picker lists the files from `list_layouts`, not only the presets: the mock's
  // unreadable `draft` file is there, and cannot be picked.
  const broken = studioPicker(page).getByRole('option', { name: 'draft (cannot be read)' });
  await expect(broken).toHaveCount(1);
  await expect(broken).toHaveAttribute('disabled', '');
  await page.getByRole('button', { name: 'Close', exact: true }).click();
  await expect(page).toHaveURL(/#\/\?face=1$/);
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
});

test('every Pairing Studio control is reachable with Tab and has a label', async ({ page }) => {
  await open(page);
  await page.goto('/#/studio?layout=book&face=1');
  const panel = studio(page);
  await expect(layout(panel, 'Book')).toHaveAttribute('data-state', 'ready');
  await expect(panel.getByRole('heading', { name: 'Pairing Studio' })).toBeFocused();

  const reached: string[] = [];
  // Tab through the panel until the focus leaves the page.
  for (let i = 0; i < 30; i++) {
    await page.keyboard.press('Tab');
    const name = await page.evaluate(() => {
      const el = document.activeElement as HTMLElement | null;
      if (!el || el === document.body) return null;
      const labelled = el.getAttribute('aria-labelledby');
      const byId = labelled ? document.getElementById(labelled)?.textContent : null;
      const label = (el as HTMLSelectElement).labels?.[0]?.textContent;
      return (el.getAttribute('aria-label') ?? byId ?? label ?? el.textContent ?? '').trim();
    });
    if (name === null) break;
    expect(name, `tab stop ${i + 1} has a label`).not.toBe('');
    reached.push(name);
  }
  const expected = [/^Layout/, /^Close$/, /^Duplicate$/, /^History$/, /^Book layout$/];
  expected.push(/^Lock width$/, /^Block 1 properties/, /^Block 1/);
  for (const name of expected) expect(reached.some((r) => name.test(r))).toBe(true);
});
