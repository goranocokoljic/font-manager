import { expect, test, type Locator, type Page } from '@playwright/test';

// E41.3: the sidebar as a quiet paper panel, measured in the browser on the wall library,
// whose collections and tags are those of the target image (1-wall-word.png).

const nav = (page: Page) => page.getByRole('navigation', { name: 'Library' });
const row = (page: Page, name: string) =>
  nav(page).getByRole('button', { name, exact: true }).first();

async function open(page: Page, query = 'mockLibrary=wall') {
  await page.goto(`/?${query}#/`);
  await expect(row(page, 'All fonts')).toBeVisible();
}

const css = (l: Locator, property: string) =>
  l.evaluate((el, p) => getComputedStyle(el).getPropertyValue(p), property);
const height = async (l: Locator) => (await l.boundingBox())!.height;

/** Sets the Settings density as the app does: on the root element. */
const compact = (page: Page) =>
  page.evaluate(() => document.documentElement.setAttribute('data-density', 'compact'));

test.describe('the panel', () => {
  for (const [scheme, surface] of [
    ['light', 'rgb(246, 245, 242)'],
    ['dark', 'rgb(38, 38, 40)'],
  ] as const) {
    test(`is the paper tint, 232 px, with a hairline, in the ${scheme} theme`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: scheme });
      await open(page);
      await expect(nav(page)).toHaveCSS('background-color', surface);
      expect((await nav(page).boundingBox())!.width).toBe(232);
      const hairline = page.getByRole('separator', { name: 'Resize sidebar' });
      const line = (await hairline.boundingBox())!;
      expect(line.width).toBe(1);
      expect(line.x).toBe(232);
      await expect(hairline).not.toHaveCSS('background-color', surface);
    });
  }
});

test.describe('the rows', () => {
  test('are 30 px with a 16 px icon, a label and a tabular count at the end', async ({ page }) => {
    await open(page);
    const all = row(page, 'All fonts');
    expect(await height(all)).toBe(30);
    const icon = all.locator('svg');
    expect((await icon.boundingBox())!.width).toBe(16);
    expect((await icon.boundingBox())!.height).toBe(16);
    const count = all.locator('.side-count');
    await expect(count).toHaveCSS('font-variant-numeric', 'tabular-nums');
    const end = (await all.boundingBox())!;
    const box = (await count.boundingBox())!;
    expect(end.x + end.width - (box.x + box.width)).toBeLessThanOrEqual(9);
  });

  test('are 24 px in the compact density, the heading keeping its type', async ({ page }) => {
    await open(page);
    await compact(page);
    expect(await height(row(page, 'All fonts'))).toBe(24);
    await expect(nav(page).locator('h2').first()).toHaveCSS('font-size', '11px');
  });

  test('tint the active row 7 %, and only that row', async ({ page }) => {
    await open(page);
    const all = row(page, 'All fonts');
    await page.mouse.move(600, 400);
    await expect(all).toHaveAttribute('aria-current', 'true');
    await expect(all).toHaveCSS('background-color', 'rgba(0, 0, 0, 0.07)');
    await expect(row(page, 'Favorites')).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
    await row(page, 'Favorites').click();
    await page.mouse.move(600, 400);
    await expect(row(page, 'Favorites')).toHaveCSS('background-color', 'rgba(0, 0, 0, 0.07)');
    await expect(all).not.toHaveAttribute('aria-current', 'true');
    await expect(page.getByRole('heading', { name: 'Favorites', level: 1 })).toBeVisible();
  });
});

test.describe('the section headings', () => {
  test('are 11 px uppercase; their actions appear on hover and on focus', async ({ page }) => {
    await open(page);
    const collections = nav(page).getByRole('region', { name: 'Collections' });
    const heading = collections.getByRole('heading', { name: 'Collections' });
    await expect(heading).toHaveCSS('font-size', '11px');
    // The label is the fold button inside: a button does not inherit the transform by itself.
    await expect(heading.getByRole('button')).toHaveCSS('text-transform', 'uppercase');

    const actions = collections.getByTestId('section-actions');
    await page.mouse.move(600, 400);
    await expect(actions).toHaveCSS('opacity', '0');
    await heading.hover();
    await expect(actions).toHaveCSS('opacity', '1');
    await page.mouse.move(600, 400);
    await expect(actions).toHaveCSS('opacity', '0');

    // By keyboard: the fold button, then the action, in sight as it takes the focus.
    const sources = nav(page).getByRole('region', { name: 'Sources' });
    await sources.getByRole('button', { name: 'Sources' }).focus();
    await expect(sources.getByTestId('section-actions')).toHaveCSS('opacity', '1');
    await page.keyboard.press('Tab');
    await expect(sources.getByRole('button', { name: 'Add folder' })).toBeFocused();
  });

  test('have no text buttons in the list', async ({ page }) => {
    await open(page);
    for (const name of ['Collections', 'Sources', 'Project sets']) {
      const actions = nav(page).getByRole('region', { name }).getByTestId('section-actions');
      const buttons = actions.getByRole('button');
      for (const b of await buttons.all()) await expect(b).toHaveText('');
      expect(await buttons.count()).toBeGreaterThan(0);
    }
  });
});

