// The Waterfall section of the family page (E50.10, board 2 `2-family-waterfall.png`).
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { axesRow, hero, openSection, sectionPanel, showSection } from './family-page';

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);
const SIZES = [10, 12, 14, 17, 21, 28, 36, 48, 64, 96];
const POLISH = 'Pchnąć w tę łódź jeża lub ośm skrzyń fig';

const serveFonts = (page: Page) =>
  page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
const lines = (page: Page) => page.getByTestId('family-waterfall-line');
const sample = (page: Page, size: number) =>
  page.locator(`[data-testid="family-waterfall-line"][data-size="${size}"] .sample`);

test('Waterfall follows Characters in the section switch', async ({ page }) => {
  await serveFonts(page);
  await page.goto('/#/family/1?face=1');
  await expect(page.getByRole('group', { name: 'Section' }).getByRole('button')).toHaveText([
    'Styles',
    'Text',
    'Characters',
    'Waterfall',
    'Features',
    'Pairings',
    'Info',
  ]);
});

test("the hero's text and face at ten sizes, 10 to 96 px, each labelled at the left in 10 px mono", async ({
  page,
}) => {
  await serveFonts(page);
  await openSection(page, 1, 'Waterfall');
  await expect(lines(page)).toHaveCount(SIZES.length);
  const text = await hero(page).innerText();
  for (const [i, size] of SIZES.entries()) {
    const line = lines(page).nth(i);
    await expect(line).toHaveAttribute('data-size', String(size));
    await expect(line).toHaveAttribute('data-face', '1');
    const label = line.locator('.label');
    await expect(label).toHaveText(String(size));
    await expect(label).toHaveCSS('font-size', '10px');
    expect(await label.evaluate((el) => getComputedStyle(el).fontFamily)).toMatch(/monospace/);
    await expect(sample(page, size)).toHaveCSS('font-size', `${size}px`);
    await expect(sample(page, size)).toHaveText(text);
    // The label sits at the start of the line, before the text.
    const l = (await label.boundingBox())!;
    const s = (await sample(page, size).boundingBox())!;
    expect(l.x + l.width).toBeLessThanOrEqual(s.x);
  }
});

test('a weight chosen on the hero carries into every line', async ({ page }) => {
  await serveFonts(page);
  await openSection(page, 1, 'Waterfall');
  await axesRow(page)
    .getByRole('slider', { name: /Weight/ })
    .fill('650');
  for (const size of SIZES)
    await expect(sample(page, size)).toHaveCSS('font-variation-settings', '"wght" 650');
});

test("the rail's language applies to every line", async ({ page }) => {
  await serveFonts(page);
  await openSection(page, 1, 'Waterfall', '?mockFaces=20');
  const rail = page.getByTestId('family-rail');
  await rail.getByRole('button', { name: 'Polish', exact: true }).click();
  await expect(hero(page)).toHaveText(POLISH);
  for (const size of SIZES) await expect(sample(page, size)).toHaveText(POLISH);
});

test('the section stays while the arrows change the family, and when the page is left', async ({
  page,
}) => {
  await serveFonts(page);
  await page.goto('/?mockLibrary=wall#/family/501');
  await showSection(page, 'Waterfall');
  await expect(page.getByTestId('family-panel')).toHaveCount(1);
  await page.keyboard.press('ArrowRight');
  await expect(page).toHaveURL(/#\/family\/502$/);
  await expect(page.getByTestId('family-panel')).toHaveAttribute('data-section', 'waterfall');
  await expect(lines(page)).toHaveCount(SIZES.length);
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('family-panel')).toHaveCount(0);
  // Back on a family page (no reload): the section is still Waterfall.
  await page.evaluate(() => (location.hash = '#/family/503'));
  await expect(sectionPanel(page, 'Waterfall')).toBeVisible();
});
