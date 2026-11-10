import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// Similar to the base (E48.3) on the mock client: the Compare sheet lists the faces
// `findSimilar` ranks closest to the base, grouped by the reasons they share. `mockFaces=3`
// adds Sample 1–3; Sample 1 and 3 copy Inter, Sample 2 copies JetBrains Mono.
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
  await page.goto('/?mockFaces=3#/');
  await page.evaluate(() => localStorage.clear());
  const tree = page.getByRole('tree', { name: 'Font list' });
  await expect(tree.getByRole('treeitem')).toHaveCount(6);
  await tree.getByRole('treeitem').first().click();
  return tree;
}

const sheet = (page: Page) => page.getByRole('dialog', { name: 'Compare' });
const band = (page: Page) => sheet(page).getByRole('region', { name: /^Similar to / });
const rows = (page: Page) => band(page).getByTestId('similar-row');
const pinned = (page: Page) =>
  sheet(page).getByRole('list', { name: 'Stacked' }).getByRole('listitem');

test('@screens the sheet lists similar faces on a band, grouped by reason; the icon brings one into the set', async ({
  page,
}) => {
  await open(page);
  await page.keyboard.press('c');

  // No Pairings panel any more; the band names the base.
  await expect(sheet(page).getByTestId('pairings-panel')).toHaveCount(0);
  await expect(band(page)).toBeVisible();
  await expect(band(page)).toHaveAccessibleName('Similar to Inter');
  await expect(rows(page).first()).toBeVisible();
  const count = await rows(page).count();
  expect(count).toBeGreaterThan(0);
  expect(count).toBeLessThanOrEqual(8);
  await expect(band(page)).not.toContainText('%');
  await expect(rows(page).filter({ hasText: 'Inter Regular' })).toHaveCount(0);

  // A sunken band, not more of the set: its own background, white cards.
  const bg = await band(page).evaluate((el) => getComputedStyle(el).backgroundColor);
  const sheetBg = await sheet(page).evaluate((el) => getComputedStyle(el).backgroundColor);
  expect(bg).not.toBe(sheetBg);
  const card = await rows(page)
    .first()
    .evaluate((el) => getComputedStyle(el).backgroundColor);
  expect(card).toBe(sheetBg);

  // Each reason once, as a header; the rows do not repeat it.
  const headers = await band(page).getByTestId('similar-reason').allInnerTexts();
  expect(new Set(headers).size).toBe(headers.length);

  // The row's icon adds Sample 1; filled now; a second click removes it.
  const row = rows(page).filter({ hasText: 'Sample 1 Regular' });
  await row.getByRole('button', { name: 'Add Sample 1 Regular to Compare' }).click();
  await expect(pinned(page)).toHaveCount(2);
  await expect(pinned(page).nth(1)).toContainText('Sample 1 Regular');
  const done = row.getByRole('button', { name: 'Remove Sample 1 Regular from Compare' });
  await expect(done).toHaveAttribute('data-in-set', 'true');
  await expect(rows(page).filter({ hasText: 'Sample 1 Regular' })).toHaveCount(1);

  if (process.env.FM_SCREENS_DIR) {
    for (const [w, h] of [
      [1280, 800],
      [1000, 700],
    ] as const) {
      await page.setViewportSize({ width: w, height: h });
      // The band's top in the middle: the end of the set above it, the band below.
      await band(page).evaluate((el) => {
        el.scrollIntoView({ block: 'start' });
        const body = el.parentElement;
        if (body) body.scrollTop -= body.clientHeight / 2;
      });
      await page.screenshot({
        path: `${process.env.FM_SCREENS_DIR}/compare-similar-${w}x${h}.png`,
      });
    }
  }

  await done.click();
  await expect(pinned(page)).toHaveCount(1);
});

test('a new base reloads the section', async ({ page }) => {
  const tree = await open(page);
  await page.keyboard.press('c');
  await expect(band(page)).toHaveAccessibleName('Similar to Inter');
  await page.keyboard.press('Escape');
  await tree.getByRole('treeitem').nth(2).click();
  await page.keyboard.press('c');
  await expect(band(page)).toHaveAccessibleName(/^Similar to Legacy Grotesk/);
});
