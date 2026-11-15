import { expect, test, type Page } from '@playwright/test';
import { openSection, sectionPanel } from './family-page';

const tab = (page: Page, name: string) => page.getByRole('tab', { name });
const bg = (page: Page) => page.evaluate(() => getComputedStyle(document.body).backgroundColor);

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto('/');
});

test('Ctrl/Cmd+, opens Settings with its eight sections; Escape closes it', async ({ page }) => {
  await page.keyboard.press('ControlOrMeta+,');
  await expect(page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible();
  for (const name of [
    'Sources',
    'Activation',
    'Appearance',
    'Online catalogs',
    'Advanced',
    'Local API',
    'AI assistants',
    'About',
  ])
    await expect(tab(page, name)).toBeVisible();
  await expect(tab(page, 'Sources')).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('list', { name: 'Folders' })).toContainText('/Users/demo/Fonts');

  await tab(page, 'Advanced').click();
  await expect(page.getByRole('textbox', { name: 'Preview cache size (MB)' })).toHaveValue('1024');
  await expect(page.getByRole('textbox', { name: 'Rescan sources every (minutes)' })).toHaveValue(
    '60',
  );
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
  await page.goto('/#/settings?section=activation');
  await openSection(page, 1, 'Info');
  await sectionPanel(page, 'Info').getByRole('button', { name: 'Activate', exact: true }).click();
  await expect(page.getByTestId('activation-state')).toHaveText('Active (permanent)');

  // Face 6 clashes with a system font: replaced without a question, for this session only.
  await openSection(page, 6, 'Info');
  await sectionPanel(page, 'Info').getByRole('button', { name: 'Activate', exact: true }).click();
  await expect(page.getByTestId('activation-state')).toHaveText('Active for this session');
  await expect(page.getByRole('alertdialog')).toHaveCount(0);
});

