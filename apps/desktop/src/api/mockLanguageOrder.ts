// The language order of the TypeScript mock (E58.1, ADR 0028): a copy of
// `fm_core::language_order::arrange` and of the Rust mock's copy.
// `fixtures/language-order/cases.json` runs against all three, so they give the
// same order for the same settings.
import order from '../../../../data/charsets/order.json';
import type { LanguageGroup, LanguagePreset } from './types';

/** The most used languages of `data/charsets/order.json`, in the file's order. */
export const mostUsedLanguages: readonly string[] = order.mostUsed;

/** The primary language and the script a locale tag names (`sr-Latn-RS`). */
const splitTag = (tag: string): [string, string | null] | null => {
  const parts = tag.trim().split(/[-_]/);
  const lang = (parts[0] ?? '').toLowerCase();
  if (!/^[a-z]{2,3}$/.test(lang)) return null;
  const raw = parts.slice(1).find((p) => /^[A-Za-z]{4}$/.test(p));
  const script = raw ? raw[0].toUpperCase() + raw.slice(1).toLowerCase() : null;
  return [lang, script];
};

/** The presets of the system's language: the named script's, else all of the language. */
export const systemLanguageCodes = (tag: string, presets: readonly LanguagePreset[]): string[] => {
  const split = splitTag(tag);
  if (!split) return [];
  const [lang, script] = split;
  const ofLanguage = presets.filter((p) => p.code.split('-')[0] === lang);
  const ofScript = script
    ? ofLanguage.filter((p) => p.code.split('-')[1] === script || p.script === script)
    : [];
  return (ofScript.length ? ofScript : ofLanguage).map((p) => p.code);
};

const byName = (a: LanguagePreset, b: LanguagePreset): number => {
  const [x, y] = [a.name.toLowerCase(), b.name.toLowerCase()];
  if (x !== y) return x < y ? -1 : 1;
  return a.code < b.code ? -1 : a.code > b.code ? 1 : 0;
};

/**
 * `presets` in the one order, each with its group: the user's languages (else the
 * system's language), then the most used, then the rest from A to Z by English name.
 */
export const arrangeLanguages = (
  presets: readonly LanguagePreset[],
  mostUsed: readonly string[],
  myLanguages: readonly string[],
  system: string | null,
): LanguagePreset[] => {
  const lead = myLanguages.length
    ? myLanguages
    : system
      ? systemLanguageCodes(system, presets)
      : [];
  const rest = new Map(presets.map((p) => [p.code, p]));
  const out: LanguagePreset[] = [];
  const take = (codes: readonly string[], group: LanguageGroup) => {
    for (const code of codes) {
      const preset = rest.get(code);
      if (!preset) continue;
      rest.delete(code);
      out.push({ ...preset, group });
    }
  };
  take(lead, 'mine');
  take(mostUsed, 'mostUsed');
  const others = [...rest.values()].sort(byName).map((p) => ({ ...p, group: 'others' as const }));
  return [...out, ...others];
};
