// E57.4: Settings › Activation and Appearance as on board 15 — choice cards and setting rows
// with switches, every choice by keyboard alone and saved at once.
import { expect, test } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light' });
});

test('Activation works by keyboard: cards with the arrow keys, switches with Space', async ({
  page,
}) => {
  await page.goto('/#/settings?section=activation');
  const mode = page.getByRole('radiogroup', { name: 'The Activate button activates fonts' });
  await expect(mode.getByRole('radio')).toHaveCount(2);
  const session = mode.getByRole('radio', { name: 'For this session' });
  const permanent = mode.getByRole('radio', { name: 'Permanently' });
  await expect(session).toHaveAccessibleDescription('Until Font Fu quits.');
  await expect(session).toHaveAttribute('aria-checked', 'true');

  // One tab stop per group: the chosen card, then the other group's chosen card.
  await page.getByRole('heading', { name: 'Activation', level: 2 }).focus();
  await page.keyboard.press('Tab');
  await expect(session).toBeFocused();
  await page.keyboard.press('ArrowRight');
  await expect(permanent).toBeFocused();
  await expect(permanent).toHaveAttribute('aria-checked', 'true');
  await page.keyboard.press('Tab');
  const ask = page.getByRole('radio', { name: 'Ask me' });
  await expect(ask).toBeFocused();
  await page.keyboard.press('ArrowRight');
  await expect(page.getByRole('radio', { name: 'Replace without asking' })).toHaveAttribute(
    'aria-checked',
    'true',
  );

  // The three switches, in order, each named by its row and described by its line.
  const reactivate = page.getByRole('switch', { name: 'Reactivate on launch' });
  await page.keyboard.press('Tab');
  await expect(reactivate).toBeFocused();
  await expect(reactivate).toHaveAccessibleDescription(
    'Activate the last session’s fonts again when the app starts.',
  );
  await page.keyboard.press('Space');
  await expect(reactivate).toHaveAttribute('aria-checked', 'true');
  await page.keyboard.press('Tab');
  const tray = page.getByRole('switch', {
    name: 'Keep running in the tray when the window is closed',
  });
  await expect(tray).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(tray).toHaveAttribute('aria-checked', 'false');
  await page.keyboard.press('Tab');
  await expect(page.getByRole('switch', { name: 'Start at login' })).toBeFocused();

  // Saved at once: a reload shows the same choices.
  await page.reload();
  await expect(page.getByRole('radio', { name: 'Permanently' })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await expect(page.getByRole('radio', { name: 'Replace without asking' })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await expect(page.getByRole('switch', { name: 'Reactivate on launch' })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await expect(
    page.getByRole('switch', { name: 'Keep running in the tray when the window is closed' }),
  ).toHaveAttribute('aria-checked', 'false');
});

test('Appearance works by keyboard: themes, light or dark, and the language', async ({ page }) => {
  await page.goto('/#/settings?section=appearance');
  const cards = page.getByTestId('theme-card');
  await expect(cards).toHaveCount(4);
  // The themes are in one row.
  const tops = await cards.evaluateAll((els) => els.map((e) => e.getBoundingClientRect().top));
  expect(new Set(tops).size).toBe(1);

  const scheme = page.getByRole('radiogroup', { name: 'Light or dark' });
  await expect(scheme.getByRole('radio')).toHaveText(['Same as the system', 'Light', 'Dark']);
  await scheme.getByRole('radio', { name: 'Same as the system' }).focus();
  await page.keyboard.press('ArrowLeft');
  await expect(scheme.getByRole('radio', { name: 'Dark' })).toBeFocused();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');

  await page.keyboard.press('Tab');
  const language = page.getByRole('combobox', { name: 'Language' });
  await expect(language).toBeFocused();
  await expect(language).toHaveAccessibleDescription(
    'System follows the language of your computer; English when the app does not have it.',
  );

  await page.reload();
  await expect(
    page.getByRole('radiogroup', { name: 'Light or dark' }).getByRole('radio', { name: 'Dark' }),
  ).toHaveAttribute('aria-checked', 'true');
});
