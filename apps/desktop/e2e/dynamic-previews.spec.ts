import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/#/');
  await expect(page.locator('[data-face="1"]')).toHaveAttribute('data-state', 'ready');
}

// MockApi: Inter (1) and JetBrains Mono (6) map Latin and Serbian Cyrillic; Legacy
// Grotesk (7) maps basic Latin and Latin-1 only.
test('a font that lacks characters of the sample previews its own letters with a fallback hint', async ({
  page,
}) => {
  await open(page);
  const controls = page.getByRole('group', { name: 'Preview settings' });
  const row = (face: number) => page.locator(`[data-face="${face}"]`);
  const hint = (face: number) => row(face).getByTestId('fallback-hint');

  await expect(row(7).locator('.sample')).toHaveText(/The quick brown fox/);
  await expect(page.getByTestId('fallback-hint')).toHaveCount(0);

  await controls.getByLabel('Sample').selectOption('custom');
  await controls.getByLabel('Custom text').fill('Љубав');
  await expect(row(1).locator('.sample')).toHaveText('Љубав');
  await expect(row(6).locator('.sample')).toHaveText('Љубав');
  await expect(hint(1)).toHaveCount(0);
  await expect(row(7).locator('.sample')).toHaveText('abcdefghijklmnopqrstuvwxyz');
  await expect(row(7).locator('.sample')).toHaveAttribute('data-fallback', 'script');
  await expect(hint(7)).toHaveText('Fallback');
  await expect(hint(7)).toBeVisible();
  await expect(hint(7)).toHaveAttribute(
    'title',
    'This font lacks Љ у б а в, so it shows its own letters.',
  );

  // Covered text again: the sample comes back and the hint goes.
  await controls.getByLabel('Custom text').fill('Größe');
  await expect(row(7).locator('.sample')).toHaveText('Größe');
  await expect(page.getByTestId('fallback-hint')).toHaveCount(0);
});
