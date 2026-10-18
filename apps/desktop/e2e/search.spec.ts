import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// Preview rows load face files from `/fmfont/<id>`; serve one fixture for all.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page, query = '') {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(`/${query}#/`);
  const tree = page.getByRole('tree', { name: 'Font list' });
  await expect(tree.getByRole('treeitem').first()).toBeVisible();
  return tree;
}

const names = (page: Page) =>
  page
    .getByRole('tree', { name: 'Font list' })
    .locator('[role="treeitem"][aria-level="1"]')
    .evaluateAll((rows) => rows.map((r) => r.getAttribute('aria-label')?.split(',')[0]));

test('/ focuses the search box and typing narrows the list', async ({ page }) => {
  await open(page, '?mockFaces=2&mockStyles=4');
  const box = page.getByRole('searchbox', { name: 'Search fonts' });
  await page.getByRole('tree', { name: 'Font list' }).focus();
  await page.keyboard.press('/');
  await expect(box).toBeFocused();
  // The key that focused the box is not typed into it.
  await expect(box).toHaveValue('');
  await page.keyboard.type('mono');
  await expect.poll(() => names(page)).toEqual(['JetBrains Mono']);
  await expect(page.getByRole('status').filter({ hasText: '1 family' })).toBeVisible();
  // Escape empties the box: the whole library is back.
  await page.keyboard.press('Escape');
  await expect(box).toHaveValue('');
  await expect.poll(() => names(page)).toHaveLength(5);
});

test('a search with no result says so', async ({ page }) => {
  await open(page);
  await page.getByRole('searchbox', { name: 'Search fonts' }).fill('zzzz');
  await expect(page.getByText('No fonts match the search and filters.')).toBeVisible();
});

test('filter chips under the title show counts and filter the list', async ({ page }) => {
  await open(page, '?mockFaces=2&mockStyles=4');
  const chips = page.getByRole('region', { name: 'Filters' });
  // Only the two extra families have italics.
  const italic = chips
    .getByRole('group', { name: 'Slant' })
    .getByRole('button', { name: /^Italic/ });
  await expect(italic).toHaveAccessibleName('Italic, 2 families');
  await expect(italic).toHaveAttribute('aria-pressed', 'false');
  await expect(chips.getByRole('group', { name: 'Format' })).toContainText('Type 1');
  await italic.click();
  await expect(italic).toHaveAttribute('aria-pressed', 'true');
  await expect.poll(() => names(page)).toEqual(['Sample 1', 'Sample 2']);
  // Counts follow the other filters: with italics, one Type 1 family is not left.
  await expect(chips.getByRole('group', { name: 'Format' })).not.toContainText('Type 1');

  // A language preset joins as a chip with the result count.
  await chips.getByRole('combobox', { name: 'Add a language' }).selectOption('sr-Cyrl');
  const serbian = chips.getByRole('button', { name: /^Serbian \(Cyrillic\)/ });
  await expect(serbian).toHaveAttribute('aria-pressed', 'true');
  await expect(serbian).toHaveAccessibleName('Serbian (Cyrillic), 2 families');

  await chips.getByRole('button', { name: 'Clear filters' }).click();
  await expect.poll(() => names(page)).toHaveLength(5);
  await expect(serbian).toBeHidden();
});

test('a chip set is saved as a smart collection', async ({ page }) => {
  await open(page);
  const chips = page.getByRole('region', { name: 'Filters' });
  await expect(chips.getByRole('button', { name: 'Save as smart collection' })).toBeHidden();
  await chips
    .getByRole('group', { name: 'Variable' })
    .getByRole('button', { name: /^Static/ })
    .click();
  await chips.getByRole('button', { name: 'Save as smart collection' }).click();
  const name = chips.getByRole('textbox', { name: 'Smart collection name' });
  await expect(name).toBeFocused();
  await name.fill('Static fonts');
  await page.keyboard.press('Enter');
  await expect(chips.getByRole('status')).toHaveText('Saved “Static fonts” as a smart collection.');
  // The same name again is refused, and the reason shows.
  await chips.getByRole('button', { name: 'Save as smart collection' }).click();
  await name.fill('Static fonts');
  await chips.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(chips.getByRole('alert')).toContainText('already exists');
});

test('results show 80 ms (debounce) plus under 50 ms after the last key on 1,000 families', async ({
  page,
}) => {
  await open(page, '?mockFaces=997');
  const box = page.getByRole('searchbox', { name: 'Search fonts' });
  await box.focus();
  const ms = await page.evaluate(async () => {
    const input = document.querySelector<HTMLInputElement>('input[type="search"]')!;
    const tree = document.querySelector('[role="tree"]')!;
    input.value = 'Sample 512';
    const t0 = performance.now();
    input.dispatchEvent(new Event('input'));
    await new Promise<void>((done) => {
      const check = () => {
        const first = tree.querySelector('[role="treeitem"]')?.getAttribute('aria-label') ?? '';
        if (first.startsWith('Sample 512,')) done();
        else requestAnimationFrame(check);
      };
      check();
    });
    return performance.now() - t0;
  });
  console.log(`search: results ${ms.toFixed(1)} ms after the last key`);
  expect(ms).toBeGreaterThanOrEqual(80);
  // The browser mock does the search in the page; leave room for the frame the check waits for.
  expect(ms).toBeLessThan(80 + 50 + 34);
});
