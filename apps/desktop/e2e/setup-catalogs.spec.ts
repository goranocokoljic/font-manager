import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// Setup step two (E55.5, board 13): the online catalogs and where their fonts go. Browser mode
// opens the setup with `?mockSetup=first`; the mock's picker returns the folders queued in
// `__fmMockPickFolders`, else `/Users/demo/Added 1`, `/Users/demo/Added 2`, …

const setup = (page: Page) => page.getByRole('main', { name: 'First-run setup' });
const sidebar = (page: Page) => page.getByRole('navigation', { name: 'Library' });
const sw = (page: Page, name: string) => setup(page).getByRole('switch', { name, exact: true });
const placeGroup = (page: Page) =>
  setup(page).getByRole('group', { name: 'Where should downloaded fonts go?' });
const landing = (page: Page) => setup(page).getByTestId('setup-landing');

const font = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function serveFonts(page: Page): Promise<void> {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: font }),
  );
}

/** The next folders the native picker returns. */
async function queuePicks(page: Page, ...paths: string[]): Promise<void> {
  await page.evaluate((p) => {
    const w = window as unknown as { __fmMockPickFolders?: string[] };
    (w.__fmMockPickFolders ??= []).push(...p);
  }, paths);
}

/** A first start: `folders` added in step one, then Continue onto the catalogs step. */
async function toCatalogs(page: Page, ...folders: string[]): Promise<void> {
  await serveFonts(page);
  await page.goto('/?mockSetup=first#/');
  await expect(setup(page)).toBeVisible();
  await queuePicks(page, ...folders);
  for (const [i, path] of folders.entries()) {
    const name = path.split('/').at(-1)!;
    await setup(page)
      .getByRole('button', { name: i === 0 ? 'Add folder…' : 'Add another folder…' })
      .click();
    await expect(setup(page).getByRole('listitem', { name, exact: true })).toBeVisible();
  }
  await setup(page).getByRole('button', { name: 'Continue' }).click();
  await expect(
    setup(page).getByRole('heading', { name: 'Use the online catalogs?' }),
  ).toBeFocused();
}

/** Continue on the catalogs step, then Finish on the updates step (E55.6): the setup closes. */
async function finish(page: Page): Promise<void> {
  await setup(page).getByRole('button', { name: 'Continue' }).click();
  await expect(
    setup(page).getByRole('heading', { name: 'Keep Font Fu up to date?' }),
  ).toBeFocused();
  await setup(page).getByRole('button', { name: 'Finish' }).click();
  await expect(setup(page)).toHaveCount(0);
}

/** Opens `catalog` in the sidebar and presses the activation dot (Install) of `family`. */
async function install(page: Page, catalog: string, family: string): Promise<void> {
  await sidebar(page)
    .getByRole('region', { name: 'Catalogs' })
    .getByRole('button', { name: catalog, exact: true })
    .click();
  const row = page
    .getByRole('list', { name: `${catalog} families` })
    .getByRole('listitem')
    .filter({ hasText: family });
  await row
    .getByRole('button', {
      name: `${family}: not in the library. Click to install and activate it.`,
    })
    .click();
}

test('both catalogs off: Continue asks nothing more and the first Install asks for the folder', async ({
  page,
}) => {
  await toCatalogs(page, '/Users/demo/Type');
  const steps = setup(page).getByRole('navigation', { name: 'Setup steps' }).getByRole('listitem');
  await expect(steps).toHaveText(['Font folders', 'Online catalogs', 'Updates']);
  await expect(steps.nth(1)).toHaveAttribute('aria-current', 'step');
  // Two switches, both off, each saying that turning it on fetches the list now.
  for (const name of ['Google Fonts', 'Fontshare']) {
    await expect(sw(page, name)).toHaveAttribute('aria-checked', 'false');
    await expect(sw(page, name)).toHaveAccessibleDescription(
      `Turning ${name} on connects to the internet to fetch its list of fonts. Previews are fetched while you browse it.`,
    );
  }
  await expect(setup(page).getByRole('switch')).toHaveCount(2);
  await expect(setup(page).getByText('Adobe Fonts')).toHaveCount(0);
  // Nothing on: no folder question, no landing lines.
  await expect(placeGroup(page)).toHaveCount(0);
  await expect(landing(page)).toHaveCount(0);
  await expect(page.getByTestId('setup-catalogs-pane')).toContainText('Nothing is downloaded');

  await finish(page);
  // Turned on later, the first Install proposes the folder, as before.
  const catalogs = sidebar(page).getByRole('region', { name: 'Catalogs' });
  await catalogs.getByRole('button', { name: 'Turn on Google Fonts' }).click();
  await install(page, 'Google Fonts', 'Inter');
  const line = page.getByTestId('catalog-install');
  await expect(
    line.getByRole('heading', { name: 'Where should installed fonts go?' }),
  ).toBeVisible();
  await expect(line).toContainText('/Users/demo/Documents/Font Fu Fonts');
});

