import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';

// E54.2: a right-click in a text field opens the app's own menu with exactly Cut, Copy, Paste
// and Select all; each does what its shortcut does, and the field keeps the focus and the
// caret. In browser mode Paste reads the browser's clipboard (the app reads it natively).

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

test.beforeEach(async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto('/#/');
});

const searchField = (page: Page) => page.getByRole('searchbox', { name: 'Search fonts' });
const previewField = (page: Page) =>
  page.getByRole('group', { name: 'Preview settings' }).getByRole('textbox', {
    name: 'Type to preview',
  });
const menu = (page: Page) => page.getByRole('menu', { name: 'Edit text' });
const item = (page: Page, name: string) => menu(page).getByRole('menuitem', { name, exact: true });
const clipboard = (page: Page) => page.evaluate(() => navigator.clipboard.readText());
const setClipboard = (page: Page, text: string) =>
  page.evaluate((t) => navigator.clipboard.writeText(t), text);
const selection = (field: Locator) =>
  field.evaluate((el: HTMLInputElement) => [el.selectionStart, el.selectionEnd]);

async function openMenu(page: Page, field: Locator) {
  await field.click({ button: 'right' });
  await expect(menu(page)).toBeVisible();
}

for (const [name, field] of [
  ['search field', searchField],
  ['preview field', previewField],
] as const) {
  test(`the ${name}: copy, cut, paste and select all do what their shortcuts do`, async ({
    page,
  }) => {
    const f = field(page);
    await f.fill('Grumpy wizards');

    // Exactly the four items.
    await f.selectText();
    await openMenu(page, f);
    await expect(menu(page).getByRole('menuitem')).toHaveText([
      /^Cut/,
      /^Copy/,
      /^Paste/,
      /^Select all/,
    ]);

    // Copy: the selection on the clipboard; the field keeps the focus and the selection.
    await item(page, 'Copy').click();
    await expect(menu(page)).toHaveCount(0);
    await expect(f).toBeFocused();
    await expect.poll(() => clipboard(page)).toBe('Grumpy wizards');
    expect(await selection(f)).toEqual([0, 14]);

    // Cut: the selection leaves the field for the clipboard.
    await setClipboard(page, '');
    await openMenu(page, f);
    await item(page, 'Cut').click();
    await expect(f).toHaveValue('');
    await expect.poll(() => clipboard(page)).toBe('Grumpy wizards');
    await expect(f).toBeFocused();

    // Paste: the clipboard's text at the caret, as typing it would.
    await setClipboard(page, 'Sphinx of black quartz');
    await openMenu(page, f);
    await item(page, 'Paste').click();
    await expect(f).toHaveValue('Sphinx of black quartz');
    await expect(f).toBeFocused();
    expect(await selection(f)).toEqual([22, 22]);

    // Select all: the whole text, the focus still in the field.
    await openMenu(page, f);
    await item(page, 'Select all').click();
    await expect(f).toBeFocused();
    expect(await selection(f)).toEqual([0, 22]);
  });
}

test('Cut and Copy are disabled without a selection', async ({ page }) => {
  const f = searchField(page);
  await f.fill('Grumpy wizards');
  await f.press('End');
  await openMenu(page, f);
  // The right-click put the caret where it pressed: no text is selected.
  expect(await selection(f).then(([a, b]) => a === b)).toBe(true);
  await expect(item(page, 'Cut')).toBeDisabled();
  await expect(item(page, 'Copy')).toBeDisabled();
  await expect(item(page, 'Paste')).toBeEnabled();
  await expect(item(page, 'Select all')).toBeEnabled();
});

test('Cut and Paste are disabled in a read-only field', async ({ page }) => {
  const f = previewField(page);
  await f.fill('Grumpy wizards');
  await f.evaluate((el: HTMLInputElement) => (el.readOnly = true));
  await f.selectText();
  await openMenu(page, f);
  await expect(item(page, 'Cut')).toBeDisabled();
  await expect(item(page, 'Copy')).toBeEnabled();
  await expect(item(page, 'Paste')).toBeDisabled();
  await expect(item(page, 'Select all')).toBeEnabled();
});

test('the menu opens at the pointer', async ({ page }) => {
  const f = searchField(page);
  const box = (await f.boundingBox())!;
  await f.click({ button: 'right', position: { x: 60, y: box.height / 2 } });
  await expect(menu(page)).toBeVisible();
  // Its corner 4 px below the pointer, once the 4 px rise of its entrance has ended.
  const layer = page.locator('[data-ui-popover]');
  const at = { x: box.x + 60, y: box.y + box.height / 2 + 4 };
  // Within a pixel: the pointer lands on whole pixels.
  await expect
    .poll(async () => Math.abs((await layer.boundingBox())!.y - at.y))
    .toBeLessThanOrEqual(1);
  expect(Math.abs((await layer.boundingBox())!.x - at.x)).toBeLessThanOrEqual(1);
});

test('the menu is keyboard-complete and the caret stays in the field', async ({ page }) => {
  const f = searchField(page);
  await f.fill('Grumpy wizards');
  await f.evaluate((el: HTMLInputElement) => el.setSelectionRange(7, 14));
  // The Menu key and Shift+F10: Chromium fires no event for them in a headless run.
  await f.dispatchEvent('contextmenu', { bubbles: true, cancelable: true, button: 0 });
  await expect(menu(page)).toBeVisible();
  await expect(f).toBeFocused();
  await expect(item(page, 'Cut')).toHaveClass(/active/);
  await page.keyboard.press('ArrowDown');
  await expect(item(page, 'Copy')).toHaveClass(/active/);
  await page.keyboard.press('Enter');
  await expect(menu(page)).toHaveCount(0);
  await expect.poll(() => clipboard(page)).toBe('wizards');
  await expect(f).toBeFocused();
  expect(await selection(f)).toEqual([7, 14]);

  // Esc closes the menu, not the field's own Escape (which clears the search).
  await f.click({ button: 'right' });
  await expect(menu(page)).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(menu(page)).toHaveCount(0);
  await expect(f).toBeFocused();
  await expect(f).toHaveValue('Grumpy wizards');
});

test('the shortcuts themselves are untouched', async ({ page }) => {
  const f = previewField(page);
  await f.fill('Grumpy wizards');
  await f.press('ControlOrMeta+a');
  expect(await selection(f)).toEqual([0, 14]);
  await f.press('ControlOrMeta+c');
  await expect.poll(() => clipboard(page)).toBe('Grumpy wizards');
  await expect(menu(page)).toHaveCount(0);
});

test('a text field inside a popover keeps its popover while a menu item runs', async ({ page }) => {
  // The foundry picker is a popover; a press on the text menu is not a press outside it.
  await page.goto('/?mockLibrary=wall#/');
  const nav = page.getByRole('navigation', { name: 'Library' });
  await nav.getByRole('button', { name: 'Foundries', exact: true }).click();
  await page.locator('#fm-foundries-list').getByTestId('all-foundries').click();
  const f = page.getByRole('searchbox', { name: 'Find a foundry' });
  await f.fill('Grumpy wizards');
  await f.selectText();
  await openMenu(page, f);
  await item(page, 'Copy').click();
  await expect.poll(() => clipboard(page)).toBe('Grumpy wizards');
  await expect(page.getByTestId('foundry-picker')).toBeVisible();
  await expect(f).toBeFocused();

  await openMenu(page, f);
  await item(page, 'Cut').click();
  await expect(f).toHaveValue('');
  await expect(page.getByTestId('foundry-picker')).toBeVisible();
});
