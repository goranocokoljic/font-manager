import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { sectionPanel, showSection } from './family-page';

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

// `?mockFaces=20` adds twenty sample families; the odd ones copy Inter (face 1) and pair with
// it, so face 1 has eight candidates. Without them the library has none.
async function openPairings(page: Page, search = '?mockFaces=20'): Promise<Locator> {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(`/${search}#/family/1?face=1`);
  return showSection(page, 'Pairings');
}
const cards = (panel: Locator) => panel.getByTestId('pairing-card');
const roleSwitch = (panel: Locator) => panel.getByRole('group', { name: 'This family' });

test('Pairings shows eight mini layouts: the family at 30 px over the candidate at 14 px', async ({
  page,
}) => {
  const panel = await openPairings(page);
  await expect(cards(panel)).toHaveCount(8);
  const first = cards(panel).first();
  await expect(first).toHaveAttribute('data-headline', '1');
  await expect(first).toHaveAttribute('data-body', '1000');
  await expect(first.locator('.headline')).toHaveCSS('font-size', '30px');
  await expect(first.locator('.body')).toHaveCSS('font-size', '14px');
  await expect(first.locator('.headline')).toBeVisible();
  await expect(first.locator('.name')).toHaveText('Sample 1');
  await expect(first.locator('.reasons')).toHaveText(
    'lower contrast for body · same designer · x-height within 1 %',
  );
});

test('the switch asks for headline faces and swaps which face plays which part', async ({
  page,
}) => {
  const panel = await openPairings(page);
  await expect(panel.getByText('This family', { exact: true })).toBeVisible();
  const forBody = roleSwitch(panel).getByRole('button', { name: 'for body', exact: true });
  await expect(
    roleSwitch(panel).getByRole('button', { name: 'for headlines', exact: true }),
  ).toHaveAttribute('aria-pressed', 'true');
  await forBody.click();
  await expect(forBody).toHaveAttribute('aria-pressed', 'true');
  const first = cards(panel).first();
  await expect(first).toHaveAttribute('data-headline', '1000');
  await expect(first).toHaveAttribute('data-body', '1');
  // The answer is the headline request's: no body rule.
  await expect(first.locator('.reasons')).toHaveText('same designer · x-height within 1 %');
});

test('hovering lifts a card on the fast time', async ({ page }) => {
  const panel = await openPairings(page);
  const card = cards(panel).first();
  await expect(card).toHaveCSS('box-shadow', 'none');
  const transition = await card.evaluate((el) => getComputedStyle(el).transitionProperty);
  expect(transition).toContain('box-shadow');
  await card.hover();
  await expect(card).not.toHaveCSS('box-shadow', 'none');
});

test('a card opens the Studio with the pairing loaded: body as the panel face, headline in its block', async ({
  page,
}) => {
  const panel = await openPairings(page);
  await cards(panel).first().click();
  await expect(page).toHaveURL(/#\/studio\?face=1000&headline=1$/);
  const studio = page.getByRole('region', { name: /^Pairing Studio: / });
  // The Studio opens on the Magazine context: its headline is block 2, under the kicker.
  await expect(page.getByTestId('studio-pairing')).toHaveText('Inter + Sample 1');
  await expect(studio.getByRole('combobox', { name: 'Font for Block 2' })).toHaveValue(/^Inter /);
  await expect(studio.getByRole('combobox', { name: 'Font for Block 1' })).toHaveValue('');
  await expect(studio.getByRole('combobox', { name: 'Font for Block 3' })).toHaveValue('');
});

test('with no candidate in the library: one line and a way to browse catalogs', async ({
  page,
}) => {
  const panel = await openPairings(page, '');
  await expect(cards(panel)).toHaveCount(0);
  const empty = panel.getByTestId('pairings-empty');
  await expect(empty).toContainText('No font in your library pairs with this family yet.');
  await empty.getByRole('button', { name: 'Browse catalogs' }).click();
  await expect(sectionPanel(page, 'Pairings')).toHaveCount(0);
  await expect(page.getByRole('heading', { name: 'Google Fonts', level: 1 })).toBeVisible();
});
