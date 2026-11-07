import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';

// Multi-select and the selection bar (E44.1, board `4-drag.png`).
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
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/?mockLibrary=wall#/');
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
  await viewButton(page, 'Grid').click();
  const wall = page.getByRole('listbox', { name: 'Font grid' });
  await expect(wall).toBeVisible();
  return wall;
}

const viewButton = (page: Page, name: string) =>
  page.getByRole('group', { name: 'View mode' }).getByRole('button', { name });
const cards = (wall: Locator) => wall.getByRole('option');
const selected = (wall: Locator) => wall.locator('[role="option"][aria-selected="true"]');
const bar = (page: Page) => page.getByRole('toolbar', { name: 'Selection' });

test.describe('multi-select on the wall', () => {
  test('click, Shift-click, Ctrl/Cmd-click, Ctrl/Cmd-A and Esc', async ({ page }) => {
    const wall = await openWall(page);
    await cards(wall).nth(0).click();
    await expect(selected(wall)).toHaveCount(1);
    await cards(wall)
      .nth(5)
      .click({ modifiers: ['Shift'] });
    await expect(selected(wall)).toHaveCount(6);
    await cards(wall)
      .nth(2)
      .click({ modifiers: ['ControlOrMeta'] });
    await expect(selected(wall)).toHaveCount(5);
    await expect(cards(wall).nth(2)).toHaveAttribute('aria-selected', 'false');
    await page.getByTestId('count-line').getByText('5 selected').waitFor();
    await expect(page.getByTestId('count-line').locator('.count')).toHaveText(
      '5 selected · Shift-click or drag a box to select · Esc clears',
    );
    // Ctrl/Cmd-A: the filtered set, every family the wall shows.
    await page.keyboard.press('ControlOrMeta+a');
    await expect(bar(page).getByTestId('selection-bar-name')).toHaveText('60 families');
    await page.keyboard.press('Escape');
    await expect(selected(wall)).toHaveCount(0);
    await expect(bar(page)).toBeHidden();
  });

  test('a box drawn on the ground selects the cards it touches', async ({ page }) => {
    const wall = await openWall(page);
    const first = (await cards(wall).nth(0).boundingBox())!;
    const second = (await cards(wall).nth(1).boundingBox())!;
    const box = (await wall.boundingBox())!;
    // From the ground above the first line, between its first two cards, down into both.
    const startX = (first.x + first.width + second.x) / 2;
    await page.mouse.move(startX, box.y + 4);
    await page.mouse.down();
    await page.mouse.move(startX + 30, first.y + 40, { steps: 4 });
    await expect(wall.getByTestId('selection-box')).toBeVisible();
    await page.mouse.up();
    await expect(wall.getByTestId('selection-box')).toBeHidden();
    await expect(selected(wall)).toHaveCount(1);
    await expect(cards(wall).nth(1)).toHaveAttribute('aria-selected', 'true');
    // Wider, from the left inset across the first line: every card of it.
    await page.mouse.move(box.x + 6, box.y + 4);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width - 30, first.y + 40, { steps: 6 });
    await page.mouse.up();
    const perLine = Number(await wall.getAttribute('data-columns'));
    await expect(selected(wall)).toHaveCount(perLine);
  });

  test('the selection survives a view switch', async ({ page }) => {
    const wall = await openWall(page);
    await cards(wall).nth(1).click();
    await cards(wall)
      .nth(3)
      .click({ modifiers: ['Shift'] });
    const keys = await selected(wall).evaluateAll((els) => els.map((e) => e.dataset.key));
    await viewButton(page, 'List').click();
    const tree = page.getByRole('tree', { name: 'Font list' });
    await expect(tree.locator('[aria-selected="true"]')).toHaveCount(3);
    await viewButton(page, 'Grid').click();
    await expect(selected(wall)).toHaveCount(3);
    expect(await selected(wall).evaluateAll((els) => els.map((e) => e.dataset.key))).toEqual(keys);
    await expect(bar(page)).toBeVisible();
  });

  for (const scheme of ['light', 'dark'] as const) {
    test(`${scheme}: a selected card has the accent ring and a 6 % accent tint`, async ({
      page,
    }) => {
      await page.emulateMedia({ colorScheme: scheme });
      const wall = await openWall(page);
      await cards(wall).nth(0).click();
      await cards(wall)
        .nth(2)
        .click({ modifiers: ['ControlOrMeta'] });
      const looks = (i: number) =>
        cards(wall)
          .nth(i)
          .locator('.card')
          .evaluate((el) => {
            const ring = getComputedStyle(el, '::after');
            const probe = document.createElement('div');
            probe.style.color = 'var(--fm-accent)';
            el.append(probe);
            const accent = getComputedStyle(probe).color;
            probe.style.color =
              'color-mix(in srgb, var(--fm-accent) 6%, var(--fm-card-bg, var(--fm-bg)))';
            const tint = getComputedStyle(probe).color;
            probe.remove();
            return {
              background: getComputedStyle(el).backgroundColor,
              ringColor: ring.borderTopColor,
              ringWidth: ring.borderTopWidth,
              ringOpacity: ring.opacity,
              accent,
              tint,
            };
          });
      // The ring's 120 ms fade.
      await page.waitForTimeout(200);
      const on = await looks(2);
      const off = await looks(1);
      expect(on.ringColor).toBe(on.accent);
      expect(on.ringWidth).toBe('2px');
      expect(on.ringOpacity).toBe('1');
      expect(on.background).toBe(on.tint);
      expect(off.ringOpacity).toBe('0');
      expect(off.background).not.toBe(on.background);
    });
  }
});

