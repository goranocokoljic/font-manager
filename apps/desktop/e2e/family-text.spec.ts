import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { WALL_FAMILIES, WALL_FAMILY_BASE } from '../src/api/mockWall';
import { axesRow, hero, openSection, sectionPanel } from './family-page';

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

// The texts as the page shows them (the modules that hold them load the locale files, which a
// spec cannot import).
const PARAGRAPH =
  'Typography is the craft of endowing human language with a durable visual form. It exists to honor content, and a page set with care invites the reader in and keeps them reading.';
const STOCK_PASSAGE = `${PARAGRAPH} The best type does not call attention to itself.`;

/** The family id of a wall family; Hedvig Letters Serif has opsz 12–24 and no other axis. */
const wallFamily = (name: string) =>
  WALL_FAMILY_BASE + WALL_FAMILIES.findIndex((f) => f.name === name);
const LONG_TEXT =
  'A preview text the user typed themselves, long enough at 81 characters to read!!!';

const serveFonts = (page: Page) =>
  page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );

async function open(page: Page, face = 1) {
  await serveFonts(page);
  return openSection(page, face, 'Text');
}

async function openWall(page: Page, family: string) {
  await serveFonts(page);
  await page.goto(`/?mockLibrary=wall#/family/${wallFamily(family)}`);
  await page
    .getByRole('group', { name: 'Section' })
    .getByRole('button', { name: 'Text', exact: true })
    .click();
  await expect(sectionPanel(page, 'Text')).toBeVisible();
}

const passage = (page: Page, px: number) =>
  page.locator(`[data-testid="family-text-passage"][data-px="${px}"]`);
const inkPassage = (page: Page) => page.getByTestId('family-text-ink-passage');
const section = (page: Page) => page.getByTestId('family-text');
const measure = (page: Page) => page.getByRole('slider', { name: 'Measure' });
const leading = (page: Page) => page.getByRole('slider', { name: 'Leading' });
const ground = (page: Page, name: 'Paper' | 'Ink') =>
  page.getByRole('group', { name: 'Ground' }).getByRole('button', { name });

const widthOf = (el: Locator) => el.evaluate((n) => n.getBoundingClientRect().width);
/** The width of 1ch in the font and size of `el`; 100 of them, as layout rounds a box to 1/64 px. */
const chOf = (el: Locator) =>
  el.evaluate((n) => {
    const probe = document.createElement('span');
    probe.style.cssText = 'display: block; width: 100ch; height: 0';
    n.append(probe);
    const width = probe.getBoundingClientRect().width / 100;
    probe.remove();
    return width;
  });

