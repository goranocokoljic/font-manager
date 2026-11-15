import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { openSection, showSection } from './family-page';

// E50.7: the Characters section's cells scale with a slider and the window, a hairline grid
// runs between them, and the specimen above sizes its box from the face's vertical metrics.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-static/robotoslab/RobotoSlab-ExtraBold.ttf',
      import.meta.url,
    ),
  ),
);

// MockApi face 1 (Inter Regular); `?mockCjk=1` gives it 32,164 more characters.
async function openGlyphs(page: Page, { cjk = false } = {}) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  const panel = await openSection(page, 1, 'Characters', cjk ? '?mockCjk=1' : '');
  await expect(panel.getByRole('gridcell').first()).toBeVisible();
  return panel;
}

const scroller = (page: Page) => page.locator('main.scroll');
const slider = (panel: Locator) => panel.getByRole('slider', { name: 'Cell size' });

/** The CSS grid's track count, the first cell's size and its glyph's font size. */
const measureRow = (panel: Locator) =>
  panel
    .getByRole('row')
    .first()
    .evaluate((row) => {
      const cell = row.querySelector('[role="gridcell"]')!;
      const r = cell.getBoundingClientRect();
      return {
        tracks: getComputedStyle(row).gridTemplateColumns.split(' ').length,
        width: r.width,
        height: r.height,
        font: parseFloat(getComputedStyle(cell.querySelector('.char')!).fontSize),
      };
    });

test('a wider window gives larger cells, and the slider sets their size, remembered', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1000, height: 700 });
  const panel = await openGlyphs(page);
  const narrow = await measureRow(panel);
  expect(narrow.width).toBeGreaterThanOrEqual(88);
  await page.setViewportSize({ width: 1600, height: 900 });
  await expect.poll(async () => (await measureRow(panel)).width).not.toBe(narrow.width);
  const wide = await measureRow(panel);
  // Larger glyphs, never smaller cells: every cell stays at least the slider's size.
  expect(wide.width).toBeGreaterThanOrEqual(88);
  expect(wide.font).toBe(Math.round(wide.height / 2));

  await expect(slider(panel)).toHaveValue('88');
  await slider(panel).fill('140');
  await expect.poll(async () => (await measureRow(panel)).width).toBeGreaterThanOrEqual(140);
  const big = await measureRow(panel);
  expect(big.font).toBe(Math.round(big.height / 2));
  expect(big.font).toBeGreaterThanOrEqual(70);
  // Remembered for this view: a reload keeps the size.
  await page.reload();
  await showSection(page, 'Characters');
  await expect(panel.getByRole('gridcell').first()).toBeVisible();
  await expect(slider(panel)).toHaveValue('140');
  expect((await measureRow(panel)).width).toBeGreaterThanOrEqual(140);
});

test('a hairline grid runs between cells, the Grid switch hides it, hover shows the code point', async ({
  page,
}) => {
  const panel = await openGlyphs(page);
  const a = panel.getByRole('gridcell', { name: 'U+0041 A', exact: true });
  const hairline = () =>
    a.evaluate((el) => {
      const s = getComputedStyle(el);
      return { width: parseFloat(s.borderInlineEndWidth), color: s.borderInlineEndColor };
    });
  const on = await hairline();
  expect(on.width).toBeGreaterThan(0);
  expect(on.width).toBeLessThanOrEqual(1);
  // 7 % ink: the foreground at 0.07 alpha.
  expect(on.color).toMatch(/0\.07\)$/);
  const toggle = panel.getByRole('switch', { name: 'Grid' });
  await expect(toggle).toHaveAttribute('aria-checked', 'true');
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-checked', 'false');
  expect((await hairline()).width).toBe(0);
  await page.reload();
  await showSection(page, 'Characters');
  await expect(panel.getByRole('switch', { name: 'Grid' })).toHaveAttribute(
    'aria-checked',
    'false',
  );

  const cp = panel.getByRole('gridcell', { name: 'U+0041 A', exact: true }).locator('.cp');
  await expect(cp).toHaveText('U+0041');
  await expect(cp).toHaveCSS('opacity', '0');
  await panel.getByRole('gridcell', { name: 'U+0041 A', exact: true }).hover();
  await expect(cp).toHaveCSS('opacity', '1');
  await expect(cp).toHaveCSS('font-size', '9px');
  await expect(cp).toHaveCSS('font-family', /mono/i);
});

