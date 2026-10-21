import { expect, test, type Page } from '@playwright/test';

// Browser mode runs on the mock client; no family has a declared license yet.
// The mock file picker returns `/Users/demo/Downloads/receipt-<n>.pdf`.
const info = (page: Page) => page.getByRole('tabpanel', { name: 'Info' });

async function openOn(page: Page, face: number) {
  await page.route('**/fmfont/*', (route) => route.fulfill({ status: 404 }));
  await page.goto(`/#/?face=${face}`);
  await expect(info(page).getByRole('heading', { name: 'Inter Regular' })).toBeVisible();
}

test('the License section declares, keeps and clears a family license from the keyboard', async ({
  page,
}) => {
  await openOn(page, 1);
  const form = info(page).getByRole('form', { name: 'Your license for this family' });
  const type = form.getByLabel('License type');
  const seats = form.getByLabel('Seats');
  const date = form.getByLabel('Purchase date');
  await expect(type).toHaveValue('');
  await expect(form.getByTestId('no-receipt')).toHaveText('None');
  await expect(form.getByRole('button', { name: 'Clear license' })).toBeDisabled();

  // Every field is labelled and reached with Tab, in order.
  await type.focus();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Tab');
  await expect(seats).toBeFocused();
  await page.keyboard.type('5');
  await page.keyboard.press('Tab');
  await expect(date).toBeFocused();
  await date.fill('2026-03-01');
  // The date field tabs through its own day, month and year parts; the
  // place radios come after it.
  await form.getByRole('radio', { name: /Next to the font/ }).focus();
  await page.keyboard.press('Shift+Tab');
  await expect(date).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(form.getByRole('radio', { name: /Next to the font/ })).toBeFocused();
  // Arrow keys move within the radio group.
  await page.keyboard.press('ArrowDown');
  await expect(form.getByRole('radio', { name: /data folder/ })).toBeChecked();
  await page.keyboard.press('Tab');
  await expect(form.getByRole('button', { name: 'Attach receipt…' })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(form.getByRole('button', { name: 'Save license' })).toBeFocused();
  await type.selectOption('web');
  await form.getByRole('button', { name: 'Save license' }).focus();
  await page.keyboard.press('Enter');
  await expect(form.getByRole('button', { name: 'Clear license' })).toBeEnabled();

  // Another face and back: the declaration is stored.
  await page.evaluate(() => (location.hash = '#/?face=6'));
  await expect(info(page).getByLabel('License type')).toHaveValue('');
  await page.evaluate(() => (location.hash = '#/?face=1'));
  await expect(type).toHaveValue('web');
  await expect(seats).toHaveValue('5');
  await expect(date).toHaveValue('2026-03-01');

  // A receipt is attached in the place picked, with the rest kept.
  await form.getByRole('radio', { name: /data folder/ }).check();
  await form.getByRole('button', { name: 'Attach receipt…' }).press('Enter');
  await expect(form.getByRole('button', { name: /Copy receipt/ })).toContainText(
    /\/app-data\/receipts\/receipt-\d+\.pdf/,
  );
  await expect(type).toHaveValue('web');
  await form.getByRole('button', { name: 'Remove receipt' }).press('Enter');
  await expect(form.getByTestId('no-receipt')).toHaveText('None');

  // Seats below 0 are refused with the API's message; the stored value stays.
  await seats.fill('-1');
  await form.getByRole('button', { name: 'Save license' }).click();
  await expect(form.getByRole('alert')).toHaveText('The number of seats cannot be below 0.');
  await expect(seats).toHaveValue('5');

  // Clearing empties every field.
  await form.getByRole('button', { name: 'Clear license' }).press('Enter');
  await expect(type).toHaveValue('');
  await expect(seats).toHaveValue('');
  await expect(date).toHaveValue('');
  await expect(form.getByRole('alert')).toHaveCount(0);
  await expect(form.getByRole('button', { name: 'Clear license' })).toBeDisabled();
});

/** WCAG contrast ratio of a License field's border against the page background. */
const borderContrast = (page: Page, label: string) =>
  info(page)
    .getByLabel(label, { exact: true })
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
  test(`the License fields are outlined at 3:1 or more (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await openOn(page, 1);
    for (const label of ['License type', 'Seats', 'Purchase date'])
      expect(await borderContrast(page, label), label).toBeGreaterThanOrEqual(3);
  });
}