function luminance(rgb: number[]): number {
  const [r, g, b] = rgb.map((v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  }) as [number, number, number];
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
const parse = (css: string) =>
  css
    .match(/\d+(\.\d+)?/g)!
    .slice(0, 3)
    .map(Number);
const ratio = (a: string, b: string) => {
  const [x, y] = [luminance(parse(a)), luminance(parse(b))];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
};
const colors = (el: Locator) =>
  el.evaluate((n) => {
    const s = getComputedStyle(n);
    return { color: s.color, background: s.backgroundColor };
  });

/** The passage shows, in the face the hero shows: never hidden, never a fallback font. */
async function expectShownInTheFace(page: Page, el: Locator) {
  await expect(el).toBeVisible();
  await expect(el).not.toHaveClass(/loading/);
  await expect(hero(page)).not.toHaveClass(/loading/);
  const face = await hero(page).evaluate((n) => getComputedStyle(n).fontFamily);
  await expect(el).toHaveCSS('font-family', face);
}

test.describe('the three columns', () => {
  test('the same passage at 12, 17 and 24 px with their labels, line height 1.45', async ({
    page,
  }) => {
    await open(page);
    const columns = page.getByTestId('family-text-row').locator('.column');
    await expect(columns).toHaveCount(3);
    for (const [i, [px, role]] of (
      [
        [12, 'captions'],
        [17, 'body'],
        [24, 'lead'],
      ] as const
    ).entries()) {
      const label = columns.nth(i).locator('.label');
      await expect(label).toContainText(`${px} px`);
      await expect(label).toContainText(role);
      await expect(passage(page, px)).toHaveCSS('font-size', `${px}px`);
      await expect(passage(page, px)).toHaveCSS('line-height', `${px * 1.45}px`);
      await expect(passage(page, px)).toHaveText(STOCK_PASSAGE);
      await expectShownInTheFace(page, passage(page, px));
    }
  });

  test('opsz follows the size in a family that has the axis', async ({ page }) => {
    await openWall(page, 'Hedvig Letters Serif');
    await expect(passage(page, 12)).toHaveCSS('font-variation-settings', '"opsz" 12');
    await expect(passage(page, 17)).toHaveCSS('font-variation-settings', '"opsz" 17');
    await expect(passage(page, 24)).toHaveCSS('font-variation-settings', '"opsz" 24');
    await expect(inkPassage(page)).toHaveCSS('font-variation-settings', '"opsz" 17');
  });

  test('a family without opsz gets none: a variable face keeps its axes, a static one nothing', async ({
    page,
  }) => {
    await open(page);
    await expect(passage(page, 12)).toHaveCSS('font-variation-settings', '"wght" 400');
    await serveFonts(page);
    await page.goto('/?mockLibrary=wall#/family/501');
    await page
      .getByRole('group', { name: 'Section' })
      .getByRole('button', { name: 'Text', exact: true })
      .click();
    await expect(passage(page, 24)).toHaveCSS('font-variation-settings', 'normal');
  });

  test('a weight chosen on the hero carries into every column', async ({ page }) => {
    await open(page);
    await axesRow(page)
      .getByRole('slider', { name: /Weight/ })
      .fill('650');
    for (const px of [12, 17, 24])
      await expect(passage(page, px)).toHaveCSS('font-variation-settings', '"wght" 650');
    await expect(inkPassage(page)).toHaveCSS('font-variation-settings', '"wght" 650');
  });
});

test.describe('the sliders', () => {
  test('measure runs 40 to 90 ch and sets the width of all three columns and the ink', async ({
    page,
  }) => {
    await open(page);
    await expect(measure(page)).toHaveAttribute('min', '40');
    await expect(measure(page)).toHaveAttribute('max', '90');
    await expect(measure(page)).toHaveValue('45');
    await expect(page.getByText('45 ch', { exact: true })).toBeVisible();

    await measure(page).fill('80');
    const wide = await Promise.all([12, 17, 24].map((px) => widthOf(passage(page, px))));
    await measure(page).fill('40');
    await expect(page.getByText('40 ch', { exact: true })).toBeVisible();
    const narrow = await Promise.all([12, 17, 24].map((px) => widthOf(passage(page, px))));
    // Half the characters, half the width, in each column and whatever the window.
    wide.forEach((w, i) => expect(narrow[i]! / w).toBeCloseTo(0.5, 2));
    // The columns share one width.
    expect(narrow[1]).toBeCloseTo(narrow[0]!, 2);
    expect(narrow[2]).toBeCloseTo(narrow[0]!, 2);
    // 40 ch as the browser measures it: the columns at the caption size, the ink at the body size.
    // Not a ratio of the two sizes: hinted text on Linux rounds a ch to whole pixels.
    expect(narrow[0]).toBeCloseTo(40 * (await chOf(passage(page, 12))), 1);
    expect(await widthOf(inkPassage(page))).toBeCloseTo(40 * (await chOf(inkPassage(page))), 1);
  });

  test('leading runs 1.10 to 1.80 and sets the line height of all three and the ink', async ({
    page,
  }) => {
    await open(page);
    await expect(leading(page)).toHaveAttribute('min', '1.1');
    await expect(leading(page)).toHaveAttribute('max', '1.8');
    await leading(page).fill('1.8');
    await expect(page.getByText('1.80', { exact: true })).toBeVisible();
    for (const px of [12, 17, 24])
      await expect(passage(page, px)).toHaveCSS('line-height', `${px * 1.8}px`);
    await expect(inkPassage(page)).toHaveCSS('line-height', `${17 * 1.8}px`);
    await leading(page).fill('1.1');
    await expect(passage(page, 12)).toHaveCSS('line-height', '13.2px');
  });

  for (const [width, height] of [
    [1280, 800],
    [1000, 700],
  ] as const) {
    test(`at the default all three columns show side by side at ${width}x${height}`, async ({
      page,
    }) => {
      await page.setViewportSize({ width, height });
      await open(page);
      const row = page.getByTestId('family-text-row');
      const [scrollWidth, clientWidth] = await row.evaluate((el) => [
        el.scrollWidth,
        el.clientWidth,
      ]);
      expect(scrollWidth).toBe(clientWidth);
      const tops = await Promise.all([12, 17, 24].map((px) => passage(page, px).boundingBox()));
      expect(tops[1]!.x).toBeGreaterThan(tops[0]!.x + tops[0]!.width);
      expect(tops[2]!.x).toBeGreaterThan(tops[1]!.x + tops[1]!.width);
    });
  }

  test('a longer measure scrolls the columns sideways inside the section, not the page', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1000, height: 700 });
    await open(page);
    await measure(page).fill('90');
    const row = page.getByTestId('family-text-row');
    const sizes = await row.evaluate((el) => [el.scrollWidth, el.clientWidth]);
    expect(sizes[0]).toBeGreaterThan(sizes[1]!);
    const scroller = page.locator('main.scroll');
    const [scrollWidth, clientWidth] = await scroller.evaluate((el) => [
      el.scrollWidth,
      el.clientWidth,
    ]);
    expect(scrollWidth).toBe(clientWidth);
  });

  test('the arrow keys scroll the focused row; they do not change the family', async ({ page }) => {
    await open(page);
    await measure(page).fill('90');
    const before = page.url();
    const row = page.getByTestId('family-text-row');
    await row.focus();
    await page.keyboard.press('ArrowRight');
    await expect.poll(() => row.evaluate((el) => el.scrollLeft)).toBeGreaterThan(0);
    expect(page.url()).toBe(before);
    // Outside the row they still do.
    await page.getByRole('heading', { level: 1 }).focus();
    await page.keyboard.press('ArrowRight');
    await expect(page).not.toHaveURL(before);
  });
});