test('the status bar counts active fonts as activation changes', async ({ page }) => {
  await page.goto('/#/?face=1');
  const count = page.getByTestId('active-count');
  await expect(count).toHaveText('0 fonts active');
  await page.getByTestId('selected-row').waitFor();
  await page.getByRole('tree', { name: 'Font list' }).focus();
  await page.keyboard.press('a');
  await expect(count).toHaveText('1 font active');
  await page.keyboard.press('a');
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

// E50.13: Settings › Sources as cards (dev-docs/design/redesign/11-sources.png). The mock
// picker answers `/Users/demo/Added 1` first; a folder it never scanned holds 12 fonts and
// its scan steps every 150 ms, so the Scanning state is seen.
test('a folder is added, scanned, unwatched and removed; the system card stays', async ({
  page,
}) => {
  await page.goto('/#/settings');
  const cards = page.getByRole('list', { name: 'Folders' });
  await page.getByRole('button', { name: 'Add folder', exact: true }).click();
  const added = cards.getByRole('listitem', { name: 'Added 1', exact: true });
  await expect(added).toBeVisible();
  await expect(added.getByText('/Users/demo/Added 1')).toBeVisible();

  // Scanning: the badge and the 3 px bar, fed by the scan events.
  await expect(added.getByText('Scanning', { exact: true })).toBeVisible();
  const bar = added.getByRole('progressbar', { name: 'Scanning Added 1' });
  await expect(bar).toBeVisible();
  expect(await bar.evaluate((el) => el.getBoundingClientRect().height)).toBe(3);

  // Done: the badge and bar leave, the count and the last scan show.
  await expect(bar).toHaveCount(0);
  await expect(added.getByText('Scanning', { exact: true })).toHaveCount(0);
  await expect(added.getByText('12 fonts')).toBeVisible();
  await expect(added.getByText(/^scanned /)).toBeVisible();

  // Watch off.
  const watch = added.getByRole('switch', { name: 'Watch Added 1' });
  await expect(watch).toHaveAttribute('aria-checked', 'true');
  await watch.click();
  await expect(watch).toHaveAttribute('aria-checked', 'false');

  // The actions show on hover, as 28 px targets.
  const remove = added.getByRole('button', { name: 'Remove Added 1 from the library' });
  await added.hover();
  await expect(remove).toBeVisible();
  expect(await remove.evaluate((el) => el.getBoundingClientRect().height)).toBe(28);

  // Remove asks once; Escape keeps it, Remove forgets it.
  await remove.click();
  const question = added.getByRole('alertdialog');
  await expect(question).toContainText('the files stay where they are');
  await page.keyboard.press('Escape');
  await expect(question).toHaveCount(0);
  await expect(page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible();
  await expect(remove).toBeFocused();
  await remove.click();
  await added.getByRole('alertdialog').getByRole('button', { name: 'Remove' }).click();
  await expect(added).toHaveCount(0);
  await expect(cards.getByRole('listitem', { name: 'Fonts', exact: true })).toBeVisible();

  // The System fonts card keeps its Show switch and its line; it is never removed.
  await expect(page.getByText('the app never writes to system folders')).toBeVisible();
  await expect(
    page.getByRole('switch', { name: 'Show system fonts in the library' }),
  ).toBeVisible();
});

test('a cloud folder and a missing one; the drop zone and the problems strip', async ({ page }) => {
  await page.goto('/?mockSources=1#/settings');
  const cards = page.getByRole('list', { name: 'Folders' });
  const missing = cards.getByRole('listitem', { name: 'Old downloads', exact: true });
  await expect(missing.getByText('Missing', { exact: true })).toBeVisible();
  await expect(missing.getByText('folder not found')).toBeVisible();
  await expect(
    missing.getByRole('button', { name: 'Show Old downloads in the file manager' }),
  ).toBeDisabled();

  const cloud = cards.getByRole('listitem', { name: 'Studio type', exact: true });
  await cloud.hover();
  await cloud.getByRole('button', { name: 'Show Studio type in the file manager' }).click();
  expect(
    await page.evaluate(
      () => (globalThis as { __fmMockShownFolders?: string[] }).__fmMockShownFolders,
    ),
  ).toEqual(['/Users/demo/Dropbox/Studio type']);

  await expect(page.getByTestId('sources-drop')).toHaveText('Choose a folder to add');
  const strip = page.getByTestId('sources-problems');
  await expect(strip).toContainText('in these folders');
  await strip.getByRole('link', { name: 'Open Problems' }).click();
  await expect(page).toHaveURL(/#\/problems/);
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
    await tab(page, 'Appearance').click();
    expect(await borderContrast(page, 'Language')).toBeGreaterThanOrEqual(3);
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

test('"Start at login" is visible on Windows, macOS and Linux alike', async ({ page }) => {
  for (const platform of ['windows', 'macos', 'linux']) {
    await page.goto(`/?mockPlatform=${platform}#/`);
    await expect(page.getByTestId('status-settings')).toBeVisible();
    await page.keyboard.press('ControlOrMeta+,');
    await tab(page, 'About').click();
    await expect(page.getByTestId('app-info')).toContainText(platform);
    await tab(page, 'Activation').click();
    await expect(page.getByRole('checkbox', { name: 'Start at login' })).toBeVisible();
    await expect(page.getByRole('checkbox', { name: 'Start at login' })).not.toBeChecked();
  }
});

test('"Start at login" stays hidden where fm-api does not offer it (Flatpak)', async ({ page }) => {
  await page.goto('/?mockStartAtLogin=3#/settings?section=activation');
  await expect(
    page.getByRole('checkbox', { name: 'Keep running in the tray when the window is closed' }),
  ).toBeVisible();
  await expect(page.getByRole('checkbox', { name: 'Start at login' })).toHaveCount(0);
});

test('"Start at login" is a labelled switch the keyboard reaches and turns on', async ({
  page,
}) => {
  await page.goto('/#/settings?section=activation');
  const tray = page.getByRole('checkbox', {
    name: 'Keep running in the tray when the window is closed',
  });
  const login = page.getByRole('checkbox', { name: 'Start at login' });
  await expect(login).not.toBeChecked();
  await expect(login).toHaveAccessibleDescription(/starts in the tray when you log in/);
  await tray.focus();
  await page.keyboard.press('Tab');
  await expect(login).toBeFocused();
  await page.keyboard.press('Space');
  await expect(login).toBeChecked();
  await expect(page.getByTestId('toast')).toHaveCount(0);
  // The saved state comes back from fm-api after a reopen.
  await page.getByRole('button', { name: 'Close' }).click();
  await page.keyboard.press('ControlOrMeta+,');
  await tab(page, 'Activation').click();
  await expect(login).toBeChecked();
});

test('"Start at login" shows an error toast when the system refuses the entry', async ({
  page,
}) => {
  await page.goto('/?mockStartAtLogin=2#/settings?section=activation');
  const login = page.getByRole('checkbox', { name: 'Start at login' });
  await login.focus();
  await page.keyboard.press('Space');
  await expect(page.getByTestId('toast')).toContainText(
    'Start at login could not be turned on: the system refused the login entry.',
  );
  await expect(login).not.toBeChecked();
});

test('the status-bar Settings link looks like a link and opens Settings', async ({ page }) => {
  const link = page.getByTestId('status-settings');
  await expect(link).toHaveAttribute('title', /^Open Settings \((Ctrl\+,|⌘,)\)$/);
  const colors = await link.evaluate((el) => ({
    link: getComputedStyle(el).color,
    text: getComputedStyle(document.documentElement).getPropertyValue('--fm-fg').trim(),
  }));
  const probe = await page.evaluate((c) => {
    const p = document.createElement('span');
    p.style.color = c;
    document.body.append(p);
    const color = getComputedStyle(p).color;
    p.remove();
    return color;
  }, colors.text);
  expect(colors.link).toBe(probe);
  await link.hover();
  await expect(link).toHaveCSS('text-decoration-line', 'underline');
  await link.click();
  await expect(page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible();
});
