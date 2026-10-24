// The sample texts a preview row can show. These are display strings only:
// nothing here inspects a font. The font-name sample uses the face's names as
// the API reports them.
import type { FaceSummary } from '../api/types';
import { t } from '../i18n';

/** Which text the preview rows show. */
export type SampleKind = 'custom' | 'pangram' | 'lorem' | 'alphabet' | 'numerals' | 'fontName';

// Getters: the menu label is read in the language shown now; the texts below are specimens.
export const SAMPLE_KINDS: { value: SampleKind; label: string }[] = (
  ['pangram', 'custom', 'lorem', 'alphabet', 'numerals', 'fontName'] as const
).map((value) => ({
  value,
  get label() {
    return t(`samples.kinds.${value}`);
  },
}));

/** The pangram language when the UI language has none. */
export const FALLBACK_LANGUAGE = 'en';

/** A language preset code as `data/charsets` writes it (`pl`, `sr-Cyrl`). */
const LANGUAGE_CODE = /^[a-z]{2,3}(-[A-Z][a-z]{3})?$/;

/** Chinese regions that write Traditional characters. */
const HANT_REGIONS = new Set(['TW', 'HK', 'MO']);

export const LOREM =
  'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.';
export const ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz';
export const NUMERALS = '0123456789 $€£¥ %‰ +−×÷= ½¼¾ #&@';

/** Whether `value` has the shape of a language preset code. */
export function isLanguageCode(value: unknown): value is string {
  return typeof value === 'string' && LANGUAGE_CODE.test(value);
}

/**
 * The pangram language for the UI languages `preferred` (BCP 47 tags, most
 * wanted first, as `navigator.languages`), out of the codes that have a
 * pangram: an exact match, then language and script (`sr-Latn-RS`), then the
 * language alone (`sr` takes the first `sr-*` code; `zh-TW` takes `zh-Hant`).
 * English when none matches.
 */
export function pickLanguage(preferred: readonly string[], available: readonly string[]): string {
  const has = (code: string) => available.includes(code);
  for (const tag of preferred) {
    const [lang = '', ...rest] = tag.split(/[-_]/);
    const primary = lang.toLowerCase();
    const script = rest.find((p) => p.length === 4);
    const region = rest.find((p) => p.length === 2)?.toUpperCase();
    const candidates = [
      script ? `${primary}-${script[0]!.toUpperCase()}${script.slice(1).toLowerCase()}` : '',
      primary === 'zh' && !script && region
        ? HANT_REGIONS.has(region)
          ? 'zh-Hant'
          : 'zh-Hans'
        : '',
      primary,
    ];
    const hit =
      candidates.find((c) => c && has(c)) ?? available.find((c) => c.startsWith(`${primary}-`));
    if (hit) return hit;
  }
  return FALLBACK_LANGUAGE;
}

/** The face's name as the preview's font-name sample shows it. */
export function fontNameSample(face: FaceSummary): string {
  return `${face.familyName} ${face.style}`;
}

/**
 * The text a row shows for `face`; `pangram` gives the pangram of a language
 * code (empty while the languages load). Empty custom text and a missing
 * pangram fall back to the font name, so a row never renders blank.
 */
export function sampleText(
  sample: { sample: SampleKind; customText: string; language: string },
  face: FaceSummary,
  pangram: (language: string) => string,
): string {
  return sampleTextFor(sample, fontNameSample(face), pangram);
}

/**
 * `sampleText` for a row named `name` that has no face in the library (a catalog
 * family, E22.2): the font-name sample and the fallbacks show `name`.
 */
export function sampleTextFor(
  sample: { sample: SampleKind; customText: string; language: string },
  name: string,
  pangram: (language: string) => string,
): string {
  switch (sample.sample) {
    case 'custom':
      return sample.customText.trim() ? sample.customText : name;
    case 'pangram':
      return pangram(sample.language) || name;
    case 'lorem':
      return LOREM;
    case 'alphabet':
      return ALPHABET;
    case 'numerals':
      return NUMERALS;
    case 'fontName':
      return name;
  }
}