test.describe('paper and ink', () => {
  for (const colorScheme of ['light', 'dark'] as const) {
    test(`Ink is light on dark and Paper dark on light in the ${colorScheme} theme`, async ({
      page,
    }) => {
      await page.emulateMedia({ colorScheme });
      await open(page);
      await expect(ground(page, 'Paper')).toHaveAttribute('aria-pressed', 'true');
      let shown = await colors(section(page));
      expect(luminance(parse(shown.background))).toBeGreaterThan(luminance(parse(shown.color)));

      await ground(page, 'Ink').click();
      await expect(ground(page, 'Ink')).toHaveAttribute('aria-pressed', 'true');
      shown = await colors(section(page));
      expect(luminance(parse(shown.background))).toBeLessThan(luminance(parse(shown.color)));
    });

    test(`every text reaches 4.5:1 on both grounds in the ${colorScheme} theme`, async ({
      page,
    }) => {
      await page.emulateMedia({ colorScheme });
      await open(page);
      for (const name of ['Paper', 'Ink'] as const) {
        await ground(page, name).click();
        const sheet = (await colors(section(page))).background;
        const block = await colors(page.getByTestId('family-text-ink'));
        const label = await colors(page.getByTestId('family-text-row').locator('.label').first());
        const hint = await colors(section(page).locator('.hint'));
        const passageColor = await colors(passage(page, 17));
        expect(ratio(passageColor.color, sheet), `${name} passage`).toBeGreaterThanOrEqual(4.5);
        expect(ratio(label.color, sheet), `${name} label`).toBeGreaterThanOrEqual(4.5);
        expect(ratio(hint.color, sheet), `${name} hint`).toBeGreaterThanOrEqual(4.5);
        expect(ratio(block.color, block.background), `${name} block`).toBeGreaterThanOrEqual(4.5);
      }
    });
  }

  test('the ink block shows the body size on the --fm-fg ground', async ({ page }) => {
    await open(page);
    const block = page.getByTestId('family-text-ink');
    await expect(inkPassage(page)).toHaveCSS('font-size', '17px');
    await expect(inkPassage(page)).toHaveText(STOCK_PASSAGE);
    await expectShownInTheFace(page, inkPassage(page));
    for (const name of ['Paper', 'Ink'] as const) {
      await ground(page, name).click();
      const sheet = await colors(section(page));
      const shown = await colors(block);
      expect(shown.background).toBe(sheet.color);
      expect(shown.color).toBe(sheet.background);
    }
  });

  test('the settings stay while the arrows change the family', async ({ page }) => {
    await open(page);
    await measure(page).fill('50');
    await leading(page).fill('1.6');
    await ground(page, 'Ink').click();
    const name = page.getByRole('heading', { level: 1 });
    const family = await name.textContent();
    await name.focus();
    await page.keyboard.press('ArrowRight');
    await expect(name).not.toHaveText(family!);
    await expect(sectionPanel(page, 'Text')).toBeVisible();
    await expect(measure(page)).toHaveValue('50');
    await expect(leading(page)).toHaveValue('1.6');
    await expect(ground(page, 'Ink')).toHaveAttribute('aria-pressed', 'true');
  });
});

