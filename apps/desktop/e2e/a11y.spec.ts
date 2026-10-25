import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';

// The accessibility baseline (E5.10): every control has a name and a visible
// focus ring, the list announces family, style and activation state, and the
// OS reduce-motion setting stops animations.

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page, route = '/') {
  await page.route('**/fmfont/*', (r) =>
    r.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(`/#${route}`);
  await expect(page.locator('#app')).not.toBeEmpty();
}

/** Interactive elements (focusable or with an interactive role) without an accessible name. */
async function unnamedControls(page: Page): Promise<string[]> {
  const controls = page.locator(
    [
      'button',
      'a[href]',
      'input:not([type=hidden])',
      'select',
      'textarea',
      '[tabindex]:not([tabindex="-1"])',
      '[role=button]',
      '[role=tab]',
      '[role=option]',
      '[role=treeitem]',
      '[role=separator][tabindex]',
      '[role=combobox]',
      '[role=listbox]',
      '[role=tree]',
      '[role=slider]',
      '[role=checkbox]',
      '[role=radio]',
      '[role=switch]',
    ].join(','),
  );
  const missing: string[] = [];
  for (const control of await controls.all()) {
    if (!(await control.isVisible())) continue;
    try {
      await expect(control).toHaveAccessibleName(/\S/, { timeout: 50 });
    } catch {
      missing.push(await control.evaluate((el) => el.outerHTML.slice(0, 160)));
    }
  }
  return missing;
}

const SETTINGS_TABS = [
  'Sources',
  'Activation',
  'Appearance',
  'Online catalogs',
  'Advanced',
  'Local API',
  'AI assistants',
  'About',
];

test('every interactive element on the main window has a name', async ({ page }) => {
  await open(page);
  await expect(page.getByRole('treeitem').first()).toBeVisible();
  expect(await unnamedControls(page)).toEqual([]);
});

for (const [mode, role, item] of [
  ['Grid', 'listbox', 'option'],
  ['Waterfall', 'tree', 'treeitem'],
] as const) {
  test(`every interactive element in the ${mode.toLowerCase()} view has a name`, async ({
    page,
  }) => {
    await open(page);
    await page
      .getByRole('group', { name: 'View mode' })
      .getByRole('button', { name: mode })
      .click();
    const view = page.getByRole(role, { name: `Font ${mode.toLowerCase()}` });
    await expect(view.getByRole(item).first()).toBeVisible();
    expect(await unnamedControls(page)).toEqual([]);
  });
}

test('every interactive element in the command palette has a name', async ({ page }) => {
  await open(page);
  await page.getByRole('tree', { name: 'Font list' }).focus();
  await page.keyboard.press('ControlOrMeta+k');
  await expect(page.getByRole('dialog')).toBeVisible();
  expect(await unnamedControls(page)).toEqual([]);
});

test('every interactive element in the Quick Collection review has a name', async ({ page }) => {
  await open(page);
  await page.getByRole('treeitem').first().click();
  await page.keyboard.press('b');
  await expect(page.getByTestId('quick-count')).not.toHaveText('0');
  await page.getByTestId('quick-collection').click();
  await expect(page.getByRole('heading', { name: 'Quick Collection' })).toBeVisible();
  await expect(page.getByRole('button', { name: /^Drop / }).first()).toBeVisible();
  expect(await unnamedControls(page)).toEqual([]);
});

test('every interactive element of the tagging panel and the Tags group has a name', async ({
  page,
}) => {
  await open(page);
  await page.getByRole('treeitem').first().click();
  await page.keyboard.press('t');
  const panel = page.getByRole('dialog', { name: /^Tags of / });
  await panel.getByRole('combobox').fill('Display');
  await page.keyboard.press('Enter');
  await expect(panel.getByRole('option', { name: /Display/ })).toBeVisible();
  expect(await unnamedControls(page)).toEqual([]);
  // The filter field's outline is the 3:1 form-field color (WCAG 1.4.11).
  const outline = await panel.getByRole('combobox').evaluate((el) => {
    const probe = document.createElement('div');
    probe.style.borderTop = '1px solid var(--fm-control-border)';
    document.body.append(probe);
    const want = getComputedStyle(probe).borderTopColor;
    probe.remove();
    return [getComputedStyle(el).borderTopColor, want];
  });
  expect(outline[0]).toBe(outline[1]);
  await page.keyboard.press('Escape');
  await expect(page.getByRole('region', { name: 'Tags' }).locator('li[data-tag]')).toHaveCount(1);
  expect(await unnamedControls(page)).toEqual([]);
});

test('every interactive element in Settings has a name', async ({ page }) => {
  await open(page, '/settings');
  for (const tab of SETTINGS_TABS) {
    const control = page.getByRole('tab', { name: tab });
    if (!(await control.count())) continue;
    await control.click();
    expect(await unnamedControls(page), tab).toEqual([]);
  }
});

test('every interactive element of the Conflicts view and the clash question has a name', async ({
  page,
}) => {
  await open(page, '/?face=6');
  await page
    .getByRole('tabpanel', { name: 'Info' })
    .getByRole('button', { name: 'Activate' })
    .click();
  await expect(page.getByRole('alertdialog')).toBeVisible();
  expect(await unnamedControls(page)).toEqual([]);
  await page.getByRole('button', { name: 'Replace for this session' }).click();
  await open(page, '/conflicts');
  await expect(page.getByRole('heading', { name: 'Font conflicts' })).toBeVisible();
  expect(await unnamedControls(page)).toEqual([]);
});

test('every interactive element of the inspector Developer tab has a name', async ({ page }) => {
  await open(page, '/?face=1');
  await page.getByRole('tab', { name: 'Developer' }).click();
  const panel = page.getByRole('tabpanel', { name: 'Developer' });
  await expect(panel.getByRole('button', { name: 'Copy the CSS snippet' })).toBeEnabled();
  expect(await unnamedControls(page)).toEqual([]);
});

test('every interactive element of the Problems view has a name', async ({ page }) => {
  await open(page, '/problems');
  await expect(page.getByRole('table', { name: 'Problems' })).toBeVisible();
  expect(await unnamedControls(page)).toEqual([]);
});

test('keyboard focus shows a visible ring on every Tab stop', async ({ page }) => {
  await open(page);
  await expect(page.getByRole('treeitem').first()).toBeVisible();
  const seen = new Set<string>();
  for (let i = 0; i < 40; i++) {
    await page.keyboard.press('Tab');
    const ring = await page.evaluate(() => {
      const el = document.activeElement;
      if (!el || el === document.body) return null;
      // The ring is an outline of at least 2 px, or a box-shadow ring (the pane divider).
      const s = getComputedStyle(el);
      const width = parseFloat(s.outlineWidth);
      const visible = (s.outlineStyle !== 'none' && width >= 2) || s.boxShadow !== 'none';
      return { id: el.outerHTML.slice(0, 120), visible };
    });
    if (!ring || seen.has(ring.id)) continue;
    seen.add(ring.id);
    expect(ring.visible, ring.id).toBe(true);
  }
  expect(seen.size).toBeGreaterThan(3);
});

test('the list announces family, style and activation state', async ({ page }) => {
  await open(page);
  const tree = page.getByRole('tree', { name: 'Font list' });
  const family = tree.getByRole('treeitem', { name: 'Inter, 1 style', exact: false });
  await expect(family).toHaveAccessibleName('Inter, 1 style, not active');
  await family.locator('.twisty').click();
  const style = tree.getByRole('treeitem', { level: 2 }).first();
  await expect(style).toHaveAccessibleName(/^Inter \S.*, not active$/);
  await style.click();
  await page.keyboard.press('Enter'); // open it in the inspector
  await page.getByRole('button', { name: 'Activate' }).click();
  await expect(style).toHaveAccessibleName(/^Inter \S.*, active for this session$/);
  await expect(family).toHaveAccessibleName('Inter, 1 style, active');
});

/** Elements that animate or transition for longer than 1 ms. */
const moving = (page: Page) =>
  page.evaluate(
    () =>
      [...document.querySelectorAll('*')]
        .map((el) => getComputedStyle(el))
        .filter(
          (s) =>
            (s.animationName !== 'none' && parseFloat(s.animationDuration) > 0.001) ||
            parseFloat(s.transitionDuration) > 0.001,
        ).length,
  );

for (const reducedMotion of ['reduce', 'no-preference'] as const) {
  test(`motion follows the OS setting: ${reducedMotion}`, async ({ page }) => {
    await page.emulateMedia({ reducedMotion });
    await open(page);
    // A probe stands for any animation a future component adds.
    await page.addStyleTag({
      content:
        '@keyframes probe{to{opacity:.5}} .probe{animation:probe 1s infinite;transition:opacity 1s}',
    });
    await page.evaluate(() => {
      const el = document.createElement('div');
      el.className = 'probe';
      document.body.append(el);
    });
    await page.getByRole('tree', { name: 'Font list' }).focus();
    await page.keyboard.press('ControlOrMeta+k');
    await expect(page.getByRole('dialog')).toBeVisible();
    if (reducedMotion === 'reduce') expect(await moving(page)).toBe(0);
    else expect(await moving(page)).toBeGreaterThan(0);
  });
}

for (const colorScheme of ['light', 'dark'] as const) {
  test(`placeholder text is at least 4.5:1 in the ${colorScheme} theme`, async ({ page }) => {
    await page.emulateMedia({ colorScheme });
    await open(page);
    await page.getByRole('tree', { name: 'Font list' }).focus();
    await page.keyboard.press('ControlOrMeta+k');
    const input = page.getByRole('dialog').getByRole('combobox');
    await expect(input).toHaveAttribute('placeholder', /\S/);
    const ratio = await input.evaluate((el) => {
      const rgb = (c: string) => (c.match(/[\d.]+/g) ?? []).slice(0, 3).map(Number);
      const lum = (c: number[]) => {
        const lin = c.map((v) => {
          const s = v / 255;
          return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
        });
        return 0.2126 * lin[0]! + 0.7152 * lin[1]! + 0.0722 * lin[2]!;
      };
      // The first opaque background behind the input.
      let node: Element | null = el;
      let bg = 'rgb(255, 255, 255)';
      while (node) {
        const c = getComputedStyle(node).backgroundColor;
        if (c !== 'rgba(0, 0, 0, 0)' && c !== 'transparent') {
          bg = c;
          break;
        }
        node = node.parentElement;
      }
      const ph = getComputedStyle(el, '::placeholder');
      if (parseFloat(ph.opacity) < 1) return 0;
      const [a, b] = [lum(rgb(ph.color)), lum(rgb(bg))].sort((x, y) => y - x);
      return (a! + 0.05) / (b! + 0.05);
    });
    expect(ratio).toBeGreaterThanOrEqual(4.5);
  });
}

test('every control of the smart-collection rule editor has a name', async ({ page }) => {
  await open(page);
  await page.getByRole('button', { name: 'New smart collection' }).click();
  const dialog = page.getByRole('dialog', { name: 'New smart collection' });
  await dialog.getByRole('button', { name: 'Add group' }).click();
  await dialog
    .getByRole('combobox', { name: 'Group 1, rule 1: what it tests' })
    .selectOption({ label: 'Weight' });
  await dialog.getByRole('button', { name: 'Add rule', exact: true }).click();
  await dialog
    .getByRole('combobox', { name: 'Rule 2: what it tests', exact: true })
    .selectOption({ label: 'Supports text' });
  await expect(dialog.getByRole('textbox', { name: 'Rule 2: text' })).toBeVisible();
  expect(await unnamedControls(page)).toEqual([]);
});

// WCAG 2.2 AA, scanned by axe-core (E19.1): every view mode, the palette, every
// inspector tab, Settings, Compare, Quick look and Problems, in both themes.

const WCAG_TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];

