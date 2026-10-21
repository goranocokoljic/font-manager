import { expect, test, type Page } from '@playwright/test';

const tab = (page: Page, name: string) => page.getByRole('tab', { name });
const bg = (page: Page) => page.evaluate(() => getComputedStyle(document.body).backgroundColor);

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto('/');
});

test('Ctrl/Cmd+, opens Settings with its five sections; Escape closes it', async ({ page }) => {
  await page.keyboard.press('ControlOrMeta+,');
  await expect(page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible();
  for (const name of ['Sources', 'Activation', 'Appearance', 'Advanced', 'About'])
    await expect(tab(page, name)).toBeVisible();
  await expect(tab(page, 'Sources')).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('list', { name: 'Sources' })).toContainText('/Users/demo/Fonts');

  await tab(page, 'Advanced').click();
  await expect(page.getByRole('spinbutton', { name: 'Preview cache size (MB)' })).toHaveValue(
    '1024',
  );
  await expect(
    page.getByRole('spinbutton', { name: 'Rescan sources every (minutes)' }),
  ).toHaveValue('60');
  await expect(
    page.getByRole('button', { name: /^Copy logs folder: .*font-manager\/logs$/ }),
  ).toBeVisible();

  await page.keyboard.press('Escape');
  await expect(page.getByRole('heading', { name: 'All fonts' })).toBeVisible();
});

test('theme and density apply at once, without a reload', async ({ page }) => {
  await page.goto('/#/settings?section=appearance');
  const light = await bg(page);
  await page.getByRole('radio', { name: 'Dark' }).check();
  await expect.poll(() => bg(page)).not.toBe(light);
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');

  await page.getByRole('radio', { name: 'Compact' }).check();
  await expect(page.locator('html')).toHaveAttribute('data-density', 'compact');
  await expect
    .poll(() => page.evaluate(() => getComputedStyle(document.documentElement).fontSize))
    .toBe('12px');

  await page.getByRole('radio', { name: 'Same as the system' }).check();
  await expect.poll(() => bg(page)).toBe(light);
});

test('the activation choice applies to the next activation', async ({ page }) => {
  await page.goto('/#/settings?section=activation');
  await page
    .getByRole('radio', { name: 'Replace the other font for this session without asking' })
    .check();
  await page.getByRole('radio', { name: 'Permanently (also after a restart)' }).check();
  await page.getByRole('button', { name: 'Close' }).click();

  // Face 1 has no clash: permanent.
  await page.goto('/#/?face=1');
  await page.getByRole('button', { name: 'Activate' }).click();
  await expect(page.getByTestId('activation-state')).toHaveText('Active (permanent)');

  // Face 6 clashes with a system font: replaced without a question, for this session only.
  await page.goto('/#/?face=6');
  await page.getByRole('button', { name: 'Activate' }).click();
  await expect(page.getByTestId('activation-state')).toHaveText('Active for this session');
  await expect(page.getByRole('alertdialog')).toHaveCount(0);
});

test('the status bar counts active fonts as activation changes', async ({ page }) => {
  await page.goto('/#/?face=1');
  const count = page.getByTestId('active-count');
  await expect(count).toHaveText('0 fonts active');
  await page.getByRole('button', { name: 'Activate' }).click();
  await expect(count).toHaveText('1 font active');
  await page.getByRole('button', { name: 'Deactivate' }).click();
  await expect(count).toHaveText('0 fonts active');
});

test('"reactivate on launch" is saved', async ({ page }) => {
  await page.goto('/#/settings?section=activation');
  const box = page.getByRole('checkbox', { name: /Reactivate on launch/ });
  await expect(box).not.toBeChecked();
  await box.check();
  await page.getByRole('button', { name: 'Close' }).click();
  await page.goto('/#/settings?section=activation');
  await expect(page.getByRole('checkbox', { name: /Reactivate on launch/ })).toBeChecked();
});

test('a folder can be added, paused and removed; the system source stays', async ({ page }) => {
  await page.goto('/#/settings');
  // The mock folder picker answers `/Users/demo/Added 1` on its first call.
  await page.getByRole('button', { name: 'Add folder…' }).click();
  const list = page.getByRole('list', { name: 'Sources' });
  const added = '/Users/demo/Added 1';
  await expect(list).toContainText(added);

  await page.getByRole('checkbox', { name: `Pause ${added}` }).check();
  await expect(page.getByRole('checkbox', { name: `Pause ${added}` })).toBeChecked();
  await page.getByRole('button', { name: `Remove ${added}` }).click();
  await expect(list).not.toContainText(added);
  await expect(page.getByRole('button', { name: 'Remove /System/Library/Fonts' })).toBeDisabled();
});

/** WCAG contrast ratio of the field's border against the page background. */
const borderContrast = (page: Page, name: string) =>
  page.getByLabel(name).evaluate((el) => {
    const rgb = (c: string) => (c.match(/\d+(\.\d+)?/g) ?? []).slice(0, 3).map(Number);
    const lum = (c: string) => {
      const [r, g, b] = rgb(c).map((v) => {
        const s = v / 255;
        return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
      });
      return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    };
    const a = lum(getComputedStyle(el).borderTopColor);
    const b = lum(getComputedStyle(document.body).backgroundColor);
    return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
  });

for (const scheme of ['light', 'dark'] as const) {
  test(`form fields are outlined at 3:1 or more (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto('/#/settings');
    await tab(page, 'Advanced').click();
    for (const name of ['Preview cache size (MB)', 'Rescan sources every (minutes)'])
      expect(await borderContrast(page, name)).toBeGreaterThanOrEqual(3);
  });
}

test('"Keep running in the tray" is on by default, saves, and About lists the platform notes', async ({
  page,
}) => {
  await page.goto('/#/settings?section=activation');
  const keep = page.getByRole('checkbox', {
    name: 'Keep running in the tray when the window is closed',
  });
  await expect(keep).toBeChecked();
  await expect(keep).toHaveAccessibleDescription(/closing the window quits Font Manager/);
  await keep.focus();
  await page.keyboard.press('Space');
  await expect(keep).not.toBeChecked();
  // Saved: it stays off after the window is opened again.
  await page.getByRole('button', { name: 'Close' }).click();
  await page.goto('/#/settings?section=activation');
  await expect(
    page.getByRole('checkbox', { name: 'Keep running in the tray when the window is closed' }),
  ).not.toBeChecked();

  await tab(page, 'About').click();
  await expect(page.getByRole('heading', { name: 'Platform notes' })).toBeVisible();
  await expect(page.getByRole('list', { name: 'Platform notes' })).toContainText(
    'no StatusNotifier host), so closing the window quits Font Manager',
  );
});
