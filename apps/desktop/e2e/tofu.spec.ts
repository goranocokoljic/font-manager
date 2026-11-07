import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { wallFixtures } from '../src/api/mockWall';
import { faceHash, hero } from './family-page';

// Tofu (E43.4, boards 3-wall-text.png and 3-wall-text-complete.png): a character a face
// lacks is a 0.62 em accent box, never a glyph from a fallback font. The wall library's
// families without Latin Extended-A cannot draw Serbian; MockApi face 7 (Legacy Grotesk)
// maps basic Latin only.
const files = wallFixtures();
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function openWall(page: Page): Promise<Locator> {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.route('**/fmfont/*', (route) => {
    const id = Number(new URL(route.request().url()).pathname.split('/').pop());
    const path = files.get(id);
    if (!path) return route.fulfill({ status: 404 });
    return route.fulfill({
      path: fileURLToPath(new URL(`../../../fixtures/fonts/${path}`, import.meta.url)),
    });
  });
  await page.goto('/?mockLibrary=wall#/');
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Grid' })
    .click();
  const wall = page.getByRole('listbox', { name: 'Font grid' });
  await expect(wall).toBeVisible();
  return wall;
}

const field = (page: Page) =>
  page
    .getByRole('group', { name: 'Preview settings' })
    .getByRole('textbox', { name: 'Type to preview' });

/** The rendered width of each box, and the width of the run of boxes `from`..`to` together. */
async function boxWidths(text: Locator) {
  return text.evaluate((el) => {
    const boxes = [...el.querySelectorAll<HTMLElement>('[role="img"]')];
    const px = parseFloat(getComputedStyle(el).fontSize);
    return { px, widths: boxes.map((b) => b.getBoundingClientRect().width) };
  });
}

/** The width the face's text takes for the run of the first `n` boxes in a row (no text between). */
async function runWidth(text: Locator, n: number) {
  return text.evaluate((el, count) => {
    const boxes = [...el.querySelectorAll<HTMLElement>('[role="img"]')].slice(0, count);
    const range = document.createRange();
    range.setStartBefore(boxes[0]!);
    range.setEndAfter(boxes[boxes.length - 1]!);
    return range.getBoundingClientRect().width;
  }, n);
}

test('a wall card draws the characters its face lacks as accent boxes and counts them', async ({
  page,
}) => {
  const wall = await openWall(page);
  // Short, with a missing character first: a card shows as much of the text as fits.
  await field(page).fill('Šta će đak');
  const gap = wall
    .getByRole('option')
    .filter({ has: page.getByTestId('missing-glyphs') })
    .first();
  await expect(gap).toBeVisible();
  await expect(gap.getByTestId('missing-glyphs')).toHaveText('3 glyphs missing · Š ć đ');

  const text = gap.getByTestId('card-live-text');
  const box = text.getByRole('img', { name: 'missing glyph Š', includeHidden: true });
  await expect(box).toBeVisible();
  await expect(box).toHaveText('');
  // Chromium computes a border in whole pixels (1.5 px shows as 1 px): the rule says 1.5 px.
  const rule = await page.evaluate(() => {
    for (const sheet of document.styleSheets)
      for (const r of sheet.cssRules)
        if (r instanceof CSSStyleRule && r.selectorText === '.fm-tofu')
          return { width: r.style.borderTopWidth, w: r.style.width };
    return null;
  });
  expect(rule).toEqual({ width: '1.5px', w: '0.62em' });
  await expect(box).toHaveCSS('border-top-left-radius', '2px');
  const accent = await page.evaluate(() =>
    getComputedStyle(document.documentElement).getPropertyValue('--fm-accent').trim(),
  );
  const accentRgb = await page.evaluate((c) => {
    const probe = document.createElement('i');
    probe.style.color = c;
    document.body.append(probe);
    const v = getComputedStyle(probe).color;
    probe.remove();
    return v;
  }, accent);
  await expect(box).toHaveCSS('border-top-color', accentRgb);
  // A light accent fill: not transparent, not the solid accent.
  const fill = await box.evaluate((b) => getComputedStyle(b).backgroundColor);
  expect(fill).not.toBe('rgba(0, 0, 0, 0)');
  expect(fill).not.toBe(accentRgb);

  const { px, widths } = await boxWidths(text);
  expect(widths.length).toBeGreaterThan(0);
  for (const w of widths) expect(w).toBeCloseTo(0.62 * px, 1);

  // A card that draws the whole text has no box and no count.
  const whole = wall
    .getByRole('option')
    .filter({ hasNot: page.getByTestId('missing-glyphs') })
    .first();
  await expect(whole.getByRole('img', { includeHidden: true })).toHaveCount(0);
});

test('a known-missing string is exactly as wide as its tofu: no fallback font draws it', async ({
  page,
}) => {
  const wall = await openWall(page);
  // Three missing characters in a row: drawn by a fallback font they would be three glyphs
  // of that font's own widths; as tofu they are three boxes of 0.62 em.
  await field(page).fill('Lepo ššš');
  const gap = wall
    .getByRole('option')
    .filter({ has: page.getByTestId('missing-glyphs') })
    .first();
  await expect(gap).toBeVisible();
  const text = gap.getByTestId('card-live-text');
  await expect(text.getByRole('img', { includeHidden: true })).toHaveCount(3);
  await expect(text.locator('.line')).toHaveText('Lepo');
  const { px, widths } = await boxWidths(text);
  const tofu = widths.reduce((a, b) => a + b, 0);
  expect(tofu).toBeCloseTo(3 * 0.62 * px, 1);
  expect(await runWidth(text, 3)).toBeCloseTo(tofu, 0);
});

test('the list rows draw tofu and say how many glyphs are missing', async ({ page }) => {
  await page.goto('/#/');
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
  await field(page).fill('Love Љубав');
  const legacy = page.getByRole('treeitem').filter({ hasText: 'Legacy Grotesk' }).first();
  await expect(legacy.getByTestId('missing-glyphs')).toHaveText('5 glyphs missing · Љ у б а в');
  await expect(legacy.locator('.sample').getByRole('img')).toHaveCount(5);
  await expect(legacy.locator('.sample')).toHaveText('Love');
});

test('the family page draws tofu in the hero and says what is missing under it', async ({
  page,
}) => {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/#/');
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
  await field(page).fill('Ljubav ššš');
  await page.evaluate((hash) => (location.hash = hash), faceHash(7));
  const specimen = hero(page);
  await expect(specimen).toBeVisible();
  await expect(specimen.getByRole('img', { name: 'missing glyph š' })).toHaveCount(3);
  await expect(page.getByTestId('family-hero').getByTestId('missing-glyphs')).toHaveText(
    '1 glyph missing · š',
  );
  const { px, widths } = await boxWidths(specimen);
  expect(px).toBe(132);
  expect(await runWidth(specimen, 3)).toBeCloseTo(3 * 0.62 * px, 0);
  for (const w of widths) expect(w).toBeCloseTo(0.62 * px, 1);
});
