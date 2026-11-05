import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { showPreviewOptions } from './select';

// The wall (E41.1): the grid view's uniform cards. The wall library has 60 families;
// `mockFaces` adds the rest for the 1,000-family frame-rate check.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function openWall(page: Page, query = '?mockLibrary=wall'): Promise<Locator> {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(`/${query}#/`);
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
  // E41.5 removes the inspector from the wall; the board has none.
  await page.getByRole('button', { name: 'Inspector', exact: true }).click();
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Grid' })
    .click();
  const wall = page.getByRole('listbox', { name: 'Font grid' });
  await expect(wall).toBeVisible();
  return wall;
}

/** Moves the size slider as a drag does: an input event at each value. */
const setSize = (page: Page, value: number) =>
  page.getByRole('slider', { name: 'Size slider' }).evaluate((el: HTMLInputElement, v) => {
    el.value = String(v);
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);

const cards = (wall: Locator) => wall.getByRole('option');

test.describe('the wall', () => {
  test('cards: white on white, hairline, 12 px radius, weight 500 name, tabular count', async ({
    page,
  }) => {
    const wall = await openWall(page);
    const card = cards(wall).first().locator('.card');
    const looks = await card.evaluate((el) => {
      const s = getComputedStyle(el);
      const name = getComputedStyle(el.querySelector('.name')!);
      const detail = getComputedStyle(el.querySelector('.detail')!);
      return {
        background: s.backgroundColor,
        radius: s.borderRadius,
        border: s.borderTopWidth,
        borderColor: s.borderTopColor,
        ground: getComputedStyle(el.closest('[role="listbox"]')!).backgroundColor,
        nameWeight: name.fontWeight,
        figures: detail.fontVariantNumeric,
        hairline: getComputedStyle(document.documentElement).getPropertyValue('--fm-border').trim(),
      };
    });
    expect(looks).toMatchObject({
      background: 'rgb(255, 255, 255)',
      ground: 'rgb(255, 255, 255)',
      radius: '12px',
      border: '1px',
      nameWeight: '500',
      figures: 'tabular-nums',
    });
    expect(looks.hairline).toContain('9%');
    expect(looks.borderColor).toMatch(/^rgba\(0, 0, 0, 0\.09/);
    await expect(cards(wall).first().locator('[data-activation]')).toHaveCount(1);
  });

  test('dark theme: the card takes the dark ground', async ({ page }) => {
    await page.emulateMedia({ colorScheme: 'dark' });
    const wall = await openWall(page);
    await expect(cards(wall).first().locator('.card')).toHaveCSS(
      'background-color',
      'rgb(28, 28, 30)',
    );
  });

  test('the specimen is centred and a wide one is never cut', async ({ page }) => {
    const wall = await openWall(page);
    await page.getByRole('textbox', { name: 'Custom text' }).fill('Supercalifragilistic');
    await setSize(page, 120);
    const thumb = cards(wall).first().getByTestId('thumbnail');
    await expect(thumb).toHaveAttribute('data-state', 'ready');
    await expect
      .poll(() =>
        thumb.evaluate((el) => {
          const s = getComputedStyle(el);
          const [w, h] = (s.maskSize || s.webkitMaskSize).split(' ').map(parseFloat);
          return {
            position: s.maskPosition || s.webkitMaskPosition,
            fitsWidth: w! <= el.clientWidth + 0.5,
            fitsHeight: h! <= el.clientHeight + 0.5,
          };
        }),
      )
      .toEqual({ position: '50% 50%', fitsWidth: true, fitsHeight: true });
  });

  test('live text shrinks to fit and is never cut', async ({ page }) => {
    const wall = await openWall(page);
    await page.getByRole('textbox', { name: 'Custom text' }).fill('Supercalifragilistic');
    await showPreviewOptions(page);
    await page.getByRole('slider', { name: 'Tracking slider' }).fill('0.1');
    await setSize(page, 120);
    const live = cards(wall).first().getByTestId('card-live-text');
    await expect(live).toHaveAttribute('data-state', 'ready');
    await expect
      .poll(() =>
        live.evaluate((el) => {
          const line = el.querySelector<HTMLElement>('.line')!;
          return {
            shrunk: parseFloat(getComputedStyle(el).fontSize) < 120,
            fits: line.offsetWidth <= el.clientWidth + 0.5,
            text: line.textContent,
          };
        }),
      )
      .toEqual({ shrunk: true, fits: true, text: 'Supercalifragilistic' });
  });

  test('four columns at 1440 px, following the width; only the lines on screen render', async ({
    page,
  }) => {
    const wall = await openWall(page, '?mockLibrary=wall&mockFaces=940');
    await setSize(page, 28);
    await expect(wall).toHaveAttribute('data-columns', '4');
    const count = await cards(wall).count();
    expect(count).toBeGreaterThan(0);
    expect(count).toBeLessThan(100);
    const first = await cards(wall).first().boundingBox();
    expect(first!.width).toBeGreaterThanOrEqual(260);
    await page.setViewportSize({ width: 1000, height: 900 });
    await expect(wall).toHaveAttribute('data-columns', '2');
    await page.setViewportSize({ width: 700, height: 900 });
    await expect(wall).toHaveAttribute('data-columns', '1');
  });

  test('the slider has the 24–120 range on the wall only', async ({ page }) => {
    await openWall(page);
    const slider = page.getByRole('slider', { name: 'Size slider' });
    await expect(slider).toHaveAttribute('min', '24');
    await expect(slider).toHaveAttribute('max', '120');
    await page
      .getByRole('group', { name: 'View mode' })
      .getByRole('button', { name: 'List' })
      .click();
    await expect(slider).toHaveAttribute('min', '8');
    await expect(slider).toHaveAttribute('max', '300');
  });

  test('the slider resizes the cards and they glide there, each keeping its element', async ({
    page,
  }) => {
    const wall = await openWall(page);
    await setSize(page, 28);
    const card = cards(wall).nth(5);
    await card.evaluate((el) => {
      (el as HTMLElement & { marked?: boolean }).marked = true;
    });
    const key = await card.getAttribute('data-key');
    const before = await card.boundingBox();

    // In one task with the input event: what the browser reports right after the update.
    await setSize(page, 100);
    const after = wall.locator(`[data-key="${key}"]`);
    await expect
      .poll(() =>
        after.evaluate((el) => (el as HTMLElement & { marked?: boolean }).marked === true),
      )
      .toBe(true);
    await expect
      .poll(async () => (await after.boundingBox())!.height)
      .toBeGreaterThan(before!.height);
    // The wall now has fewer columns: the sixth card moved along both axes.
    await expect(wall).toHaveAttribute('data-columns', '2');

    const moved = await page.evaluate(async () => {
      const input = document.querySelector<HTMLInputElement>('input[aria-label="Size slider"]')!;
      input.value = '60';
      input.dispatchEvent(new Event('input', { bubbles: true }));
      await new Promise((r) => requestAnimationFrame(r));
      return document
        .getAnimations()
        .filter((a): a is CSSTransition => a instanceof CSSTransition)
        .filter((a) => (a.effect as KeyframeEffect).target?.matches('.slot'))
        .map((a) => a.transitionProperty);
    });
    expect(moved.length).toBeGreaterThan(0);
    expect(new Set(moved)).toEqual(new Set(['transform']));
  });

  test('a moved anchor card keeps its place near the top, without flying across the scroll', async ({
    page,
  }) => {
    const wall = await openWall(page, '?mockLibrary=wall&mockFaces=940');
    await setSize(page, 40);
    await expect(wall).toHaveAttribute('data-columns', '4');
    await wall.evaluate((el) => {
      el.scrollTop = 100 * 179; // Line 100 at size 40: 116 + 53 + 10.
    });
    await expect
      .poll(() =>
        wall.evaluate((el) => el.querySelector('[data-key]') !== null && el.scrollTop > 0),
      )
      .toBe(true);
    await page.waitForTimeout(150);
    const moved = await page.evaluate(async () => {
      const box = document.querySelector('[role="listbox"]')!;
      const y = (el: Element) => el.getBoundingClientRect().top - box.getBoundingClientRect().top;
      // The card at the top of the viewport.
      const top = [...box.querySelectorAll('[data-key]')]
        .filter((c) => y(c) >= 0)
        .sort((a, b) => y(a) - y(b))[0]!;
      const key = top.getAttribute('data-key')!;
      const before = y(top);
      const input = document.querySelector<HTMLInputElement>('input[aria-label="Size slider"]')!;
      input.value = '44';
      input.dispatchEvent(new Event('input', { bubbles: true }));
      await new Promise((r) => requestAnimationFrame(r));
      await new Promise((r) => requestAnimationFrame(r));
      return Math.abs(y(box.querySelector(`[data-key="${key}"]`)!) - before);
    });
    // Mid-glide it is about where it was; a fly-in across the scroll would be 1,200 px away.
    expect(moved).toBeLessThan(40);
  });

  test('opening the wall does not glide: cards start at their places', async ({ page }) => {
    const wall = await openWall(page);
    await page
      .getByRole('group', { name: 'View mode' })
      .getByRole('button', { name: 'List' })
      .click();
    await expect(wall).toHaveCount(0);
    const gliding = await page.evaluate(async () => {
      const grid = [
        ...document.querySelectorAll('[role="group"][aria-label="View mode"] button'),
      ].find(
        (b) => b.textContent?.includes('Grid') || b.getAttribute('aria-label') === 'Grid',
      ) as HTMLElement;
      grid.click();
      const seen = new Set<number>();
      for (let i = 0; i < 6; i++) {
        await new Promise((r) => requestAnimationFrame(r));
        const moving = document
          .getAnimations()
          .filter((a) => (a.effect as KeyframeEffect).target?.matches('.slot'));
        seen.add(moving.length);
      }
      return Math.max(...seen);
    });
    await expect(cards(wall).first()).toBeVisible();
    expect(gliding).toBe(0);
  });

  test('a selected card has a 2 px accent ring; a hovered one lifts', async ({ page }) => {
    const wall = await openWall(page);
    const first = cards(wall).first();
    const second = cards(wall).nth(1);
    await first.click();
    await expect(first).toHaveAttribute('aria-selected', 'true');
    await expect
      .poll(() =>
        first.locator('.card').evaluate((el) => {
          const s = getComputedStyle(el, '::after');
          return {
            width: s.borderTopWidth,
            style: s.borderTopStyle,
            opacity: s.opacity,
          };
        }),
      )
      .toEqual({ width: '2px', style: 'solid', opacity: '1' });
    const ringColor = await first
      .locator('.card')
      .evaluate((el) => getComputedStyle(el, '::after').borderTopColor);
    const accent = await page.evaluate(() => {
      const probe = document.createElement('div');
      probe.style.color = 'var(--fm-accent)';
      document.body.append(probe);
      const c = getComputedStyle(probe).color;
      probe.remove();
      return c;
    });
    expect(ringColor).toBe(accent);
    // Not selected: no ring.
    expect(
      await second.locator('.card').evaluate((el) => getComputedStyle(el, '::after').opacity),
    ).toBe('0');

    const shadow = () => second.locator('.card').evaluate((el) => getComputedStyle(el).boxShadow);
    expect(await shadow()).toBe('none');
    await second.hover();
    await expect.poll(shadow).not.toBe('none');
  });

  test('Space opens Quick Look, A toggles activation, Enter acts as in the other views', async ({
    page,
  }) => {
    const wall = await openWall(page);
    const family = wall.getByRole('option', { name: /^Crimson Pro,/ });
    await family.click();
    await page.keyboard.press('Space');
    const look = page.getByRole('dialog', { name: /^Quick look: Crimson Pro/ });
    await expect(look).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(look).toHaveCount(0);

    await family.click();
    await expect(family).toHaveAttribute('aria-label', /not active/);
    await page.keyboard.press('a');
    await expect(family).not.toHaveAttribute('aria-label', /not active/);

    // Until the family page exists (E42.1), Enter opens the family's styles, as today.
    const before = await cards(wall).count();
    await family.click();
    await page.keyboard.press('Enter');
    await expect(wall.locator('.slot.face').first()).toBeVisible();
    expect(await cards(wall).count()).toBeGreaterThanOrEqual(before);
  });

  test('the slider holds 60 fps with 1,000 families', async ({ page }) => {
    const wall = await openWall(page, '?mockLibrary=wall&mockFaces=940');
    // 1,000 families at 4 per line: 250 lines, a little over 144 px each at the default size.
    await expect
      .poll(() => wall.locator('.spacer').evaluate((el) => el.getBoundingClientRect().height))
      .toBe(10 + 250 * 144);
    await expect(cards(wall).first().getByTestId('thumbnail')).toHaveAttribute(
      'data-state',
      'ready',
    );
    // Median frame rate of `frames` animation frames; `work` runs in every frame.
    const measure = (work: boolean, frames: number) =>
      page.evaluate(
        ([work, frames]) =>
          new Promise<number>((resolve) => {
            const input = document.querySelector<HTMLInputElement>(
              'input[aria-label="Size slider"]',
            )!;
            const stamps: number[] = [];
            let size = 24;
            let step = 1;
            const tick = (now: number) => {
              stamps.push(now);
              if (work) {
                size += step;
                if (size >= 120 || size <= 24) step = -step;
                input.value = String(size);
                input.dispatchEvent(new Event('input', { bubbles: true }));
              }
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
        [work, frames] as const,
      );
    // What this machine's animation frames run at when nothing happens: 60 on a display at
    // 60 Hz. A machine that cannot reach 55 idle (one macOS runner gave 30, others gave 60)
    // cannot prove the target, so the test says so as a skip; it still fails if the slider
    // costs frames there.
    const idle = await measure(false, 60);
    const fps = await measure(true, 180);
    console.log(`wall slider: ${fps.toFixed(1)} fps median (idle ${idle.toFixed(1)})`);
    if (idle < 55) {
      expect(fps).toBeGreaterThanOrEqual(idle - 5);
      test.skip(true, `not measured: animation frames run at ${idle.toFixed(1)} fps when idle`);
    }
    expect(fps).toBeGreaterThanOrEqual(55);
  });
});
