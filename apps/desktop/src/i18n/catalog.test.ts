import { afterEach, describe, expect, it } from 'vitest';
import { computed } from 'vue';
import {
  CATALOGS,
  FALLBACK_LOCALE,
  LOCALES,
  errorMessage,
  formatDate,
  formatNumber,
  locale,
  resolveLocale,
  setLocale,
  t,
  type Catalog,
  type MessageKey,
  type PluralForms,
} from '.';

type Entry = string | PluralForms | Catalog;

const PLURAL_FORMS = new Set(['zero', 'one', 'two', 'few', 'many', 'other']);

function isPlural(value: Entry): value is PluralForms {
  return (
    typeof value === 'object' &&
    typeof (value as PluralForms).other === 'string' &&
    Object.keys(value).every((k) => PLURAL_FORMS.has(k))
  );
}

/** Every message as `dotted.key → text` (a plural's forms joined, for its placeholders). */
function flatten(catalog: Catalog, prefix = ''): Map<string, { plural: boolean; text: string }> {
  const out = new Map<string, { plural: boolean; text: string }>();
  for (const [k, v] of Object.entries(catalog)) {
    const key = `${prefix}${k}`;
    if (typeof v === 'string') out.set(key, { plural: false, text: v });
    else if (isPlural(v)) out.set(key, { plural: true, text: Object.values(v).join(' ') });
    else for (const [ik, iv] of flatten(v, `${key}.`)) out.set(ik, iv);
  }
  return out;
}

const placeholders = (text: string) => [...new Set(text.match(/\{\w+\}/g) ?? [])].sort();

/** The keys of `en.json` that `catalog` lacks, and the ones it has that English does not. */
function compare(catalog: Catalog) {
  const en = flatten(CATALOGS[FALLBACK_LOCALE]);
  const other = flatten(catalog);
  return {
    missing: [...en.keys()].filter((k) => !other.has(k)),
    extra: [...other.keys()].filter((k) => !en.has(k)),
    shape: [...en.entries()]
      .filter(([k, v]) => other.has(k) && other.get(k)!.plural !== v.plural)
      .map(([k]) => k),
    placeholders: [...en.entries()]
      .filter(
        ([k, v]) =>
          other.has(k) &&
          // English shows the core's own sentence for an error; others say it by code.
          !k.startsWith('errors.') &&
          placeholders(other.get(k)!.text).join() !== placeholders(v.text).join(),
      )
      .map(([k]) => k),
  };
}

afterEach(() => setLocale('en'));

describe('locale catalogs', () => {
  it('ship English and a second locale', () => {
    expect(LOCALES).toContain('en');
    expect(LOCALES.length).toBeGreaterThanOrEqual(2);
  });

  it('the comparison reports a missing key, an extra key and a plural that is not one', () => {
    const report = compare({
      meta: { name: 'Test' },
      app: { nope: 'x' },
      shell: { tray: { active: 'not counted' } },
    });
    expect(report.missing).toContain('app.name');
    expect(report.missing).toContain('common.close');
    expect(report.extra).toEqual(['app.nope']);
    expect(report.shape).toEqual(['shell.tray.active']);
  });

  for (const tag of Object.keys(CATALOGS).filter((l) => l !== FALLBACK_LOCALE)) {
    it(`${tag} has every key of en.json, with the same plurals and placeholders`, () => {
      expect(compare(CATALOGS[tag])).toEqual({
        missing: [],
        extra: [],
        shape: [],
        placeholders: [],
      });
    });

    it(`${tag} names itself and is not English`, () => {
      const en = flatten(CATALOGS[FALLBACK_LOCALE]);
      const other = flatten(CATALOGS[tag]);
      const same = [...en.keys()].filter((k) => other.get(k)?.text === en.get(k)?.text);
      // Proper names and words the language shares stay; the rest is translated.
      expect(same.length / en.size).toBeLessThan(0.1);
      expect(typeof (CATALOGS[tag].meta as Catalog).name).toBe('string');
    });
  }
});

describe('t', () => {
  it('reads a message in the locale, fills placeholders and falls back to English', () => {
    expect(t('common.close')).toBe('Close');
    setLocale('de');
    expect(t('common.close')).toBe('Schließen');
    expect(t('app.helpAction', { page: 'Quellen' })).toBe('Hilfe: Quellen');
    expect(t('no.such.key' as MessageKey)).toBe('no.such.key');
  });

  it('uses the plural rules of the locale and formats counts with Intl', () => {
    expect(t('shell.tray.active', { count: 0 })).toBe('No fonts active');
    expect(t('shell.tray.active', { count: 1 })).toBe('1 font active');
    expect(t('shell.tray.active', { count: 1234 })).toBe('1,234 fonts active');
    setLocale('de');
    expect(t('shell.tray.active', { count: 1 })).toBe('1 Schrift aktiv');
    expect(t('shell.tray.active', { count: 1234 })).toBe('1.234 Schriften aktiv');
  });

  it('formats numbers and dates for the locale', () => {
    const day = Date.UTC(2026, 9, 21, 12);
    expect(formatNumber(1234.5)).toBe('1,234.5');
    const en = formatDate(day, { dateStyle: 'long', timeZone: 'UTC' });
    setLocale('de');
    expect(formatNumber(1234.5)).toBe('1.234,5');
    expect(formatDate(day, { dateStyle: 'long', timeZone: 'UTC' })).toBe('21. Oktober 2026');
    expect(en).toBe('October 21, 2026');
  });

  it('follows setLocale inside a computed, without a restart', () => {
    const label = computed(() => t('app.openSettings'));
    expect(label.value).toBe('Open settings');
    setLocale('de');
    expect(label.value).toBe('Einstellungen öffnen');
    expect(locale()).toBe('de');
    setLocale('xx');
    expect(locale()).toBe('en');
  });
});

describe('resolveLocale', () => {
  it('follows the OS for System and falls back to English', () => {
    expect(resolveLocale('system', ['de-AT', 'en-US'])).toBe('de');
    expect(resolveLocale(undefined, ['fr-FR', 'de'])).toBe('de');
    expect(resolveLocale('system', ['fr-FR', 'ja'])).toBe('en');
    expect(resolveLocale('system', [])).toBe('en');
  });

  it('takes an explicit choice over the OS', () => {
    expect(resolveLocale('en', ['de-DE'])).toBe('en');
    expect(resolveLocale('de', ['en-US'])).toBe('de');
  });
});

describe('errorMessage', () => {
  const error = { code: 'notFound' as const, message: 'This face is gone.', detail: null };

  it("shows the core's English message in English", () => {
    expect(errorMessage(error)).toBe('This face is gone.');
  });

  it('shows the catalog message for the code in another locale', () => {
    setLocale('de');
    expect(errorMessage(error)).toBe('Das Gesuchte wurde nicht gefunden.');
    expect(errorMessage({ ...error, code: 'busy' })).toMatch(/andere Aufgabe/);
  });

  it("falls back to the core's message for a code the catalog does not know", () => {
    setLocale('de');
    expect(errorMessage({ code: 'fromTheFuture', message: 'New problem.' })).toBe('New problem.');
    expect(errorMessage(new Error('boom'))).toBe(t('errors.internal', { message: '' }));
  });
});
