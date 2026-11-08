import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { readPdf } from './pdf-text';
import { optionLabels, selectOption } from './select';

// Layout blocks load face files from `/fmfont/<id>`; serve one fixture for all.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);
const staticFixture = readFileSync(
  fileURLToPath(
    new URL('../../../fixtures/fonts/fontshare/public-sans/PublicSans-Bold.ttf', import.meta.url),
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
const bar = (page: Page) => page.getByTestId('studio-toolbar');
const barButton = (page: Page, name: string) =>
  bar(page).getByRole('button', { name, exact: true });
const contexts = (page: Page) => bar(page).getByRole('group', { name: 'Context' });
const contextButton = (page: Page, name: string) =>
  contexts(page).getByRole('button', { name, exact: true });
const pairing = (page: Page) => bar(page).getByTestId('studio-pairing');
const status = (page: Page) => studio(page).getByTestId('studio-status');
const layout = (scope: Page | Locator, name: string) =>
  scope.getByRole('region', { name: `${name} layout` });
const blocks = (region: Locator) => region.getByTestId('layout-block');
const sheet = (region: Locator) => region.getByTestId('layout-page');
const fileAction = (page: Page, name: string) =>
  page.getByRole('toolbar', { name: 'Layout file' }).getByRole('button', { name, exact: true });

test('Layout mode opens the layout and face in Pairing Studio; Back returns to the library as it was', async ({
  page,
}) => {
  await open(page);
  const tree = page.getByRole('tree', { name: 'Font list' });
  const mono = tree.getByRole('treeitem', { name: /^JetBrains Mono/ });
  await mono.click();
  await modeButton(page, 'Layout').click();
  await selectOption(presetPicker(page), 'columns');
  await expect(layout(page, 'Columns')).toContainText('JetBrains Mono');
  // In Layout mode with a face shown, the button is there.
  await expect(openButton(page)).toBeEnabled();
  const libraryUrl = page.url();

  await openButton(page).click();
  await expect(page).toHaveURL(/#\/studio\?layout=columns&face=\d+$/);
  const panel = studio(page);
  await expect(panel).toBeVisible();
  await expect(panel).toHaveAccessibleName(/^Pairing Studio: JetBrains Mono /);
  // No headline face of its own: the pairing is the panel's face twice.
  await expect(pairing(page)).toHaveText('JetBrains Mono + JetBrains Mono');

  // The layout's blocks are live text in the face, in the same components as Layout mode.
  const columns = layout(panel, 'Columns');
  await expect(columns).toHaveAttribute('data-state', 'ready');
  await expect(blocks(columns).first()).toBeVisible();
  const family = await blocks(columns)
    .first()
    .evaluate((el) => getComputedStyle(el).fontFamily);
  expect(family).toMatch(/^"?fm-face-\d+/);

  // Columns is not a context: the switch shows none pressed. Picking one shows its file.
  for (const b of await contexts(page).getByRole('button').all())
    await expect(b).toHaveAttribute('aria-pressed', 'false');
  await contextButton(page, 'Web hero').click();
  await expect(page).toHaveURL(/layout=studio-web-hero/);
  await expect(contextButton(page, 'Web hero')).toHaveAttribute('aria-pressed', 'true');
  await expect(sheet(layout(panel, 'Web hero'))).toHaveAttribute('data-page-width', '1440');
  await expect(pairing(page)).toHaveText('JetBrains Mono + JetBrains Mono');

  // Back goes to the library: same URL, same mode, same preset and selection.
  await barButton(page, 'Back').click();
  await expect(studio(page)).toBeHidden();
  expect(page.url()).toBe(libraryUrl);
  await expect(modeButton(page, 'Layout')).toHaveAttribute('aria-pressed', 'true');
  await expect(presetPicker(page)).toHaveAttribute('data-value', 'columns');
  await expect(layout(page, 'Columns')).toContainText('JetBrains Mono');
  await modeButton(page, 'List').click();
  await expect(openButton(page)).toHaveCount(0);
  await expect(tree.getByRole('treeitem', { name: /^JetBrains Mono/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
});

test('one toolbar: Back, title, pairing, the five contexts, Compare, Shuffle, Save, Export', async ({
  page,
}) => {
  await open(page);
  await page.goto('/#/studio?face=1');
  await expect(studio(page)).toBeVisible();
  // In this order, in one bar.
  const items = await bar(page)
    .locator('button, h1, [data-testid="studio-pairing"]')
    .evaluateAll((els) =>
      els.map((el) => (el.getAttribute('aria-label') ?? el.textContent)!.trim()),
    );
  expect(items).toEqual([
    'Back',
    'Pairing Studio',
    expect.stringMatching(/^.+ \+ .+$/),
    'Magazine',
    'Web hero',
    'Mobile UI',
    'Video title',
    'Poster',
    'Compare 2 pairings',
    'Shuffle unlocked',
    'Save',
    'Export PNG',
  ]);
  // One bar: the first panel has no bar of its own and the page has no head row.
  const bars = await studio(page)
    .locator('header')
    .evaluateAll((els) => els.filter((el) => el.getBoundingClientRect().height > 4).length);
  expect(bars).toBe(1);
  await expect(studio(page).getByRole('toolbar', { name: 'Layout file' })).toHaveCount(0);
  await expect(studio(page).getByRole('combobox', { name: 'Layout' })).toHaveCount(0);
  // All of the bar is on one row.
  const tops = await bar(page)
    .locator('button, h1')
    .evaluateAll((els) => els.map((el) => Math.round(el.getBoundingClientRect().top / 8)));
  expect(new Set(tops).size).toBe(1);
  // Shuffle works on the roles (E45.3).
  await expect(barButton(page, 'Shuffle unlocked')).toHaveAttribute('aria-disabled', 'false');
  await expect(barButton(page, 'Shuffle unlocked')).toHaveAttribute(
    'title',
    /^Give every unlocked/,
  );
});

const CONTEXTS = [
  ['Magazine', 'Magazine page', 'studio-magazine', 794, 1123, 'A4', 76],
  ['Web hero', 'Web hero', 'studio-web-hero', 1440, 900, '1440 × 900', 120],
  ['Mobile UI', 'Mobile UI', 'studio-mobile-ui', 390, 844, '390 × 844', 16],
  ['Video title', 'Video title', 'studio-video-title', 1920, 1080, '1920 × 1080', 96],
  ['Poster', 'Poster', 'studio-poster', 794, 1123, 'A4', 57],
] as const;

test('each context is a layout file on its page at 100 %, on the neutral ground', async ({
  page,
}) => {
  await open(page);
  // Without a layout in the URL, the Studio opens on the Magazine context.
  await page.goto('/#/studio?face=1');
  const panel = studio(page);
  await expect(contextButton(page, 'Magazine')).toHaveAttribute('aria-pressed', 'true');

  for (const [button, name, id, width, height, pageName, margin] of CONTEXTS) {
    await contextButton(page, button).click();
    await expect(page).toHaveURL(new RegExp(`layout=${id}(&|$)`));
    await expect(contextButton(page, button)).toHaveAttribute('aria-pressed', 'true');
    const region = layout(panel, name);
    await expect(region).toHaveAttribute('data-state', 'ready');
    const page_ = sheet(region);
    await expect(page_).toHaveAttribute('data-page-width', String(width));
    await expect(page_).toHaveAttribute('data-page-height', String(height));
    // The real size in CSS pixels: 100 %.
    const box = (await page_.boundingBox())!;
    expect(Math.round(box.width)).toBe(width);
    expect(box.height).toBeGreaterThanOrEqual(height - 1);
    await expect(region.getByTestId('layout-sheet-label')).toHaveText(`${pageName} · 100 %`);
    // The text keeps the page's width inside its margins.
    const row = await blocks(region)
      .first()
      .evaluate((el) => el.closest('.row')!.getBoundingClientRect().width);
    expect(Math.abs(row - (width - 2 * margin))).toBeLessThanOrEqual(1);
    // A page has no width lock: its size decides.
    await expect(region.getByTestId('layout-width-lock')).toHaveCount(0);
    // Safe-area guides on the video frame only.
    await expect(region.getByTestId('layout-safe-area')).toHaveCount(
      button === 'Video title' ? 2 : 0,
    );
  }
});

test('the page sits on #ecebe7 with a soft shadow; the hint under it says hover, click, drag', async ({
  page,
}) => {
  await open(page);
  await page.goto('/#/studio?face=1');
  const region = layout(studio(page), 'Magazine page');
  await expect(region).toHaveAttribute('data-state', 'ready');
  await expect(region).toHaveCSS('background-color', 'rgb(236, 235, 231)');
  await expect(sheet(region)).toHaveCSS('background-color', 'rgb(255, 255, 255)');
  await expect(sheet(region)).not.toHaveCSS('box-shadow', 'none');
  const hint = region.getByTestId('layout-sheet-hint');
  await expect(hint).toHaveText(
    'Hover a block to see its role · click to select it and edit in the rail · drag sideways for size, up and down for leading · Enter edits the text',
  );
  // The hint is under the page.
  const pageBox = (await sheet(region).boundingBox())!;
  const hintBox = (await hint.boundingBox())!;
  expect(hintBox.y).toBeGreaterThanOrEqual(pageBox.y + pageBox.height - 1);

  // A block's controls stay off the page until the pointer or the focus is on the block.
  const badge = region.getByTestId('block-size-badge').nth(1);
  const tools = badge.locator('xpath=ancestor::div[contains(@class, "tools")][1]');
  await expect(tools).toHaveCSS('opacity', '0');
  await blocks(region).nth(1).hover();
  await expect(tools).toHaveCSS('opacity', '1');
  const badgeBox = (await badge.boundingBox())!;
  expect(badgeBox.x + badgeBox.width).toBeLessThanOrEqual(pageBox.x);
  await page.mouse.move(0, 0);
  await expect(tools).toHaveCSS('opacity', '0');
  await badge.focus();
  await expect(tools).toHaveCSS('opacity', '1');

  // Dark: the ground is surface-2.
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect(region).toHaveCSS('background-color', 'rgb(38, 38, 40)');
});

test('Mobile UI uses the iOS text styles; Video title draws title and action safe', async ({
  page,
}) => {
  await open(page);
  await page.goto('/#/studio?layout=studio-mobile-ui&face=1');
  const mobile = layout(studio(page), 'Mobile UI');
  await expect(mobile).toHaveAttribute('data-state', 'ready');
  // Large Title 34/41, Headline 17/22, Body 17/22, Footnote 13/18 on the 390 px page.
  const sizes = await blocks(mobile).evaluateAll((els) =>
    els.map((el) => {
      const s = getComputedStyle(el);
      return [Math.round(parseFloat(s.fontSize)), Math.round(parseFloat(s.lineHeight))];
    }),
  );
  expect(sizes).toEqual([
    [34, 41],
    [17, 22],
    [17, 22],
    [13, 18],
  ]);

  await contextButton(page, 'Video title').click();
  const video = layout(studio(page), 'Video title');
  await expect(video).toHaveAttribute('data-state', 'ready');
  const frame = (await sheet(video).boundingBox())!;
  const guides = video.getByTestId('layout-safe-area');
  const insets = await guides.evaluateAll((els) =>
    els.map((el) => {
      const r = el.getBoundingClientRect();
      const p = el.parentElement!.getBoundingClientRect();
      return [Math.round(r.left - p.left), Math.round(r.top - p.top)];
    }),
  );
  // Action safe 3.5 %, title safe 5 % of 1920 × 1080.
  expect(insets).toEqual([
    [67, 38],
    [96, 54],
  ]);
  expect(frame.width).toBe(1920);
  await expect(guides.first()).toHaveAttribute('aria-hidden', 'true');
});

test('the palette opens Pairing Studio; Save and Export in the toolbar; Escape closes', async ({
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
  await expect(barButton(page, 'Save')).toHaveAttribute('aria-disabled', 'true');
  await expect(pairing(page)).not.toContainText('unsaved');

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
  await expect(pairing(page)).toContainText('· unsaved');
  // The PNG is made from the saved file: Export asks for a save first.
  await expect(barButton(page, 'Export PNG')).toHaveAttribute('aria-disabled', 'true');
  await expect(barButton(page, 'Export PNG')).toHaveAttribute('title', /^Save first/);
  // It stays on the Tab path, so its title tells why; pressing it does nothing.
  await barButton(page, 'Export PNG').focus();
  await page.keyboard.press('Enter');
  await expect(status(page)).toHaveText('');
  await expect(barButton(page, 'Save')).toHaveAttribute('aria-disabled', 'false');
  await barButton(page, 'Save').click();
  await expect(status(page)).toHaveText('Saved.');
  await expect(pairing(page)).not.toContainText('unsaved');
  await expect(barButton(page, 'Save')).toHaveAttribute('aria-disabled', 'true');

  // Saved: Export renders the file through the core.
  await expect(barButton(page, 'Export PNG')).toHaveAttribute('aria-disabled', 'false');
  await barButton(page, 'Export PNG').click();
  await expect(status(page)).toHaveText('Exported to /mock/exports/book-paper-2x.png');

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
  // The compared panel's picker lists the files from `list_layouts`, not only the presets:
  // the contexts and the mock's unreadable `draft` file are there; `draft` cannot be picked.
  await barButton(page, 'Compare 2 pairings').click();
  const picker = studio(page).getByRole('combobox', { name: 'Layout' });
  const names = await optionLabels(picker);
  expect(names).toEqual(expect.arrayContaining(['Book', 'Quote', 'Magazine page', 'Web hero']));
  await picker.click();
  const broken = page
    .locator('.ui-popover')
    .getByRole('option', { name: 'draft (cannot be read)' });
  await expect(broken).toHaveCount(1);
  await expect(broken).toHaveAttribute('aria-disabled', 'true');
  await picker.press('Escape');
  await barButton(page, 'Back').click();
  await expect(page).toHaveURL(/#\/\?face=1$/);
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
});

/** The names of the Tab stops after the focused element, until the focus leaves the page. */
async function tabStops(page: Page, max: number): Promise<string[]> {
  const reached: string[] = [];
  for (let i = 0; i < max; i++) {
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
  return reached;
}

test('every Pairing Studio control is reachable with Tab and has a label', async ({ page }) => {
  await open(page);
  await page.goto('/#/studio?layout=book&face=1');
  const panel = studio(page);
  await expect(layout(panel, 'Book')).toHaveAttribute('data-state', 'ready');
  const heading = panel.getByRole('heading', { name: 'Pairing Studio' });
  await expect(heading).toBeFocused();
  // Back is just before the title.
  await page.keyboard.press('Shift+Tab');
  await expect(barButton(page, 'Back')).toBeFocused();
  await heading.focus();

  const reached = await tabStops(page, 30);
  const expected = [/^Magazine$/, /^Web hero$/, /^Mobile UI$/, /^Video title$/, /^Poster$/];
  expected.push(/^Compare 2 pairings$/, /^Shuffle unlocked$/, /^Save$/, /^Export PNG$/);
  expected.push(/^Book layout$/, /^Lock width$/, /^Block 1 properties/, /^Font for Block 1$/);
  expected.push(/^Block 1/, /^Font for Block 2$/);
  for (const name of expected)
    expect(
      reached.some((r) => name.test(r)),
      String(name),
    ).toBe(true);
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
  const panelFamily = (await pairing(page).textContent())!.split(' + ')[1]!;

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
  // The toolbar names the pairing: the headline block's face + the panel's.
  await expect(pairing(page)).toHaveText(`JetBrains Mono + ${panelFamily}`);

  // Escape in an open list closes the list only, not the studio.
  await picker.focus();
  await page.keyboard.press('ArrowDown');
  await expect(picker).toHaveAttribute('aria-expanded', 'true');
  await page.keyboard.press('Escape');
  await expect(picker).toHaveAttribute('aria-expanded', 'false');
  await expect(panel).toBeVisible();

  // The choice is not part of the layout file: nothing to save.
  await expect(barButton(page, 'Save')).toHaveAttribute('aria-disabled', 'true');

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
const compare = (page: Page) => barButton(page, 'Compare 2 pairings');

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

test('Compare 2 pairings: a copy beside the first, independent, closed again, kept for the session', async ({
  page,
}) => {
  await open(page);
  await page.goto('/#/studio?layout=book&face=1');
  await expect(panels(page)).toHaveCount(1);
  const first = panelNamed(page, 'Panel 1: Book');
  await expect(layout(first, 'Book')).toHaveAttribute('data-state', 'ready');
  await expect(compare(page)).toHaveAttribute('aria-pressed', 'false');
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

  // Compare opens a copy beside it: same layout, same text, same block fonts.
  await compare(page).click();
  await expect(compare(page)).toHaveAttribute('aria-pressed', 'true');
  await expect(panels(page)).toHaveCount(2);
  const second = panelNamed(page, 'Panel 2: Book');
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

  // Equal widths.
  const widths = await panels(page).evaluateAll((els) =>
    els.map((el) => el.getBoundingClientRect().width),
  );
  expect(Math.abs(widths[1]! - widths[0]!)).toBeLessThanOrEqual(1);

  // Back to the library and into the studio again: the panels are as they were.
  await barButton(page, 'Back').click();
  await expect(studio(page)).toBeHidden();
  await page.evaluate(() => (location.hash = '#/studio?layout=book&face=1'));
  await expect(panels(page)).toHaveCount(2);
  await expect(blocks(layout(panelNamed(page, 'Panel 1: Book'), 'Book')).first()).toHaveText(
    'Copied heading',
  );
  const again = panelNamed(page, 'Panel 2: Book');
  await expect(blocks(layout(again, 'Book')).first()).toHaveText('Only panel 2');
  await expect.poll(() => familyOf(again)).toBe(mono);

  // "Close panel 2" ends the comparison; the focus goes to Compare, which brings a fresh copy.
  await again.getByRole('button', { name: 'Close panel 2' }).click();
  await expect(panels(page)).toHaveCount(1);
  await expect(compare(page)).toBeFocused();
  await expect(compare(page)).toHaveAttribute('aria-pressed', 'false');
  expect(await familyOf(first)).toBe(inter);
  await page.keyboard.press('Enter');
  await expect(panels(page)).toHaveCount(2);
  await expect.poll(() => familyOf(panelNamed(page, 'Panel 2: Book'))).toBe(inter);
  // Pressed again, Compare puts the first panel back alone.
  await compare(page).click();
  await expect(panels(page)).toHaveCount(1);
});

test('with two pairings every panel control is on the Tab path', async ({ page }) => {
  await open(page);
  await page.setViewportSize({ width: 1000, height: 700 });
  await page.goto('/#/studio?layout=book&face=1');
  await expect(layout(studio(page), 'Book')).toHaveAttribute('data-state', 'ready');
  await compare(page).click();
  await expect(panels(page)).toHaveCount(2);
  const row = panels(page).first().locator('..');
  await expect(row).toHaveCSS('overflow-x', 'auto');

  await studio(page).getByRole('heading', { name: 'Pairing Studio' }).focus();
  const reached = await tabStops(page, 80);
  const count = (re: RegExp) => reached.filter((r) => re.test(r)).length;
  expect(count(/^Compare 2 pairings$/)).toBe(1);
  expect(count(/^Close panel 2$/)).toBe(1);
  expect(count(/^Layout/)).toBe(1);
  expect(count(/^Font for Block 1$/)).toBe(2);
  expect(count(/^Duplicate$/)).toBe(1);
  expect(count(/^Print panel 2$/)).toBe(1);
});

test('"Print..." prints one panel alone, as live text in its fonts, and the PDF holds its text', async ({
  page,
}) => {
  // Browser mode prints through window.print(); count the calls instead of opening a dialog.
  await page.addInitScript(() => {
    const w = window as unknown as { fmPrints: number };
    w.fmPrints = 0;
    window.print = () => {
      w.fmPrints++;
    };
  });
  await open(page);
  // A static font: Chromium's PDF embeds it with its PostScript name (a variable font
  // becomes an unnamed Type 3 font), so the PDF can show which font set the text.
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: staticFixture }),
  );
  // A fresh document, so no face is still loaded from the other fixture.
  await page.goto('about:blank');
  await page.goto('/#/studio?layout=book&face=1');
  const first = panelNamed(page, 'Panel 1: Book');
  await expect(layout(first, 'Book')).toHaveAttribute('data-state', 'ready');
  await compare(page).click();
  const second = panelNamed(page, 'Panel 2: Book');
  await expect(layout(second, 'Book')).toHaveAttribute('data-state', 'ready');
  // Text that only one panel has, to tell the printed panel from the other on paper.
  const setHeading = async (panel: Locator, text: string) => {
    await blocks(layout(panel, 'Book')).first().dblclick();
    await panel.getByRole('textbox', { name: 'Block 1 text' }).fill(text);
    await page.keyboard.press('ControlOrMeta+Enter');
    await expect(blocks(layout(panel, 'Book')).first()).toHaveText(text);
  };
  await setHeading(first, 'Left out of print');
  await setHeading(second, 'Printed panel two');

  // The button has a label and is on the Tab path right after the panel's layout picker.
  const print = second.getByRole('button', { name: 'Print panel 2', exact: true });
  await expect(print).toHaveAttribute('title', /Print/);
  await second.getByRole('combobox', { name: 'Layout' }).focus();
  await page.keyboard.press('Tab');
  await expect(print).toBeFocused();
  await page.keyboard.press('Enter');
  await expect
    .poll(() => page.evaluate(() => (window as unknown as { fmPrints: number }).fmPrints))
    .toBe(1);
  await expect(second).toHaveAttribute('data-print-target', '');
  await expect(first).not.toHaveAttribute('data-print-target');

  // Under print media only that panel's layout shows: no app chrome, no other panel.
  await page.emulateMedia({ media: 'print' });
  await expect(first).toBeHidden();
  await expect(compare(page)).toBeHidden();
  await expect(studio(page).getByRole('heading', { name: 'Pairing Studio' })).toBeHidden();
  await expect(print).toBeHidden();
  await expect(second.getByRole('heading', { name: 'Panel 2: Book' })).toBeHidden();
  await expect(second.getByRole('toolbar', { name: 'Layout file' })).toBeHidden();
  await expect(second.getByTestId('block-size-badge').first()).toBeHidden();
  await expect(second.getByTestId('layout-width-lock')).toBeHidden();
  const printed = blocks(layout(second, 'Book'));
  await expect(printed.first()).toBeVisible();
  // Live text in the face, at the layout's locked width.
  expect(await printed.first().evaluate((el) => getComputedStyle(el).fontFamily)).toMatch(
    /^"?fm-face-\d+/,
  );
  const width = await layout(second, 'Book')
    .getByTestId('layout-page')
    .evaluate((el) => parseFloat(getComputedStyle(el).getPropertyValue('--fm-layout-width')));
  expect(width).toBeGreaterThan(0);
  const rowWidth = await printed
    .first()
    .evaluate((el) => el.closest('.row')!.getBoundingClientRect().width);
  expect(Math.abs(rowWidth - width)).toBeLessThanOrEqual(1);

  // The PDF of that print holds the panel's text, set in the embedded layout font.
  await page.evaluate(() => document.fonts.ready);
  const pdf = readPdf(await page.pdf({ format: 'A4' }));
  const squash = (s: string) => s.replace(/\s+/g, '');
  expect(squash(pdf.text)).toContain(squash('Printed panel two'));
  expect(squash(pdf.text)).not.toContain(squash('Left out of print'));
  expect(squash(pdf.text)).not.toContain(squash('Compare 2 pairings'));
  expect(squash(pdf.text)).not.toContain(squash('Print…'));
  const heading = pdf.runs.find((r) => squash(r.text).includes(squash('Printed panel two')));
  expect(heading?.font).toMatch(/^[A-Z]{6}\+PublicSans-Bold$/);
  expect(pdf.fonts).toContain(heading?.font);

  // On screen nothing changed; the first panel prints alone from the command palette.
  await page.emulateMedia({ media: 'screen' });
  await expect(first).toBeVisible();
  await page.keyboard.press('ControlOrMeta+k');
  await page.getByRole('combobox', { name: 'Search commands' }).fill('Print the Studio page');
  await expect(page.getByRole('option', { name: /Print the Studio page/ })).toBeVisible();
  await page.keyboard.press('Enter');
  await expect
    .poll(() => page.evaluate(() => (window as unknown as { fmPrints: number }).fmPrints))
    .toBe(2);
  await page.emulateMedia({ media: 'print' });
  await expect(second).toBeHidden();
  await expect(blocks(layout(first, 'Book')).first()).toBeVisible();
});

// ---- E45.2 roles rail ------------------------------------------------------------------

const rail = (page: Page) => studio(page).getByTestId('studio-roles');
const roleCards = (page: Page) => rail(page).getByTestId('studio-role');
const roleCard = (page: Page, role: string) =>
  roleCards(page).filter({ has: page.getByRole('heading', { name: role, exact: true }) });
const rolePicker = (page: Page, role: string) =>
  rail(page).getByRole('combobox', { name: `Font for ${role}`, exact: true });
const familyOf = (block: Locator) => block.evaluate((el) => getComputedStyle(el).fontFamily);
/** The menu of an open role picker: the listbox the combobox controls. */
const menuOf = async (picker: Locator) =>
  picker.page().locator(`[id="${await picker.getAttribute('aria-controls')}"]`);

/** Opens role `role`'s picker, searches `text` and takes the family `name`. */
async function pickRoleFont(page: Page, role: string, text: string, name: string) {
  await rolePicker(page, role).click();
  await expect(rolePicker(page, role)).toHaveAttribute('aria-expanded', 'true');
  const menu = await menuOf(rolePicker(page, role));
  await expect(menu).toBeVisible();
  await page.keyboard.type(text);
  await menu.getByRole('option', { name, exact: true }).click();
  await expect(menu).toBeHidden();
}

test('a 300 px rail lists the context roles as cards: name, lock, font picker, style and size', async ({
  page,
}) => {
  await open(page);
  await page.goto('/#/studio?face=1');
  const region = layout(studio(page), 'Magazine page');
  await expect(region).toHaveAttribute('data-state', 'ready');
  await expect(rail(page)).toBeVisible();
  const railBox = (await rail(page).boundingBox())!;
  expect(Math.round(railBox.width)).toBe(300);
  // The rail is left of the page.
  expect(railBox.x).toBeLessThan((await sheet(region).boundingBox())!.x);
  await expect(roleCards(page).getByRole('heading')).toHaveText([
    'Headline',
    'Deck',
    'Body',
    'Caption',
  ]);
  // No per-block pickers beside the page: the rail has them.
  await expect(region.getByTestId('block-font')).toHaveCount(0);

  const headline = roleCard(page, 'Headline');
  // The style of the face and the size / leading of the role's block: 40 pt on 1.05.
  await expect(headline.getByTestId('studio-role-detail')).toHaveText(/^\S.* · 40 pt · 1\.05$/);

  // The lock toggles, per role.
  const lock = headline.getByRole('button', { name: 'Lock Headline' });
  await expect(lock).toHaveAttribute('aria-pressed', 'false');
  await lock.click();
  await expect(lock).toHaveAttribute('aria-pressed', 'true');
  await expect(lock).toHaveAttribute('title', /^Locked/);
  await expect(roleCard(page, 'Body').getByRole('button', { name: 'Lock Body' })).toHaveAttribute(
    'aria-pressed',
    'false',
  );

  // The picker searches the library; each family is set in itself.
  const picker = rolePicker(page, 'Headline');
  await picker.click();
  await expect(picker).toHaveAttribute('aria-expanded', 'true');
  const menu = await menuOf(picker);
  await expect(menu).toBeVisible();
  await expect(menu.getByRole('option').nth(1)).toBeVisible();
  const optionFamilies = await menu
    .locator('.label')
    .evaluateAll((els) => els.map((el) => (el as HTMLElement).style.fontFamily));
  expect(optionFamilies.length).toBeGreaterThan(1);
  for (const f of optionFamilies) expect(f).toMatch(/fm-face-\d+/);
  await page.keyboard.type('mono');
  await expect(menu.getByRole('option', { name: 'JetBrains Mono', exact: true })).toBeVisible();
  const before = await familyOf(blocks(region).nth(1));
  await menu.getByRole('option', { name: 'JetBrains Mono', exact: true }).click();
  await expect(picker).toHaveText('JetBrains Mono');
  await expect(picker).toHaveAttribute('aria-expanded', 'false');
  await expect.poll(() => familyOf(blocks(region).nth(1))).not.toBe(before);
  await expect(pairing(page)).toHaveText(/^JetBrains Mono \+ /);
  // The button shows the family in itself too.
  expect(
    await picker.locator('.value').evaluate((el) => (el as HTMLElement).style.fontFamily),
  ).toMatch(/fm-face-\d+/);

  // Caption sets both of its blocks: the kicker and the caption.
  await pickRoleFont(page, 'Caption', 'mono', 'JetBrains Mono');
  const mono = await familyOf(blocks(region).nth(1));
  await expect.poll(() => familyOf(blocks(region).nth(0))).toBe(mono);
  await expect.poll(() => familyOf(blocks(region).nth(4))).toBe(mono);
  expect(await familyOf(blocks(region).nth(3))).not.toBe(mono);

  // Each context defines its own roles.
  await contextButton(page, 'Web hero').click();
  await expect(roleCards(page).getByRole('heading')).toHaveText([
    'Eyebrow',
    'Headline',
    'Body',
    'Button',
  ]);
  await contextButton(page, 'Video title').click();
  await expect(roleCards(page).getByRole('heading')).toHaveText(['Headline', 'Subtitle']);
});

test('blocks show their role only on hover; a click selects one and rings its card', async ({
  page,
}) => {
  await open(page);
  await page.goto('/#/studio?face=1');
  const region = layout(studio(page), 'Magazine page');
  await expect(region).toHaveAttribute('data-state', 'ready');
  const marks = region.getByTestId('block-role-mark');
  await expect(marks).toHaveText(['Caption', 'Headline', 'Deck', 'Body', 'Caption']);
  const headline = blocks(region).nth(1);
  const headlineMark = marks.nth(1);
  const tools = headlineMark.locator('xpath=ancestor::div[contains(@class, "tools")][1]');
  const row = headline.locator('xpath=ancestor::div[contains(@class, "row")][1]');
  const outline = (b: Locator) =>
    b.evaluate((el) => {
      const s = getComputedStyle(el);
      return { width: s.outlineWidth, color: s.outlineColor };
    });

  // At rest: no mark, no ring.
  await expect(tools).toHaveCSS('opacity', '0');
  expect((await outline(headline)).color).toBe('rgba(0, 0, 0, 0)');
  // Hover: the mark in the margin and a hairline ring.
  await headline.hover();
  await expect(tools).toHaveCSS('opacity', '1');
  const sheetBox = (await sheet(region).boundingBox())!;
  expect((await headlineMark.boundingBox())!.x).toBeLessThan(sheetBox.x);
  await expect.poll(async () => (await outline(headline)).color).not.toBe('rgba(0, 0, 0, 0)');
  expect((await outline(headline)).width).toBe('1px');
  await page.mouse.move(0, 0);
  await expect(tools).toHaveCSS('opacity', '0');

  // Click: selected, with the accent ring; the mark stays; the Headline card has the ring.
  await headline.click();
  await expect(row).toHaveAttribute('data-selected', '');
  await page.mouse.move(0, 0);
  await expect(tools).toHaveCSS('opacity', '1');
  await expect.poll(async () => (await outline(headline)).width).toBe('2px');
  const accent = await roleCard(page, 'Headline').evaluate(
    (el) => getComputedStyle(el).borderTopColor,
  );
  expect((await outline(headline)).color).toBe(accent);
  await expect(roleCard(page, 'Headline')).toHaveAttribute('aria-current', 'true');
  // The focus stays on the block, so its keys work.
  await expect(headline).toBeFocused();

  // The card's ring follows the selection.
  await blocks(region).nth(3).click();
  await expect(roleCard(page, 'Body')).toHaveAttribute('aria-current', 'true');
  await expect(roleCard(page, 'Headline')).not.toHaveAttribute('aria-current', 'true');
  await expect(row).not.toHaveAttribute('data-selected', '');
  // The kicker and the caption share the Caption card.
  await blocks(region).nth(4).click();
  await expect(roleCard(page, 'Caption')).toHaveAttribute('aria-current', 'true');
  // The focus selects too.
  await blocks(region).nth(2).focus();
  await expect(roleCard(page, 'Deck')).toHaveAttribute('aria-current', 'true');
});

test('on a selected block: drag sideways for size, up and down for leading; Enter edits its text', async ({
  page,
}) => {
  await open(page);
  await page.goto('/#/studio?face=1');
  const region = layout(studio(page), 'Magazine page');
  await expect(region).toHaveAttribute('data-state', 'ready');
  const deck = blocks(region).nth(2);
  await deck.click();
  await expect(roleCard(page, 'Deck')).toHaveAttribute('aria-current', 'true');
  const size = Number(await deck.getAttribute('data-size'));
  const leading = Number(await deck.getAttribute('data-line-height'));
  const box = (await deck.boundingBox())!;
  const x = box.x + box.width / 2;
  const y = box.y + 10;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + 20, y, { steps: 4 });
  await page.mouse.move(x + 40, y, { steps: 4 });
  await page.mouse.move(x + 40, y - 30, { steps: 4 });
  await page.mouse.up();
  await expect.poll(async () => Number(await deck.getAttribute('data-size'))).toBeGreaterThan(size);
  await expect
    .poll(async () => Number(await deck.getAttribute('data-line-height')))
    .not.toBe(leading);
  // The card shows the new size, and the block is still the selected one.
  await expect(roleCard(page, 'Deck').getByTestId('studio-role-detail')).not.toHaveText(
    new RegExp(` ${size} pt `),
  );
  await expect(roleCard(page, 'Deck')).toHaveAttribute('aria-current', 'true');

  // Enter edits the selected block's text.
  await deck.focus();
  await page.keyboard.press('Enter');
  const editor = region.getByTestId('layout-block-editor');
  await expect(editor).toBeFocused();
  await editor.fill('A new deck for the page.');
  await page.keyboard.press('Control+Enter');
  await expect(deck).toHaveText('A new deck for the page.');
  await expect(pairing(page)).toContainText('unsaved');
});

test('opening the Studio with two faces (a pairing card or a drop) fills Headline and Body', async ({
  page,
}) => {
  await open(page);
  // The URL a pairing card (E42.7) and a drop of two fonts on the Studio (E44.2) open: the body
  // face (JetBrains Mono) and the headline face (Inter).
  await page.goto('/#/studio?face=6&headline=1');
  const region = layout(studio(page), 'Magazine page');
  await expect(region).toHaveAttribute('data-state', 'ready');
  const headlineName = 'Inter';
  const bodyName = 'JetBrains Mono';
  await expect(rolePicker(page, 'Headline')).toHaveText(headlineName);
  await expect(rolePicker(page, 'Body')).toHaveText(bodyName);
  await expect(pairing(page)).toHaveText(`${headlineName} + ${bodyName}`);
  // The other roles follow the body face.
  await expect(rolePicker(page, 'Deck')).toHaveText(bodyName);
  await expect(rolePicker(page, 'Caption')).toHaveText(bodyName);
  expect(await familyOf(blocks(region).nth(1))).not.toBe(await familyOf(blocks(region).nth(3)));
  // Another context gets the pairing too.
  await contextButton(page, 'Mobile UI').click();
  await expect(rolePicker(page, 'Headline')).toHaveText(headlineName);
  await expect(rolePicker(page, 'Body')).toHaveText(bodyName);
});

// ---- E45.3 shuffle and suggestions --------------------------------------------------------

const ROLES = ['Headline', 'Deck', 'Body', 'Caption'] as const;
const pickers = (page: Page) => Promise.all(ROLES.map((r) => rolePicker(page, r).textContent()));
const tryList = (page: Page) => rail(page).getByTestId('studio-try');
const candidates = (page: Page) => tryList(page).getByTestId('studio-try-candidate');
const UNDO_HINT = /(Ctrl\+Z|⌘Z) takes it back\./;

/** Opens the Studio on the Magazine context, with twenty sample families that pair with Inter. */
async function openWithLibrary(page: Page) {
  await open(page);
  await page.goto('/?mockFaces=20#/studio?face=1');
  const region = layout(studio(page), 'Magazine page');
  await expect(region).toHaveAttribute('data-state', 'ready');
  // Every crossfade the page starts: a copy of the old block laid over the new one.
  await page.evaluate(() => {
    const w = window as unknown as { fades: number };
    w.fades = 0;
    new MutationObserver((records) => {
      for (const r of records)
        for (const n of r.addedNodes)
          if (n instanceof HTMLElement && n.matches('.fm-crossfade[aria-hidden="true"]')) w.fades++;
    }).observe(document.body, { childList: true, subtree: true });
  });
  return region;
}
const fades = (page: Page) => page.evaluate(() => (window as unknown as { fades: number }).fades);

test('Shuffle unlocked keeps the locked role, gives the others the next face and crossfades', async ({
  page,
}) => {
  const region = await openWithLibrary(page);
  const shuffle = barButton(page, 'Shuffle unlocked');
  await expect(shuffle).not.toHaveAttribute('aria-disabled', 'true');
  await expect(shuffle).toHaveAttribute('title', /pairs with the locked ones/);
  await expect(rolePicker(page, 'Headline')).toHaveText('Inter');
  await roleCard(page, 'Headline').getByRole('button', { name: 'Lock Headline' }).click();
  const headlineFamily = await familyOf(blocks(region).nth(1));

  await shuffle.click();
  await expect(status(page)).toHaveText(new RegExp(`^Shuffled 3 roles\\. ${UNDO_HINT.source}`));
  const first = await pickers(page);
  expect(first[0]).toBe('Inter');
  // Each unlocked role has a face of its own from the library, a different family each.
  for (const name of first.slice(1)) expect(name).not.toBe('Inter');
  expect(new Set(first.slice(1)).size).toBe(3);
  expect(await familyOf(blocks(region).nth(1))).toBe(headlineFamily);
  await expect.poll(() => familyOf(blocks(region).nth(3))).not.toBe(headlineFamily);
  // The blocks crossfaded: one copy per changed block (Caption has two).
  await expect.poll(() => fades(page)).toBe(4);
  // The copies go when the fade ends: five blocks again.
  await expect(blocks(region)).toHaveCount(5);

  // The next press moves on to the next candidate of each role.
  await shuffle.click();
  await expect.poll(() => pickers(page).then((p) => p[2])).not.toBe(first[2]);
  const second = await pickers(page);
  expect(second[0]).toBe('Inter');
  for (let i = 1; i < 4; i++) expect(second[i]).not.toBe(first[i]);

  // Ctrl/Cmd-Z takes back one shuffle, then the one before.
  await page.keyboard.press('ControlOrMeta+z');
  await expect(status(page)).toHaveText('Undone.');
  expect(await pickers(page)).toEqual(first);
  await page.keyboard.press('ControlOrMeta+z');
  expect(await pickers(page)).toEqual(['Inter', 'Inter', 'Inter', 'Inter']);
  await expect(pairing(page)).toHaveText('Inter + Inter');

  // Every role locked: nothing to shuffle, and it says so.
  for (const r of ROLES.slice(1))
    await roleCard(page, r)
      .getByRole('button', { name: `Lock ${r}` })
      .click();
  await shuffle.click();
  await expect(status(page)).toHaveText('Every role is locked: unlock one to shuffle it.');
});

test('Try for <role>: three candidates with reasons; a click sets that role alone; Ctrl+Z undoes it', async ({
  page,
}) => {
  const region = await openWithLibrary(page);
  // No block selected: the first role.
  await expect(tryList(page).getByRole('heading')).toHaveText('Try for Headline');
  await blocks(region).nth(3).click();
  await expect(tryList(page).getByRole('heading')).toHaveText('Try for Body');
  await expect(candidates(page)).toHaveCount(3);
  for (const c of await candidates(page).all()) {
    await expect(c.getByTestId('studio-try-reasons')).not.toBeEmpty();
    expect(
      await c.locator('.family').evaluate((el) => (el as HTMLElement).style.fontFamily),
    ).toMatch(/fm-face-\d+/);
  }
  const name = (await candidates(page).first().locator('.family').textContent())!.trim();
  const reasons = (await candidates(page).first().getByTestId('studio-try-reasons').textContent())!;
  await expect(candidates(page).first()).toHaveAccessibleName(
    `Set Body in ${name}: ${reasons.split(' · ').join(', ')}`,
  );
  // By keyboard: Enter on the first candidate.
  await candidates(page).first().focus();
  await page.keyboard.press('Enter');
  await expect(rolePicker(page, 'Body')).toHaveText(name);
  expect(await pickers(page)).toEqual(['Inter', 'Inter', name, 'Inter']);
  // The list moves on: the picked family is the one being replaced now.
  await expect(candidates(page).filter({ hasText: name })).toHaveCount(0);
  await expect.poll(() => fades(page)).toBe(1);
  // Its button went, so the focus is on the candidate now first, not on the page; it is said.
  await expect(candidates(page).first()).toBeFocused();
  await expect(tryList(page).getByRole('status')).toHaveText(`Body set in ${name}.`);

  await page.keyboard.press('ControlOrMeta+z');
  await expect(rolePicker(page, 'Body')).toHaveText('Inter');

  // Typing in a field keeps Ctrl/Cmd-Z for the field: the shuffle is not undone.
  await barButton(page, 'Shuffle unlocked').click();
  await expect(status(page)).toHaveText(UNDO_HINT);
  const shuffled = await pickers(page);
  await blocks(region).nth(3).press('Enter');
  const field = region.getByRole('textbox');
  await expect(field).toBeFocused();
  await page.keyboard.type('x');
  await page.keyboard.press('ControlOrMeta+z');
  expect(await pickers(page)).toEqual(shuffled);
});

test('with nothing in the library to pair, the Try list links to the catalogs', async ({
  page,
}) => {
  await open(page);
  await page.goto('/#/studio?face=1');
  const region = layout(studio(page), 'Magazine page');
  await expect(region).toHaveAttribute('data-state', 'ready');
  // Suggestions come from the library only: the mock library has nothing that pairs with Inter.
  await barButton(page, 'Shuffle unlocked').click();
  await expect(status(page)).toHaveText(
    'Nothing else in your library pairs with the locked roles.',
  );
  const empty = tryList(page).getByTestId('studio-try-empty');
  await expect(empty).toContainText('Nothing in your library pairs with this for Headline yet.');
  await expect(candidates(page)).toHaveCount(0);
  await empty.getByRole('button', { name: 'Browse catalogs' }).click();
  await expect(studio(page)).toHaveCount(0);
  await expect(page.getByRole('heading', { name: 'Google Fonts', level: 1 })).toBeVisible();
});
