import { expect, test, type Page } from '@playwright/test';

// The smart-collection rule editor (E14.1), in browser mode on the mock client:
// Inter is sans serif with Latin and Cyrillic, Legacy Grotesk sans serif with Latin
// only, JetBrains Mono monospace with Cyrillic. No face covers Greek.
const sidebar = (page: Page) => page.getByRole('region', { name: 'Collections' });
const row = (page: Page, name: string) =>
  sidebar(page).getByRole('listitem', { name, exact: true });
const editor = (page: Page) => page.getByRole('dialog', { name: /smart collection|Edit rules/ });
const list = (page: Page) => page.getByRole('tree', { name: 'Font list' });

async function openNew(page: Page) {
  await page.goto('/');
  await sidebar(page).getByRole('button', { name: 'New smart collection' }).click();
  await expect(editor(page)).toBeVisible();
}

test('"Sans serif AND (Cyrillic OR Greek)" is built, saved, and the sidebar count matches the list', async ({
  page,
}) => {
  await openNew(page);
  const dialog = editor(page);
  await dialog.getByRole('textbox', { name: 'Name' }).fill('Sans Cyrillic or Greek');
  await expect(
    dialog.getByRole('combobox', { name: 'Fonts match all or any of the rules' }),
  ).toHaveValue('and');
  await dialog
    .getByRole('combobox', { name: 'Rule 1: what it tests', exact: true })
    .selectOption({ label: 'Class' });
  await dialog
    .getByRole('combobox', { name: 'Rule 1: value', exact: true })
    .selectOption({ label: 'Sans serif' });
  const status = dialog.getByRole('status');
  await expect(status).toHaveText('2 families match');

  await dialog.getByRole('button', { name: 'Add group' }).click();
  await expect(
    dialog.getByRole('combobox', { name: 'Group 1: fonts match all or any of its rules' }),
  ).toHaveValue('or');
  await dialog
    .getByRole('combobox', { name: 'Group 1, rule 1: what it tests' })
    .selectOption({ label: 'Supports language' });
  await dialog
    .getByRole('combobox', { name: 'Group 1, rule 1: value' })
    .selectOption({ label: 'Serbian (Cyrillic)' });
  await dialog.getByRole('button', { name: 'Add a rule to group 1' }).click();
  await dialog
    .getByRole('combobox', { name: 'Group 1, rule 2: what it tests' })
    .selectOption({ label: 'Supports language' });
  await dialog
    .getByRole('combobox', { name: 'Group 1, rule 2: value' })
    .selectOption({ label: 'Greek' });
  // The live count is the API's answer for the draft rule.
  await expect(status).toHaveText('1 family matches');

  await dialog.getByRole('button', { name: 'Save' }).click();
  await expect(dialog).toBeHidden();
  const saved = row(page, 'Sans Cyrillic or Greek');
  await expect(saved).toBeVisible();
  await expect(saved.getByText('Smart')).toBeVisible();
  // The new collection is open: the list shows its families, as many as the sidebar counts.
  await expect(page.getByRole('heading', { name: 'Sans Cyrillic or Greek' })).toBeVisible();
  await expect(list(page).getByRole('treeitem', { name: /Inter/ })).toBeVisible();
  const count = Number(await saved.locator('.count').textContent());
  await expect(list(page).getByRole('treeitem')).toHaveCount(count);
  expect(count).toBe(1);

  // Edit rules: the saved rule comes back, a change is saved through the new method.
  await saved.getByRole('button', { name: 'Edit rules of Sans Cyrillic or Greek' }).click();
  await expect(dialog.getByRole('heading')).toHaveText('Edit rules of “Sans Cyrillic or Greek”');
  await expect(dialog.getByRole('combobox', { name: 'Rule 1: value', exact: true })).toHaveValue(
    'sans',
  );
  await expect(dialog.getByRole('combobox', { name: 'Group 1, rule 2: value' })).toHaveValue('el');
  await dialog.getByRole('button', { name: 'Remove group 1', exact: true }).click();
  await expect(status).toHaveText('2 families match');
  await dialog.getByRole('button', { name: 'Save' }).click();
  await expect(dialog).toBeHidden();
  await expect(saved.getByLabel('2 fonts', { exact: true })).toBeVisible();
  await expect(list(page).getByRole('treeitem')).toHaveCount(2);
});

test('a rule fm-core refuses shows its message inline and nothing is saved', async ({ page }) => {
  await openNew(page);
  const dialog = editor(page);
  await dialog.getByRole('textbox', { name: 'Name' }).fill('Upside down');
  await dialog
    .getByRole('combobox', { name: 'Rule 1: what it tests', exact: true })
    .selectOption({ label: 'Weight' });
  await dialog.getByRole('combobox', { name: 'Rule 1: from', exact: true }).selectOption('700');
  await dialog.getByRole('combobox', { name: 'Rule 1: to', exact: true }).selectOption('300');
  await dialog.getByRole('button', { name: 'Save' }).click();
  await expect(dialog.getByRole('alert')).toHaveText(
    'The lowest weight of a rule is above its highest.',
  );
  await expect(dialog).toBeVisible();

  // Every rule removed: an empty group.
  await dialog.getByRole('button', { name: 'Remove rule 1', exact: true }).click();
  await dialog.getByRole('button', { name: 'Save' }).click();
  await expect(dialog.getByRole('alert')).toHaveText(
    "Every group of a smart collection's rules needs at least one rule.",
  );
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  await expect(sidebar(page).locator('li[data-collection]')).toHaveCount(0);
});

test('rule rows are added, filled and removed from the keyboard', async ({ page }) => {
  await page.goto('/');
  const open = sidebar(page).getByRole('button', { name: 'New smart collection' });
  await open.focus();
  await page.keyboard.press('Enter');
  const dialog = editor(page);
  // Focus starts in the dialog's first field.
  await expect(dialog.getByRole('textbox', { name: 'Name' })).toBeFocused();
  await page.keyboard.type('Keyboard');

  const add = dialog.getByRole('button', { name: 'Add rule', exact: true });
  await add.focus();
  await page.keyboard.press('Enter');
  // The new row's first control takes focus.
  const tests = dialog.getByRole('combobox', { name: 'Rule 2: what it tests', exact: true });
  await expect(tests).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(dialog.getByRole('combobox', { name: 'Rule 2: value', exact: true })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(dialog.getByRole('button', { name: 'Remove rule 2', exact: true })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(tests).toHaveCount(0);
  await expect(add).toBeFocused();

  // Add group, then remove it, without the mouse.
  await page.keyboard.press('Tab');
  await expect(dialog.getByRole('button', { name: 'Add group' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(
    dialog.getByRole('combobox', { name: 'Group 1, rule 1: what it tests' }),
  ).toBeFocused();
  await dialog.getByRole('button', { name: 'Remove group 1', exact: true }).focus();
  await page.keyboard.press('Enter');
  await expect(dialog.getByRole('group')).toHaveCount(0);

  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  await expect(open).toBeFocused();
});
