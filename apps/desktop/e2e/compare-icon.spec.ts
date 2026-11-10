import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';

// E48.4: one compare icon with two states. Outline adds, filled in the accent removes; on a
// wall card it is hidden at rest, shown on hover, and always shown while the family is in
// the set. A 28 px target with a visible focus ring.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function openWall(page: Page): Promise<Locator> {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/?mockLibrary=wall#/');
  await page.evaluate(() => localStorage.clear());
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Grid' })
    .click();
  const wall = page.getByRole('listbox', { name: 'Font grid' });
  await expect(wall).toBeVisible();
  return wall;
}

/** A wall card's compare icon: in the box right after the card, never inside it. */
const iconOf = (card: Locator) =>
  card.locator('xpath=following-sibling::*[1]').getByTestId('card-compare');

const opacity = (el: Locator) => el.evaluate((e) => Number(getComputedStyle(e).opacity));

test('a wall card shows the icon on hover, adds and removes, and keeps it while in the set', async ({
  page,
}) => {
  const wall = await openWall(page);
  const card = wall.getByRole('option').first();
  const icon = iconOf(card);
  // The icon sits beside the card: an option holds no button.
  await expect(card.getByRole('button')).toHaveCount(0);
  const name = (await card.locator('[data-family-name]').textContent())!.trim();

  // Hidden at rest.
  await page.mouse.move(2, 2);
  await expect.poll(() => opacity(icon)).toBe(0);
  // Shown on hover; a 28 px target.
  await card.hover();
  await expect.poll(() => opacity(icon)).toBe(1);
  const box = (await icon.boundingBox())!;
  expect(Math.round(box.width)).toBe(28);
  expect(Math.round(box.height)).toBe(28);
  await expect(icon).toHaveAttribute('aria-label', `Add ${name} to Compare`);
  await expect(icon).toHaveAttribute('data-in-set', 'false');

  // A click adds the family; the icon fills in the accent and stays after the pointer leaves.
  await icon.click();
  await expect(icon).toHaveAttribute('data-in-set', 'true');
  await expect(icon).toHaveAttribute('aria-label', `Remove ${name} from Compare`);
  // The icon is hidden from assistive technology; the card's name says it is in Compare.
  await expect(card).toHaveAttribute('aria-label', /, in Compare$/);
  const [fills, accent] = await icon.evaluate((e) => {
    const probe = document.createElement('span');
    probe.style.color = 'var(--fm-accent)';
    document.body.append(probe);
    const out = [
      [...e.querySelectorAll('rect')].map((r) => getComputedStyle(r).fill),
      getComputedStyle(probe).color,
    ] as const;
    probe.remove();
    return out;
  });
  expect(fills).toEqual([accent, accent]);
  await page.mouse.move(2, 2);
  await expect.poll(() => opacity(icon)).toBe(1);
  // The card was not selected by the icon.
  await expect(card).toHaveAttribute('aria-selected', 'false');

  // A second click removes it; the icon hides again at rest.
  await card.hover();
  await icon.click();
  await expect(icon).toHaveAttribute('data-in-set', 'false');
  await page.mouse.move(2, 2);
  await expect.poll(() => opacity(icon)).toBe(0);
});

test('the selection bar reads Compare <n>; the sheet faces have the filled icon with a focus ring', async ({
  page,
}) => {
  const wall = await openWall(page);
  const cards = wall.getByRole('option');
  await cards.nth(0).click();
  await cards.nth(1).click({ modifiers: ['ControlOrMeta'] });
  await cards.nth(2).click({ modifiers: ['ControlOrMeta'] });
  const compare = page.getByTestId('bar-compare');
  await expect(compare).toHaveText('Compare 3');
  await compare.click();
  await expect(page.getByTestId('toast')).toContainText('3 fonts added to Compare (3 of 4).');

  const sheet = page.getByRole('dialog', { name: 'Compare' });
  await expect(sheet).toBeVisible();
  const icons = sheet.getByTestId('face-remove');
  await expect(icons).toHaveCount(3);
  // No text Remove button is left in the faces.
  await expect(
    sheet.locator('.face').getByRole('button', { name: 'Remove', exact: true }),
  ).toHaveCount(0);

  // Tab reaches a face's icon: it shows (focus-within) with the 2 px accent ring.
  const first = icons.first();
  for (let i = 0; i < 30; i++) {
    if (await first.evaluate((el) => el === document.activeElement)) break;
    await page.keyboard.press('Tab');
  }
  await expect(first).toBeFocused();
  await expect.poll(() => opacity(sheet.locator('.face-actions').first())).toBe(1);
  const ring = await first.evaluate((e) => {
    const s = getComputedStyle(e);
    return { style: s.outlineStyle, width: s.outlineWidth };
  });
  expect(ring).toEqual({ style: 'solid', width: '2px' });
  const box = (await first.boundingBox())!;
  expect(Math.round(box.width)).toBe(28);
  expect(Math.round(box.height)).toBe(28);

  // Enter removes it; the focus lands on the next face's icon.
  const second = await icons.nth(1).getAttribute('data-remove');
  await page.keyboard.press('Enter');
  await expect(icons).toHaveCount(2);
  await expect(icons.first()).toBeFocused();
  await expect(icons.first()).toHaveAttribute('data-remove', second!);
});

test.describe('@screens', () => {
  const dir = process.env.FM_SCREENS_DIR;
  test.skip(!dir, 'FM_SCREENS_DIR is not set');
  for (const [width, height] of [
    [1280, 800],
    [1000, 700],
  ] as const) {
    test(`the compare icon on the wall and in the sheet at ${width}x${height}`, async ({
      page,
    }) => {
      const wall = await openWall(page);
      await page.setViewportSize({ width, height });
      const cards = wall.getByRole('option');
      await cards.nth(0).hover();
      await iconOf(cards.nth(0)).click();
      await cards.nth(2).hover();
      await iconOf(cards.nth(2)).click();
      await cards.nth(1).hover();
      await page.screenshot({ path: `${dir}/compare-icon-wall-${width}x${height}.png` });
      await cards.nth(3).click();
      await cards.nth(4).click({ modifiers: ['ControlOrMeta'] });
      await page.screenshot({ path: `${dir}/compare-icon-bar-${width}x${height}.png` });
      await page.getByTestId('bar-compare').click();
      const sheet = page.getByRole('dialog', { name: 'Compare' });
      await sheet.locator('.face').first().hover();
      await page.screenshot({ path: `${dir}/compare-icon-sheet-${width}x${height}.png` });
    });
  }
});