test.describe('the passage', () => {
  async function typeOnWall(page: Page, text: string) {
    await serveFonts(page);
    await page.goto('/?mockLibrary=wall#/');
    await page
      .getByRole('group', { name: 'View mode' })
      .getByRole('button', { name: 'Grid' })
      .click();
    await page
      .getByRole('group', { name: 'Preview settings' })
      .getByRole('textbox', { name: 'Type to preview' })
      .fill(text);
  }

  async function showTextOfFirstFamily(page: Page) {
    await page.evaluate((id) => (location.hash = `#/family/${id}`), WALL_FAMILY_BASE);
    await page
      .getByRole('group', { name: 'Section' })
      .getByRole('button', { name: 'Text', exact: true })
      .click();
  }

  test('is the preview text when it is longer than 80 characters', async ({ page }) => {
    expect([...LONG_TEXT]).toHaveLength(81);
    await typeOnWall(page, LONG_TEXT);
    await showTextOfFirstFamily(page);
    for (const px of [12, 17, 24]) await expect(passage(page, px)).toHaveText(LONG_TEXT);
    await expect(inkPassage(page)).toHaveText(LONG_TEXT);
  });

  test('is the stock passage when the preview text is 80 characters or fewer', async ({ page }) => {
    await typeOnWall(page, LONG_TEXT.slice(0, 80));
    await showTextOfFirstFamily(page);
    await expect(passage(page, 17)).toHaveText(STOCK_PASSAGE);
  });
});

test.describe('the Paragraph preset', () => {
  async function openFirstCard(page: Page, preset: string) {
    await serveFonts(page);
    await page.goto('/?mockLibrary=wall#/');
    await page
      .getByRole('group', { name: 'View mode' })
      .getByRole('button', { name: 'Grid' })
      .click();
    const wall = page.getByRole('listbox', { name: 'Font grid' });
    await expect(wall).toBeVisible();
    await page
      .getByRole('group', { name: 'Preview settings' })
      .getByRole('group', { name: 'Preset' })
      .getByRole('button', { name: preset })
      .click();
    await wall.getByRole('option').first().dblclick();
    await expect(page).toHaveURL(/#\/family\//);
  }

  test('opens a family on the Text section, reading the Paragraph sample', async ({ page }) => {
    await openFirstCard(page, 'Paragraph');
    await expect(sectionPanel(page, 'Text')).toBeVisible();
    await expect(passage(page, 17)).toHaveText(PARAGRAPH);
  });

  test('any other preset opens on Styles', async ({ page }) => {
    await openFirstCard(page, 'Pangram');
    await expect(sectionPanel(page, 'Styles')).toBeVisible();
    await expect(sectionPanel(page, 'Text')).toHaveCount(0);
  });
});

test.describe('@screens', () => {
  const dir = process.env.FM_SCREENS_DIR;
  test.skip(!dir, 'FM_SCREENS_DIR is not set');
  for (const [width, height] of [
    [1280, 800],
    [1000, 700],
  ] as const) {
    test(`the reading test on paper and ink at ${width}x${height}`, async ({ page }) => {
      await page.setViewportSize({ width, height });
      await openWall(page, 'Hedvig Letters Serif');
      await expect(passage(page, 17)).toBeVisible();
      await section(page).scrollIntoViewIfNeeded();
      await page.waitForTimeout(300);
      await page.screenshot({ path: `${dir}/text-paper-${width}x${height}.png` });
      await ground(page, 'Ink').click();
      await page.mouse.move(0, 0);
      await page.waitForTimeout(300);
      await page.screenshot({ path: `${dir}/text-ink-${width}x${height}.png` });
    });
  }
});
