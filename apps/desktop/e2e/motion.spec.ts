import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// The motion tokens as the page really resolves them (E40.4). styles/motion.test.ts checks
// that components read nothing else, and a11y.spec.ts that nothing moves under "reduce".

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page) {
  await page.route('**/fmfont/*', (r) =>
    r.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/#/');
  await expect(page.getByRole('treeitem').first()).toBeVisible();
}

const token = (page: Page, name: string) =>
  page.evaluate((n) => getComputedStyle(document.documentElement).getPropertyValue(n).trim(), name);

for (const [reducedMotion, fast, move, pageTime, pageExit] of [
  ['no-preference', '120ms', '200ms', '280ms', '220ms'],
  ['reduce', '0ms', '0ms', '0ms', '0ms'],
] as const) {
  test(`the duration tokens under ${reducedMotion}`, async ({ page }) => {
    await page.emulateMedia({ reducedMotion });
    await open(page);
    expect(await token(page, '--fm-motion-fast')).toBe(fast);
    expect(await token(page, '--fm-motion-move')).toBe(move);
    expect(await token(page, '--fm-motion-page')).toBe(pageTime);
    expect(await token(page, '--fm-motion-page-exit')).toBe(pageExit);
    // The one fade reduced motion keeps (E46.1).
    expect(await token(page, '--fm-motion-reduced-fade')).toBe('120ms');
    expect(await token(page, '--fm-ease')).toBe('cubic-bezier(0.2, 0, 0, 1)');
  });
}

test('the tokens follow the setting when it changes while the app runs', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await open(page);
  expect(await token(page, '--fm-motion-move')).toBe('200ms');
  await page.emulateMedia({ reducedMotion: 'reduce' });
  expect(await token(page, '--fm-motion-move')).toBe('0ms');
});

// The real helpers in the real engine: what the browser says is animating.
const EASE = 'cubic-bezier(0.2, 0, 0, 1)';
type Probe = typeof import('./motion-probe');

/** Runs a function of motion-probe.ts in the page, through the dev server. */
function probe<K extends keyof Probe>(page: Page, name: K, ...args: Parameters<Probe[K]>) {
  return page.evaluate(
    async ([fn, arg]) => {
      const url = '/e2e/motion-probe.ts';
      const module = await import(url);
      return module[fn](arg);
    },
    [name, args[0]] as const,
  ) as ReturnType<Probe[K]>;
}

test.describe('the helpers', () => {
  test('FmFade animates opacity at the speed it was given, on the one curve', async ({ page }) => {
    await open(page);
    const seen = await probe(page, 'enterFade', { speed: 'fast' });
    expect(seen).toEqual([{ property: 'opacity', duration: 120, easing: EASE, from: '0' }]);
  });

  test('FmSlide animates transform and opacity only', async ({ page }) => {
    await open(page);
    const seen = await probe(page, 'enterSlide', { from: 'end', speed: 'page' });
    expect(seen).toEqual([
      { property: 'opacity', duration: 280, easing: EASE, from: '0' },
      { property: 'transform', duration: 280, easing: EASE, from: 'translate(24px)' },
    ]);
  });

  test('FmSlide from the end comes from the other side in right-to-left chrome', async ({
    page,
  }) => {
    await open(page);
    await page.evaluate(() => {
      document.documentElement.dir = 'rtl';
    });
    const seen = await probe(page, 'enterSlide', { from: 'end' });
    expect(seen.find((t) => t.property === 'transform')?.from).toBe('translate(-24px)');
  });

  test('FmSlide from the bottom moves on the block axis, by its distance', async ({ page }) => {
    await open(page);
    const seen = await probe(page, 'enterSlide', { from: 'bottom', distance: '16px' });
    expect(seen.find((t) => t.property === 'transform')?.from).toBe('translate(0px, 16px)');
  });

  test('FmFlipGroup animates a move by transform only', async ({ page }) => {
    await open(page);
    const seen = await probe(page, 'swapInFlipGroup');
    expect(seen.map((t) => t.property)).toEqual(['transform', 'transform']);
    expect(seen.every((t) => t.duration === 200 && t.easing === EASE)).toBe(true);
  });

  test('withViewTransition uses the engine, and fn runs', async ({ page }) => {
    await open(page);
    expect(await probe(page, 'changeInViewTransition')).toEqual({ ran: true, started: true });
  });

  test('under reduced motion nothing animates and fn still runs', async ({ page }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await open(page);
    expect(await probe(page, 'enterFade', {})).toEqual([]);
    expect(await probe(page, 'enterSlide', {})).toEqual([]);
    expect(await probe(page, 'swapInFlipGroup')).toEqual([]);
    expect(await probe(page, 'changeInViewTransition')).toEqual({ ran: true, started: false });
  });
});
