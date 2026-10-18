import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// Preview rows load face files from `/fmfont/<id>`; serve one fixture for all.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page, query = '') {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(`/${query}#/`);
  const tree = page.getByRole('tree', { name: 'Font list' });
  await expect(tree.getByRole('treeitem').first()).toBeVisible();
  return tree;
}

const SAMPLE_1 = 'Sample 1, 4 styles';

test('10,000 families scroll with recycled rows at a steady frame rate', async ({ page }) => {
  // Three sample families plus 9,997 extra ones.
  const tree = await open(page, '?mockFaces=9997');
  await expect(tree.getByRole('treeitem').first()).toHaveAttribute('aria-setsize', '10000');
  const result = await tree.evaluate(async (el) => {
    const initial = new Set(el.querySelectorAll('[role="treeitem"]'));
    const rowsBefore = initial.size;
    // Keyed rows are moved (removed and re-added) as the window shifts; count only
    // treeitem elements that were not there at the start.
    let created = 0;
    const observer = new MutationObserver((records) => {
      for (const r of records)
        for (const n of r.addedNodes)
          if (n instanceof Element && n.getAttribute('role') === 'treeitem' && !initial.has(n))
            created += 1;
    });
    observer.observe(el, { childList: true, subtree: true });
    // Scroll 2.5 rows a frame for 600 frames (about 10 s at 60 fps) and time each frame.
    const step = el.scrollHeight / 10_000 / 0.4;
    const frames: number[] = [];
    await new Promise<void>((done) => {
      let last = performance.now();
      const tick = (now: number) => {
        frames.push(now - last);
        last = now;
        el.scrollTop += step;
        if (frames.length < 600) requestAnimationFrame(tick);
        else done();
      };
      requestAnimationFrame(tick);
    });
    observer.disconnect();
    frames.shift(); // the first interval is not a scrolled frame
    const sorted = [...frames].sort((a, b) => a - b);
    const mean = frames.reduce((a, b) => a + b, 0) / frames.length;
    return {
      rowsBefore,
      rowsAfter: el.querySelectorAll('[role="treeitem"]').length,
      reused: [...el.querySelectorAll('[role="treeitem"]')].every((n) => initial.has(n)),
      created,
      fps: 1000 / mean,
      p95: sorted[Math.floor(sorted.length * 0.95)]!,
      scrolled: el.scrollTop,
    };
  });
  test.info().annotations.push({
    type: 'perf',
    description: `fps ${result.fps.toFixed(1)}, p95 frame ${result.p95.toFixed(1)} ms`,
  });
  console.log(`font list: ${result.fps.toFixed(1)} fps, p95 frame ${result.p95.toFixed(1)} ms`);
  expect(result.scrolled).toBeGreaterThan(0);
  // Row recycling: the same row elements the whole way, none created while scrolling.
  expect(result.created).toBe(0);
  expect(result.reused).toBe(true);
  expect(result.rowsAfter).toBe(result.rowsBefore);
  expect(result.rowsBefore).toBeLessThan(40);
  // The 60 fps budget is checked on the reference machine (FM_PERF=1); CI runners vary.
  if (process.env.FM_PERF) expect(result.fps).toBeGreaterThanOrEqual(58);
});

test('expanding a family shows its styles inline', async ({ page }) => {
  const tree = await open(page, '?mockFaces=3&mockStyles=4');
  const family = tree.getByRole('treeitem', { name: SAMPLE_1 });
  await expect(family).toHaveAttribute('aria-expanded', 'false');
  await family.locator('.twisty').click();
  await expect(family).toHaveAttribute('aria-expanded', 'true');
  const styles = tree.locator('[role="treeitem"][aria-level="2"]');
  await expect(styles).toHaveCount(4);
  await expect(styles).toHaveText([/Regular/, /Italic/, /Bold/, /Bold Italic/]);
  // Each style previews in its own face, right under the family.
  const order = await tree
    .locator('[role="treeitem"]')
    .evaluateAll((rows) => rows.map((r) => r.getAttribute('aria-label')));
  expect(order.slice(3, 8)).toEqual([
    `${SAMPLE_1}, not active`,
    'Sample 1 Regular, not active',
    'Sample 1 Italic, not active',
    'Sample 1 Bold, not active',
    'Sample 1 Bold Italic, not active',
  ]);
  await expect(styles.nth(2).locator('.row')).toHaveAttribute('data-state', 'ready');
  await family.locator('.twisty').click();
  await expect(styles).toHaveCount(0);
});

