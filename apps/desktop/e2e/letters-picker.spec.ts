import { expect, test, type Page } from '@playwright/test';
import { openFilters } from './filters';

// The letters are the controls (E49.6, boards 8-letters-picker.png and 8-letters-filtered.png):
// in the Letters preset each typed letter is a button; one whose glyph has a trait opens a
// picker of the trait's values with their counts in the current result. A choice is an
// ordinary chip by the Filters button that stays when the preset changes. The wall library
// of the mock gives its families fixed trait detections.

async function openWall(page: Page): Promise<void> {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto('/?mockLibrary=wall#/');
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Grid' })
    .click();
  await expect(page.getByRole('listbox', { name: 'Font grid' })).toBeVisible();
}

const controls = (page: Page) => page.getByRole('group', { name: 'Preview settings' });
const field = (page: Page) => controls(page).getByRole('textbox', { name: 'Type to preview' });
const preset = (page: Page, name: string) =>
  controls(page).getByRole('group', { name: 'Preset' }).getByRole('button', { name });
const letterRow = (page: Page) => controls(page).getByRole('group', { name: 'Letters' });
const picker = (page: Page) => page.getByTestId('letter-picker');
const filterBar = (page: Page) => page.getByRole('region', { name: 'Filters' });
const traitChips = (page: Page) => filterBar(page).getByTestId('trait-chip');
const badge = (page: Page) => filterBar(page).getByRole('button', { name: /^Filters/ });
const line = (page: Page) => page.getByTestId('count-line');

/** Letters on, `letters` typed, the field left: the letters show as buttons. */
async function typeLetters(page: Page, letters: string): Promise<void> {
  await preset(page, 'Letters').click();
  await field(page).fill(letters);
  await field(page).press('Escape');
  await expect(letterRow(page)).toBeVisible();
}

test('click a letter, choose a shape, the chip stays when the preset changes, remove it', async ({
  page,
}) => {
  await openWall(page);
  await typeLetters(page, 'ROagx');
  const r = letterRow(page).getByRole('button', { name: 'R, choose its shape' });
  await expect(letterRow(page).getByRole('button')).toHaveText(['R', 'O', 'a', 'g', 'x']);
  await expect(r).toHaveAttribute('title', 'shape');
  await expect(controls(page).getByTestId('preview-hint')).toHaveText(
    'click an underlined letter for its shape',
  );
  // The hairline: 1.5 px at 22 % ink, darker on hover.
  const underline = () => r.evaluate((el) => getComputedStyle(el, '::after').backgroundColor);
  const rest = await underline();
  expect(await r.evaluate((el) => getComputedStyle(el, '::after').height)).toBe('1.5px');
  // x has no trait in the catalog: no hairline, no tooltip.
  const x = letterRow(page).getByRole('button', { name: 'x', exact: true });
  expect(await x.evaluate((el) => getComputedStyle(el, '::after').content)).toBe('none');
  await expect(x).not.toHaveAttribute('title', /./);
  await r.hover();
  await expect.poll(underline).not.toBe(rest);

  const total = Number((await line(page).textContent())!.match(/[\d,]+/)![0].replace(',', ''));
  await r.click();
  await expect(picker(page)).toBeVisible();
  const radios = picker(page).getByRole('radio');
  await expect(radios).toHaveCount(4);
  await expect(picker(page).getByRole('radio', { name: /Any/ })).toBeChecked();
  const straight = picker(page)
    .locator('label')
    .filter({ hasText: /^Straight/ });
  await expect(straight).toContainText(/\d+ famil/);
  const straightCount = Number((await straight.textContent())!.match(/(\d+) famil/)![1]);
  expect(straightCount).toBeGreaterThan(0);
  expect(straightCount).toBeLessThan(total);

  await straight.click();
  await expect(picker(page)).toBeHidden();
  const chosen = letterRow(page).getByRole('button', { name: 'R, shape straight' });
  await expect(chosen).toBeFocused();
  await expect(traitChips(page)).toHaveCount(1);
  await expect(traitChips(page)).toHaveAttribute('aria-label', 'Remove filter R leg straight');
  await expect(badge(page)).toContainText('1');
  await expect(line(page)).toContainText(`${straightCount} families`);
  await expect(line(page)).toContainText('R leg straight');
  await expect(controls(page).getByTestId('preview-hint')).toHaveText(
    'R narrowed · click a letter to change',
  );

  // The chip is an ordinary filter: Word keeps it.
  await preset(page, 'Word').click();
  await expect(letterRow(page)).toHaveCount(0);
  await expect(traitChips(page)).toHaveCount(1);
  await expect(line(page)).toContainText(`${straightCount} families`);

  // No Letters group in the filter drawer.
  const drawer = await openFilters(page);
  await expect(drawer.getByRole('group', { name: /letter|shape/i })).toHaveCount(0);
  await page.keyboard.press('Escape');

  await traitChips(page).click();
  await expect(traitChips(page)).toHaveCount(0);
  await expect(line(page)).not.toContainText('R leg straight');
});

