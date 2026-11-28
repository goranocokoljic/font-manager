import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Browser, type Locator, type Page } from '@playwright/test';

// E58.7: the interface size grows everything but the fonts on show (ADR 0029). Browser
// mode zooms the root as the webview's zoom would; every font surface takes it back with
// `fm-true-size`. A size is measured as the browser draws it: the computed font size
// times the element's effective zoom (`currentCSSZoom`), in screen pixels.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

type Size = 'default' | 'large' | 'extraLarge';
const SIZES: { size: Exclude<Size, 'default'>; factor: number }[] = [
  { size: 'large', factor: 1.25 },
  { size: 'extraLarge', factor: 1.5 },
];

/** A page of the mock at one interface size, saved in its settings before it loads. */
async function pageAt(browser: Browser, size: Size): Promise<Page> {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.addInitScript(
    (s) => sessionStorage.setItem('fm.mock.settings.v1', JSON.stringify({ interfaceSize: s })),
    size,
  );
  return page;
}

/** The text size this element is drawn at, in screen pixels. */
const drawnPx = (el: Locator) =>
  el.evaluate((node) => {
    const zoom = (node as Element & { currentCSSZoom?: number }).currentCSSZoom ?? 1;
    return parseFloat(getComputedStyle(node).fontSize) * zoom;
  });

interface Surface {
  name: string;
  open: (page: Page) => Promise<void>;
  specimen: (page: Page) => Locator;
  chrome: (page: Page) => Locator;
}

const SURFACES: Surface[] = [
  {
    name: 'the wall',
    open: async (page) => {
      await page.goto('/?mockLibrary=wall#/');
      await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
    },
    specimen: (page) =>
      page.getByRole('tree', { name: 'Font list' }).locator('.sample:not(.deferred)').first(),
    chrome: (page) =>
      page.getByRole('tree', { name: 'Font list' }).locator('[data-family-name]').first(),
  },
  {
    name: 'the family page',
    open: async (page) => {
      await page.goto('/#/family/1?face=1');
      await expect(page.getByTestId('family-specimen')).toBeVisible();
    },
    specimen: (page) => page.getByTestId('family-specimen'),
    chrome: (page) =>
      page
        .getByRole('group', { name: 'Section' })
        .getByRole('button', { name: 'Text', exact: true }),
  },
  {
    name: 'Settings',
    open: async (page) => {
      await page.goto('/#/settings?section=appearance');
      await expect(page.getByTestId('theme-specimen').first()).toBeVisible();
    },
    specimen: (page) => page.getByTestId('theme-specimen').first(),
    chrome: (page) => page.getByRole('heading', { level: 2 }).first(),
  },
];

for (const surface of SURFACES) {
  for (const { size, factor } of SIZES) {
    test(`${surface.name} at ${factor * 100}%: the specimen keeps its size, the interface text grows`, async ({
      browser,
    }) => {
      const base = await pageAt(browser, 'default');
      await surface.open(base);
      const specimen = await drawnPx(surface.specimen(base));
      const chrome = await drawnPx(surface.chrome(base));
      await base.close();

      const page = await pageAt(browser, size);
      await surface.open(page);
      await expect(page.locator('html')).toHaveCSS('zoom', String(factor));
      expect(await drawnPx(surface.specimen(page))).toBeCloseTo(specimen, 2);
      expect(await drawnPx(surface.chrome(page))).toBeCloseTo(chrome * factor, 2);
      await page.close();
    });
  }
}

/** The cards in the wall's first row. */
async function firstRowCards(page: Page): Promise<number> {
  await page.goto('/?mockLibrary=wall#/');
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Grid' })
    .click();
  const cards = page.getByRole('listbox', { name: 'Font grid' }).getByRole('option');
  await expect(cards.first()).toBeVisible();
  const tops = await cards.evaluateAll((els) => els.map((e) => e.getBoundingClientRect().top));
  return tops.filter((t) => Math.abs(t - tops[0]) < 1).length;
}

/** The screen height of the first card's specimen: its thumbnail mask, drawn at the size set. */
async function thumbHeight(page: Page): Promise<number> {
  const thumb = page.locator('[data-testid="thumbnail"][data-state="ready"]').first();
  await expect(thumb).toBeVisible();
  return thumb.evaluate((node) => {
    const zoom = (node as Element & { currentCSSZoom?: number }).currentCSSZoom ?? 1;
    const size = (node as HTMLElement).style.maskSize || (node as HTMLElement).style.webkitMaskSize;
    return parseFloat(size.split(' ')[1]) * zoom;
  });
}

test('a larger interface shows fewer columns on the wall; the card specimen keeps its size', async ({
  browser,
}) => {
  const base = await pageAt(browser, 'default');
  const columns = await firstRowCards(base);
  const thumb = await thumbHeight(base);
  await base.close();

  const page = await pageAt(browser, 'extraLarge');
  expect(await firstRowCards(page)).toBeLessThan(columns);
  expect(await thumbHeight(page)).toBeCloseTo(thumb, 1);
  await page.close();
});

test('a change of size applies at once, without a reload', async ({ browser }) => {
  const page = await pageAt(browser, 'default');
  await page.goto('/#/settings?section=appearance');
  const heading = page.getByRole('heading', { level: 2 }).first();
  await expect(heading).toBeVisible();
  const before = await drawnPx(heading);
  const specimen = await drawnPx(page.getByTestId('theme-specimen').first());
  // No size picker yet (E58.8): the client the page uses, as the picker will call it.
  await page.evaluate(async () => {
    const mod = await import('/src/api/client.ts');
    await mod.useClient().updateSettings({ interfaceSize: 'large' });
  });
  await expect(page.locator('html')).toHaveCSS('zoom', '1.25');
  expect(await drawnPx(heading)).toBeCloseTo(before * 1.25, 2);
  expect(await drawnPx(page.getByTestId('theme-specimen').first())).toBeCloseTo(specimen, 2);
  await page.close();
});