test('arrows, Enter and Tab work in the list, and the inspector follows', async ({ page }) => {
  const tree = await open(page, '?mockFaces=3&mockStyles=4');
  await tree.getByRole('treeitem', { name: 'Inter, 1 style' }).click();
  await expect(tree).toBeFocused();
  await expect(page).toHaveURL(/face=1\b/);
  const info = page.getByRole('tabpanel', { name: 'Info' });
  await expect(info.getByRole('heading', { name: 'Inter Regular' })).toBeVisible();

  await page.keyboard.press('ArrowDown');
  await expect(tree).toHaveAttribute('aria-activedescendant', 'fm-row-family-3');
  await expect(info.getByRole('heading', { name: 'JetBrains Mono Regular' })).toBeVisible();

  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown'); // Sample 1
  await page.keyboard.press('Enter');
  await expect(tree.getByRole('treeitem', { name: SAMPLE_1 })).toHaveAttribute(
    'aria-expanded',
    'true',
  );
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown'); // Italic
  const italic = tree.getByRole('treeitem', { name: 'Sample 1 Italic, not active', exact: true });
  await expect(italic).toHaveAttribute('aria-selected', 'true');
  await expect(info.getByRole('heading', { name: 'Sample 1 Italic' })).toBeVisible();
  await page.keyboard.press('ArrowLeft'); // to the family
  await page.keyboard.press('ArrowLeft'); // collapse
  await expect(italic).toHaveCount(0);

  // Tab leaves the list in one press (to the inspector's divider, then the inspector);
  // Shift+Tab comes back to the same row.
  await page.keyboard.press('Tab');
  await expect(page.getByRole('separator', { name: 'Resize inspector' })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(page.locator('#fm-inspector')).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await page.keyboard.press('Shift+Tab');
  await expect(tree).toBeFocused();
  await expect(tree).toHaveAttribute('aria-activedescendant', 'fm-row-family-1000');
});

test('Enter on a style opens the inspector when it is hidden', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 700 }); // inspector collapsed
  const tree = await open(page, '?mockFaces=3&mockStyles=4');
  await expect(page.locator('#fm-inspector')).toHaveCount(0);
  await tree.getByRole('treeitem', { name: SAMPLE_1 }).click();
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('Enter');
  await expect(
    page.getByRole('tabpanel', { name: 'Info' }).getByRole('heading', { name: 'Sample 1 Regular' }),
  ).toBeVisible();
});

test('multi-select with Shift, Ctrl/Cmd and the mouse', async ({ page }) => {
  const tree = await open(page, '?mockFaces=3&mockStyles=4');
  const status = page.locator('.toolbar [role="status"]');
  const selected = tree.locator('[role="treeitem"][aria-selected="true"]');
  await tree.getByRole('treeitem', { name: 'Inter, 1 style' }).click();
  await page.keyboard.press('Shift+ArrowDown');
  await page.keyboard.press('Shift+ArrowDown');
  await expect(selected).toHaveCount(3);
  await expect(status).toHaveText('3 selected');

  await tree
    .getByRole('treeitem', { name: 'Sample 3, 4 styles' })
    .click({ modifiers: ['ControlOrMeta'] });
  await expect(selected).toHaveCount(4);
  await tree
    .getByRole('treeitem', { name: 'JetBrains Mono, 1 style' })
    .click({ modifiers: ['ControlOrMeta'] });
  await expect(selected).toHaveCount(3);

  await tree.getByRole('treeitem', { name: 'Legacy Grotesk, 1 style' }).click();
  await tree.getByRole('treeitem', { name: 'Sample 2, 4 styles' }).click({ modifiers: ['Shift'] });
  await expect(selected).toHaveCount(3);

  await page.keyboard.press('ControlOrMeta+a');
  await expect(selected).toHaveCount(6);
  await expect(status).toHaveText('6 selected');
  await page.keyboard.press('ArrowUp');
  await expect(selected).toHaveCount(1);
  await expect(status).toHaveText('');
});

for (const scheme of ['light', 'dark'] as const) {
  test(`selected rows are marked at 3:1 contrast or more (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    const tree = await open(page, '?mockFaces=3&mockStyles=4');
    await tree.getByRole('treeitem', { name: 'Inter, 1 style' }).click();
    await page.keyboard.press('Shift+ArrowDown');
    await page.keyboard.press('Shift+ArrowDown');
    const marks = await tree.evaluate((el) => {
      const rgb = (c: string) => (c.match(/[\d.]+/g) ?? []).slice(0, 3).map(Number);
      const lum = (c: string) => {
        const [r, g, b] = rgb(c).map((v) => {
          const s = v / 255;
          return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
        });
        return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!;
      };
      const ratio = (a: string, b: string) => {
        const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
        return (hi! + 0.05) / (lo! + 0.05);
      };
      const background = getComputedStyle(el).backgroundColor;
      return [...el.querySelectorAll('[role="treeitem"]')].map((row) => {
        const bar = getComputedStyle(row, '::before');
        return {
          selected: row.getAttribute('aria-selected') === 'true',
          cursor: row.id === el.getAttribute('aria-activedescendant'),
          bar: bar.content !== 'none' && bar.content !== 'normal',
          width: bar.width,
          contrast: ratio(bar.backgroundColor, background),
        };
      });
    });
    const selected = marks.filter((m) => m.selected);
    expect(selected).toHaveLength(3);
    // Rows away from the cursor too: the cursor ring is not the only cue.
    expect(selected.filter((m) => !m.cursor)).toHaveLength(2);
    for (const m of selected) {
      expect(m.bar).toBe(true);
      expect(m.width).toBe('4px');
      expect(m.contrast).toBeGreaterThanOrEqual(3);
    }
    for (const m of marks.filter((m) => !m.selected)) expect(m.bar).toBe(false);
  });
}
