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
  expected.push(/^Lock width$/, /^Block 1 properties/, /^Font for Block 1$/, /^Block 1/);
  expected.push(/^Font for Block 2$/);
  for (const name of expected) expect(reached.some((r) => name.test(r))).toBe(true);
});

test('a block gets a face of its own from its font picker; "Use panel font" resets it', async ({
  page,
}) => {
  await open(page);
  await page.goto('/#/studio?layout=book&face=1');
  const panel = studio(page);
  const book = layout(panel, 'Book');
  await expect(book).toHaveAttribute('data-state', 'ready');
  const heading = blocks(book).nth(0);
  const body = blocks(book).nth(1);
  const familyOf = (block: Locator) => block.evaluate((el) => getComputedStyle(el).fontFamily);
  // Both blocks start in the panel's face.
  expect(await familyOf(heading)).toBe(await familyOf(body));

  // Keyboard only: Tab from the heading's size badge reaches its font picker.
  await panel.getByRole('button', { name: /^Block 1 properties/ }).focus();
  await page.keyboard.press('Tab');
  const picker = panel.getByRole('combobox', { name: 'Font for Block 1' });
  await expect(picker).toBeFocused();
  await expect(panel.getByRole('combobox', { name: 'Font for Block 2' })).toBeVisible();
  await expect(picker).toHaveAttribute('aria-expanded', 'true');

  // The list comes from the library search: typing narrows it.
  await page.keyboard.type('JetBrains');
  const list = panel.getByRole('listbox', { name: 'Fonts' });
  await expect(list.getByRole('option')).toHaveText(['JetBrains Mono']);
  await page.keyboard.press('Enter');
  await expect(picker).toHaveAttribute('aria-expanded', 'false');
  await expect(picker).toHaveValue(/^JetBrains Mono /);

  // The heading is set in its own face, as live text; the body keeps the panel's face.
  await expect.poll(() => familyOf(heading)).not.toBe(await familyOf(body));
  expect(await familyOf(heading)).toMatch(/^"?fm-face-\d+/);
  expect(await familyOf(body)).toMatch(/^"?fm-face-1"?,/);
  await expect(book).toHaveAttribute('data-state', 'ready');

  // Escape in an open list closes the list only, not the studio.
  await picker.focus();
  await page.keyboard.press('ArrowDown');
  await expect(picker).toHaveAttribute('aria-expanded', 'true');
  await page.keyboard.press('Escape');
  await expect(picker).toHaveAttribute('aria-expanded', 'false');
  await expect(panel).toBeVisible();

  // The choice is not part of the layout file: nothing to save.
  await expect(fileAction(page, 'Save')).toBeDisabled();

  // "Use panel font" puts the heading back in the panel's face.
  const reset = panel.getByRole('button', { name: 'Use panel font for Block 1' });
  await reset.focus();
  await page.keyboard.press('Enter');
  await expect.poll(() => familyOf(heading)).toBe(await familyOf(body));
  await expect(reset).toHaveCount(0);
  await expect(picker).toHaveValue('');
});

const panels = (page: Page) => studio(page).getByRole('region', { name: /^Panel \d: / });
const panelNamed = (page: Page, name: string) =>
  studio(page).getByRole('region', { name, exact: true });
const addPanel = (page: Page) => studio(page).getByRole('button', { name: 'Add panel' });

/** Sets block 1 of `panel` in `family` with the keyboard, through its font picker. */
async function headingFont(page: Page, panel: Locator, family: string) {
  const picker = panel.getByRole('combobox', { name: 'Font for Block 1' });
  await picker.focus();
  await page.keyboard.type(family);
  await expect(
    panel.getByRole('listbox', { name: 'Fonts' }).getByRole('option').first(),
  ).toHaveText(family);
  await page.keyboard.press('Enter');
  await expect(picker).toHaveValue(new RegExp(`^${family} `));
}

test('panels side by side: copies, independent, close one, kept for the session', async ({
  page,
}) => {
  await open(page);
  await page.goto('/#/studio?layout=book&face=1');
  await expect(panels(page)).toHaveCount(1);
  const first = panelNamed(page, 'Panel 1: Book');
  await expect(layout(first, 'Book')).toHaveAttribute('data-state', 'ready');
  // The first panel is the library's layout: it has no "Close panel" of its own.
  await expect(first.getByRole('button', { name: /^Close panel/ })).toHaveCount(0);

  // Panel 1: an unsaved heading and a heading font of its own.
  await blocks(layout(first, 'Book')).first().dblclick();
  await first.getByRole('textbox', { name: 'Block 1 text' }).fill('Copied heading');
  await page.keyboard.press('ControlOrMeta+Enter');
  await headingFont(page, first, 'Inter');
  const familyOf = (panel: Locator) =>
    blocks(layout(panel, 'Book'))
      .first()
      .evaluate((el) => getComputedStyle(el).fontFamily);
  const inter = await familyOf(first);

  // "Add panel" opens a copy beside it: same layout, same text, same block fonts.
  await addPanel(page).click();
  await expect(panels(page)).toHaveCount(2);
  const second = panelNamed(page, 'Panel 2: Book');
  await expect(second.getByRole('heading', { name: 'Panel 2: Book' })).toBeFocused();
  await expect(blocks(layout(second, 'Book')).first()).toHaveText('Copied heading');
  await expect(second.getByRole('combobox', { name: 'Font for Block 1' })).toHaveValue(/^Inter /);
  await expect.poll(() => familyOf(second)).toBe(inter);

  // Each panel is independent: font and text edits in panel 2 leave panel 1 as it was.
  await headingFont(page, second, 'JetBrains Mono');
  await expect.poll(() => familyOf(second)).not.toBe(inter);
  const mono = await familyOf(second);
  expect(await familyOf(first)).toBe(inter);
  await blocks(layout(second, 'Book')).first().dblclick();
  await second.getByRole('textbox', { name: 'Block 1 text' }).fill('Only panel 2');
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(blocks(layout(second, 'Book')).first()).toHaveText('Only panel 2');
  await expect(blocks(layout(first, 'Book')).first()).toHaveText('Copied heading');

  // A third panel copies the panel that had the focus last (panel 2), then gets its own font.
  await addPanel(page).click();
  await expect(panels(page)).toHaveCount(3);
  const third = panelNamed(page, 'Panel 3: Book');
  await expect(blocks(layout(third, 'Book')).first()).toHaveText('Only panel 2');
  await headingFont(page, third, 'Legacy Grotesk');
  await expect.poll(() => familyOf(third)).not.toBe(mono);
  const legacy = await familyOf(third);
  expect(new Set([inter, mono, legacy]).size).toBe(3);

  // Equal widths.
  const widths = await panels(page).evaluateAll((els) =>
    els.map((el) => el.getBoundingClientRect().width),
  );
  for (const w of widths) expect(Math.abs(w - widths[0]!)).toBeLessThanOrEqual(1);

  // "Close panel" removes one; the others move up and keep their fonts and text.
  await second.getByRole('button', { name: 'Close panel 2' }).click();
  await expect(panels(page)).toHaveCount(2);
  await expect(addPanel(page)).toBeFocused();
  const kept = panelNamed(page, 'Panel 2: Book');
  await expect.poll(() => familyOf(kept)).toBe(legacy);
  expect(await familyOf(first)).toBe(inter);

  // Back to the library and into the studio again: the panels are as they were.
  await studio(page).getByRole('button', { name: 'Close', exact: true }).click();
  await expect(studio(page)).toBeHidden();
  await page.evaluate(() => (location.hash = '#/studio?layout=book&face=1'));
  await expect(panels(page)).toHaveCount(2);
  await expect(blocks(layout(panelNamed(page, 'Panel 1: Book'), 'Book')).first()).toHaveText(
    'Copied heading',
  );
  const again = panelNamed(page, 'Panel 2: Book');
  await expect(blocks(layout(again, 'Book')).first()).toHaveText('Only panel 2');
  await expect.poll(() => familyOf(again)).toBe(legacy);

  // Up to four panels; at four, "Add panel" says so and adds none.
  await addPanel(page).click();
  await addPanel(page).click();
  await expect(panels(page)).toHaveCount(4);
  await expect(addPanel(page)).toHaveAttribute('aria-disabled', 'true');
  // It stays on the Tab path, so its title tells why; pressing it does nothing.
  await addPanel(page).focus();
  await page.keyboard.press('Enter');
  await expect(panels(page)).toHaveCount(4);
});

test('a narrow window scrolls the panels sideways; every panel control is on the Tab path', async ({
  page,
}) => {
  await open(page);
  await page.setViewportSize({ width: 1000, height: 700 });
  await page.goto('/#/studio?layout=book&face=1');
  await expect(layout(studio(page), 'Book')).toHaveAttribute('data-state', 'ready');
  await addPanel(page).click();
  await addPanel(page).click();
  await expect(panels(page)).toHaveCount(3);
  const row = panels(page).first().locator('..');
  const overflow = await row.evaluate((el) => el.scrollWidth - el.clientWidth);
  expect(overflow).toBeGreaterThan(0);
  await expect(row).toHaveCSS('overflow-x', 'auto');

  await studio(page).getByRole('heading', { name: 'Pairing Studio' }).focus();
  const reached: string[] = [];
  for (let i = 0; i < 80; i++) {
    await page.keyboard.press('Tab');
    const name = await page.evaluate(() => {
      const el = document.activeElement as HTMLElement | null;
      if (!el || el === document.body) return null;
      const label = (el as HTMLSelectElement).labels?.[0]?.textContent;
      return (el.getAttribute('aria-label') ?? label ?? el.textContent ?? '').trim();
    });
    if (name === null) break;
    expect(name, `tab stop ${i + 1} has a label`).not.toBe('');
    reached.push(name);
  }
  const count = (re: RegExp) => reached.filter((r) => re.test(r)).length;
  expect(count(/^Add panel$/)).toBe(1);
  expect(count(/^Close panel 2$/)).toBe(1);
  expect(count(/^Close panel 3$/)).toBe(1);
  expect(count(/^Layout/)).toBe(3);
  expect(count(/^Font for Block 1$/)).toBe(3);
  expect(count(/^Duplicate$/)).toBe(3);
});
