import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';

const font = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

const views = (mode: string) => JSON.stringify({ all: { mode } });

async function setup(page: Page, mode?: string) {
  await page.route('**/fmfont/*', (r) =>
    r.fulfill({ status: 200, contentType: 'font/ttf', body: font }),
  );
  if (mode) {
    await page.addInitScript((v) => localStorage.setItem('fm.views.v1', v), views(mode));
  }
}

const tree = (page: Page) => page.getByRole('tree', { name: 'Font list' });
const familyRows = (page: Page) => tree(page).locator('[aria-level="1"]');
const position = (page: Page) => page.getByTestId('family-position');
const onFamilyPage = (page: Page) => expect(page).toHaveURL(/#\/family\/\d+/);

/**
 * The key of a family row all on screen, under the table's header, so a click on it does not
 * scroll. The virtualised view renders the rows at a new position a frame after the scroll, so
 * this waits for one. A key and not `nth()`: an index means another row once the list redraws.
 */
async function rowOnScreen(viewport: Locator): Promise<string> {
  const find = () =>
    viewport.evaluate((el) => {
      const box = el.getBoundingClientRect();
      return [...el.querySelectorAll<HTMLElement>('[data-key^="family:"]')]
        .find((row) => {
          const r = row.getBoundingClientRect();
          return r.top >= box.top + 60 && r.bottom <= box.bottom;
        })
        ?.getAttribute('data-key');
    });
  let key: string | null | undefined;
  await expect.poll(async () => (key = await find())).toBeTruthy();
  return key!;
}

test.describe('opening a family page', () => {
  for (const mode of ['list', 'table', 'grid', 'waterfall']) {
    test(`a double-click and O open it from the ${mode} view`, async ({ page }) => {
      await setup(page, mode);
      await page.goto('/?mockLibrary=wall#/');
      const row = page.locator('[data-key="family:501"]').first();
      await row.dblclick({ position: { x: 200, y: 20 } });
      await onFamilyPage(page);
      await expect(page).toHaveURL(/#\/family\/501$/);
      await page.keyboard.press('Escape');
      await expect(page).not.toHaveURL(/family/);
      // The double-click left the cursor on that row, so O opens the same family.
      await page.keyboard.press('o');
      await expect(page).toHaveURL(/#\/family\/501$/);
    });
  }

  test('the Layout view opens the family it shows', async ({ page }) => {
    await setup(page, 'layout');
    await page.goto('/?mockLibrary=wall#/');
    await expect(page.getByRole('combobox').first()).toBeVisible();
    await page.keyboard.press('o');
    await onFamilyPage(page);
  });

  test('a double-click on a fold button only folds', async ({ page }) => {
    await setup(page);
    await page.goto('/?mockLibrary=wall#/');
    await familyRows(page).nth(1).locator('.twisty').dblclick();
    await expect(page).not.toHaveURL(/family/);
  });

  test('the page push shares the specimen between the row and the hero', async ({ page }) => {
    await setup(page);
    await page.goto('/?mockLibrary=wall#/');
    await familyRows(page).nth(1).click();
    // Records the shared name as the transition starts, and what the push runs at.
    await page.evaluate(() => {
      const w = window as unknown as { probe: Record<string, unknown> };
      w.probe = {};
      const start = document.startViewTransition.bind(document);
      document.startViewTransition = ((cb: () => Promise<void>) => {
        const named = document.querySelectorAll('[data-key]');
        w.probe.source = [...named].some(
          (el) =>
            (el as HTMLElement).style.viewTransitionName === 'fm-family-specimen' ||
            el.querySelector<HTMLElement>('.sample')?.style.viewTransitionName ===
              'fm-family-specimen',
        );
        const t = start(cb);
        void t.ready.then(() => {
          const group = document.getAnimations().find((a) => {
            const fx = a.effect as KeyframeEffect;
            return fx.pseudoElement === '::view-transition-group(fm-family-specimen)';
          });
          w.probe.duration = group?.effect?.getComputedTiming().duration;
        });
        return t;
      }) as typeof document.startViewTransition;
    });
    await page.keyboard.press('o');
    await onFamilyPage(page);
    await expect
      .poll(() =>
        page.evaluate(() => (window as never as { probe: { duration?: number } }).probe.duration),
      )
      .toBe(280);
    expect(
      await page.evaluate(() => (window as never as { probe: { source: boolean } }).probe.source),
    ).toBe(true);
    // Once it ended no row keeps the name, or the next push would have two.
    await expect(page.locator('html')).not.toHaveClass(/fm-family-opening/);
    await page.keyboard.press('Escape');
    await expect(page).not.toHaveURL(/family/);
    await expect(page.locator('html')).not.toHaveClass(/fm-family-closing/);
    const left = await page.evaluate(
      () =>
        [...document.querySelectorAll<HTMLElement>('[data-key], .sample')].filter(
          (el) => el.style.viewTransitionName,
        ).length,
    );
    expect(left).toBe(0);
  });

  test('without view transitions the page slides in over a fade', async ({ page }) => {
    await setup(page);
    await page.addInitScript(() => {
      Object.defineProperty(document, 'startViewTransition', { value: undefined });
    });
    await page.goto('/?mockLibrary=wall#/');
    await familyRows(page).nth(1).click();
    await page.keyboard.press('o');
    const slide = page.locator('.family-page.slide-in');
    await expect(slide).toBeVisible();
    expect(await slide.evaluate((el) => getComputedStyle(el).animationDuration)).toBe('0.28s');
    expect(await slide.evaluate((el) => getComputedStyle(el).animationName)).toBe(
      await slide.evaluate(() => {
        const sheetRule = [...document.styleSheets]
          .flatMap((s) => [...s.cssRules])
          .find((r) => r instanceof CSSKeyframesRule && r.name.startsWith('fm-family-slide'));
        return (sheetRule as CSSKeyframesRule).name;
      }),
    );
  });

  test('a deep link does not slide', async ({ page }) => {
    await setup(page);
    await page.addInitScript(() => {
      Object.defineProperty(document, 'startViewTransition', { value: undefined });
    });
    await page.goto('/?mockLibrary=wall#/family/501');
    await expect(page.locator('.family-page')).toBeVisible();
    await expect(page.locator('.family-page.slide-in')).toHaveCount(0);
  });
});

test.describe('going back', () => {
  for (const [name, press] of [
    ['Esc', 'Escape'],
    ['Alt+←', 'Alt+ArrowLeft'],
  ] as const) {
    test(`${name} pops the page and restores scroll and selection`, async ({ page }) => {
      await setup(page);
      await page.goto('/?mockLibrary=wall#/');
      const list = tree(page);
      await familyRows(page).first().click();
      await list.evaluate((el) => (el.scrollTop = 1500));
      await expect.poll(() => list.evaluate((el) => el.scrollTop)).toBe(1500);
      const row = list.locator(`[data-key="${await rowOnScreen(list)}"]`);
      await row.click({ position: { x: 200, y: 20 } });
      await expect(row).toHaveAttribute('aria-selected', 'true');
      const label = await row.getAttribute('aria-label');
      const url = page.url();
      const before = await list.evaluate((el) => el.scrollTop);
      await page.keyboard.press('o');
      await onFamilyPage(page);

      await page.keyboard.press(press);
      await expect(page).toHaveURL(url);
      await expect(
        tree(page).getByRole('treeitem', { name: label!, selected: true }),
      ).toBeVisible();
      expect(await tree(page).evaluate((el) => el.scrollTop)).toBe(before);
      // The list has the keyboard again.
      await page.keyboard.press('ArrowDown');
      await expect(tree(page).getByRole('treeitem', { name: label!, selected: true })).toHaveCount(
        0,
      );
    });
  }

  for (const mode of ['list', 'table', 'grid', 'waterfall']) {
    test(`Back comes back to the same scroll position in the ${mode} view`, async ({ page }) => {
      await setup(page, mode);
      await page.goto('/?mockLibrary=wall#/');
      const viewport = page
        .locator('[data-key^="family:"]')
        .first()
        .locator('xpath=ancestor::*[@tabindex="0"][1]');
      // Not on a line's edge: only the exact place passes.
      await viewport.evaluate((el) => (el.scrollTop = 1537));
      await expect.poll(() => viewport.evaluate((el) => el.scrollTop)).toBeGreaterThan(1000);
      const key = await rowOnScreen(viewport);
      await page
        .locator(`[data-key="${key}"]`)
        .first()
        .click({ position: { x: 200, y: 20 } });
      const before = await viewport.evaluate((el) => el.scrollTop);
      await page.keyboard.press('o');
      await onFamilyPage(page);
      await page.keyboard.press('Escape');
      await expect(page).not.toHaveURL(/family/);
      await expect(page.locator(`[data-key="${key}"]`).first()).toBeVisible();
      expect(await viewport.evaluate((el) => el.scrollTop)).toBe(before);
    });
  }

  test('the Layout view moves its specimen and gets its scroll back', async ({ page }) => {
    await page.setViewportSize({ width: 1000, height: 500 });
    await setup(page, 'layout');
    await page.goto('/?mockLibrary=wall#/');
    const layout = page.locator('.layout');
    await expect(layout).toHaveAttribute('data-state', 'ready');
    const top = await layout.evaluate((el) => {
      el.scrollTop = 60;
      return el.scrollTop;
    });
    expect(top).toBeGreaterThan(0);
    // The shared name on each side, as each transition takes its snapshot of that side.
    await page.evaluate(() => {
      const w = window as never as { named: boolean[] };
      w.named = [];
      const isNamed = () =>
        [...document.querySelectorAll<HTMLElement>('.layout .cell')].some(
          (el) => el.style.viewTransitionName === 'fm-family-specimen',
        );
      const start = document.startViewTransition.bind(document);
      document.startViewTransition = ((cb: () => Promise<void>) => {
        const going = document.documentElement.classList.contains('fm-family-opening');
        if (going) w.named.push(isNamed());
        return start(async () => {
          await cb();
          if (!going) w.named.push(isNamed());
        });
      }) as typeof document.startViewTransition;
    });
    await page.keyboard.press('o');
    await onFamilyPage(page);
    await expect(page.locator('html')).not.toHaveClass(/fm-family-opening/);
    await page.keyboard.press('Escape');
    await expect(page).not.toHaveURL(/family/);
    await expect(page.locator('html')).not.toHaveClass(/fm-family-closing/);
    await expect(layout).toHaveAttribute('data-state', 'ready');
    await expect.poll(() => layout.evaluate((el) => el.scrollTop)).toBe(top);
    expect(await page.evaluate(() => (window as never as { named: boolean[] }).named)).toEqual([
      true,
      true,
    ]);
    // Back gives the keyboard to the view again.
    await expect(layout).toBeFocused();
  });

  test('Back comes back to the same place inside an expanded family', async ({ page }) => {
    await setup(page, 'list');
    await page.goto('/?mockLibrary=wall#/');
    const list = tree(page);
    await familyRows(page).nth(1).click();
    await page.keyboard.press('ArrowRight');
    const styles = list.locator('[aria-level="2"]');
    await expect.poll(() => styles.count()).toBeGreaterThan(3);
    const line = await familyRows(page)
      .nth(1)
      .evaluate((el) => (el as HTMLElement).offsetHeight);
    // The third style of the family is the top row.
    await list.evaluate((el, top) => (el.scrollTop = top), line * 4 + 5);
    await expect.poll(() => list.evaluate((el) => el.scrollTop)).toBe(line * 4 + 5);
    await page.keyboard.press('o');
    await onFamilyPage(page);
    await page.keyboard.press('Escape');
    await expect(page).not.toHaveURL(/family/);
    await expect(tree(page)).toBeVisible();
    expect(await tree(page).evaluate((el) => el.scrollTop)).toBe(line * 4 + 5);
  });

  test('the button pops the page, and a selected style survives it', async ({ page }) => {
    await setup(page);
    await page.goto('/?mockLibrary=wall#/');
    await familyRows(page).nth(1).click();
    await page.keyboard.press('ArrowRight');
    await page.keyboard.press('ArrowDown');
    const style = tree(page).getByRole('treeitem', { selected: true });
    const label = await style.getAttribute('aria-label');
    const entries = await page.evaluate(() => history.length);
    await page.keyboard.press('o');
    await onFamilyPage(page);
    await page.getByRole('button', { name: 'Library' }).click();
    await expect(page).not.toHaveURL(/family/);
    await expect(tree(page).getByRole('treeitem', { name: label!, selected: true })).toBeVisible();
    // It popped: forward goes back to the page, nothing was pushed on top.
    expect(await page.evaluate(() => history.length)).toBe(entries + 1);
    await page.goBack();
    expect(page.url()).not.toMatch(/family/);
  });

  test('a deep link goes to the library, with nothing to pop', async ({ page }) => {
    await setup(page);
    await page.goto('/?mockLibrary=wall#/family/501');
    await expect(position(page)).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(tree(page)).toBeVisible();
    await expect(page).not.toHaveURL(/family/);
  });

  test('Esc closes an open dialog first and stays on the page', async ({ page }) => {
    await setup(page);
    await page.goto('/?mockLibrary=wall#/family/501');
    await page.getByRole('button', { name: 'Tag', exact: true }).click();
    const panel = page.getByRole('dialog', { name: /^Tags of / });
    await expect(panel).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(panel).toBeHidden();
    await onFamilyPage(page);
  });

  test('Esc in the collection menu closes only the menu', async ({ page }) => {
    await setup(page);
    await page.goto('/?mockLibrary=wall#/family/501');
    const add = page.getByRole('combobox', { name: 'Add to collection' });
    await add.click();
    await expect(add).toHaveAttribute('aria-expanded', 'true');
    await page.keyboard.press('Escape');
    await expect(add).toHaveAttribute('aria-expanded', 'false');
    await onFamilyPage(page);
  });
});

test.describe('the top bar', () => {
  test('shows the place in the list and the arrows move through it', async ({ page }) => {
    await setup(page);
    await page.goto('/?mockLibrary=wall#/family/501');
    await expect(position(page)).toContainText('2 of 60');
    await expect(position(page)).toContainText('next family');
    await page.keyboard.press('ArrowRight');
    await expect(page).toHaveURL(/#\/family\/502$/);
    await expect(position(page)).toContainText('3 of 60');
    await page.keyboard.press('ArrowLeft');
    await page.keyboard.press('ArrowLeft');
    await expect(position(page)).toContainText('1 of 60');
    await expect(page.getByRole('button', { name: 'Previous family' })).toBeDisabled();
    await page.getByRole('button', { name: 'Next family' }).click();
    await expect(position(page)).toContainText('2 of 60');
  });

  test('the arrows follow the filtered order, and Back still goes to the library', async ({
    page,
  }) => {
    await setup(page);
    await page.goto('/?mockLibrary=wall#/');
    // Families that are not next to each other without the filter.
    await page.getByRole('searchbox').first().fill('Serif');
    await expect(page.getByTestId('active-count')).toBeVisible();
    await expect.poll(() => familyRows(page).count()).toBeLessThan(10);
    const shown = await familyRows(page).count();
    expect(shown).toBeGreaterThan(1);
    const second = await familyRows(page).nth(1).getAttribute('data-key');
    await familyRows(page).first().click();
    await expect(page).toHaveURL(/face=\d+/);
    const url = page.url();
    await page.keyboard.press('o');
    await expect(position(page)).toContainText(`1 of ${shown}`);
    await page.keyboard.press('ArrowRight');
    await expect(page).toHaveURL(new RegExp(`#/family/${second!.replace('family:', '')}$`));
    await expect(position(page)).toContainText(`2 of ${shown}`);
    await page.keyboard.press('Escape');
    // A pop: the arrows added no history, so the list's own URL comes back with its face.
    await expect(page).toHaveURL(url);
    await expect(tree(page)).toBeVisible();
    await expect(page.getByRole('searchbox').first()).toHaveValue('Serif');
  });

  test('has the four icon buttons and the primary action on the accent', async ({ page }) => {
    await setup(page);
    await page.goto('/?mockLibrary=wall#/family/501');
    for (const name of ['Favorite', 'Tag', 'Find similar']) {
      await expect(page.getByRole('button', { name, exact: true })).toBeVisible();
    }
    await expect(page.getByRole('combobox', { name: 'Add to collection' })).toBeVisible();
    const activate = page.getByRole('button', { name: 'Activate family' });
    const colors = await activate.evaluate((el) => {
      const probe = document.createElement('div');
      document.body.append(probe);
      probe.style.color = 'var(--fm-accent)';
      probe.style.backgroundColor = 'var(--fm-on-accent)';
      const s = getComputedStyle(probe);
      const want = { accent: s.color, onAccent: s.backgroundColor };
      probe.remove();
      const got = getComputedStyle(el);
      return { want, bg: got.backgroundColor, fg: got.color };
    });
    expect(colors.bg).toBe(colors.want.accent);
    expect(colors.fg).toBe(colors.want.onAccent);
  });

  test('Activate family activates it and then offers to deactivate', async ({ page }) => {
    await setup(page);
    await page.goto('/?mockLibrary=wall#/family/501');
    await page.getByRole('button', { name: 'Activate family' }).click();
    await expect(page.getByRole('button', { name: 'Deactivate family' })).toBeVisible();
    await page.getByRole('button', { name: 'Deactivate family' }).click();
    await expect(page.getByRole('button', { name: 'Activate family' })).toBeVisible();
  });

  test('Favorite toggles and stays for the family', async ({ page }) => {
    await setup(page);
    // Past the wall's seeded favorites: the first 23 faces start as favorites.
    await page.goto('/?mockLibrary=wall#/family/530');
    const fav = page.getByRole('button', { name: 'Favorite', exact: true });
    await expect(fav).toHaveAttribute('aria-pressed', 'false');
    await fav.click();
    await expect(fav).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('ArrowRight');
    await expect(fav).toHaveAttribute('aria-pressed', 'false');
    await page.keyboard.press('ArrowLeft');
    await expect(fav).toHaveAttribute('aria-pressed', 'true');
  });

  test('Add to collection puts the family in a manual collection', async ({ page }) => {
    await setup(page);
    await page.goto('/?mockLibrary=wall#/');
    const sidebar = page.getByRole('region', { name: 'Collections' });
    await sidebar.getByRole('button', { name: 'New collection', exact: true }).click();
    const input = sidebar.getByRole('textbox', { name: 'New collection name' });
    await input.fill('Picks');
    await input.press('Enter');
    await expect(sidebar.getByRole('listitem', { name: 'Picks', exact: true })).toBeVisible();
    await familyRows(page).nth(1).click();
    await page.keyboard.press('o');
    await onFamilyPage(page);
    await page.getByRole('combobox', { name: 'Add to collection' }).click();
    await page.getByRole('option', { name: 'Picks' }).click();
    await expect(page.getByTestId('toast')).toContainText('Added to Picks');
    // The keyboard reaches it too, and the arrows stay in the menu while it is open.
    await page.getByRole('combobox', { name: 'Add to collection' }).click();
    await expect(page.getByRole('option', { name: 'Picks' })).toBeVisible();
    // The tint alone is too faint to show which option Enter takes.
    await expect(page.getByRole('option', { selected: true })).toHaveCSS('box-shadow', /inset$/);
    await page.keyboard.press('ArrowRight');
    await onFamilyPage(page);
    const url = page.url();
    await page.keyboard.press('Enter');
    await expect(page.getByRole('option', { name: 'Picks' })).toBeHidden();
    expect(page.url()).toBe(url);
  });

  test('Find similar goes back and shows the results', async ({ page }) => {
    await setup(page);
    await page.goto('/?mockLibrary=wall#/family/501');
    await page.getByRole('button', { name: 'Find similar' }).click();
    await expect(page).not.toHaveURL(/family/);
    await expect(page.getByText(/^Similar to/).first()).toBeVisible();
  });
});

test.describe('the page frame', () => {
  test('has the hero, the axis row, the switch, the main column and the rail', async ({ page }) => {
    await setup(page);
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto('/?mockLibrary=wall#/family/501');
    await expect(page.getByTestId('family-hero')).toBeVisible();
    await expect(page.getByTestId('family-axes')).toBeVisible();
    await expect(page.getByRole('group', { name: 'Section' })).toBeVisible();
    const main = (await page.getByTestId('family-main').boundingBox())!;
    const rail = (await page.getByTestId('family-rail').boundingBox())!;
    expect(main.width).toBeGreaterThanOrEqual(560);
    expect(rail.width).toBe(320);
    expect(rail.x).toBeGreaterThan(main.x + main.width - 1);
    expect(rail.y).toBe(main.y);
  });

  test('the rail wraps under the main column below 1100 px', async ({ page }) => {
    await setup(page);
    await page.setViewportSize({ width: 1099, height: 800 });
    await page.goto('/?mockLibrary=wall#/family/501');
    await expect(page.getByTestId('family-rail')).toBeVisible();
    const main = (await page.getByTestId('family-main').boundingBox())!;
    const rail = (await page.getByTestId('family-rail').boundingBox())!;
    expect(rail.y).toBeGreaterThanOrEqual(main.y + main.height);
    await page.setViewportSize({ width: 1100, height: 800 });
    await expect
      .poll(async () => (await page.getByTestId('family-rail').boundingBox())!.y)
      .toBe((await page.getByTestId('family-main').boundingBox())!.y);
  });

  test('a section switch replaces only the panel, in a 120 ms crossfade', async ({ page }) => {
    await setup(page);
    await page.goto('/?mockLibrary=wall#/family/501');
    const panel = page.getByTestId('family-panel');
    await expect(panel).toHaveAttribute('data-section', 'styles');
    // Marks on the DOM nodes: a part that was mounted again loses its mark.
    await page.evaluate(() => {
      for (const id of ['family-hero', 'family-axes', 'family-rail']) {
        (document.querySelector(`[data-testid="${id}"]`) as unknown as { kept: boolean }).kept =
          true;
      }
    });
    const heroTop = (await page.getByTestId('family-hero').boundingBox())!.y;
    const crossfade = page.evaluate(
      () =>
        new Promise<{ duration: string; both: number }>((resolve) => {
          const seen = new MutationObserver(() => {
            const el = document.querySelector<HTMLElement>('.fm-fade-enter-active');
            if (!el) return;
            seen.disconnect();
            resolve({
              duration: getComputedStyle(el).transitionDuration,
              both: document.querySelectorAll('[data-testid="family-panel"]').length,
            });
          });
          seen.observe(document.body, { subtree: true, attributes: true, childList: true });
        }),
    );
    await page.getByRole('button', { name: 'Features' }).click();
    const { duration, both } = await crossfade;
    expect(duration).toBe('0.12s');
    expect(both).toBe(2);
    await expect(panel).toHaveCount(1);
    await expect(panel).toHaveAttribute('data-section', 'features');
    expect((await page.getByTestId('family-hero').boundingBox())!.y).toBe(heroTop);
    const kept = await page.evaluate(() =>
      ['family-hero', 'family-axes', 'family-rail'].map(
        (id) =>
          (document.querySelector(`[data-testid="${id}"]`) as unknown as { kept?: boolean }).kept,
      ),
    );
    expect(kept).toEqual([true, true, true]);
  });

  test('the section stays while the arrows change the family', async ({ page }) => {
    await setup(page);
    await page.goto('/?mockLibrary=wall#/family/501');
    await page.getByRole('button', { name: 'Info' }).click();
    await expect(page.getByTestId('family-panel')).toHaveCount(1);
    await page.keyboard.press('ArrowRight');
    await expect(page).toHaveURL(/#\/family\/502$/);
    await expect(page.getByTestId('family-panel')).toHaveAttribute('data-section', 'info');
  });
});

test.describe('deep links', () => {
  test('open the page straight from the URL, with its font loaded', async ({ page }) => {
    await setup(page);
    await page.goto('/?mockLibrary=wall#/family/500');
    await expect(page.getByRole('heading', { name: 'Asap Sharp', level: 1 })).toBeVisible();
    const specimen = page.getByTestId('family-specimen');
    await expect(specimen).toBeVisible();
    await expect(specimen).not.toHaveClass(/loading/);
    await expect(position(page)).toContainText('1 of 60');
  });

  test('a link to no family says so and leads back', async ({ page }) => {
    await setup(page);
    await page.goto('/?mockLibrary=wall#/family/abc');
    await expect(page.getByText('No such family')).toBeVisible();
    await page.getByTestId('empty-action').click();
    await expect(tree(page)).toBeVisible();
  });

  test('a link to a family that is gone shows the error', async ({ page }) => {
    await setup(page);
    await page.goto('/?mockLibrary=wall#/family/99999');
    await expect(page.getByRole('alert')).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(tree(page)).toBeVisible();
  });
});

// E46.1: push, pop and the arrows, as the browser runs them.
const EASE = 'cubic-bezier(0.2, 0, 0, 1)';
type Seen = { pseudo: string; duration: number; easing: string };
type Probe = {
  transitions: { rootClass: string; seen: Seen[] }[];
  fades: { page: boolean; content: boolean; duration: string }[];
};

/**
 * Records every view transition (the root class it ran under and the pseudo-elements it
 * animated, read once it is ready) and every start of the fade-in of motion/fadeIn.ts.
 */
async function record(page: Page) {
  await page.addInitScript(() => {
    const probe: Probe = { transitions: [], fades: [] };
    (window as unknown as { probe: Probe }).probe = probe;
    document.addEventListener(
      'animationstart',
      (e) => {
        const el = e.target as HTMLElement;
        if (e.animationName !== 'fm-fade-in') return;
        probe.fades.push({
          page: el.parentElement?.id === 'app',
          content: el.matches('.family-page > .scroll'),
          duration: getComputedStyle(el).animationDuration,
        });
      },
      true,
    );
    const start = document.startViewTransition?.bind(document);
    if (!start) return;
    document.startViewTransition = ((cb: () => Promise<void>) => {
      const entry = { rootClass: document.documentElement.className, seen: [] as Seen[] };
      probe.transitions.push(entry);
      const t = start(cb);
      void t.ready.then(() => {
        for (const a of document.getAnimations()) {
          const fx = a.effect as KeyframeEffect;
          if (!fx.pseudoElement?.startsWith('::view-transition')) continue;
          // A CSS animation's timing function is the easing of its keyframes.
          entry.seen.push({
            pseudo: fx.pseudoElement,
            duration: Number(fx.getComputedTiming().duration),
            easing: String(fx.getKeyframes()[0]?.easing),
          });
        }
      });
      return t;
    }) as typeof document.startViewTransition;
  });
}
const probe = (page: Page) => page.evaluate(() => (window as unknown as { probe: Probe }).probe);
const lastTransition = async (page: Page) => (await probe(page)).transitions.at(-1);

/** The wall, with the second family opened from it, and the page's content drawn. */
async function openSecond(page: Page) {
  await page.goto('/?mockLibrary=wall#/');
  await familyRows(page).nth(1).click();
  await page.keyboard.press('o');
  await onFamilyPage(page);
  await expect(page.getByTestId('family-meta')).toBeVisible();
}

test.describe('page motion', () => {
  test('Back reverses the push at 220 ms on the exit curve', async ({ page }) => {
    await setup(page);
    await record(page);
    await openSecond(page);
    await expect(page.locator('html')).not.toHaveClass(/fm-family-opening/);
    await page.keyboard.press('Escape');
    await expect(page).not.toHaveURL(/family/);
    await expect.poll(async () => (await lastTransition(page))?.seen.length).toBeGreaterThan(0);
    const back = (await lastTransition(page))!;
    expect(back.rootClass).toContain('fm-family-closing');
    // The wall comes back in the root, the hero shrinks back into its card.
    for (const pseudo of [
      '::view-transition-old(root)',
      '::view-transition-new(root)',
      '::view-transition-group(fm-family-specimen)',
    ]) {
      expect(back.seen).toContainEqual({ pseudo, duration: 220, easing: 'ease-in' });
    }
    expect(back.seen.every((s) => s.duration === 220 && s.easing === 'ease-in')).toBe(true);
  });

  test('the arrows crossfade only the page content, at 200 ms', async ({ page }) => {
    await setup(page);
    await record(page);
    await openSecond(page);
    await expect(page.locator('html')).not.toHaveClass(/fm-family-opening/);
    const before = page.url();
    const bar = await page.locator('.family-page > .bar').boundingBox();
    await page.keyboard.press('ArrowRight');
    await expect(page).not.toHaveURL(before);
    await expect.poll(async () => (await lastTransition(page))?.seen.length).toBeGreaterThan(0);
    const stepped = (await lastTransition(page))!;
    expect(stepped.rootClass).toContain('fm-family-stepping');
    // The content's old and new snapshots fade over each other; the root (the top bar) and
    // the hero on its own do not move.
    expect([...new Set(stepped.seen.map((s) => s.pseudo))].sort()).toEqual([
      '::view-transition-group(fm-family-content)',
      '::view-transition-new(fm-family-content)',
      '::view-transition-old(fm-family-content)',
    ]);
    expect(stepped.seen.every((s) => s.duration === 200 && s.easing === EASE)).toBe(true);
    await expect(page.locator('html')).not.toHaveClass(/fm-family-stepping/);
    expect(await page.locator('.family-page > .bar').boundingBox()).toEqual(bar);
    // The new snapshot was the next family's page, not an empty one.
    await expect(page.getByTestId('family-meta')).toBeVisible();
  });

  test('two quick presses move two places while the first change is still pending', async ({
    page,
  }) => {
    await setup(page);
    // A slow webview: the route changes a while after the press, as on a loaded macOS runner.
    await page.addInitScript(() => {
      const start = document.startViewTransition.bind(document);
      document.startViewTransition = ((cb: () => Promise<void>) =>
        start(async () => {
          await new Promise((r) => setTimeout(r, 300));
          await cb();
        })) as typeof document.startViewTransition;
    });
    await page.goto('/?mockLibrary=wall#/family/502');
    await expect(position(page)).toContainText('3 of 60');
    await page.keyboard.press('ArrowLeft');
    await page.keyboard.press('ArrowLeft');
    await expect(position(page)).toContainText('1 of 60');
    // At the first family a third press has nowhere to go.
    await page.keyboard.press('ArrowLeft');
    await page.keyboard.press('ArrowRight');
    await expect(position(page)).toContainText('2 of 60');
  });

  test('without view transitions the arrows fade the new content in', async ({ page }) => {
    await setup(page);
    await page.addInitScript(() => {
      Object.defineProperty(document, 'startViewTransition', { value: undefined });
    });
    await record(page);
    await openSecond(page);
    const before = page.url();
    await page.keyboard.press('ArrowRight');
    await expect(page).not.toHaveURL(before);
    await expect
      .poll(async () => (await probe(page)).fades)
      .toContainEqual({ page: false, content: true, duration: '0.2s' });
  });

  test('under reduced motion push, arrows and pop are a 120 ms fade', async ({ page }) => {
    await setup(page);
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await record(page);
    await openSecond(page);
    // No shared element, no slide: the new page fades in.
    await expect
      .poll(async () => (await probe(page)).fades)
      .toEqual([{ page: true, content: false, duration: '0.12s' }]);
    await expect(page.locator('.family-page.slide-in')).toHaveCount(0);
    const before = page.url();
    await page.keyboard.press('ArrowRight');
    await expect(page).not.toHaveURL(before);
    await expect.poll(async () => (await probe(page)).fades.length).toBe(2);
    expect((await probe(page)).fades[1]).toEqual({ page: false, content: true, duration: '0.12s' });
    await page.keyboard.press('Escape');
    await expect(page).not.toHaveURL(/family/);
    await expect.poll(async () => (await probe(page)).fades.length).toBe(3);
    expect((await probe(page)).fades[2]).toEqual({ page: true, content: false, duration: '0.12s' });
    // Reduced motion runs no view transition at all.
    expect((await probe(page)).transitions).toEqual([]);
  });
});
