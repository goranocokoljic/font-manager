import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';

// E46.4: drag, activation, Quick Look and the Studio, in the real app, and the places where
// nothing may move (dev-docs/redesign/MOTION.md). What the engine runs is recorded while the
// user acts (motion-probe.ts `recordMotion`): a transition the page never starts is not there.

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

const EASE = 'cubic-bezier(0.2, 0, 0, 1)';
type Probe = typeof import('./motion-probe');
type Moved = Awaited<ReturnType<Probe['recorded']>>[number];

function probe<K extends keyof Probe>(page: Page, name: K) {
  return page.evaluate(async (fn) => {
    const url = '/e2e/motion-probe.ts';
    const module = await import(url);
    return module[fn]();
  }, name) as ReturnType<Probe[K]>;
}

const record = (page: Page) => probe(page, 'recordMotion');
const recorded = async (page: Page): Promise<Moved[]> => (await probe(page, 'recorded'))!;

/** The moves of the element marked `testid` (or a part of it) on `property`. */
const movesOf = (seen: Moved[], testid: string, property: string) =>
  seen.filter((m) => m.testid === testid && m.property === property);

async function serveFonts(page: Page) {
  await page.route('**/fmfont/*', (r) =>
    r.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
}

async function open(page: Page) {
  await serveFonts(page);
  await page.goto('/#/');
  await expect(
    page.getByRole('tree', { name: 'Font list' }).getByRole('treeitem').first(),
  ).toBeVisible();
}

async function openWall(page: Page): Promise<Locator> {
  await page.setViewportSize({ width: 1440, height: 900 });
  await serveFonts(page);
  await page.goto('/?mockLibrary=wall#/');
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Grid' })
    .click();
  const wall = page.getByRole('listbox', { name: 'Font grid' });
  await expect(wall.getByRole('option').first()).toBeVisible();
  return wall;
}

async function centre(el: Locator) {
  const box = (await el.boundingBox())!;
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

/** Drags the wall's first card onto Editorial and drops it there. */
async function dragToEditorial(page: Page, wall: Locator): Promise<void> {
  const card = wall.getByRole('option').first();
  await card.click();
  const editorial = page
    .getByRole('region', { name: 'Collections' })
    .getByRole('listitem', { name: 'Editorial', exact: true });
  const count = editorial.getByTestId('collection-count');
  await expect(count).toHaveText('31');
  const a = await centre(card);
  await page.mouse.move(a.x, a.y);
  await page.mouse.down();
  await page.mouse.move(a.x + 20, a.y + 20, { steps: 4 });
  await expect(page.getByTestId('drag-ghost')).toBeVisible();
  const b = await centre(editorial);
  await page.mouse.move(b.x, b.y, { steps: 6 });
  await page.mouse.move(b.x + 1, b.y, { steps: 2 });
  await expect(count).toHaveText(/→/);
  await page.mouse.up();
  await expect(page.getByTestId('drag-ghost-flight')).toHaveCount(0);
  await expect(count).not.toHaveText(/→|^31$/);
  await page.waitForTimeout(200);
}

/** Opens Quick Look on the list's second font; reports what moved as the sheet came in. */
async function openQuickLook(page: Page) {
  const tree = page.getByRole('tree', { name: 'Font list' });
  await tree.focus();
  await page.keyboard.press('ArrowDown');
  await record(page);
  await page.keyboard.press('Space');
  const sheet = page.getByTestId('quick-look');
  await expect(sheet).toBeVisible();
  await page.waitForTimeout(100);
  return { sheet, seen: await recorded(page) };
}

/** Opens the Studio from the library; reports what moved as it came in. */
async function openStudio(page: Page) {
  await open(page);
  await record(page);
  await page.goto('/#/studio?face=1');
  await expect(page.getByTestId('studio-roles').first()).toBeVisible();
  await page.waitForTimeout(100);
  return recorded(page);
}

/** Activates the list's first family with A; reports what moved. */
async function activateFirst(page: Page) {
  const tree = page.getByRole('tree', { name: 'Font list' });
  await tree.getByRole('treeitem').first().click();
  await record(page);
  await page.keyboard.press('a');
  await expect(tree.locator('[data-activation="active"]').first()).toBeAttached();
  await page.waitForTimeout(100);
  return recorded(page);
}

test.describe('drag, activation, Quick Look and the Studio (E46.4)', () => {
  test('drag: the ghost lifts in 120 ms, flies in in 200 ms, the count crossfades in 120 ms', async ({
    page,
  }) => {
    const wall = await openWall(page);
    await record(page);
    await dragToEditorial(page, wall);
    const seen = await recorded(page);
    for (const property of ['transform', 'box-shadow'])
      expect(movesOf(seen, 'drag-ghost-lift', property)[0]).toMatchObject({
        duration: 120,
        easing: EASE,
      });
    expect(
      movesOf(seen, 'drag-ghost-flight', 'transform').find((m) => m.classes.includes('flight')),
    ).toMatchObject({ duration: 200, easing: EASE });
    // The count: the old number fades out over the new one, both at once, in 120 ms.
    const count = seen.filter((m) => m.property === 'opacity' && m.classes.includes('side-count'));
    expect(count.length).toBeGreaterThanOrEqual(2);
    expect(count.every((m) => m.duration === 120 && m.easing === EASE)).toBe(true);
  });

  test('activation: the dot fills by scale 0 → 1 in 120 ms', async ({ page }) => {
    await open(page);
    const dot = (await activateFirst(page)).filter(
      (m) => m.pseudo === '::before' && m.classes.includes('dot') && m.property === 'transform',
    );
    expect(dot.length).toBeGreaterThan(0);
    expect(dot[0]).toMatchObject({ duration: 120, easing: EASE });
    expect(dot[0]!.from.transform).toMatch(/^(scale\(0\)|matrix\(0, 0, 0, 0, 0, 0\))$/);
  });

  test('activation on the family page: the switch knob slides by transform in 120 ms', async ({
    page,
  }) => {
    await serveFonts(page);
    await page.goto('/#/family/1?face=1');
    const knobSwitch = page.getByRole('switch', { name: /^Active: / }).first();
    await expect(knobSwitch).toBeVisible();
    await record(page);
    await knobSwitch.click();
    await expect(knobSwitch).toHaveAttribute('aria-checked', 'true');
    await page.waitForTimeout(100);
    const knob = (await recorded(page)).filter((m) => m.classes.includes('knob'));
    expect(knob.map((m) => m.property)).toEqual(['transform']);
    expect(knob[0]).toMatchObject({ duration: 120, easing: EASE });
    // And the hero's state dot fills by scale.
    await expect(page.getByTestId('family-meta').locator('.dot')).toBeVisible();
  });

  test('Quick Look rises 8 px over a fade in 200 ms over a fixed blur; the arrows crossfade in 120 ms', async ({
    page,
  }) => {
    await open(page);
    const { sheet, seen } = await openQuickLook(page);
    const enter = seen
      .filter((m) => m.testid === 'quick-look' && m.classes.includes('sheet'))
      .sort((a, b) => a.property.localeCompare(b.property));
    expect(enter).toEqual([
      expect.objectContaining({ property: 'opacity', duration: 200, easing: EASE }),
      expect.objectContaining({
        property: 'transform',
        duration: 200,
        easing: EASE,
        from: expect.objectContaining({ transform: 'translate(0px, 8px)' }),
      }),
    ]);
    // The blur is there at once and never animates.
    await expect(sheet.locator('..')).toHaveCSS('backdrop-filter', 'blur(12px)');
    expect(seen.filter((m) => m.property.includes('filter'))).toEqual([]);

    const before = await sheet.getByRole('heading').textContent();
    await record(page);
    await page.keyboard.press('ArrowDown');
    await expect(sheet.getByRole('heading')).not.toHaveText(before!);
    await page.waitForTimeout(100);
    // The specimen and its copy: the copy fades out over the new one.
    const arrows = (await recorded(page)).filter(
      (m) => m.testid === 'quick-look' && m.classes.includes('body'),
    );
    expect(arrows.length).toBeGreaterThanOrEqual(2);
    expect(arrows.every((m) => m.duration === 120 && m.easing === EASE)).toBe(true);
  });

  test('the Studio opens: the paper scales .98 → 1 over a fade and the rail slides in, in 280 ms', async ({
    page,
  }) => {
    const seen = await openStudio(page);
    const paper = seen.find((m) => m.property === 'fm-paper-in');
    expect(paper).toMatchObject({ kind: 'animation', duration: 280, easing: EASE });
    expect(paper!.from.opacity).toBe('0');
    expect(paper!.from.transform).toMatch(/^(scale\(0\.98\)|matrix\(0\.98, 0, 0, 0\.98, 0, 0\))$/);
    const rail = seen
      .filter((m) => m.testid === 'studio-roles' && m.classes.includes('rail'))
      .sort((a, b) => a.property.localeCompare(b.property));
    expect(rail.map((m) => m.property)).toEqual(['opacity', 'transform']);
    expect(rail.every((m) => m.duration === 280 && m.easing === EASE)).toBe(true);
    expect(rail[1]!.from.transform).toBe('translate(-24px)');
  });

  test('only the opening moves: a context picked later shows at once', async ({ page }) => {
    await openStudio(page);
    await page.waitForFunction(() => !document.querySelector('.fm-paper-in'));
    await record(page);
    await page
      .getByTestId('studio-toolbar')
      .getByRole('group', { name: 'Context' })
      .getByRole('button', { name: 'Poster', exact: true })
      .click();
    await expect(page).toHaveURL(/layout=studio-poster/);
    await page.waitForTimeout(100);
    const seen = await recorded(page);
    expect(seen.filter((m) => m.property === 'fm-paper-in')).toEqual([]);
    expect(seen.filter((m) => m.testid === 'studio-roles')).toEqual([]);
  });
});

test.describe('nothing animates while the user types or moves by keyboard (E46.4)', () => {
  test('typing in the preview field animates nothing', async ({ page }) => {
    await openWall(page);
    const field = page.getByRole('textbox', { name: 'Type to preview' });
    await field.click();
    await page.waitForTimeout(200);
    await record(page);
    await field.pressSequentially('Hamburgefonstiv', { delay: 20 });
    await page.waitForTimeout(200);
    expect(await recorded(page)).toEqual([]);
  });

  test('search results animate nothing as they change', async ({ page }) => {
    const wall = await openWall(page);
    const box = page.getByRole('searchbox', { name: 'Search fonts' });
    await box.click();
    await page.waitForTimeout(200);
    const first = await wall.getByRole('option').first().getAttribute('aria-label');
    await record(page);
    await box.pressSequentially('mono', { delay: 20 });
    await expect(wall.getByRole('option').first()).not.toHaveAttribute('aria-label', first!);
    await page.waitForTimeout(200);
    await box.fill('');
    await expect(wall.getByRole('option').first()).toHaveAttribute('aria-label', first!);
    await page.waitForTimeout(200);
    expect(await recorded(page)).toEqual([]);
  });

  test('on the Table view nothing animates: the cursor, activation, a search', async ({ page }) => {
    await open(page);
    await page
      .getByRole('group', { name: 'View mode' })
      .getByRole('button', { name: 'Table' })
      .click();
    const table = page.getByRole('treegrid', { name: 'Font table' });
    await expect(table.getByRole('row').nth(1)).toBeVisible();
    await table.getByRole('row').nth(1).click();
    await page.waitForTimeout(300);
    await record(page);
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('a');
    await expect(page.getByTestId('toast')).toBeVisible();
    await page.getByRole('searchbox', { name: 'Search fonts' }).fill('inter');
    await page.waitForTimeout(200);
    const seen = await recorded(page);
    // The docks over the bottom edge (the toast that says the fonts are active, the selection
    // bar the search clears) are not the table: they keep their own moments (E46.3).
    const overlay = (m: Moved) => m.testid === 'toast' || m.classes.split(' ').includes('dock');
    expect(seen.filter((m) => !overlay(m))).toEqual([]);
    // And a row's dot says so: no transition, while the list's dot has one.
    await expect(table.getByRole('row').nth(1).locator('.dot')).toHaveCSS(
      'transition-duration',
      '0s',
    );
  });

  test('keyboard focus shows its ring at once', async ({ page }) => {
    await open(page);
    await page.getByRole('searchbox', { name: 'Search fonts' }).focus();
    await page.waitForTimeout(200);
    await record(page);
    for (let i = 0; i < 6; i++) await page.keyboard.press('Tab');
    await page.waitForTimeout(100);
    await expect(page.locator(':focus-visible').first()).toHaveCSS('outline-style', 'solid');
    expect((await recorded(page)).filter((m) => /outline|box-shadow/.test(m.property))).toEqual([]);
  });
});

test.describe('reduced motion (E46.4, MOTION.md)', () => {
  test.beforeEach(async ({ page }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
  });

  test('activation changes at once', async ({ page }) => {
    await open(page);
    expect(
      (await activateFirst(page)).filter(
        (m) => m.classes.includes('dot') || m.classes.includes('knob'),
      ),
    ).toEqual([]);
  });

  test('Quick Look only fades, on the reduced fade, with no blur; the arrows cut', async ({
    page,
  }) => {
    await open(page);
    const { sheet, seen } = await openQuickLook(page);
    expect(seen.filter((m) => m.testid === 'quick-look')).toEqual([
      expect.objectContaining({ property: 'opacity', duration: 120, easing: EASE }),
    ]);
    await expect(sheet.locator('..')).toHaveCSS('backdrop-filter', 'none');
    const before = await sheet.getByRole('heading').textContent();
    await record(page);
    await page.keyboard.press('ArrowDown');
    await expect(sheet.getByRole('heading')).not.toHaveText(before!);
    await page.waitForTimeout(100);
    expect(await recorded(page)).toEqual([]);
  });

  test('the Studio only fades, on the reduced fade', async ({ page }) => {
    const seen = await openStudio(page);
    expect(seen.filter((m) => m.kind === 'animation')).toEqual([
      expect.objectContaining({ property: 'fm-fade-in', duration: 120, easing: EASE }),
    ]);
    expect(seen.filter((m) => m.testid === 'studio-roles')).toEqual([
      expect.objectContaining({ property: 'opacity', duration: 120, easing: EASE }),
    ]);
  });

  test('the drag ghost follows without the tilt and the drop is a cut', async ({ page }) => {
    const wall = await openWall(page);
    await record(page);
    await dragToEditorial(page, wall);
    const seen = await recorded(page);
    expect(seen.filter((m) => m.testid.startsWith('drag-ghost'))).toEqual([]);
    expect(seen.filter((m) => m.classes.includes('side-count'))).toEqual([]);
  });
});