/**
 * WCAG 2.5.3 (label in name) for the nodes axe flags. A font item's visible label
 * is its name (the `<bdi>`); its sample text and badges are preview content, which
 * axe cannot tell from a label. A glyph cell shows only its glyph (aria-hidden).
 * True when the label's words appear, in order, in the accessible name.
 */
async function labelInName(page: Page, selector: string): Promise<boolean> {
  return page.evaluate((sel) => {
    const el = document.querySelector(sel);
    if (!el) return false;
    const words = (s: string) =>
      s
        .toLowerCase()
        .split(/[^\p{L}\p{N}]+/u)
        .filter(Boolean);
    let label: string;
    if (el.matches('[data-key]')) label = el.querySelector('bdi')?.textContent ?? '';
    else if (el.matches('[role=gridcell][data-code-point]')) {
      const shown = el.cloneNode(true) as Element;
      shown.querySelectorAll('[aria-hidden="true"]').forEach((n) => n.remove());
      label = shown.textContent ?? '';
    } else return false;
    const want = words(label);
    const name = words(el.getAttribute('aria-label') ?? '');
    if (!want.length) return true;
    for (let i = 0; i + want.length <= name.length; i++)
      if (want.every((w, j) => name[i + j] === w)) return true;
    return false;
  }, selector);
}

