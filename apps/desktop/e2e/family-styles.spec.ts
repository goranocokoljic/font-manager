import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { axesRow, hero } from './family-page';
import { selectOption } from './select';

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

// The small MockApi's family 1 (Inter): one variable face, wght 100–900, default 400, instances
// Thin, Regular, Bold, Black. The wall's families: 500 Asap Sharp (variable, wght + wdth 75–125),
// 501 Crimson Pro (static: Light, Regular, Italic, Medium, Bold, Bold Italic).
const INTER = '/#/family/1';
const ASAP = '/?mockLibrary=wall#/family/500';
const CRIMSON = '/?mockLibrary=wall#/family/501';

async function open(page: Page, url: string) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(url);
  await expect(page.getByTestId('family-styles')).toBeVisible();
}

const list = (page: Page) => page.getByTestId('family-styles');
const row = (page: Page, name: string | RegExp) => list(page).getByRole('row', { name });
const ringed = (page: Page) => list(page).locator('[aria-current="true"]');
const views = (page: Page) => page.getByRole('group', { name: 'Show the styles as' });
const wght = (page: Page) => axesRow(page).getByRole('slider', { name: /Weight/ });
const sample = (r: Locator) => r.locator('.family-sample');

/** The names of the ringed rows, once they have settled. */
const expectRinged = (page: Page, ...names: string[]) =>
  expect
    .poll(() => ringed(page).evaluateAll((els) => els.map((el) => el.getAttribute('aria-label'))))
    .toEqual(names);

async function pin537(page: Page) {
  await page.goto('/#/');
  const sidebar = page.getByRole('region', { name: 'Collections' });
  await sidebar.getByRole('button', { name: 'New collection', exact: true }).click();
  const input = sidebar.getByRole('textbox', { name: 'New collection name' });
  await input.fill('Brand');
  await input.press('Enter');
  await expect(sidebar.getByRole('listitem', { name: 'Brand', exact: true })).toBeVisible();
  await page.evaluate(() => (location.hash = '#/family/1?face=1'));
  await wght(page).fill('537');
  await selectOption(axesRow(page).getByRole('combobox', { name: 'Save in' }), { label: 'Brand' });
  await axesRow(page).getByRole('button', { name: 'Pin 537 as my instance' }).click();
  await expect(page.getByTestId('family-pin')).toHaveCount(1);
}

