// Every list of languages in the app shows them in the order `list_languages` answers
// (ADR 0028): the user's languages, the most used, then the others from A to Z. The core
// decides that order; these helpers only keep it. None of them sorts.
import type { LanguagePreset } from '../api/types';
import type { SelectOption } from '../components/ui/types';

/** The languages the filter drawer shows before "Show more": the first ten of the answer. */
export const LANGUAGES_SHOWN = 10;

/** True where `list[i]` is the first language after the user's ones: a menu draws a line. */
export function startsAfterMine(
  list: readonly Pick<LanguagePreset, 'group'>[],
  i: number,
): boolean {
  return i > 0 && list[i - 1]?.group === 'mine' && list[i]?.group !== 'mine';
}

/**
 * The options of a language menu, in the order of `list`, with a hairline before the first
 * language that is not one of the user's (`group` from the core).
 */
export function languageMenu<T extends string>(
  list: readonly LanguagePreset[],
  option: (language: LanguagePreset) => SelectOption<T>,
): SelectOption<T>[] {
  return list.map((l, i) => ({
    ...option(l),
    ...(startsAfterMine(list, i) ? { separatorBefore: true } : {}),
  }));
}

/** The presets whose code is in `codes`, in the API's order (not in the order of `codes`). */
export function presetsIn(
  presets: readonly LanguagePreset[],
  codes: Iterable<string>,
): LanguagePreset[] {
  const wanted = new Set(codes);
  return presets.filter((p) => wanted.has(p.code));
}

/**
 * The languages a sample can switch to (Compare copies): those of `codes` that have a pangram
 * or sample glyphs, in the API's order, so the user's languages lead.
 */
export function sampleLanguages(
  presets: readonly LanguagePreset[],
  codes: Iterable<string>,
): LanguagePreset[] {
  return presetsIn(presets, codes).filter((p) => !!(p.pangram ?? p.sampleGlyphs));
}

/** What follows the user's languages in the API's answer (Settings › Languages, E58.2). */
export interface LanguageRest {
  /** The languages that complete the first `LANGUAGES_SHOWN`, each with its place (1-based). */
  first: { preset: LanguagePreset; place: number }[];
  /** After the first ten: the most used ones, by name in the answer's order. */
  more: LanguagePreset[];
  /** After those: every other language, A to Z as the API answers them. */
  others: LanguagePreset[];
}

/**
 * Splits the API's answer after the user's languages (`group: 'mine'`): what completes the
 * first `LANGUAGES_SHOWN` (the ten the filter drawer shows), then what is behind "Show
 * more". It reads only each preset's place and `group`; it never sorts.
 */
export function languageRest(list: readonly LanguagePreset[]): LanguageRest {
  const rest: LanguageRest = { first: [], more: [], others: [] };
  list.forEach((preset, i) => {
    if (preset.group === 'mine') return;
    if (i < LANGUAGES_SHOWN) rest.first.push({ preset, place: i + 1 });
    else if (preset.group === 'mostUsed') rest.more.push(preset);
    else rest.others.push(preset);
  });
  return rest;
}
