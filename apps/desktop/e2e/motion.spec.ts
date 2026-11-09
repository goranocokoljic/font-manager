import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';

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

// E46.3: the drawer, the popovers, the selection bar and the toasts, in the real app.
const DRAWER = '[role="dialog"][aria-label="Filter options"]';
const SCRIM = '[data-testid="drawer-scrim"]';
const BAR = '.dock:has([data-testid="selection-bar"])';
const TOAST = '[data-testid="toast"]';
const REDUCED_FADE = [{ property: 'opacity', duration: 120, easing: EASE, from: '0' }];

async function openDrawer(page: Page) {
  await probe(page, 'watchEnter', [DRAWER, SCRIM]);
  await page
    .getByRole('region', { name: 'Filters' })
    .getByRole('button', { name: /^Filters/ })
    .click();
  return (await probe(page, 'enterSeen'))!;
}

const SORT_MENU = '[role="group"][aria-label="Sort order"]';
const OPTIONS = '[role="group"][aria-label="Preview options"]';

/** Opens a popover that lives in the page with `opener`, and reports its enter. */
async function enterPanel(page: Page, panel: string, opener: Locator) {
  await probe(page, 'watchEnter', [panel]);
  await opener.click();
  return (await probe(page, 'enterSeen'))![panel];
}

/** Selects two rows (the bar comes up), then activates them (the toast comes up). */
async function selectAndActivate(page: Page) {
  await probe(page, 'watchEnter', [BAR]);
  const tree = page.getByRole('tree', { name: 'Font list' });
  await tree.getByRole('treeitem').first().click();
  await page.keyboard.press('Shift+ArrowDown');
  const bar = (await probe(page, 'enterSeen'))![BAR];
  await probe(page, 'watchEnter', [TOAST]);
  await page.keyboard.press('a');
  const toast = (await probe(page, 'enterSeen'))![TOAST];
  return { bar, toast };
}

test.describe('drawers, popovers, selection bar and toasts (E46.3)', () => {
  test('the filters drawer slides its full width from the start edge over a 6 % scrim', async ({
    page,
  }) => {
    await open(page);
    const seen = await openDrawer(page);
    const drawer = seen[DRAWER]!;
    expect(drawer.map((t) => t.property)).toEqual(['opacity', 'transform']);
    expect(drawer.every((t) => t.duration === 200 && t.easing === EASE)).toBe(true);
    // Its full width, whatever that is: the percentage of its own box.
    expect(drawer.find((t) => t.property === 'transform')?.from).toBe('translate(-100%)');
    expect(seen[SCRIM]).toEqual([{ property: 'opacity', duration: 200, easing: EASE, from: '0' }]);
    await expect(page.locator(SCRIM)).toHaveCSS('background-color', 'rgba(0, 0, 0, 0.06)');
    // The scrim only tints: the wall behind takes the press.
    await expect(page.locator(SCRIM)).toHaveCSS('pointer-events', 'none');
  });

  test('a popover fades in and rises 4 px in 120 ms', async ({ page }) => {
    await open(page);
    expect(await probe(page, 'enterPopover')).toEqual([
      { property: 'opacity', duration: 120, easing: EASE, from: '0' },
      { property: 'transform', duration: 120, easing: EASE, from: 'translate(0px, 4px)' },
    ]);
  });

  test('the sort menu and the Preview options popover rise 4 px in 120 ms, too', async ({
    page,
  }) => {
    const RISE = [
      { property: 'opacity', duration: 120, easing: EASE, from: '0' },
      { property: 'transform', duration: 120, easing: EASE, from: 'translate(0px, 4px)' },
    ];
    await open(page);
    expect(
      await enterPanel(page, SORT_MENU, page.getByRole('button', { name: 'change sort order' })),
    ).toEqual(RISE);
    await page.keyboard.press('Escape');
    await expect(page.locator(SORT_MENU)).toHaveCount(0);
    expect(
      await enterPanel(
        page,
        OPTIONS,
        page.getByRole('button', { name: 'Preview options' }).first(),
      ),
    ).toEqual(RISE);
  });

  test('the selection bar and the toast rise 16 px over a fade in 200 ms', async ({ page }) => {
    await open(page);
    const { bar, toast } = await selectAndActivate(page);
    for (const seen of [bar, toast]) {
      expect(seen).toEqual([
        { property: 'opacity', duration: 200, easing: EASE, from: '0' },
        { property: 'transform', duration: 200, easing: EASE, from: 'translate(0px, 16px)' },
      ]);
    }
  });

  test('under reduced motion every one of them only fades, on the reduced fade', async ({
    page,
  }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await open(page);
    const drawer = await openDrawer(page);
    expect(drawer[DRAWER]).toEqual(REDUCED_FADE);
    expect(drawer[SCRIM]).toEqual(REDUCED_FADE);
    await page.keyboard.press('Escape');
    await expect(page.locator(DRAWER)).toBeHidden();
    expect(await probe(page, 'enterPopover')).toEqual(REDUCED_FADE);
    const { bar, toast } = await selectAndActivate(page);
    expect(bar).toEqual(REDUCED_FADE);
    expect(toast).toEqual(REDUCED_FADE);
    // A helper without `reduced="fade"` still cuts (the other moments of the table).
    expect(await probe(page, 'enterSlide', {})).toEqual([]);
  });
});