test('Escape closes the picker, focus back on the letter; Any takes a shape off', async ({
  page,
}) => {
  await openWall(page);
  await typeLetters(page, 'RO');
  const o = letterRow(page).getByRole('button', { name: 'O, choose its shape' });
  await o.click();
  await expect(picker(page).getByRole('radio', { name: /Any/ })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(picker(page)).toBeHidden();
  await expect(o).toBeFocused();
  // Tab from the picker (teleported to the end of the page) comes back to the letter.
  await page.keyboard.press('Enter');
  await expect(picker(page).getByRole('radio', { name: /Any/ })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(picker(page)).toBeHidden();
  await expect(o).toBeFocused();

  await o.click();
  await picker(page).locator('label').filter({ hasText: 'Round' }).click();
  await expect(traitChips(page)).toHaveCount(1);
  const chosen = letterRow(page).getByRole('button', { name: 'O, shape round' });
  // The chosen letter's hairline is the accent.
  const accent = await chosen.evaluate((el) => {
    const probe = document.createElement('span');
    probe.style.color = 'var(--fm-accent)';
    el.append(probe);
    const color = getComputedStyle(probe).color;
    probe.remove();
    return color;
  });
  await expect
    .poll(() => chosen.evaluate((el) => getComputedStyle(el, '::after').backgroundColor))
    .toBe(accent);
  await chosen.click();
  await picker(page).locator('label').filter({ hasText: 'Any' }).click();
  await expect(traitChips(page)).toHaveCount(0);

  // A press outside closes it too.
  await o.click();
  await expect(picker(page)).toBeVisible();
  await page.mouse.click(1400, 880);
  await expect(picker(page)).toBeHidden();
});

test('R:straight O:round in the search box makes the same chips', async ({ page }) => {
  await openWall(page);
  const box = page.getByRole('searchbox', { name: 'Search fonts' });
  await box.fill('R:straight O:round');
  await expect(traitChips(page)).toHaveCount(2);
  await expect(box).toHaveValue('');
  await expect(traitChips(page).nth(0)).toHaveAttribute('title', 'R:straight');
  await expect(traitChips(page).nth(1)).toHaveAttribute('title', 'O:round');
  await expect(line(page)).toContainText('R leg straight');
  await expect(line(page)).toContainText('R leg straight · O round');
  // The letters show what the box chose.
  await typeLetters(page, 'RO');
  await expect(letterRow(page).getByRole('button', { name: 'R, shape straight' })).toBeVisible();
  await expect(letterRow(page).getByRole('button', { name: 'O, shape round' })).toBeVisible();
});

test('the command palette has "Match by letters…", which switches to Letters', async ({ page }) => {
  await openWall(page);
  await expect(preset(page, 'Letters')).toHaveAttribute('aria-pressed', 'false');
  await page.getByRole('listbox', { name: 'Font grid' }).focus();
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('Match by letters');
  await page.keyboard.press('Enter');
  await expect(preset(page, 'Letters')).toHaveAttribute('aria-pressed', 'true');
  await expect(field(page)).toBeFocused();
});

// The two states of board 8 (8-letters-picker.png, 8-letters-filtered.png) for UI review:
// FM_SCREENS_DIR=<dir> pnpm test:e2e --grep @screens
for (const [width, height] of [
  [1280, 800],
  [1000, 700],
] as const) {
  test(`screens: the letter picker and the narrowed wall at ${width}x${height} @screens`, async ({
    page,
  }) => {
    const dir = process.env.FM_SCREENS_DIR;
    test.skip(!dir, 'FM_SCREENS_DIR is not set');
    await openWall(page);
    await typeLetters(page, 'ROag');
    await page.setViewportSize({ width, height });
    await letterRow(page).getByRole('button', { name: 'R, choose its shape' }).click();
    await expect(
      picker(page)
        .getByText(/\d+ famil/)
        .first(),
    ).toBeVisible();
    await page.screenshot({ path: `${dir}/letters-picker-${width}x${height}.png` });
    await picker(page)
      .locator('label')
      .filter({ hasText: /^Straight/ })
      .click();
    await letterRow(page).getByRole('button', { name: 'O, choose its shape' }).click();
    await picker(page).locator('label').filter({ hasText: 'Round' }).click();
    await expect(traitChips(page)).toHaveCount(2);
    await page.screenshot({ path: `${dir}/letters-filtered-${width}x${height}.png` });
  });
}