/** The WCAG 2.2 AA violations on the page now, one line each. */
async function violations(page: Page): Promise<string[]> {
  // Sheets and the palette rise in; scan them where they come to rest.
  await page.waitForFunction(() =>
    document.getAnimations().every((a) => a.playState !== 'running'),
  );
  const result = await new AxeBuilder({ page }).withTags(WCAG_TAGS).analyze();
  // The 24×24 target check (2.5.8) is part of the run.
  expect(
    [...result.passes, ...result.violations, ...result.inapplicable].map((r) => r.id),
  ).toContain('target-size');
  const found: string[] = [];
  for (const v of result.violations)
    for (const node of v.nodes) {
      const selector = node.target.join(' ');
      if (v.id === 'label-content-name-mismatch' && (await labelInName(page, selector))) continue;
      found.push(`${v.id}: ${selector}: ${node.failureSummary?.replace(/\s+/g, ' ')}`);
    }
  return found;
}

const VIEW_MODES = ['List', 'Table', 'Grid', 'Waterfall', 'Layout'];
const INSPECTOR_TABS = ['Info', 'Axes', 'Features', 'Glyphs', 'Developer'];

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`WCAG 2.2 AA scan, ${colorScheme} theme`, () => {
    test.beforeEach(async ({ page }) => {
      await page.emulateMedia({ colorScheme });
    });

    /** The app follows the OS theme; the page really is in `colorScheme`. */
    async function inTheme(page: Page) {
      const bg = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
      expect(bg).toBe(colorScheme === 'dark' ? 'rgb(28, 28, 30)' : 'rgb(255, 255, 255)');
    }

    test('the main window in every view mode', async ({ page }) => {
      await open(page);
      await expect(page.getByRole('treeitem').first()).toBeVisible();
      await inTheme(page);
      const modes = page.getByRole('group', { name: 'View mode' });
      for (const mode of VIEW_MODES) {
        await modes.getByRole('button', { name: mode }).click();
        await expect(modes.getByRole('button', { name: mode })).toHaveAttribute(
          'aria-pressed',
          'true',
        );
        expect(await violations(page), mode).toEqual([]);
      }
    });

    test('the command palette and Quick look', async ({ page }) => {
      await open(page);
      await page.getByRole('treeitem').first().click();
      await page.keyboard.press('ControlOrMeta+k');
      await expect(page.getByRole('dialog', { name: 'Command palette' })).toBeVisible();
      expect(await violations(page), 'palette').toEqual([]);
      await page.keyboard.press('Escape');
      await page.keyboard.press('Space');
      await expect(page.getByRole('dialog', { name: /^Quick look: / })).toBeVisible();
      expect(await violations(page), 'quick look').toEqual([]);
    });

    test('every inspector tab', async ({ page }) => {
      await open(page, '/?face=1');
      await inTheme(page);
      for (const tab of INSPECTOR_TABS) {
        await page.getByRole('tab', { name: tab }).click();
        await expect(page.getByRole('tabpanel', { name: tab })).toBeVisible();
        expect(await violations(page), tab).toEqual([]);
      }
    });

    test('every Settings tab', async ({ page }) => {
      await open(page, '/settings');
      await inTheme(page);
      for (const tab of SETTINGS_TABS) {
        await page.getByRole('tab', { name: tab }).click();
        await expect(page.getByRole('tab', { name: tab })).toHaveAttribute('aria-selected', 'true');
        expect(await violations(page), tab).toEqual([]);
      }
    });

    test('Compare', async ({ page }) => {
      await page.route('**/fmfont/*', (r) =>
        r.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
      );
      await page.goto('/?mockFaces=3#/');
      const tree = page.getByRole('tree', { name: 'Font list' });
      await tree.getByRole('treeitem').first().click();
      for (let i = 0; i < 3; i++) {
        await page.keyboard.press('c');
        await page.keyboard.press('ArrowDown');
      }
      await page.getByRole('button', { name: /^Compare \d\/4$/ }).click();
      const sheet = page.getByRole('dialog', { name: 'Compare' });
      await expect(sheet).toBeVisible();
      await inTheme(page);
      for (const mode of ['Side by side', 'Stacked', 'Overlay']) {
        await sheet.getByRole('radio', { name: mode }).check();
        expect(await violations(page), mode).toEqual([]);
      }
    });

    test('Problems', async ({ page }) => {
      await open(page, '/problems');
      await expect(page.getByRole('table', { name: 'Problems' })).toBeVisible();
      await inTheme(page);
      expect(await violations(page)).toEqual([]);
    });
  });
}

