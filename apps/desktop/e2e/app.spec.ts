import { expect, test } from '@playwright/test';

test('opens the shell and shows the app info from the API client', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Font Manager' })).toBeVisible();
  const info = page.getByTestId('app-info');
  await expect(info).toContainText('Font Manager (mock)');
  await expect(info).toContainText(/\d+\.\d+\.\d+/);
});

test('an unknown route lands on home', async ({ page }) => {
  await page.goto('/#/no/such/page');
  await expect(page).toHaveURL(/#\/$/);
  await expect(page.getByTestId('app-info')).toBeVisible();
});

for (const scheme of ['light', 'dark'] as const) {
  test(`theme follows the OS: ${scheme}`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto('/');
    await expect(page.getByTestId('app-info')).toBeVisible();
    const colors = await page.evaluate(() => {
      const style = getComputedStyle(document.body);
      return { bg: style.backgroundColor, fg: style.color };
    });
    const expected =
      scheme === 'dark'
        ? { bg: 'rgb(28, 28, 30)', fg: 'rgb(236, 236, 236)' }
        : { bg: 'rgb(255, 255, 255)', fg: 'rgb(26, 26, 26)' };
    expect(colors).toEqual(expected);
  });
}

test('theme changes live when the OS appearance changes', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto('/');
  await expect(page.getByTestId('app-info')).toBeVisible();
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect
    .poll(() => page.evaluate(() => getComputedStyle(document.body).backgroundColor))
    .toBe('rgb(28, 28, 30)');
});
