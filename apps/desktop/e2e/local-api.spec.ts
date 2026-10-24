import { expect, test } from '@playwright/test';

// Settings → Local API (E25.1): the address and the masked token, with Show,
// Copy and Regenerate, all reachable and named for the keyboard.

test.beforeEach(async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.goto('/#/settings?section=localApi');
});

test('the Local API section shows the address and a masked token', async ({ page }) => {
  await expect(page.getByRole('heading', { name: 'Local API', level: 2 })).toBeVisible();
  await expect(page.getByRole('tab', { name: 'Local API' })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await expect(
    page.getByRole('button', { name: 'Copy local API address: http://127.0.0.1:47613' }),
  ).toBeVisible();
  const token = page.getByRole('textbox', { name: 'Token' });
  await expect(token).toHaveValue(/^•{64}$/);
  await expect(token).toHaveAttribute('readonly', '');
});

test('Show, Copy and Regenerate work from the keyboard', async ({ page }) => {
  const token = page.getByRole('textbox', { name: 'Token' });
  await token.focus();

  // Tab order: token → Show → Copy token → Regenerate.
  await page.keyboard.press('Tab');
  const show = page.getByRole('button', { name: 'Show' });
  await expect(show).toBeFocused();
  await expect(show).toHaveAttribute('aria-pressed', 'false');
  await page.keyboard.press('Enter');
  await expect(token).toHaveValue(/^[0-9a-f]{64}$/);
  const first = await token.inputValue();
  await expect(page.getByRole('button', { name: 'Hide' })).toHaveAttribute('aria-pressed', 'true');

  await page.keyboard.press('Tab');
  await expect(page.getByRole('button', { name: 'Copy token' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('status').filter({ hasText: 'Copied' })).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(first);

  await page.keyboard.press('Tab');
  const regenerate = page.getByRole('button', { name: 'Regenerate' });
  await expect(regenerate).toBeFocused();
  await expect(regenerate).toHaveAccessibleDescription(/stop working until they get the new one/);
  await page.keyboard.press('Enter');
  await expect(
    page.getByRole('status').filter({ hasText: 'The old token no longer works.' }),
  ).toBeVisible();
  await expect(token).toHaveValue(/^[0-9a-f]{64}$/);
  expect(await token.inputValue()).not.toBe(first);

  // Hide masks it again.
  await page.getByRole('button', { name: 'Hide' }).press('Enter');
  await expect(token).toHaveValue(/^•{64}$/);
});
