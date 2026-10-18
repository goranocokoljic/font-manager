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

const SETTINGS_TABS = ['Sources', 'Activation', 'Appearance', 'Advanced'];

test('every interactive element on the main window has a name', async ({ page }) => {
  await open(page);
  await expect(page.getByRole('treeitem').first()).toBeVisible();
  expect(await unnamedControls(page)).toEqual([]);
});

test('every interactive element in the command palette has a name', async ({ page }) => {
  await open(page);
  await page.getByRole('tree', { name: 'Font list' }).focus();
  await page.keyboard.press('ControlOrMeta+k');
  await expect(page.getByRole('dialog')).toBeVisible();
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

test('keyboard focus shows a visible ring on every Tab stop', async ({ page }) => {
  await open(page);
  await expect(page.getByRole('treeitem').first()).toBeVisible();
  const seen = new Set<string>();
  for (let i = 0; i < 40; i++) {
    await page.keyboard.press('Tab');
    const ring = await page.evaluate(() => {
      const el = document.activeElement;
      if (!el || el === document.body) return null;
      // The ring is an outline on the element, or on a :focus-within wrapper
      // drawn as a box-shadow (the list's cursor row).
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
