import { expect, test, type Page } from '@playwright/test';

// The first-run setup (E55.4, board 13). Browser mode has no open step unless the page is
// opened with `?mockSetup=first`, a first start: no watched folder yet. The mock's picker
// returns `/Users/demo/Added 1`, `/Users/demo/Added 2`, … and its scans step slowly.

const setup = (page: Page) => page.getByRole('main', { name: 'First-run setup' });
const folders = (page: Page) => setup(page).getByRole('list', { name: 'Your font folders' });
const row = (page: Page, name: string) =>
  folders(page).getByRole('listitem', { name, exact: true });

async function firstStart(page: Page): Promise<void> {
  await page.goto('/?mockSetup=first#/');
  await expect(setup(page)).toBeVisible();
}

test('without the query parameter the library shows, not the setup', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('region', { name: 'Sources' })).toBeVisible();
  await expect(setup(page)).toHaveCount(0);
});

test('a first start: two folders added, one removed, then Finish', async ({ page }) => {
  await firstStart(page);
  // Only the open steps that have a screen; the folders step is first and current.
  const steps = setup(page).getByRole('navigation', { name: 'Setup steps' }).getByRole('listitem');
  await expect(steps).toHaveText(['Font folders', 'Online catalogs']);
  await expect(steps.first()).toHaveAttribute('aria-current', 'step');
  // The focus is on the step's heading.
  const heading = setup(page).getByRole('heading', { name: 'Where are your fonts?' });
  await expect(heading).toBeFocused();
  await expect(
    setup(page).getByText('Fonts installed in the system are already included.'),
  ).toBeVisible();
  await expect(setup(page).getByText('Your library is empty')).toBeVisible();
  // While no folder is added: Skip, no Continue.
  await expect(setup(page).getByRole('button', { name: 'Skip for now' })).toBeVisible();
  await expect(setup(page).getByRole('button', { name: 'Finish' })).toHaveCount(0);

  await setup(page).getByRole('button', { name: 'Add folder…' }).click();
  const first = row(page, 'Added 1');
  await expect(first).toBeVisible();
  await expect(first.getByText('/Users/demo/Added 1')).toBeVisible();
  // The scan shows on the row through the scan events, then the face count.
  await expect(first.getByRole('progressbar', { name: 'Reading Added 1' })).toBeVisible();
  await expect(first.getByRole('progressbar')).toHaveCount(0);
  await expect(first.getByText(/^\d+ fonts?$/)).toBeVisible();
  // The pane: the number found and the first families.
  const pane = page.getByTestId('setup-library');
  await expect(pane.getByText('Your library so far')).toBeVisible();
  await expect(pane.getByTestId('setup-found')).toHaveText(/^\d+ famil(y|ies) found$/);
  await expect(
    pane.getByRole('list', { name: 'Families found so far' }).getByRole('listitem').first(),
  ).toBeVisible();

  // The picker can be used again and again.
  await setup(page).getByRole('button', { name: 'Add another folder…' }).click();
  await expect(row(page, 'Added 2')).toBeVisible();
  await expect(setup(page).getByRole('button', { name: 'Skip for now' })).toHaveCount(0);

  // Remove takes it out of the library; the focus stays in the step.
  await row(page, 'Added 2')
    .getByRole('button', { name: 'Remove Added 2 from the library' })
    .click();
  await expect(row(page, 'Added 2')).toHaveCount(0);
  await expect(row(page, 'Added 1')).toBeVisible();
  await expect(
    row(page, 'Added 1').getByRole('button', { name: 'Remove Added 1 from the library' }),
  ).toBeFocused();

  // Continue goes to the catalogs step (setup-catalogs.spec.ts); Finish there to the library,
  // with the folder in it.
  await setup(page).getByRole('button', { name: 'Continue' }).click();
  await expect(steps.nth(1)).toHaveAttribute('aria-current', 'step');
  await setup(page).getByRole('button', { name: 'Finish' }).click();
  await expect(setup(page)).toHaveCount(0);
  const sources = page.getByRole('region', { name: 'Sources' });
  await expect(sources.getByText('Added 1')).toBeVisible();
  await expect(sources.getByText('Added 2')).toHaveCount(0);
});