test.describe('named styles', () => {
  test('one row per face: name and meta in 150 px, the text at 38 px', async ({ page }) => {
    await open(page, CRIMSON);
    await expect(list(page).getByRole('row')).toHaveCount(6);
    const italic = row(page, 'Italic, 400 · italic');
    await expect(italic).toContainText('Italic');
    await expect(italic.locator('.meta')).toHaveText('400 · italic');
    await expect(row(page, 'Regular, 400').locator('.meta')).toHaveText('400');
    expect((await italic.locator('.label').boundingBox())!.width).toBe(150);
    await expect(sample(italic)).toHaveCSS('font-size', '38px');
  });

  test('Copy CSS and Compare show on hover and on focus only', async ({ page }) => {
    await open(page, CRIMSON);
    const regular = row(page, 'Regular, 400');
    const actions = regular.locator('.hover-actions');
    await page.mouse.move(0, 0);
    await expect(actions).toHaveCSS('opacity', '0');
    await regular.hover();
    await expect(actions).toHaveCSS('opacity', '1');
    await expect(
      regular.getByRole('button', { name: 'Copy CSS for the Regular style' }),
    ).toBeVisible();
    await page.mouse.move(0, 0);
    await expect(actions).toHaveCSS('opacity', '0');
    await regular.focus();
    await expect(actions).toHaveCSS('opacity', '1');
  });

  test('the switch activates that face only, in the accent, and deactivates it', async ({
    page,
  }) => {
    await open(page, CRIMSON);
    const bold = row(page, 'Bold, 700').getByRole('switch', { name: 'Active: Bold' });
    const other = row(page, 'Regular, 400').getByRole('switch');
    await expect(bold).toHaveAttribute('aria-checked', 'false');
    await bold.click();
    await expect(bold).toHaveAttribute('aria-checked', 'true');
    await expect(other).toHaveAttribute('aria-checked', 'false');
    await expect(page.getByTestId('family-meta')).toContainText('Partly active');
    const accent = await bold.evaluate(() => {
      const probe = document.createElement('div');
      probe.style.color = 'var(--fm-accent)';
      document.body.append(probe);
      const color = getComputedStyle(probe).color;
      probe.remove();
      return color;
    });
    await expect(bold).toHaveCSS('background-color', accent);
    await bold.click();
    await expect(bold).toHaveAttribute('aria-checked', 'false');
    await expect(page.getByTestId('family-meta')).toContainText('Inactive');
  });

  test('Copy CSS copies what the row shows', async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await open(page, INTER);
    await wght(page).fill('640');
    const face = row(page, 'Regular, 400');
    await face.hover();
    await face.getByRole('button', { name: 'Copy CSS for the Regular style' }).click();
    await expect
      .poll(() => page.evaluate(() => navigator.clipboard.readText()))
      .toBe('font-variation-settings: "wght" 640;');
  });

  test('Compare pins that face', async ({ page }) => {
    await open(page, CRIMSON);
    const bold = row(page, 'Bold, 700');
    await bold.hover();
    await bold.getByRole('button', { name: 'Compare Bold' }).click();
    await expect(page.getByTestId('toast')).toContainText('Crimson Pro');
    await expect(page.getByTestId('toast')).toContainText('Bold');
    await expect(page.getByTestId('toast')).toContainText('pinned to Compare');
  });

  test('the ring tells an upright style from an italic of the same weight', async ({ page }) => {
    await open(page, CRIMSON);
    await row(page, 'Regular, 400').click({ position: { x: 20, y: 20 } });
    await expectRinged(page, 'Regular, 400');
    await row(page, 'Italic, 400 · italic').click({ position: { x: 20, y: 20 } });
    await expect(page).toHaveURL(/face=\d+/);
    await expectRinged(page, 'Italic, 400 · italic');
  });

  test('a click on the switch does not set the hero', async ({ page }) => {
    await open(page, CRIMSON);
    const before = page.url();
    await row(page, 'Bold, 700').getByRole('switch').click();
    await expect(row(page, 'Bold, 700').getByRole('switch')).toHaveAttribute(
      'aria-checked',
      'true',
    );
    expect(page.url()).toBe(before);
  });
});