test.describe('the sections', () => {
  test('mark a collection with its dot and a smart one with a star, not a pill', async ({
    page,
  }) => {
    await open(page);
    const region = nav(page).getByRole('region', { name: 'Collections' });
    const item = (name: string) => region.getByRole('listitem', { name, exact: true });
    const dot = item('Editorial').getByTestId('collection-dot');
    await expect(dot).toBeVisible();
    await expect(dot).toHaveCSS('width', '8px');
    await expect(item('Editorial').getByTestId('collection-smart')).toHaveCount(0);

    const smart = item('Variable');
    await expect(smart.getByTestId('collection-dot')).toHaveCount(0);
    const star = smart.getByTestId('collection-smart');
    const label = smart.getByRole('button', { name: 'Variable', exact: true });
    expect((await star.boundingBox())!.x).toBeLessThan((await label.boundingBox())!.x);
    expect(await css(star, 'border-top-width')).toBe('0px');
  });

  test('ring a focused label round its row, off the tint, open or not', async ({ page }) => {
    await open(page);
    const label = nav(page)
      .getByRole('region', { name: 'Collections' })
      .getByRole('button', { name: 'Editorial', exact: true });
    const expectRing = async (button: Locator) => {
      await expect(button).toBeFocused();
      await expect(button).toHaveCSS('outline-style', 'none');
      const ringed = button.locator('xpath=..');
      await expect(ringed).toHaveCSS('outline-style', 'solid');
      await expect(ringed).toHaveCSS('outline-width', '2px');
      await expect(ringed).toHaveCSS('outline-offset', '2px');
    };
    // Back and forth with Tab: a pointer's focus does not show the ring.
    await label.focus();
    await page.keyboard.press('Shift+Tab');
    await page.keyboard.press('Tab');
    await expectRing(label);

    await label.click();
    await expect(label).toHaveAttribute('aria-current', 'true');
    await page.keyboard.press('Shift+Tab');
    await page.keyboard.press('Tab');
    await expectRing(label);

    const catalog = page.getByTestId('catalog-adobe-fonts');
    await catalog.focus();
    await page.keyboard.press('Shift+Tab');
    await page.keyboard.press('Tab');
    await expectRing(catalog);
  });

  test('count Problems once, at the foot, with a dot and the palette key', async ({ page }) => {
    await open(page);
    const problems = page.getByRole('region', { name: 'Problems' });
    await expect(problems).toHaveCount(1);
    const link = problems.getByTestId('problem-count');
    await expect(link).toHaveText(/^\d+ problems?$/);
    await expect(problems.getByTestId('problem-dot')).toBeVisible();
    // Nowhere else: not as a smart collection, not in the status bar.
    await expect(nav(page).getByRole('listitem', { name: 'Problems' })).toHaveCount(0);
    await expect(page.locator('footer').getByText(/problems?$/)).toHaveCount(0);

    const chord = nav(page).getByRole('button', { name: /Commands/ });
    await expect(chord.locator('kbd')).toHaveText(/K$/);
    await chord.click();
    await expect(page.getByRole('dialog', { name: 'Command palette' })).toBeVisible();
    await page.keyboard.press('Escape');

    // At the bottom of the window's column, with the groups above it.
    const foot = (await link.boundingBox())!;
    const column = (await nav(page).boundingBox())!;
    expect(column.y + column.height - (foot.y + foot.height)).toBeLessThan(16);
  });

  test('keep the foot in sight while a short window scrolls the groups', async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 420 });
    await open(page);
    await nav(page).evaluate((el) => el.scrollTo(0, el.scrollHeight / 2));
    await expect(page.getByTestId('problem-count')).toBeInViewport();
  });

  test('leave the versions to About', async ({ page }) => {
    await open(page);
    await expect(nav(page)).not.toContainText(/\d+\.\d+\.\d+/);
    await expect(page.locator('footer')).not.toContainText(/\d+\.\d+\.\d+/);
    await page.goto('/?mockLibrary=wall#/settings?section=about');
    await page.getByRole('tab', { name: 'About' }).click();
    const info = page.getByTestId('app-info');
    await expect(info).toContainText('Platform');
    await expect(info).toContainText(/\d+\.\d+\.\d+/);
  });
});

test.describe('in right-to-left chrome', () => {
  test.use({ locale: 'ar-XB' });

  test('the panel mirrors: hairline on its left, counts at the left of the rows', async ({
    page,
  }) => {
    await open(page);
    const box = (await nav(page).boundingBox())!;
    const line = (await page.getByRole('separator', { name: 'Resize sidebar' }).boundingBox())!;
    expect(line.x + line.width).toBeLessThanOrEqual(box.x + 0.5);
    const all = row(page, 'All fonts');
    const label = (await all.locator('.side-label').boundingBox())!;
    const count = (await all.locator('.side-count').boundingBox())!;
    expect(count.x + count.width).toBeLessThanOrEqual(label.x + label.width);
    const icon = (await all.locator('svg').boundingBox())!;
    expect(icon.x).toBeGreaterThan(label.x);
  });
});
