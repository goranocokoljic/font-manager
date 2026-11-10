import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { wallFixtures } from '../src/api/mockWall';
import { openFilters } from './filters';
import { showPreviewOptions } from './select';

// The Letters preset (E49.7, board 8-letters-wall.png): the preview field takes glyphs and
// every card shows exactly those, spaced evenly, at 1.2× the slider; a glyph a family lacks
// is tofu, never a fallback text. The wall library serves each family's own fixture, so a
// Latin-only family really lacks Cyrillic.
const files = wallFixtures();

async function openWall(page: Page, query = '?mockLibrary=wall'): Promise<Locator> {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.route('**/fmfont/*', (route) => {
    const id = Number(new URL(route.request().url()).pathname.split('/').pop());
    const path = files.get(id) ?? files.values().next().value!;
    return route.fulfill({
      path: fileURLToPath(new URL(`../../../fixtures/fonts/${path}`, import.meta.url)),
    });
  });
  await page.goto(`/${query}#/`);
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Grid' })
    .click();
  const wall = page.getByRole('listbox', { name: 'Font grid' });
  await expect(wall).toBeVisible();
  return wall;
}

const controls = (page: Page) => page.getByRole('group', { name: 'Preview settings' });
const field = (page: Page) => controls(page).getByRole('textbox', { name: 'Type to preview' });
const chip = (page: Page, name: string) =>
  controls(page).getByRole('group', { name: 'Preset' }).getByRole('button', { name });
const cards = (wall: Locator) => wall.getByRole('option');
const line = (page: Page) => page.getByTestId('count-line');

/** The family names on the wall, in order. */
const names = (wall: Locator) =>
  wall
    .locator('[data-family-name]')
    .evaluateAll((els) => els.map((e) => e.textContent!.trim()).slice(0, 12));

/** Every card as live text (outline on), so the text a card draws can be read. */
async function liveCards(page: Page) {
  await showPreviewOptions(page);
  await page.getByRole('checkbox', { name: 'Outline' }).check();
  await page.keyboard.press('Escape');
}

test('the empty field hints R O a g e; a tap adds a letter; cards show only the glyphs', async ({
  page,
}) => {
  const wall = await openWall(page);
  await chip(page, 'Letters').click();
  await expect(chip(page, 'Letters')).toHaveAttribute('aria-pressed', 'true');
  const hint = controls(page).getByTestId('preview-hint');
  await expect(hint).toContainText('Type the letters that matter');
  await expect(hint).toContainText('tell faces apart');
  for (const g of ['R', 'O', 'a', 'g', 'e'])
    await expect(hint.getByRole('button', { name: `Add ${g}`, exact: true })).toBeVisible();
  await hint.getByRole('button', { name: 'Add R', exact: true }).click();
  await expect(field(page)).toHaveValue('R');
  // The tap-to-add hint has done its work; the shape hint of E49.6 takes its place.
  await expect(hint.getByRole('button')).toHaveCount(0);
  await expect(hint).not.toContainText('Type the letters that matter');
  await expect(field(page)).toBeFocused();
  await page.keyboard.press('End');
  await page.keyboard.type('Oag');
  await expect(field(page)).toHaveValue('ROag');

  await liveCards(page);
  const texts = wall.getByTestId('card-live-text');
  await expect(texts.first()).toBeVisible();
  const all = await texts.evaluateAll((els) => els.map((e) => e.textContent));
  expect(all.length).toBeGreaterThanOrEqual(8);
  for (const text of all) expect(text).toBe('R O a g');
  await expect(wall.getByTestId('fallback-hint')).toHaveCount(0);
});