test.describe('the ramp switch', () => {
  test('a static family has none', async ({ page }) => {
    await open(page, CRIMSON);
    await expect(views(page)).toHaveCount(0);
    await expect(list(page).getByRole('row')).toHaveCount(6);
  });

  test('a variable family with only weight offers Named styles and Weight ramp', async ({
    page,
  }) => {
    await open(page, INTER);
    await expect(views(page).getByRole('button')).toHaveText(['Named styles · 1', 'Weight ramp']);
  });

  test('the weight ramp has every 100, with the instance names where they exist', async ({
    page,
  }) => {
    await open(page, INTER);
    await views(page).getByRole('button', { name: 'Weight ramp' }).click();
    const rows = list(page).getByTestId('family-ramp-row');
    await expect(rows.locator('.title')).toHaveText(
      ['100', '200', '300', '400', '500', '600', '700', '800', '900'].map((v) => v),
    );
    await expect(rows.locator('.meta')).toHaveText([
      'Thin',
      '',
      '',
      'Regular',
      '',
      '',
      'Bold',
      '',
      'Black',
    ]);
    await expect(sample(rows.nth(2))).toHaveCSS('font-variation-settings', '"wght" 300');
    // A ramp row is a place on the axis, not a face: it has no switch of its own.
    await expect(rows.nth(2).getByRole('switch')).toHaveCount(0);
  });

  test('a click on a ramp row sets the hero weight', async ({ page }) => {
    await open(page, INTER);
    await views(page).getByRole('button', { name: 'Weight ramp' }).click();
    await row(page, '700, Bold').click();
    await expect(hero(page)).toHaveCSS('font-variation-settings', '"wght" 700');
    await expect(wght(page)).toHaveValue('700');
    await expectRinged(page, '700, Bold');
  });

  test('the ring on both lists follows the hero within ±12', async ({ page }) => {
    await open(page, INTER);
    await expectRinged(page, 'Regular, 400');
    await wght(page).fill('412');
    await expectRinged(page, 'Regular, 400');
    await wght(page).fill('413');
    await expectRinged(page);

    await views(page).getByRole('button', { name: 'Weight ramp' }).click();
    await expectRinged(page);
    await wght(page).fill('412');
    await expectRinged(page, '400, Regular');
    await wght(page).fill('388');
    await expectRinged(page, '400, Regular');
    await wght(page).fill('387');
    await expectRinged(page);
  });

  test('with more axes it offers one ramp per axis, weight first, nine steps on the others', async ({
    page,
  }) => {
    await open(page, ASAP);
    await expect(views(page).getByRole('button')).toHaveText([
      'Named styles · 1',
      'Weight',
      'Width',
    ]);
    await wght(page).fill('700');
    await views(page).getByRole('button', { name: 'Width', exact: true }).click();
    const rows = list(page).getByTestId('family-ramp-row');
    await expect(rows).toHaveCount(9);
    await expect(rows.first().locator('.title')).toHaveText('75');
    await expect(rows.last().locator('.title')).toHaveText('125');
    // The weight stays where the hero has it while the width is ramped.
    await expect(sample(rows.last())).toHaveCSS(
      'font-variation-settings',
      /"wght" 700.*"wdth" 125|"wdth" 125.*"wght" 700/,
    );
    await rows.last().click();
    const hasBoth = /"wght" 700.*"wdth" 125|"wdth" 125.*"wght" 700/;
    await expect(hero(page)).toHaveCSS('font-variation-settings', hasBoth);
    await expectRinged(page, '125');
  });

  test('the Weight ramp of a family with more axes holds the other axis too', async ({ page }) => {
    await open(page, ASAP);
    await axesRow(page).getByRole('slider', { name: /Width/ }).fill('90');
    await views(page).getByRole('button', { name: 'Weight', exact: true }).click();
    const rows = list(page).getByTestId('family-ramp-row');
    await expect(rows).toHaveCount(9);
    await expect(sample(rows.first())).toHaveCSS(
      'font-variation-settings',
      /"wght" 100.*"wdth" 90|"wdth" 90.*"wght" 100/,
    );
  });

  test('going to a family without the axis falls back to the named list', async ({ page }) => {
    await open(page, ASAP);
    await views(page).getByRole('button', { name: 'Width', exact: true }).click();
    await expect(list(page).getByTestId('family-ramp-row')).toHaveCount(9);
    await page.keyboard.press('ArrowRight');
    await expect(page).toHaveURL(/#\/family\/501$/);
    await expect(list(page).getByTestId('family-style-row')).toHaveCount(6);
  });
});

test.describe('pinned instances', () => {
  test('appear with a yours badge, count in the switch, and have no switch', async ({ page }) => {
    await page.route('**/fmfont/*', (route) =>
      route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
    );
    await pin537(page);
    const pinned = page.getByTestId('family-pin');
    await expect(pinned.locator('.title')).toContainText('Weight 537');
    await expect(pinned.locator('.title')).toContainText('yours');
    // The badge is only on the pin: the face's row has none.
    await expect(row(page, 'Regular, 400').locator('.title')).not.toContainText('yours');
    await expect(pinned.getByRole('switch')).toHaveCount(0);
    await expect(views(page).getByRole('button').first()).toHaveText('Named styles · 2');
  });

  test('can be deleted from the row menu, and the focus stays in the list', async ({ page }) => {
    await page.route('**/fmfont/*', (route) =>
      route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
    );
    await pin537(page);
    const pinned = page.getByTestId('family-pin');
    await pinned.getByRole('button', { name: 'More for Weight 537' }).click();
    await page.getByRole('menuitem', { name: 'Delete Weight 537' }).click();
    await expect(pinned).toHaveCount(0);
    await expect(page.getByTestId('toast')).toContainText('Deleted “Weight 537”.');
    await expect(row(page, 'Regular, 400')).toBeFocused();
  });

  test('Escape closes only the menu and gives the focus back', async ({ page }) => {
    await page.route('**/fmfont/*', (route) =>
      route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
    );
    await pin537(page);
    const more = page
      .getByTestId('family-pin')
      .getByRole('button', { name: 'More for Weight 537' });
    await more.click();
    await expect(page.getByRole('menuitem')).toBeFocused();
    await page.keyboard.press('Escape');
    await expect(page.getByRole('menu')).toHaveCount(0);
    await expect(more).toBeFocused();
    await expect(page).toHaveURL(/#\/family\/1/);
    await expect(page.getByTestId('family-pin')).toHaveCount(1);
  });

  test('Enter sets the hero to the coordinates of the pin', async ({ page }) => {
    await page.route('**/fmfont/*', (route) =>
      route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
    );
    await pin537(page);
    await wght(page).fill('200');
    await page.getByTestId('family-pin').focus();
    await page.keyboard.press('Enter');
    await expect(hero(page)).toHaveCSS('font-variation-settings', '"wght" 537');
    await expect(page.getByTestId('family-pin')).toHaveAttribute('aria-current', 'true');
  });
});

test.describe('the keyboard', () => {
  test('↑/↓ move between rows and stop at the ends, with one tab stop', async ({ page }) => {
    await open(page, CRIMSON);
    const rows = list(page).getByRole('row');
    await expect(list(page).locator('[role="row"][tabindex="0"]')).toHaveCount(1);
    await rows.first().focus();
    await page.keyboard.press('ArrowUp');
    await expect(rows.first()).toBeFocused();
    await page.keyboard.press('ArrowDown');
    await expect(rows.nth(1)).toBeFocused();
    await page.keyboard.press('ArrowDown');
    await expect(rows.nth(2)).toBeFocused();
    await page.keyboard.press('ArrowUp');
    await expect(rows.nth(1)).toBeFocused();
    await expect(rows.nth(1)).toHaveAttribute('tabindex', '0');
    await expect(rows.first()).toHaveAttribute('tabindex', '-1');
    // The controls of the row with the tab stop are in the tab order; those of another are not.
    for (const name of [/^Copy CSS/, /^Compare/]) {
      await expect(rows.nth(1).getByRole('button', { name })).toHaveAttribute('tabindex', '0');
      await expect(rows.first().getByRole('button', { name })).toHaveAttribute('tabindex', '-1');
    }
    await expect(rows.nth(1).getByRole('switch')).toHaveAttribute('tabindex', '0');
    await expect(rows.first().getByRole('switch')).toHaveAttribute('tabindex', '-1');
    await rows.last().focus();
    await page.keyboard.press('ArrowDown');
    await expect(rows.last()).toBeFocused();
  });

  test('Space toggles the activation of the focused row, and keeps the focus', async ({ page }) => {
    await open(page, CRIMSON);
    const medium = row(page, 'Medium, 500');
    await medium.focus();
    await page.keyboard.press('Space');
    await expect(medium.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
    await expect(medium).toBeFocused();
    await expect(row(page, 'Bold, 700').getByRole('switch')).toHaveAttribute(
      'aria-checked',
      'false',
    );
    await page.keyboard.press('Space');
    await expect(medium.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
  });

  test('Space on the switch itself toggles once', async ({ page }) => {
    await open(page, CRIMSON);
    const toggle = row(page, 'Regular, 400').getByRole('switch');
    await toggle.focus();
    await page.keyboard.press('Space');
    await expect(toggle).toHaveAttribute('aria-checked', 'true');
  });

  test('Enter sets the hero to the row: a static style becomes the hero face', async ({ page }) => {
    await open(page, CRIMSON);
    const bold = row(page, 'Bold, 700');
    await bold.focus();
    await page.keyboard.press('Enter');
    await expect(page).toHaveURL(/face=\d+/);
    await expectRinged(page, 'Bold, 700');
    await expect(bold.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
  });

  test("Enter puts a variable hero back at the row's weight", async ({ page }) => {
    await open(page, INTER);
    await wght(page).fill('640');
    await row(page, 'Regular, 400').focus();
    await page.keyboard.press('Enter');
    await expect(hero(page)).toHaveCSS('font-variation-settings', '"wght" 400');
    await expectRinged(page, 'Regular, 400');
  });

  test('on the ramp ↑/↓ and Enter work, and Space does nothing', async ({ page }) => {
    await open(page, INTER);
    await views(page).getByRole('button', { name: 'Weight ramp' }).click();
    const rows = list(page).getByRole('row');
    await rows.first().focus();
    await page.keyboard.press('ArrowDown');
    await expect(rows.nth(1)).toBeFocused();
    await page.keyboard.press('Enter');
    await expect(hero(page)).toHaveCSS('font-variation-settings', '"wght" 200');
    await page.keyboard.press('Space');
    await expect(page.getByTestId('family-meta')).toContainText('Inactive');
    await expect(rows.nth(1)).toBeFocused();
  });

  test('← and → still change the family from a row', async ({ page }) => {
    await open(page, CRIMSON);
    await list(page).getByRole('row').first().focus();
    await page.keyboard.press('ArrowRight');
    await expect(page).toHaveURL(/#\/family\/502$/);
  });
});

for (const colorScheme of ['light', 'dark'] as const) {
  test(`the ring reaches 3:1 against the page in the ${colorScheme} theme`, async ({ page }) => {
    await page.emulateMedia({ colorScheme });
    await open(page, INTER);
    await expectRinged(page, 'Regular, 400');
    const { ratio, width, style } = await ringed(page).evaluate((el) => {
      const channels = (c: string) => (c.match(/[\d.]+/g) ?? []).map(Number);
      const lum = ([r, g, b]: number[]) => {
        const lin = [r!, g!, b!].map((v) => {
          const s = v / 255;
          return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
        });
        return 0.2126 * lin[0]! + 0.7152 * lin[1]! + 0.0722 * lin[2]!;
      };
      const ring = getComputedStyle(el, '::after');
      const pageBg = channels(
        getComputedStyle(document.querySelector('.family-page')!).backgroundColor,
      );
      const [a, b] = [lum(channels(ring.borderTopColor)), lum(pageBg)].sort((x, y) => y - x);
      return {
        ratio: (a! + 0.05) / (b! + 0.05),
        width: ring.borderTopWidth,
        style: ring.borderTopStyle,
      };
    });
    expect(width).toBe('2px');
    expect(style).toBe('solid');
    expect(ratio).toBeGreaterThanOrEqual(3);
  });
}

test.describe('@screens', () => {
  const dir = process.env.FM_SCREENS_DIR;
  test.skip(!dir, 'FM_SCREENS_DIR is not set');
  for (const [width, height] of [
    [1280, 800],
    [1000, 700],
  ] as const) {
    test(`the named styles and the ramp at ${width}x${height}`, async ({ page }) => {
      await page.setViewportSize({ width, height });
      await open(page, CRIMSON);
      await row(page, 'Bold, 700').getByRole('switch').click();
      await row(page, 'Regular, 400').hover();
      await page.screenshot({ path: `${dir}/styles-named-${width}x${height}.png` });
      await open(page, ASAP);
      await views(page).getByRole('button', { name: 'Weight', exact: true }).click();
      await expect(list(page).getByTestId('family-ramp-row')).toHaveCount(9);
      await page.screenshot({ path: `${dir}/styles-ramp-${width}x${height}.png` });
    });
  }
});
