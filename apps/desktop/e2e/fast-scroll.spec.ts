import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// E35.8: while the list moves fast, rows start no face loads and show their name in
// the UI font (`data-state="deferred"`); once it settles, the rows on screen load.
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

test('a fast scroll shows labels while the list moves and faces once it settles', async ({
  page,
}) => {
  const requested = await serveFonts(page);
  await page.goto('/?mockFaces=1000#/');
  const list = page.getByRole('tree', { name: 'Font list' });
  await expect(list.locator('.row[data-state="ready"]').first()).toBeVisible();

  // A fling: one viewport per frame for 20 frames. At the last frame, read what the
  // rows show, before anything can settle.
  const during = await list.evaluate(
    (el) =>
      new Promise<{ deferred: number; labelled: number; loading: number }>((resolve) => {
        let frames = 0;
        const step = () => {
          el.scrollTop += el.clientHeight;
          if (++frames < 20) return requestAnimationFrame(step);
          // One more frame so Vue has patched the rows of the last scroll.
          requestAnimationFrame(() => {
            // A row whose family's styles are not fetched yet is a placeholder
            // with the family name; one with a face shows its name in the sample slot.
            const deferred = [...el.querySelectorAll('[data-state="deferred"]')];
            resolve({
              deferred: deferred.length,
              labelled: deferred.filter((r) =>
                r.querySelector('[data-testid="deferred-sample"], .label')?.textContent?.trim(),
              ).length,
              loading: el.querySelectorAll('.row[data-state="loading"]').length,
            });
          });
        };
        requestAnimationFrame(step);
      }),
  );
  expect(during.deferred).toBeGreaterThan(0);
  expect(during.labelled).toBe(during.deferred);
  expect(during.loading).toBe(0);
  const flung = requested.length;

  // Settled: every row on screen previews its face, and new faces were asked for.
  await expect(list.locator('[data-state="deferred"]')).toHaveCount(0);
  await expect(list.locator('.row[data-state="loading"]')).toHaveCount(0);
  const rows = await list.getByRole('treeitem').count();
  await expect(list.locator('.row[data-state="ready"]')).toHaveCount(rows);
  expect(requested.length).toBeGreaterThan(flung);
});

test('a key that moves the cursor far is settled at once', async ({ page }) => {
  await serveFonts(page);
  await page.goto('/?mockFaces=1000#/');
  const list = page.getByRole('tree', { name: 'Font list' });
  await expect(list.locator('.row[data-state="ready"]').first()).toBeVisible();
  await list.focus();
  await page.keyboard.press('End');
  await expect(list.getByRole('treeitem').last()).toHaveAttribute('aria-posinset', '1003');
  await expect(list.locator('[data-state="deferred"]')).toHaveCount(0);
  await expect(list.locator('.row[data-state="ready"]').last()).toBeVisible();
});