test.describe('the selection bar', () => {
  test('rises on the first selection: dark, white on it, a toolbar reachable with Tab', async ({
    page,
  }) => {
    const wall = await openWall(page);
    await expect(bar(page)).toBeHidden();
    await cards(wall).nth(0).click();
    await cards(wall)
      .nth(3)
      .click({ modifiers: ['ControlOrMeta'] });
    await cards(wall)
      .nth(5)
      .click({ modifiers: ['ControlOrMeta'] });
    await expect(bar(page)).toBeVisible();
    await expect(bar(page).getByTestId('selection-bar-name')).toHaveText('3 families');
    await expect(bar(page)).toHaveCSS('background-color', 'rgb(20, 20, 19)');
    await expect(bar(page)).toHaveCSS('color', 'rgb(255, 255, 255)');
    await expect(bar(page).getByRole('button')).toHaveText([
      'Activate',
      'Tag',
      'Compare',
      'Studio',
      '',
    ]);
    await expect(bar(page).getByRole('combobox', { name: 'Add to collection' })).toHaveText(
      'Collection',
    );
    // Centred at the bottom of the library.
    const b = (await bar(page).boundingBox())!;
    const w = (await wall.boundingBox())!;
    expect(Math.abs(b.x + b.width / 2 - (w.x + w.width / 2))).toBeLessThan(2);
    expect(w.y + w.height - (b.y + b.height)).toBeGreaterThan(8);
    // Tab from the wall reaches the bar's first action; Esc in the bar clears the selection.
    await wall.focus();
    for (let i = 0; i < 40; i++) {
      await page.keyboard.press('Tab');
      if (
        await bar(page)
          .getByRole('button', { name: 'Activate' })
          .evaluate((el) => el === document.activeElement)
      )
        break;
    }
    await expect(bar(page).getByRole('button', { name: 'Activate' })).toBeFocused();
    await page.keyboard.press('Escape');
    await expect(bar(page)).toBeHidden();
    await expect(selected(wall)).toHaveCount(0);
    await expect(wall).toBeFocused();
  });

  test('rises 16 px over the move duration', async ({ page }) => {
    const wall = await openWall(page);
    await cards(wall)
      .nth(0)
      .click({ modifiers: ['ControlOrMeta'] });
    const dock = page.locator('.dock');
    const start = await dock.evaluate((el) => {
      const s = getComputedStyle(el);
      return { transform: s.transform, duration: s.transitionDuration };
    });
    expect(start.duration).toContain('0.2s');
    // translateY(16px) as a matrix at the start of the enter transition.
    expect(start.transform).toMatch(/matrix\(1, 0, 0, 1, 0, 1?\d(\.\d+)?\)/);
    await expect(dock).toHaveCSS('transform', 'none');
  });

  test('Activate and Collection act on the whole selection', async ({ page }) => {
    const wall = await openWall(page);
    await cards(wall).nth(0).click();
    await cards(wall)
      .nth(1)
      .click({ modifiers: ['Shift'] });
    await bar(page).getByRole('button', { name: 'Activate' }).click();
    await expect(cards(wall).nth(0).locator('[data-activation="active"]')).toHaveCount(1);
    await expect(cards(wall).nth(1).locator('[data-activation="active"]')).toHaveCount(1);
    await expect(bar(page).getByRole('button', { name: 'Deactivate' })).toBeVisible();

    // A collection to add to (browser mode starts without one).
    const sidebar = page.getByRole('region', { name: 'Collections' });
    await sidebar.getByRole('button', { name: 'New collection', exact: true }).click();
    const input = sidebar.getByRole('textbox', { name: 'New collection name' });
    await input.fill('Editorial');
    await input.press('Enter');
    const item = sidebar.getByRole('listitem', { name: 'Editorial', exact: true });
    await expect(item).toBeVisible();
    const collection = bar(page).getByRole('combobox', { name: 'Add to collection' });
    await collection.click();
    await page.getByRole('option', { name: 'Editorial' }).click();
    await expect(page.getByTestId('toast')).toContainText('Added to Editorial');
    await expect(collection).toBeFocused();
    // Empty before; now the faces of both families (the count is of fonts).
    await expect(item).toHaveText(/^Editorial[1-9][0-9]*$/);
  });
});
