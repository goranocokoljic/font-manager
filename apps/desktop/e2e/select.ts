// Drives the app's Select (`src/components/ui/UiSelect.vue`) the way a user does: click
// the combobox button, click an option in the menu it opens. The same shape as
// Playwright's `selectOption` for the native control it replaced: a string matches the
// option's value or its label; `{ label }` and `{ index }` work as before.
import { expect, type Locator } from '@playwright/test';

export type Choice = string | { label?: string; value?: string; index?: number };

const MENU = '.ui-popover [role="listbox"]';

/** Chooses `choice` in the Select at `combobox` and waits for its menu to close. */
export async function selectOption(combobox: Locator, choice: Choice): Promise<void> {
  const page = combobox.page();
  await combobox.click();
  const menu = page.locator(MENU);
  await expect(menu).toBeVisible();
  let option: Locator;
  if (typeof choice === 'string') {
    const byValue = menu.locator(`[role="option"][data-value="${choice}"]`);
    option = (await byValue.count())
      ? byValue
      : menu.getByRole('option', { name: choice, exact: true });
  } else if (choice.value !== undefined) {
    option = menu.locator(`[role="option"][data-value="${choice.value}"]`);
  } else if (choice.label !== undefined) {
    option = menu.getByRole('option', { name: choice.label, exact: true });
  } else {
    option = menu.getByRole('option').nth(choice.index ?? 0);
  }
  await option.click();
  await expect(menu).toBeHidden();
}

/** The labels of the Select's options; the menu is closed again. */
export async function optionLabels(combobox: Locator): Promise<string[]> {
  const page = combobox.page();
  await combobox.click();
  const menu = page.locator(MENU);
  await expect(menu).toBeVisible();
  const labels = await menu.getByRole('option').allTextContents();
  await combobox.press('Escape');
  await expect(menu).toBeHidden();
  return labels.map((l) => l.trim());
}

/** Opens the preview bar's Options row (line height, tracking, colours…) if it is closed. */
export async function showPreviewOptions(page: import('@playwright/test').Page): Promise<void> {
  const toggle = page
    .getByRole('group', { name: 'Preview settings' })
    .getByRole('button', { name: 'Options' });
  if ((await toggle.getAttribute('aria-expanded')) !== 'true') await toggle.click();
}
