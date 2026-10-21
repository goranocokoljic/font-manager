import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// E11.3: the coverage list of the Glyphs tab, on the MockApi.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-static/robotoslab/RobotoSlab-ExtraBold.ttf',
      import.meta.url,
    ),
  ),
);

// MockApi face 1 (Inter Regular) has Latin, Latin Extended-A and Cyrillic;
// face 7 (Legacy Grotesk) Latin only, so Polish and Serbian Latin are partial.
async function openCoverage(page: Page, face: number) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(`/#/?face=${face}`);
  await page.getByRole('tab', { name: 'Glyphs' }).click();
  const panel = page.getByRole('tabpanel', { name: 'Glyphs' });
  const coverage = panel.getByRole('region', { name: 'Coverage' });
  await expect(coverage.getByRole('list', { name: 'Unicode blocks' })).toBeVisible();
  return { panel, coverage };
}

test('clicking a block jumps the glyph map to that block', async ({ page }) => {
  const { panel, coverage } = await openCoverage(page, 1);
  const blocks = coverage.getByRole('list', { name: 'Unicode blocks' }).getByRole('button');
  await expect(blocks).toHaveText([
    /Basic Latin\s*95\/128/,
    /Latin-1 Supplement\s*96\/128/,
    /Latin Extended-A\s*128\/128/,
    /Cyrillic\s*60\/256/,
  ]);
  await coverage.getByRole('button', { name: /^Cyrillic: 60 of 256 characters/ }).click();
  await expect(panel.getByRole('rowheader')).toHaveCount(1);
  await expect(panel.getByRole('rowheader')).toContainText('Cyrillic');
  await expect(panel.getByRole('combobox', { name: 'Block' })).toHaveValue('Cyrillic');
  await expect(panel.getByRole('grid', { name: 'Glyphs' })).toBeFocused();
});

test('every coverage row is keyboard-reachable with a label; a partial language lists what it lacks', async ({
  page,
}) => {
  const { coverage } = await openCoverage(page, 7);
  const languages = coverage.getByRole('list', { name: 'Languages' }).getByRole('button');
  await expect(languages).toHaveText([
    /English\s*complete/,
    /Serbian \(Latin\)\s*22\/27/,
    /Polish\s*24\/32/,
  ]);

  // Tab from the first row through every row: each gets focus and has a label.
  const rows = coverage.getByRole('listitem').getByRole('button');
  const count = await rows.count();
  expect(count).toBe(5);
  await rows.first().focus();
  const reached: string[] = [];
  for (let i = 0; i < count; i++) {
    const label = await page.evaluate(() => document.activeElement?.getAttribute('aria-label'));
    expect(label, `row ${i + 1} has a label`).toBeTruthy();
    reached.push(label!);
    await page.keyboard.press('Tab');
  }
  expect(reached).toEqual([
    'Basic Latin: 95 of 128 characters. Show in the glyph map.',
    'Latin-1 Supplement: 96 of 128 characters. Show in the glyph map.',
    'English: complete',
    'Serbian (Latin): 22 of 27 characters',
    'Polish: 24 of 32 characters',
  ]);

  // Enter on a partial language shows its missing characters.
  const polish = coverage.getByRole('button', { name: 'Polish: 24 of 32 characters' });
  await polish.focus();
  await page.keyboard.press('Enter');
  await expect(polish).toHaveAttribute('aria-expanded', 'true');
  await expect(coverage.getByLabel('Missing from Polish')).toHaveText(
    /ą\s*ć\s*ę\s*ł\s*ń\s*ś\s*ź\s*ż/,
  );
});