// The spec's keyboard model, from the keyboard alone (E19.1, WCAG 2.1.1, 2.1.2,
// 2.4.3, 2.4.11): / · Ctrl/Cmd+K · Space · A · Shift+A · C · 1–5 · Ctrl/Cmd+, ·
// arrows · Enter · Tab between columns.

/** A short description of the focused element. */
const focused = (page: Page) =>
  page.evaluate(() => {
    const el = document.activeElement;
    if (!el || el === document.body) return 'body';
    const role = el.getAttribute('role') ?? '';
    return `${el.tagName.toLowerCase()} ${role} ${el.getAttribute('aria-label') ?? ''}`.trim();
  });

/** True when some point of the focused element is on top, not under a sheet or toast (2.4.11). */
const focusSeen = (page: Page) =>
  page.evaluate(() => {
    const el = document.activeElement;
    if (!el || el === document.body) return true;
    const r = el.getBoundingClientRect();
    const xs = [r.left + 1, (r.left + r.right) / 2, r.right - 1];
    const ys = [r.top + 1, (r.top + r.bottom) / 2, r.bottom - 1];
    return xs.some((x) =>
      ys.some((y) => {
        const hit = document.elementFromPoint(x, y);
        return !!hit && (hit === el || el.contains(hit) || hit.contains(el));
      }),
    );
  });