test("one catalog on, 'Inside my font folder': the landing line, then Install asks nothing", async ({
  page,
}) => {
  await toCatalogs(page, '/Users/demo/Type', '/Users/demo/Client type');
  await sw(page, 'Google Fonts').click();
  await expect(sw(page, 'Google Fonts')).toHaveAttribute('aria-checked', 'true');
  // Where downloaded fonts go: the font folder first, a select with several.
  const inside = placeGroup(page).getByRole('radio', { name: 'Inside my font folder' });
  await expect(inside).toBeChecked();
  const select = setup(page).getByTestId('setup-font-folder-select');
  await expect(select).toContainText('/Users/demo/Type');
  await select.click();
  await page.getByRole('option', { name: '/Users/demo/Client type' }).click();
  // Only the catalog that is on gets a line.
  await expect(landing(page).getByRole('listitem')).toHaveText([
    '/Users/demo/Client type/Google Fonts/<Family>',
  ]);
  await expect(page.getByTestId('setup-catalogs-pane')).toContainText('/Users/demo/Client type');
  await expect(setup(page).getByTestId('setup-cloud-line')).toHaveCount(0);

  await finish(page);
  await install(page, 'Google Fonts', 'Inter');
  const line = page.getByTestId('catalog-install');
  await expect(page.getByTestId('catalog-install-status')).toHaveText(
    /^Installed Inter: \d+ styles? active$/,
  );
  await expect(line.getByRole('heading')).toHaveCount(0);

  // Settings → Online catalogs: the download folder and each catalog's path.
  await page.goto('/#/settings?section=catalogs');
  await expect(page.getByTestId('settings-download-folder-path')).toHaveText(
    '/Users/demo/Client type',
  );
  await expect(page.getByTestId('settings-catalog-paths').getByRole('listitem')).toHaveText([
    'Google Fonts: /Users/demo/Client type/Google Fonts/<Family>',
    'Fontshare: /Users/demo/Client type/Fontshare/<Family>',
  ]);
});

test("one catalog on, 'Another folder': the proposal, the picker, then Install asks nothing", async ({
  page,
}) => {
  await toCatalogs(page, '/Users/demo/Type');
  await sw(page, 'Fontshare').click();
  const other = placeGroup(page).getByRole('radio', { name: 'Another folder' });
  await other.check();
  // `<Documents>/Font Fu Fonts` is proposed, as CatalogInstall does.
  await expect(setup(page).getByTestId('setup-other-folder')).toHaveText(
    '/Users/demo/Documents/Font Fu Fonts',
  );
  await expect(landing(page).getByRole('listitem')).toHaveText([
    '/Users/demo/Documents/Font Fu Fonts/Fontshare/<Family>',
  ]);
  await queuePicks(page, '/Users/demo/Downloaded type');
  await setup(page).getByRole('button', { name: 'Choose…' }).click();
  await expect(setup(page).getByTestId('setup-other-folder')).toHaveText(
    '/Users/demo/Downloaded type',
  );
  await expect(landing(page).getByRole('listitem')).toHaveText([
    '/Users/demo/Downloaded type/Fontshare/<Family>',
  ]);

  await finish(page);
  await install(page, 'Fontshare', 'Lato');
  await expect(page.getByTestId('catalog-install-status')).toHaveText(
    /^Installed Lato: \d+ styles? active$/,
  );
  await expect(page.getByTestId('catalog-install').getByRole('heading')).toHaveCount(0);
});

test("without a font folder only 'Another folder' is offered", async ({ page }) => {
  await serveFonts(page);
  await page.goto('/?mockSetup=first#/');
  await setup(page).getByRole('button', { name: 'Skip for now' }).click();
  await expect(
    setup(page).getByRole('heading', { name: 'Use the online catalogs?' }),
  ).toBeFocused();
  await sw(page, 'Google Fonts').click();
  await expect(placeGroup(page).getByRole('radio')).toHaveCount(1);
  await expect(placeGroup(page).getByRole('radio', { name: 'Another folder' })).toBeChecked();
});

test('a system font folder shows the message of the API; a cloud font folder names the license', async ({
  page,
}) => {
  await toCatalogs(page, '/Users/demo/Dropbox/Studio type');
  await sw(page, 'Google Fonts').click();
  await sw(page, 'Fontshare').click();
  // The font folder syncs: one line, and a link to each catalog's license.
  const cloud = setup(page).getByTestId('setup-cloud-line');
  await expect(cloud).toContainText(
    'This folder syncs: its files go to everyone who shares it. Each catalog’s license decides whether its fonts may be shared.',
  );
  await expect(cloud.getByRole('button', { name: 'Read the Google Fonts license' })).toBeVisible();
  await expect(cloud.getByRole('button', { name: 'Read the Fontshare license' })).toBeVisible();
  await expect(landing(page).getByRole('listitem')).toHaveText([
    '/Users/demo/Dropbox/Studio type/Google Fonts/<Family>',
    '/Users/demo/Dropbox/Studio type/Fontshare/<Family>',
  ]);

  // Another folder, inside the system fonts: refused with the API's words; the step stays.
  await placeGroup(page).getByRole('radio', { name: 'Another folder' }).check();
  await expect(cloud).toHaveCount(0);
  await queuePicks(page, '/System/Library/Fonts/Mine');
  await setup(page).getByRole('button', { name: 'Choose…' }).click();
  await setup(page).getByRole('button', { name: 'Continue' }).click();
  await expect(setup(page).getByRole('alert')).toHaveText(
    'Fonts cannot be installed into a system font folder. Choose another folder.',
  );
  await expect(
    setup(page).getByRole('heading', { name: 'Use the online catalogs?' }),
  ).toBeVisible();
});
