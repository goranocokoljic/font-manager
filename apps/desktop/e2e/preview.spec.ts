import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// The mock client loads face files from `/fmfont/<id>` (the app's `fmfont`
// scheme); here every face is served a real variable fixture font.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function serveFonts(page: Page): Promise<string[]> {
  const requested: string[] = [];
  await page.route('**/fmfont/*', (route) => {
    requested.push(new URL(route.request().url()).pathname);
    return route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture });
  });
  return requested;
}

/** Rows on screen, faces registered by the preview rows, and how many have loaded. */
function previewFaces(page: Page) {
  return page.evaluate(() => {
    const faces = [...document.fonts].filter((f) => f.family.includes('fm-face-'));
    return {
      rows: document.querySelectorAll('[role="treeitem"]').length,
      total: faces.length,
      loaded: faces.filter((f) => f.status === 'loaded').length,
    };
  });
}

/** Waits until the scroll event has run and the list has re-rendered. */
function nextFrames(page: Page) {
  return page.evaluate(
    () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
  );
}

test('a preview row renders its sample in the face loaded by id', async ({ page }) => {
  const requested = await serveFonts(page);
  await page.goto('/#/');
  const row = page.locator('[data-face="1"]');
  await expect(row).toHaveAttribute('data-state', 'ready');
  expect(requested).toContain('/fmfont/1');
  const sample = row.locator('.sample');
  await expect(sample).toHaveCSS('font-family', /fm-face-1/);
  expect(await page.evaluate(() => document.fonts.check('28px "fm-face-1"'))).toBe(true);
  // Inter is variable (wght 100–900, default 400); JetBrains Mono is static.
  await expect(sample).toHaveCSS('font-variation-settings', '"wght" 400');
  await expect(page.locator('[data-face="6"] .sample')).toHaveCSS(
    'font-variation-settings',
    'normal',
  );
});

test('a face that cannot load falls back and says so', async ({ page }) => {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 404, contentType: 'text/plain', body: 'Face was not found.' }),
  );
  await page.goto('/#/');
  const row = page.locator('[data-face="6"]');
  await expect(row).toHaveAttribute('data-state', 'failed');
  await expect(row).toContainText('Preview not available');
});

test('1,000 rows scrolled do not leak font faces', async ({ page }) => {
  // About 60 scroll steps, each waiting for its faces to load: some 20 s on a fast
  // machine, so a slow CI runner (macOS) needs more than the default 30 s.
  test.slow();
  await serveFonts(page);
  await page.goto('/?mockFaces=1000#/');
  const list = page.getByRole('tree', { name: 'Font list' });
  await expect(list.getByRole('treeitem').first()).toBeVisible();
  let steps = 0;
  for (;;) {
    const atEnd = await list.evaluate((el) => {
      el.scrollTop += el.clientHeight / 2;
      return el.scrollTop + el.clientHeight >= el.scrollHeight - 1;
    });
    await nextFrames(page);
    // A step can be fast enough to defer the rows (E35.8): wait for them to settle.
    await expect(list.locator('[data-state="deferred"]')).toHaveCount(0);
    await expect(list.locator('[data-state="loading"]')).toHaveCount(0);
    const faces = await previewFaces(page);
    // The rows on screen plus at most 48 released faces (DEFAULT_MAX_IDLE) stay registered.
    expect(faces.total).toBeLessThanOrEqual(faces.rows + 48);
    expect(faces.loaded).toBeGreaterThanOrEqual(faces.rows);
    steps += 1;
    if (atEnd) break;
  }
  expect(steps).toBeGreaterThan(50);
  await expect(list.getByRole('treeitem').last()).toHaveAttribute('aria-posinset', '1003');
});
