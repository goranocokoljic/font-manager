import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// E48.8: at 1,000×700 the comparison name field shows its whole placeholder, in every
// locale; it never ends in an ellipsis.

const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);
const catalog = (tag: string) =>
  JSON.parse(
    readFileSync(fileURLToPath(new URL(`../src/locales/${tag}.json`, import.meta.url)), 'utf8'),
  ) as { compare: { namePlaceholder: string } };

/** The placeholder's width and the room the field has for it, in px. */
async function fit(page: Page) {
  const field = page.getByTestId('compare-sheet').locator('form.save input');
  await expect(field).toBeVisible();
  return field.evaluate((el: HTMLInputElement) => {
    const style = getComputedStyle(el);
    const ctx = document.createElement('canvas').getContext('2d')!;
    ctx.font = `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
    return {
      placeholder: el.placeholder,
      text: ctx.measureText(el.placeholder).width,
      room: el.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight),
      headOverflows: el.closest('header')!.scrollWidth > el.closest('header')!.clientWidth,
    };
  });
}

/** Every locale the app ships: `src/locales/<tag>.json`. */
const TAGS = readdirSync(fileURLToPath(new URL('../src/locales/', import.meta.url)))
  .filter((f) => f.endsWith('.json'))
  .map((f) => f.replace(/\.json$/, ''));

for (const tag of TAGS) {
  test.describe(`the comparison name field in ${tag}`, () => {
    test.use({ locale: tag === 'en' ? 'en-US' : tag, viewport: { width: 1000, height: 700 } });

    test('shows its whole placeholder at 1,000×700', async ({ page }) => {
      await page.route('**/fmfont/*', (route) =>
        route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
      );
      await page.goto('/?mockFaces=3#/');
      await page.evaluate(() => localStorage.clear());
      const tree = page.getByRole('tree').first();
      await tree.getByRole('treeitem').first().click();
      await page.keyboard.press('c');
      const got = await fit(page);
      expect(got.placeholder).toBe(catalog(tag).compare.namePlaceholder);
      expect(got.text, `"${got.placeholder}" fits`).toBeLessThanOrEqual(got.room);
      expect(got.headOverflows, 'the sheet header keeps its controls in view').toBe(false);
    });
  });
}
