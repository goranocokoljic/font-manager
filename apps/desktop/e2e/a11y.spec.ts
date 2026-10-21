import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
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

const SETTINGS_TABS = ['Sources', 'Activation', 'Appearance', 'Advanced', 'About'];

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
