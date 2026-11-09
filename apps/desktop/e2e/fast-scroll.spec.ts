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

  // A fling: one viewport per frame for 20 frames. Read what the rows show in every
  // frame (the rows of the previous frame's scroll, patched by then) and keep the frame
  // with the most deferred rows. One read after the last frame is a race on a slow
  // machine: the CI macOS VM can rest for SETTLE_MS between that frame and the read, and
  // then it sees a settled list. The claim is about the rows while the list moves.
  const during = await list.evaluate(
    (el) =>
      new Promise<{ deferred: number; labelled: number; loading: number }>((resolve) => {
        let frames = 0;
        let best = { deferred: 0, labelled: 0, loading: 0 };
        const read = () => {
          // A row whose family's styles are not fetched yet is a placeholder
          // with the family name; one with a face shows its name in the sample slot.
          // `loading` is read in that same frame: rows that were loading before the fling
          // (the test waits for the first row only) are still in flight in the first frames
          // and have left the viewport by the frame deepest into the fling.
          const deferred = [...el.querySelectorAll('[data-state="deferred"]')];
          if (deferred.length <= best.deferred) return;
          best = {
            deferred: deferred.length,
            labelled: deferred.filter((r) =>
              r.querySelector('[data-testid="deferred-sample"], .label')?.textContent?.trim(),
            ).length,
            loading: el.querySelectorAll('.row[data-state="loading"]').length,
          };
        };
        const step = () => {
          read();
          el.scrollTop += el.clientHeight;
          if (++frames < 20) return requestAnimationFrame(step);
          // One more frame so Vue has patched the rows of the last scroll.
          requestAnimationFrame(() => {
            read();
            resolve(best);
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
