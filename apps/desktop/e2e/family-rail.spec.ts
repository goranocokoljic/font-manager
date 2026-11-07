import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { hero, showSection } from './family-page';

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);
const POLISH = 'Pchnąć w tę łódź jeża lub ośm skrzyń fig';

// `?mockFaces=20` gives face 1 (Inter) candidates to pair with. It covers English, Polish and
// both Serbian presets of the mock in full.
async function openRail(page: Page, search = '?mockFaces=20') {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(`/${search}#/family/1?face=1`);
  const rail = page.getByTestId('family-rail');
  await expect(rail.getByTestId('rail-language').first()).toBeVisible();
  return rail;
}
const chip = (rail: ReturnType<Page['getByTestId']>, name: string) =>
  rail.getByRole('button', { name, exact: true });

test('"Supports N languages": the complete ones as chips', async ({ page }) => {
  const rail = await openRail(page);
  await expect(rail.getByRole('heading', { name: 'Supports 4 languages' })).toBeVisible();
  await expect(rail.getByTestId('rail-language')).toHaveText([
    'English',
    'Polish',
    'Serbian (Latin)',
    'Serbian (Cyrillic)',
  ]);
  await expect(rail.getByTestId('rail-languages-more')).toHaveCount(0);
});

test("a chip previews the page in the language's pangram; again, or back to my text, restores", async ({
  page,
}) => {
  const rail = await openRail(page);
  const mine = await hero(page).innerText();
  expect(mine).not.toBe(POLISH);
  const polish = chip(rail, 'Polish');
  await polish.click();
  await expect(hero(page)).toHaveText(POLISH);
  await expect(polish).toHaveAttribute('aria-pressed', 'true');
  const accent = await page.evaluate(() =>
    getComputedStyle(document.documentElement).getPropertyValue('--fm-accent').trim(),
  );
  expect(accent).not.toBe('');
  const accentRgb = await page.evaluate((c) => {
    const probe = document.createElement('span');
    probe.style.color = c;
    document.body.append(probe);
    const rgb = getComputedStyle(probe).color;
    probe.remove();
    return rgb;
  }, accent);
  // Polled: the chip's colours ease in.
  await expect
    .poll(() => polish.evaluate((el) => getComputedStyle(el).backgroundColor))
    .toBe(accentRgb);
  await expect(rail.getByTestId('rail-language-chosen')).toContainText(
    'Every specimen on this page now shows Polish.',
  );
  // Every specimen: the Styles rows show it too.
  await expect(page.getByTestId('family-style-sample').first()).toHaveText(POLISH);

  await polish.click();
  await expect(hero(page)).toHaveText(mine);
  await expect(polish).toHaveAttribute('aria-pressed', 'false');

  await polish.click();
  await expect(hero(page)).toHaveText(POLISH);
  await rail.getByRole('button', { name: 'back to my text' }).click();
  await expect(hero(page)).toHaveText(mine);
  await expect(rail.getByTestId('rail-language-chosen')).toHaveCount(0);
});

test("under the chips, a 22 px line of the language's glyphs in the face", async ({ page }) => {
  const rail = await openRail(page);
  const line = rail.getByTestId('family-rail-sample');
  await chip(rail, 'Polish').click();
  await expect(line).toHaveText('Ąćęłńóśźż');
  await expect(line).toHaveCSS('font-size', '22px');
  await expect(line).toBeVisible();
  // The face itself, never a fallback: the same held family as the hero.
  const family = await hero(page).evaluate((el) => getComputedStyle(el).fontFamily);
  await expect(line).toHaveCSS('font-family', family);
});

test('"Pairs well with": the top two, one reason each, and Open in Pairing Studio', async ({
  page,
}) => {
  const rail = await openRail(page);
  const pairs = rail.getByTestId('rail-pairing');
  await expect(pairs).toHaveCount(2);
  await expect(pairs.first().locator('.pair-name')).toHaveText('Sample 1');
  // One reason: the first fm-core gives.
  await expect(pairs.first().locator('.pair-reason')).toHaveText('same designer');
  await rail.getByRole('button', { name: 'Open in Pairing Studio' }).click();
  await expect(page).toHaveURL(/#\/studio\?face=1000&headline=1$/);
});

test('facts: licence marks, format with files and size, Used in opens the collection', async ({
  page,
}) => {
  const rail = await openRail(page);
  await expect(rail.getByTestId('rail-licence').locator('.mark')).toHaveCount(2);
  await expect(rail.getByTestId('rail-format')).toContainText(/\d+ files? · [\d.]+ (kB|MB)/);
  await expect(rail.getByTestId('rail-used-in')).toHaveText('—');

  await page.getByRole('button', { name: 'Library', exact: true }).click();
  const sidebar = page.getByRole('region', { name: 'Collections' });
  await sidebar.getByRole('button', { name: 'New collection', exact: true }).click();
  const input = sidebar.getByRole('textbox', { name: 'New collection name' });
  await input.fill('Picks');
  await input.press('Enter');
  await expect(sidebar.getByRole('listitem', { name: 'Picks', exact: true })).toBeVisible();
  await page.evaluate(() => (location.hash = '#/family/1?face=1'));
  await page.getByRole('combobox', { name: 'Add to collection' }).click();
  await page.getByRole('option', { name: 'Picks' }).click();
  const used = page.getByTestId('rail-used-in').getByRole('button', { name: 'Picks' });
  await expect(used).toBeVisible();
  await used.click();
  await expect(page).not.toHaveURL(/family/);
  await expect(
    sidebar.getByRole('listitem', { name: 'Picks', exact: true }).getByRole('button', {
      name: 'Picks',
      exact: true,
    }),
  ).toHaveAttribute('aria-current', 'true');
});

test('"Show only fonts that can do Polish" filters the wall on the way back', async ({ page }) => {
  const rail = await openRail(page);
  await expect(rail.getByTestId('rail-filter-wall')).toHaveCount(0);
  await chip(rail, 'Polish').click();
  const offer = rail.getByRole('button', { name: 'Show only fonts that can do Polish' });
  await offer.click();
  await expect(offer).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('button', { name: 'Library', exact: true }).click();
  await expect(page).not.toHaveURL(/family/);
  await expect(page.getByRole('button', { name: 'Remove filter Polish' })).toBeVisible();
});

test('without the offer chosen, back leaves the wall unfiltered', async ({ page }) => {
  const rail = await openRail(page);
  await chip(rail, 'Polish').click();
  await page.getByRole('button', { name: 'Library', exact: true }).click();
  await expect(page).not.toHaveURL(/family/);
  await expect(page.getByRole('button', { name: 'Remove filter Polish' })).toHaveCount(0);
});

test('the chosen language stays while the section changes', async ({ page }) => {
  const rail = await openRail(page);
  await chip(rail, 'Polish').click();
  await showSection(page, 'Features');
  await expect(hero(page)).toHaveText(POLISH);
  await expect(chip(rail, 'Polish')).toHaveAttribute('aria-pressed', 'true');
});

test.describe('@screens', () => {
  const dir = process.env.FM_SCREENS_DIR;
  test.skip(!dir, 'FM_SCREENS_DIR is not set');
  for (const [width, height] of [
    [1280, 800],
    [1000, 700],
  ] as const) {
    test(`the rail previewing in Polish at ${width}x${height}`, async ({ page }) => {
      await page.setViewportSize({ width, height });
      const rail = await openRail(page);
      await chip(rail, 'Polish').click();
      await rail.scrollIntoViewIfNeeded();
      await page.waitForTimeout(400);
      await page.screenshot({ path: `${dir}/family-rail-${width}x${height}.png` });
    });
  }
});
