import { expect, test, type Page } from '@playwright/test';

// Browser mode runs on the mock client; no face or family has notes yet.
const info = (page: Page) => page.getByRole('tabpanel', { name: 'Info' });

async function openTableOn(page: Page, face: number) {
  await page.route('**/fmfont/*', (route) => route.fulfill({ status: 404 }));
  await page.goto(`/#/?face=${face}`);
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Table' })
    .click();
  const grid = page.getByRole('treegrid', { name: 'Font table' });
  await expect(grid.locator('.spacer [role="row"]').first()).toBeVisible();
  return grid;
}

test('the Info tab saves face and family notes on blur, and the Notes column shows the family note', async ({
  page,
}) => {
  const grid = await openTableOn(page, 1);
  const panel = info(page);
  await expect(panel.getByRole('heading', { name: 'Inter Regular' })).toBeVisible();
  const notesCell = grid.getByRole('row', { name: /^Inter,/ }).locator('[data-column="notes"]');
  await expect(notesCell).toHaveText('');

  // Labelled multi-line fields, one for the face and one for its family.
  const faceNotes = panel.getByRole('textbox', { name: 'Notes', exact: true });
  const familyNotes = panel.getByRole('textbox', { name: 'Family notes' });
  await expect(faceNotes).toHaveJSProperty('tagName', 'TEXTAREA');
  await expect(familyNotes).toHaveJSProperty('tagName', 'TEXTAREA');

  await familyNotes.fill('Licensed for the 2026 rebrand\nDesktop only');
  // Nothing is saved while typing.
  await expect(notesCell).toHaveText('');
  // Leaving the field saves; the list hears of it through `libraryChanged`.
  await faceNotes.focus();
  await expect(notesCell).toHaveText(/Licensed for the 2026 rebrand/);
  await faceNotes.fill('Headlines only');
  await page.keyboard.press('Tab');
  // The face note is the face's: the family's Notes cell does not show it.
  await expect(notesCell).not.toContainText('Headlines only');

  // Another face and back: both are stored.
  await page.evaluate(() => (location.hash = '#/?face=6'));
  await expect(panel.getByRole('textbox', { name: 'Family notes' })).toHaveValue('');
  await page.evaluate(() => (location.hash = '#/?face=1'));
  await expect(panel.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    'Headlines only',
  );
  await expect(panel.getByRole('textbox', { name: 'Family notes' })).toHaveValue(
    'Licensed for the 2026 rebrand\nDesktop only',
  );

  // An empty field clears the family note.
  await panel.getByRole('textbox', { name: 'Family notes' }).fill('');
  await page.keyboard.press('Tab');
  await expect(notesCell).toHaveText('');
});

/** WCAG contrast ratio of the field's border against the page background. */
const borderContrast = (page: Page, name: string) =>
  info(page)
    .getByRole('textbox', { name, exact: true })
    .evaluate((el) => {
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
  test(`the notes fields are outlined at 3:1 or more (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto('/#/?face=1');
    await expect(info(page).getByRole('heading', { name: 'Inter Regular' })).toBeVisible();
    for (const name of ['Notes', 'Family notes'])
      expect(await borderContrast(page, name)).toBeGreaterThanOrEqual(3);
  });
}