test('Skip for now goes on to the library, which still offers Add folder', async ({ page }) => {
  await firstStart(page);
  await setup(page).getByRole('button', { name: 'Skip for now' }).press('Enter');
  await expect(
    setup(page).getByRole('heading', { name: 'Use the online catalogs?' }),
  ).toBeFocused();
  await setup(page).getByRole('button', { name: 'Finish' }).click();
  await expect(setup(page)).toHaveCount(0);
  // The step is recorded as skipped (stores/setup.test.ts); the library's Sources keep Add folder.
  await expect(
    page.getByRole('region', { name: 'Sources' }).getByRole('button', { name: 'Add folder' }),
  ).toBeVisible();
});

test('reopened from Settings: every step with the current folders, a close button', async ({
  page,
}) => {
  await page.goto('/#/settings?section=advanced');
  await page.getByRole('button', { name: 'Run setup again' }).click();
  await expect(setup(page)).toBeVisible();
  // The step names are links; the folder already in the library is a row.
  await expect(setup(page).getByRole('button', { name: 'Go to Font folders' })).toBeVisible();
  await expect(row(page, 'Fonts')).toBeVisible();
  await expect(row(page, 'Fonts').getByText('/Users/demo/Fonts')).toBeVisible();
  await expect(setup(page).getByRole('heading', { name: 'Where are your fonts?' })).toBeFocused();
  // Esc (or the close button) leaves without changing anything.
  await page.keyboard.press('Escape');
  await expect(setup(page)).toHaveCount(0);
  await page.getByRole('button', { name: 'Run setup again' }).click();
  await setup(page).getByRole('button', { name: 'Close the setup' }).click();
  await expect(setup(page)).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Run setup again' })).toBeVisible();
});

test('a folder added in the setup reopened from Settings shows its scan and count', async ({
  page,
}) => {
  // Settings subscribes the sources store to the scan events and stops it as it unmounts:
  // the setup that takes its place must still see the scan of a folder added in it.
  await page.goto('/#/settings?section=advanced');
  await page.getByRole('button', { name: 'Run setup again' }).click();
  await setup(page).getByRole('button', { name: 'Add another folder…' }).click();
  const added = row(page, 'Added 1');
  await expect(added.getByRole('progressbar', { name: 'Reading Added 1' })).toBeVisible();
  await expect(added.getByRole('progressbar')).toHaveCount(0);
  await expect(added.getByText(/^[1-9]\d* fonts?$/)).toBeVisible();
  await expect(added.getByText('Read just now')).toBeVisible();
  // Closed again, Settings › Sources still follows the events.
  await setup(page).getByRole('button', { name: 'Close the setup' }).click();
  await page.getByRole('tab', { name: 'Sources' }).click();
  const card = page.getByRole('listitem', { name: 'Added 1', exact: true });
  await card.getByRole('button', { name: 'Rescan Added 1' }).click();
  await expect(card.getByRole('progressbar', { name: 'Scanning Added 1' })).toBeVisible();
});

test('the setup is keyboard-complete', async ({ page }) => {
  await firstStart(page);
  // Tab from the heading reaches Add folder…; Enter adds; Tab reaches Continue.
  await page.keyboard.press('Tab');
  await expect(setup(page).getByRole('button', { name: 'Add folder…' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(row(page, 'Added 1')).toBeVisible();
  const tabTo = async (name: string) => {
    const target = setup(page).getByRole('button', { name, exact: true });
    for (let i = 0; i < 8 && !(await target.evaluate((el) => el === document.activeElement)); i++)
      await page.keyboard.press('Tab');
    await expect(target).toBeFocused();
  };
  await tabTo('Continue');
  await page.keyboard.press('Enter');
  // The catalogs step: Tab reaches a switch, Space turns it on; Tab reaches Finish.
  await expect(
    setup(page).getByRole('heading', { name: 'Use the online catalogs?' }),
  ).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(setup(page).getByRole('switch', { name: 'Google Fonts' })).toBeFocused();
  await page.keyboard.press('Space');
  await expect(setup(page).getByRole('switch', { name: 'Google Fonts' })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await tabTo('Finish');
  await page.keyboard.press('Enter');
  await expect(setup(page)).toHaveCount(0);
});