for (const [w, h] of [
  [1280, 800],
  [1000, 700],
] as const) {
  test(`the whole specimen line fits, its box runs from ascender to descender (${w}×${h})`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: w, height: h });
    const panel = await openGlyphs(page);
    const specimen = panel.getByTestId('glyph-specimen');
    await expect(specimen).toContainText('Hamburgefonstiv gjpqy');
    // The face file loads, the line is measured in it and shrinks to fit.
    await expect
      .poll(() =>
        specimen.evaluate(
          (el) =>
            el.querySelector('.specimen-text')!.getBoundingClientRect().right <=
            el.querySelector('.specimen-line')!.getBoundingClientRect().right + 1,
        ),
      )
      .toBe(true);
    const box = await specimen.evaluate((el) => {
      const r0 = el.getBoundingClientRect();
      const rulers = Object.fromEntries(
        [...el.querySelectorAll<HTMLElement>('[data-ruler]')].map((r) => {
          const label = r.querySelector('.ruler-label')!;
          return [
            r.dataset.ruler!,
            {
              y: r.getBoundingClientRect().top - r0.top,
              label: getComputedStyle(label).fontSize,
              right: r0.right - label.getBoundingClientRect().right,
            },
          ];
        }),
      );
      // A zero-size inline block after the text: its bottom edge is the text's baseline.
      const text = el.querySelector('.specimen-text')!;
      const probe = document.createElement('span');
      probe.style.cssText = 'display:inline-block;width:0;height:0';
      text.after(probe);
      const baseline = probe.getBoundingClientRect().bottom - r0.top;
      probe.remove();
      const line = el.querySelector<HTMLElement>('.specimen-line')!;
      return {
        size: parseFloat(getComputedStyle(line).fontSize),
        height: r0.height,
        rulers,
        baseline,
        textRight: text.getBoundingClientRect().right,
        lineRight: line.getBoundingClientRect().right,
      };
    });
    // Every letter, g j p q y included, is drawn inside the line: none is cut off at the side.
    expect(box.textRight).toBeLessThanOrEqual(box.lineRight + 1);
    expect(box.size).toBeLessThanOrEqual(96);
    expect(box.size).toBeGreaterThanOrEqual(24);
    // MockApi face 1: 1000 upm, ascender 900, descender -250, x-height 520, at the fitted size.
    const at = (units: number) => ((900 - units) * box.size) / 1000;
    expect(box.height).toBeCloseTo(at(-250), 0);
    expect(box.rulers.ascender!.y).toBeCloseTo(0, 0);
    expect(box.rulers.xHeight!.y).toBeCloseTo(at(520), 0);
    expect(box.rulers.baseline!.y).toBeCloseTo(at(0), 0);
    expect(box.rulers.descender!.y).toBeCloseTo(at(-250), 0);
    // 10 px labels at the right.
    for (const r of Object.values(box.rulers)) {
      expect(r.label).toBe('10px');
      expect(Math.abs(r.right)).toBeLessThan(1.5);
    }
    // The text sits on the baseline ruler, so its descenders end inside the box.
    expect(Math.abs(box.baseline - box.rulers.baseline!.y)).toBeLessThan(1.5);
    await expect(specimen).toHaveCSS('overflow-y', 'visible');
  });
}

test('the grid scrolls at 60 fps with 48 px cells', async ({ page }) => {
  test.setTimeout(90_000);
  const panel = await openGlyphs(page, { cjk: true });
  await slider(panel).fill('48');
  await expect.poll(async () => (await measureRow(panel)).width).toBeLessThan(60);
  const el = scroller(page);
  // 32k characters: far more than the 812 of the budget.
  const total = await el.evaluate((e) => e.scrollHeight - e.clientHeight);
  expect(total).toBeGreaterThan(20_000);
  // Median frame rate of `frames` animation frames; `step` px are scrolled in each frame.
  const measure = (step: number, frames: number) =>
    el.evaluate(
      (e, [step, frames]) =>
        new Promise<number>((resolve) => {
          const stamps: number[] = [];
          const tick = (now: number) => {
            stamps.push(now);
            if (step) e.scrollTop = (e.scrollTop + step) % (e.scrollHeight - e.clientHeight);
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
  // As in letters.spec.ts: a sweep counts only when the idle rate around it reached 55, so a
  // slow sweep is the grid's and not the runner's; the CI macOS VM renders busy frames at
  // half rate, so there a counted sweep is held to 28.
  const budget = process.env.CI && process.platform === 'darwin' ? 28 : 55;
  await measure(40, 120);
  let last = { before: 0, fps: 0, after: 0 };
  for (let round = 0; round < 3; round++) {
    await el.evaluate((e) => (e.scrollTop = 0));
    const before = await measure(0, 60);
    const fps = await measure(40, 240);
    const after = await measure(0, 60);
    last = { before, fps, after };
    console.log(
      `characters scroll: ${fps.toFixed(1)} fps median (idle ${before.toFixed(1)} before, ${after.toFixed(1)} after)`,
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
