import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

// E19.2: the Language setting relabels the UI without a reload, and "System"
// follows the OS (here the browser's) language.

type Entry = string | { [key: string]: Entry };
const catalog = (tag: string): Entry =>
  JSON.parse(
    readFileSync(fileURLToPath(new URL(`../src/locales/${tag}.json`, import.meta.url)), 'utf8'),
  ) as Entry;

function flatten(entry: Entry, prefix = '', out = new Map<string, string>()) {
  if (typeof entry === 'string') out.set(prefix, entry);
  else for (const [k, v] of Object.entries(entry)) flatten(v, prefix ? `${prefix}.${k}` : k, out);
  return out;
}

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/**
 * The English catalog's strings that German says differently, as whole-text
 * matchers (a `{placeholder}` matches anything). Shared words ("Name") and the
 * app's name are not English-only, so they are left out.
 */
const englishOnly: RegExp[] = (() => {
  const en = flatten(catalog('en'));
  const de = flatten(catalog('de'));
  const german = new Set(de.values());
  return (
    [...en]
      // A word German uses too ("Format", "App") is no sign of English.
      .filter(
        ([key, text]) =>
          de.get(key) !== text &&
          !german.has(text) &&
          /\p{L}{3}/u.test(text.replace(/\{\w+\}/g, '')),
      )
      .filter(([key]) => !key.startsWith('meta.') && !key.startsWith('errors.'))
      .map(
        ([, text]) =>
          new RegExp(
            `^${text
              .split(/\{\w+\}/)
              .map(escape)
              .join('.+')}$`,
            's',
          ),
      )
  );
})();

/** Library data of the mock, not UI text: the source folder `/Users/demo/Fonts`. */
const MOCK_DATA = new Set(['Fonts']);

/** Every text the user reads or hears now: text nodes and text attributes. */
function shownTexts(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const out: string[] = [];
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      const el = n.parentElement;
      if (!el || el.closest('script, style, [hidden]')) continue;
      const text = n.textContent?.trim();
      if (text) out.push(text);
    }
    for (const el of document.body.querySelectorAll('[aria-label], [title], [placeholder], [alt]'))
      for (const a of ['aria-label', 'title', 'placeholder', 'alt']) {
        const v = el.getAttribute(a)?.trim();
        if (v) out.push(v);
      }
    return out;
  });
}

async function expectNoEnglish(page: Page, where: string) {
  const texts = await shownTexts(page);
  expect(texts.length, where).toBeGreaterThan(5);
  const english = texts.filter((t) => !MOCK_DATA.has(t) && englishOnly.some((re) => re.test(t)));
  expect(english, `${where} shows English catalog strings`).toEqual([]);
}

test('choosing Deutsch relabels the main window, Settings and the palette at once', async ({
  page,
}) => {
  await page.goto('/#/settings?section=appearance');
  await expect(page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible();
  await page.getByRole('combobox', { name: 'Language' }).selectOption('de');

  // No reload: the same page relabels.
  await expect(page.getByRole('heading', { name: 'Einstellungen', level: 1 })).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('lang', 'de');
  await expectNoEnglish(page, 'Settings');

  await page.keyboard.press('Escape');
  await expect(page.getByRole('heading', { name: 'Alle Schriften' })).toBeVisible();
  // The first family is selected, so the inspector's Info tab shows too.
  await expect(page.getByRole('tab', { name: 'Infos' })).toBeVisible();
  await expectNoEnglish(page, 'the main window');

  await page.keyboard.press('ControlOrMeta+k');
  await expect(page.getByRole('dialog')).toBeVisible();
  await expect(page.getByRole('option', { name: /Einstellungen öffnen/ })).toBeVisible();
  await expectNoEnglish(page, 'the command palette');
  await page.keyboard.press('Escape');

  // And back.
  await page.goto('/#/settings?section=appearance');
  await page.getByRole('combobox', { name: 'Sprache' }).selectOption('en');
  await expect(page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible();
});

test.describe('with a German OS', () => {
  test.use({ locale: 'de-DE' });

  test('"System" shows German, and choosing English overrides it', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByRole('heading', { name: 'Alle Schriften' })).toBeVisible();
    await expectNoEnglish(page, 'the main window');

    await page.goto('/#/settings?section=appearance');
    const language = page.getByRole('combobox', { name: 'Sprache' });
    await expect(language).toHaveValue('system');
    await expect(language.locator('option[value="system"]')).toHaveText('System (Deutsch)');
    await language.selectOption('en');
    await expect(page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible();
  });
});