test('Tab moves between the columns and wraps around: the focus is never trapped', async ({
  page,
}) => {
  await open(page);
  await expect(page.getByRole('treeitem').first()).toBeVisible();
  const stops: string[] = [];
  for (let i = 0; i < 300; i++) {
    await page.keyboard.press('Tab');
    const now = await focused(page);
    expect(await focusSeen(page), now).toBe(true);
    if (now === stops[0]) break;
    stops.push(now);
  }
  // Back at the first stop: Tab left the page and came round again.
  expect(await focused(page)).toBe(stops[0]);
  const at = (stop: string) => stops.indexOf(stop);
  const columns = [
    'nav  Library',
    'div separator Resize sidebar',
    'main  Fonts',
    'div tree Font list',
    'div separator Resize inspector',
    'aside  Inspector',
  ];
  for (const c of columns) expect(at(c), c).toBeGreaterThanOrEqual(0);
  expect(columns.map(at)).toEqual(columns.map(at).sort((a, b) => a - b));
});

test('every action of the keyboard model works without a mouse', async ({ page }) => {
  await open(page);
  const tree = page.getByRole('tree', { name: 'Font list' });
  await expect(tree.getByRole('treeitem').first()).toBeVisible();

  // / goes to search.
  await page.keyboard.press('/');
  const search = page.getByRole('searchbox', { name: 'Search fonts' });
  await expect(search).toBeFocused();

  // Ctrl/Cmd+K opens the palette; Escape closes it and the focus goes back.
  await page.keyboard.press('ControlOrMeta+k');
  const palette = page.getByRole('dialog', { name: 'Command palette' });
  await expect(palette).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(palette).toBeHidden();
  await expect(search).toBeFocused();

  // Tab between columns: from the search box back to its column, then into the list.
  await page.keyboard.press('Shift+Tab');
  await expect(page.getByRole('main', { name: 'Fonts' })).toBeFocused();
  // Arrows on the column move into the list; arrows and Enter work the list.
  await page.keyboard.press('ArrowDown');
  await expect(tree).toBeFocused();
  await page.keyboard.press('ArrowDown');
  const family = tree.getByRole('treeitem', { level: 1 }).first();
  await expect(tree).toHaveAttribute('aria-activedescendant', (await family.getAttribute('id'))!);
  await page.keyboard.press('Enter');
  await expect(family).toHaveAttribute('aria-expanded', 'true');
  await page.keyboard.press('ArrowDown');
  const style = tree.getByRole('treeitem', { level: 2 }).first();
  await expect(tree).toHaveAttribute('aria-activedescendant', (await style.getAttribute('id'))!);

  // Space: quick look; Space again closes it and the focus is back on the list.
  await page.keyboard.press('Space');
  const look = page.getByRole('dialog', { name: /^Quick look: Inter / });
  await expect(look).toBeVisible();
  const close = look.getByRole('button', { name: 'Close' });
  await expect(close).toBeFocused();
  expect(await focusSeen(page)).toBe(true);
  await page.keyboard.press('Tab');
  await expect(close).toBeFocused();
  await page.keyboard.press('Space');
  await expect(look).toBeHidden();
  await expect(tree).toBeFocused();

  // A and Shift+A: session and permanent activation of the cursor row.
  await page.keyboard.press('a');
  await expect(style).toHaveAccessibleName(/, active for this session$/);
  await page.keyboard.press('Shift+A');
  await expect(style).toHaveAccessibleName(/, active permanently$/);
  await page.keyboard.press('Shift+A');
  await expect(style).toHaveAccessibleName(/, not active$/);

  // C pins the font to Compare; the toast does not hide the focus.
  await page.keyboard.press('c');
  await expect(page.getByTestId('toast')).toContainText('(1 of 4)');
  await expect(tree).toBeFocused();
  expect(await focusSeen(page)).toBe(true);

  // 1–5: the view modes, in the toolbar's order.
  const modes = page.getByRole('group', { name: 'View mode' });
  for (const [key, mode] of [
    ['2', 'Table'],
    ['3', 'Grid'],
    ['4', 'Waterfall'],
    ['5', 'Layout'],
    ['1', 'List'],
  ] as const) {
    await page.keyboard.press(key);
    await expect(modes.getByRole('button', { name: mode })).toHaveAttribute('aria-pressed', 'true');
  }

  // Ctrl/Cmd+, opens Settings.
  await page.keyboard.press('ControlOrMeta+,');
  await expect(page).toHaveURL(/#\/settings/);
  await expect(page.getByRole('tab', { name: 'Sources' })).toBeVisible();
});

test('the focus stays in a sheet and returns to where it was when the sheet closes', async ({
  page,
}) => {
  await open(page);
  const tree = page.getByRole('tree', { name: 'Font list' });
  await tree.getByRole('treeitem').first().click();
  await expect(tree).toBeFocused();

  await page.keyboard.press('c');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('c');
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('open compare');
  await page.keyboard.press('Enter');
  const compare = page.getByRole('dialog', { name: 'Compare' });
  await expect(compare).toBeVisible();
  for (let i = 0; i < 12; i++) {
    await page.keyboard.press('Tab');
    const inside = await page.evaluate(() => !!document.activeElement?.closest('[role=dialog]'));
    expect(inside).toBe(true);
    expect(await focusSeen(page)).toBe(true);
  }
  await page.keyboard.press('Escape');
  await expect(compare).toBeHidden();
  await expect(tree).toBeFocused();

  await page.keyboard.press('t');
  const tags = page.getByRole('dialog', { name: /^Tags of / });
  await expect(tags).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(tags).toBeHidden();
  await expect(tree).toBeFocused();

  await page.keyboard.press('ControlOrMeta+k');
  await expect(page.getByRole('dialog', { name: 'Command palette' })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(tree).toBeFocused();
});

// Status changes are announced (E19.1, WCAG 4.1.3): scan progress, activation
// changes and toasts are written into live regions that are in the page before
// the change, so a screen reader reads them.

test('scan progress, activation changes and toasts are announced in live regions', async ({
  page,
}) => {
  await open(page);
  const tree = page.getByRole('tree', { name: 'Font list' });
  await expect(tree.getByRole('treeitem').first()).toBeVisible();

  // Scan progress: the sources' status region is there, empty, before the scan.
  const scan = page.getByTestId('scan-status');
  await expect(scan).toHaveRole('status');
  await expect(scan).toHaveText('');
  const sources = page.getByRole('region', { name: 'Sources' });
  await sources.getByRole('button', { name: 'Rescan Fonts' }).click();
  await expect(scan).toHaveText(/^Scanning Fonts: \d+ of 7$/);
  await expect(scan).toHaveText('');

  // Activation: the status bar count is a status region; A changes it.
  const active = page.getByTestId('active-count');
  await expect(active).toHaveRole('status');
  await expect(active).toHaveText('0 fonts active');
  await tree.getByRole('treeitem').first().click();
  await page.keyboard.press('a');
  await expect(active).toHaveText(/^[1-9]\d* fonts? active$/);

  // Toasts: the message lands inside the always-present polite region.
  const toast = page.getByTestId('toast');
  await expect(toast).toBeVisible();
  const region = toast.locator('xpath=..');
  await expect(region).toHaveRole('status');
  await expect(region).toHaveAttribute('aria-live', 'polite');
  await expect(region).toContainText(/Activated/);
});