test('the slider applies at 1.2× and the glyphs sit centred in the card', async ({ page }) => {
  const wall = await openWall(page);
  await chip(page, 'Letters').click();
  await field(page).fill('ROag');
  await page.getByRole('slider', { name: 'Size slider' }).evaluate((el: HTMLInputElement) => {
    el.value = '40';
    el.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await liveCards(page);
  const text = cards(wall).first().getByTestId('card-live-text');
  await expect(text).toHaveCSS('font-size', '48px');
  const [card, glyphs] = await Promise.all([cards(wall).first().boundingBox(), text.boundingBox()]);
  const cardMid = card!.x + card!.width / 2;
  const textMid = glyphs!.x + glyphs!.width / 2;
  expect(Math.abs(cardMid - textMid)).toBeLessThan(4);
});

test('a family that lacks a glyph shows tofu for it, never a fallback text', async ({ page }) => {
  const wall = await openWall(page);
  // Word first: a Latin-only family draws none of "Жж" and falls back to its own letters.
  await field(page).fill('Жж');
  await expect(wall.getByTestId('fallback-hint').first()).toBeVisible();

  await chip(page, 'Letters').click();
  await field(page).fill('Жж');
  const lacking = cards(wall)
    .filter({ has: page.getByTestId('missing-glyphs') })
    .first();
  await expect(lacking).toBeVisible();
  await expect(lacking.getByTestId('missing-glyphs')).toHaveText('2 glyphs missing · Ж ж');
  const text = lacking.getByTestId('card-live-text');
  await expect(
    text.getByRole('img', { name: 'missing glyph Ж', exact: true, includeHidden: true }),
  ).toBeVisible();
  await expect(text.getByRole('img', { includeHidden: true })).toHaveCount(2);
  await expect(wall.getByTestId('fallback-hint')).toHaveCount(0);
});

test('filters and the sort apply unchanged; preset and glyphs are kept per view', async ({
  page,
}) => {
  const wall = await openWall(page);
  const drawer = await openFilters(page);
  await drawer
    .getByRole('group', { name: 'Class' })
    .getByRole('button', { name: /^Sans serif/ })
    .click();
  await drawer.getByRole('button', { name: 'Serbian (Cyrillic)' }).click();
  await drawer.getByRole('button', { name: 'Close' }).click();
  await line(page).getByRole('button', { name: 'change sort order' }).click();
  const menu = page.getByRole('group', { name: 'Sort order' });
  await menu.getByRole('radio', { name: 'Date added' }).check();
  await page.keyboard.press('Escape');
  await expect(line(page)).toContainText('Sorted by date added');

  const status = line(page).getByRole('status');
  const count = await status.textContent();
  await expect.poll(() => names(wall)).not.toEqual([]);
  const before = await names(wall);

  await chip(page, 'Letters').click();
  await field(page).fill('ROag');
  await expect(status).toHaveText(count!);
  await expect(line(page)).toContainText('Sorted by date added');
  await expect.poll(() => names(wall)).toEqual(before);

  await page.reload();
  await expect(page.getByRole('listbox', { name: 'Font grid' })).toBeVisible();
  await expect(chip(page, 'Letters')).toHaveAttribute('aria-pressed', 'true');
  await expect(field(page)).toHaveValue('ROag');
});

test('the wall scrolls at 60 fps with 1,000 families in Letters at 4 glyphs', async ({ page }) => {
  // Up to three rounds of 240 frames: 12 s each on a runner that drops to 20 fps.
  test.setTimeout(90_000);
  const wall = await openWall(page, '?mockLibrary=wall&mockFaces=940');
  await chip(page, 'Letters').click();
  await field(page).fill('ROag');
  await expect(cards(wall).first().getByTestId('thumbnail')).toHaveAttribute('data-state', 'ready');
  const total = await wall.evaluate((el) => el.scrollHeight - el.clientHeight);
  expect(total).toBeGreaterThan(20_000);
  // Median frame rate of `frames` animation frames; `step` px are scrolled in each frame.
  const measure = (step: number, frames: number) =>
    wall.evaluate(
      (el, [step, frames]) =>
        new Promise<number>((resolve) => {
          const stamps: number[] = [];
          const tick = (now: number) => {
            stamps.push(now);
            if (step) el.scrollTop = (el.scrollTop + step) % (el.scrollHeight - el.clientHeight);
            if (stamps.length < frames) requestAnimationFrame(tick);
            else {
              const gaps = stamps
                .slice(1)
                .map((t, i) => t - stamps[i]!)
                .sort((a, b) => a - b);
              resolve(1000 / gaps[Math.floor(gaps.length / 2)]!);
            }
          };
          requestAnimationFrame(tick);
        }),
      [step, frames] as const,
    );
  // As in wall.spec.ts: a sweep counts only when the idle rate before and after it reached
  // 55, so a slow sweep is the wall's and not the runner's; the CI macOS VM renders busy
  // frames at half rate, so there a counted sweep is held to 28.
  const budget = process.env.CI && process.platform === 'darwin' ? 28 : 55;
  // Back at the top and at rest: nothing animates, every card in the window has its thumbnail.
  const rest = async () => {
    await wall.evaluate((el) => (el.scrollTop = 0));
    await expect
      .poll(
        () =>
          wall.evaluate(
            (el) =>
              el.getAnimations({ subtree: true }).length +
              el.querySelectorAll('[data-testid="thumbnail"]:not([data-state="ready"])').length,
          ),
        { timeout: 10_000 },
      )
      .toBe(0);
  };
  // The sweep that counts goes over a stretch whose faces are in: one pass first, not counted.
  // On a first pass every new card's font file comes through the route handler above (a round
  // trip to the test process for each face, which the app's asset protocol does not have), and
  // with a second worker on the runner those frames ran late: the Windows runner gave 30.0 fps
  // in the full suite, twice, and 59.9 alone. Measured there on 2026-11-10 with this test's
  // sweep: 80 of 240 frames late on the first pass, 0 of 240 on the second (276 cards mounted
  // in each). As the slider test in wall.spec.ts, this one counts frames, not font loads.
  await measure(40, 240);
  let last = { before: 0, fps: 0, after: 0 };
  for (let round = 0; round < 3; round++) {
    await rest();
    const before = await measure(0, 60);
    const fps = await measure(40, 240);
    const after = await measure(0, 60);
    last = { before, fps, after };
    console.log(
      `letters scroll: ${fps.toFixed(1)} fps median (idle ${before.toFixed(1)} before, ${after.toFixed(1)} after)`,
    );
    if (before >= 55 && after >= 55) {
      expect(fps).toBeGreaterThanOrEqual(budget);
      return;
    }
  }
  const idle = Math.min(last.before, last.after);
  expect(last.fps).toBeGreaterThanOrEqual(idle - 5);
  test.skip(true, `not measured: animation frames run at ${idle.toFixed(1)} fps when idle`);
});
