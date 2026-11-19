import { expect, test, type Page } from '@playwright/test';

// Setup step three (E55.6, board 13): check for updates automatically. Browser mode opens the
// setup with `?mockSetup=first`; `?mockUpdate=1` offers a patch, `=5` is a Flatpak copy (it
// does not update itself). Every request to the release server the mock sees is kept in
// `window.__fmMockUpdateChecks` (`manual` or `automatic`, the shell's quiet check).

const setup = (page: Page) => page.getByRole('main', { name: 'First-run setup' });
const steps = (page: Page) =>
  setup(page).getByRole('navigation', { name: 'Setup steps' }).getByRole('listitem');
const updatesSwitch = (page: Page) =>
  setup(page).getByRole('switch', { name: 'Check for updates automatically', exact: true });

async function checks(page: Page): Promise<string[]> {
  return page.evaluate(
    () => (window as unknown as { __fmMockUpdateChecks?: string[] }).__fmMockUpdateChecks ?? [],
  );
}

/** A first start: step one skipped, step two answered, on to the updates step. */
async function toUpdates(page: Page, mockUpdate = 1): Promise<void> {
  await page.goto(`/?mockSetup=first&mockUpdate=${mockUpdate}#/`);
  await expect(setup(page)).toBeVisible();
  await setup(page).getByRole('button', { name: 'Skip for now' }).click();
  await expect(
    setup(page).getByRole('heading', { name: 'Use the online catalogs?' }),
  ).toBeFocused();
  await setup(page).getByRole('button', { name: 'Continue' }).click();
  await expect(
    setup(page).getByRole('heading', { name: 'Keep Font Fu up to date?' }),
  ).toBeFocused();
}

/** Settings → About, in the same page (the mock keeps its state). */
async function openAbout(page: Page): Promise<void> {
  await page.evaluate(() => (location.hash = '#/settings?section=about'));
  await expect(page.getByRole('region', { name: 'Updates' })).toBeVisible();
}

test('on: one question, preselected; no request until Finish; then the first check runs quietly', async ({
  page,
}) => {
  await toUpdates(page);
  await expect(steps(page)).toHaveText(['Font folders', 'Online catalogs', 'Updates']);
  await expect(steps(page).nth(2)).toHaveAttribute('aria-current', 'step');
  // One question, on; its line says when; "Install when I quit" (E55.9) is not offered.
  await expect(setup(page).getByRole('switch')).toHaveCount(1);
  await expect(updatesSwitch(page)).toHaveAttribute('aria-checked', 'true');
  await expect(updatesSwitch(page)).toHaveAccessibleDescription(
    'When Font Fu starts and once a day while it runs',
  );
  await expect(setup(page).getByText(/quit/i)).toHaveCount(0);
  await expect(setup(page).getByRole('button', { name: 'Finish' })).toBeVisible();
  // What a check does: the host as the API names it; nothing sent; nothing installed.
  const pane = page.getByTestId('setup-updates-pane');
  await expect(pane.getByTestId('setup-release-host')).toHaveText('github.com');
  await expect(pane).toContainText('Asks github.com for the newest version');
  await expect(pane).toContainText(
    'One request to the release server when Font Fu starts, and one a day while it runs.',
  );
  await expect(pane).toContainText('Sends nothing about you or your fonts');
  await expect(pane).toContainText('Installs nothing by itself');

  // Nothing asks the release server while the step is open, even after Back and Continue.
  await setup(page).getByRole('button', { name: 'Back' }).click();
  await setup(page).getByRole('button', { name: 'Continue' }).click();
  await expect(updatesSwitch(page)).toHaveAttribute('aria-checked', 'true');
  expect(await checks(page), 'no request before Finish').toEqual([]);

  await setup(page).getByRole('button', { name: 'Finish' }).click();
  await expect(setup(page)).toHaveCount(0);
  // The first check runs quietly: no dialog opens.
  await expect.poll(() => checks(page)).toEqual(['automatic']);
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await openAbout(page);
  const updates = page.getByRole('region', { name: 'Updates' });
  await expect(
    updates.getByRole('radio', { name: 'Tell me when there is an update' }),
  ).toBeChecked();
  await expect(updates).toContainText('Version 0.1.1 is available.');
});

test("off: the setting stays off and 'Check for updates' works as before", async ({ page }) => {
  await toUpdates(page);
  await updatesSwitch(page).click();
  await expect(updatesSwitch(page)).toHaveAttribute('aria-checked', 'false');
  await setup(page).getByRole('button', { name: 'Finish' }).click();
  await expect(setup(page)).toHaveCount(0);
  await openAbout(page);
  expect(await checks(page)).toEqual([]);
  const updates = page.getByRole('region', { name: 'Updates' });
  await expect(updates.getByRole('radio', { name: 'Off' })).toBeChecked();
  await updates.getByRole('button', { name: 'Check for updates' }).click();
  await expect(page.getByRole('dialog', { name: 'Font Fu 0.1.1 is available' })).toBeVisible();
  expect(await checks(page)).toEqual(['manual']);
});

test('where this copy does not update itself, the step is not shown and not counted', async ({
  page,
}) => {
  await page.goto('/?mockSetup=first&mockUpdate=5#/');
  await expect(setup(page)).toBeVisible();
  await expect(steps(page)).toHaveText(['Font folders', 'Online catalogs']);
  await setup(page).getByRole('button', { name: 'Skip for now' }).click();
  // The catalogs step is the last: Finish closes the setup.
  await setup(page).getByRole('button', { name: 'Finish' }).click();
  await expect(setup(page)).toHaveCount(0);
  // Run again from Settings: still no updates step.
  await page.evaluate(() => (location.hash = '#/settings?section=advanced'));
  await page.getByRole('button', { name: 'Run setup again' }).click();
  await expect(steps(page)).toHaveText(['Font folders', 'Online catalogs']);
  expect(await checks(page)).toEqual([]);
});

// Screenshots for the UI review (`FM_SCREENS_DIR=<dir> pnpm test:e2e --grep @screens`).
test('the updates step at both review sizes @screens', async ({ browser }) => {
  const dir = process.env.FM_SCREENS_DIR;
  test.skip(!dir, 'FM_SCREENS_DIR is not set');
  for (const [width, height] of [
    [1280, 800],
    [1000, 700],
  ] as const) {
    // A fresh context each time: the mock keeps the answered steps for the page's session.
    const context = await browser.newContext({ viewport: { width, height } });
    const page = await context.newPage();
    await toUpdates(page);
    await page.screenshot({ path: `${dir}/setup-updates-${width}x${height}.png` });
    await context.close();
  }
});
