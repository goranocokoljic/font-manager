// E47.2: the theme picker in Settings › Appearance and "Theme: <name>" in the palette.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

type Tokens = Record<'accent' | 'bg' | 'surface', string>;
const themeFile = (id: string): { light: Tokens; dark: Tokens } =>
  JSON.parse(
    readFileSync(
      fileURLToPath(new URL(`../../../data/themes/${id}.json`, import.meta.url)),
      'utf8',
    ),
  );
const indigo = themeFile('indigo');
const moss = themeFile('moss');
const paper = themeFile('paper');

const rootToken = (page: Page, name: string) =>
  page.evaluate((n) => getComputedStyle(document.documentElement).getPropertyValue(n).trim(), name);
const card = (page: Page, id: string) =>
  page.locator(`[data-testid="theme-card"][data-theme-id="${id}"]`);

/** WCAG contrast of two computed `rgb(…)` colors. */
function contrast(a: string, b: string): number {
  const lum = (c: string) => {
    const [r, g, bl] = c
      .match(/[\d.]+/g)!
      .slice(0, 3)
      .map(Number);
    const lin = (v: number) => {
      const x = v / 255;
      return x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
    };
    return 0.2126 * lin(r!) + 0.7152 * lin(g!) + 0.0722 * lin(bl!);
  };
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (hi! + 0.05) / (lo! + 0.05);
}

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light' });
});

for (const scheme of ['light', 'dark'] as const) {
  test(`the theme cards show a live mini wall in the ${scheme} mode, the choice in its accent`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto('/#/settings?section=appearance');
    const group = page.getByRole('group', { name: 'Theme', exact: true });
    await expect(group.getByRole('radio')).toHaveCount(4);
    await expect(group.getByRole('radio', { name: 'Paper' })).toBeChecked();
    const mossCard = card(page, 'moss');
    await expect(mossCard.getByTestId('theme-specimen')).toHaveCount(3);
    // The card is drawn on the theme's own surface, for the current mode.
    const bgOf = (sel: string) =>
      mossCard
        .locator(sel)
        .first()
        .evaluate((el) => getComputedStyle(el).backgroundColor);
    await expect(mossCard).toHaveCSS('background-color', hex(moss[scheme].surface));
    expect(await bgOf('.specimen')).toBe(hex(moss[scheme].bg));
    await expect(mossCard.locator('.specimen.selected')).toHaveCSS(
      'border-top-color',
      hex(moss[scheme].accent),
    );

    // Choosing marks the card with a 3:1 ring and repaints the window.
    await mossCard.click();
    await expect(group.getByRole('radio', { name: 'Moss' })).toBeChecked();
    await expect(mossCard).toHaveClass(/checked/);
    expect(await rootToken(page, '--fm-accent')).toBe(moss[scheme].accent);
    const ring = await mossCard.evaluate((el) => getComputedStyle(el).borderTopColor);
    const panel = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
    expect(contrast(ring, panel)).toBeGreaterThanOrEqual(3);
    const plain = await card(page, 'graphite').evaluate(
      (el) => getComputedStyle(el).borderTopColor,
    );
    expect(plain).not.toBe(ring);
  });
}

test('a choice crossfades the window at 200 ms and saves themeId', async ({ page }) => {
  await page.goto('/#/settings?section=appearance');
  await expect(card(page, 'paper')).toHaveClass(/checked/);
  // Records each view transition and whether the theme class times it.
  await page.evaluate(() => {
    const w = window as unknown as { fmTransitions: string[] };
    w.fmTransitions = [];
    const start = document.startViewTransition?.bind(document);
    if (!start) return;
    document.startViewTransition = ((cb: () => void) => {
      const root = document.documentElement;
      w.fmTransitions.push(
        root.classList.contains('fm-theme-change')
          ? getComputedStyle(root).getPropertyValue('--fm-motion-move').trim()
          : 'other',
      );
      return start(cb);
    }) as typeof document.startViewTransition;
  });
  await card(page, 'indigo').click();
  await expect(page.getByRole('radio', { name: 'Indigo' })).toBeChecked();
  await expect.poll(() => rootToken(page, '--fm-accent')).toBe(indigo.light.accent);
  expect(
    await page.evaluate(() => (window as unknown as { fmTransitions: string[] }).fmTransitions),
  ).toEqual(['200ms']);
  await expect(page.locator('html')).not.toHaveClass(/fm-theme-change/);
  const saved = await page.evaluate(() =>
    JSON.parse(sessionStorage.getItem('fm.mock.settings.v1') ?? '{}'),
  );
  expect(saved.themeId).toBe('indigo');
});

test('the palette offers "Theme: <name>" and the choice survives a restart', async ({ page }) => {
  await page.goto('/');
  await page.evaluate(() => localStorage.clear());
  expect(await rootToken(page, '--fm-accent')).toBe(paper.light.accent);
  await page.keyboard.press('ControlOrMeta+k');
  const palette = page.getByRole('dialog', { name: 'Command palette' });
  for (const name of ['Paper', 'Graphite', 'Indigo', 'Moss'])
    await expect(palette.getByRole('option').filter({ hasText: `Theme: ${name}` })).toHaveCount(1);
  await page.keyboard.type('Theme: Moss');
  await expect(palette.getByRole('option').first()).toContainText('Theme: Moss');
  await page.keyboard.press('Enter');
  await expect(palette).toBeHidden();
  await expect.poll(() => rootToken(page, '--fm-accent')).toBe(moss.light.accent);

  // Not a view preference: nothing in localStorage holds it; the settings do.
  const local = await page.evaluate(() => JSON.stringify({ ...localStorage }));
  expect(local).not.toContain('moss');
  await page.reload();
  await expect.poll(() => rootToken(page, '--fm-accent')).toBe(moss.light.accent);
});

/** `#rrggbb` as the computed `rgb(r, g, b)`. */
function hex(value: string): string {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(value.slice(i, i + 2), 16));
  return `rgb(${r}, ${g}, ${b})`;
}
