import { expect, test } from '@playwright/test';

// Browser mode runs on the mock client: it starts with the set "Client brand" (2
// fonts), and its open dialog picks a teammate's set file that names one font of the
// sample library and two it does not have.
test('"Export set…" and "Import set…" work from the keyboard and list what is missing', async ({
  page,
}) => {
  await page.goto('/');
  const sets = page.getByRole('region', { name: 'Project sets' });
  const set = sets.getByRole('button', { name: /^Client brand/ });
  await expect(set).toBeVisible();

  // Tab from a set reaches its labelled "Export set…"; Enter saves the set file.
  await set.focus();
  await page.keyboard.press('Tab');
  const exportSet = sets.getByRole('button', { name: 'Export the set Client brand' });
  await expect(exportSet).toBeFocused();
  await expect(exportSet).toHaveText('Export set…');
  await expect(exportSet).toHaveCSS('opacity', '1');
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('toast')).toContainText('Saved the set: 2 fonts.');

  // Shift+Tab back past the set reaches "Import set…".
  await set.focus();
  await page.keyboard.press('Shift+Tab');
  const importSet = sets.getByRole('button', { name: 'Import set…' });
  await expect(importSet).toBeFocused();
  await page.keyboard.press('Enter');

  const note = sets.getByRole('status');
  await expect(note).toContainText('Imported the set “Team brand”.');
  await expect(note).toContainText('2 fonts not in this library');
  await expect(note.getByRole('list', { name: '2 fonts not in this library' })).toHaveText(
    /Quarto Bold\s*Gotham Rounded Medium/,
  );
  // The new set holds the one font this library has.
  await expect(sets.getByRole('button', { name: /^Team brand/ })).toContainText('1');

  await note.getByRole('button', { name: 'Close the import note' }).click();
  await expect(note).toBeHidden();
});
