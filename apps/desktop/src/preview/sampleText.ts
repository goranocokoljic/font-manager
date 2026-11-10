// The sample texts a preview row can show. These are display strings only:
// nothing here inspects a font. The font-name sample uses the face's names as
// the API reports them.
import type { FaceSummary } from '../api/types';
import { t } from '../i18n';

/** Which text the preview rows show. */
export type SampleKind =
  'custom' | 'pangram' | 'lorem' | 'alphabet' | 'numerals' | 'paragraph' | 'fontName' | 'letters';

// Getters: the menu label is read in the language shown now; the texts below are specimens.
export const SAMPLE_KINDS: { value: SampleKind; label: string }[] = (
  [
    'pangram',
    'custom',
    'lorem',
    'alphabet',
    'numerals',
    'paragraph',
    'fontName',
    'letters',
  ] as const
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
export const LETTERS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz';
export const NUMERALS = '0123456789';
export const SYMBOLS = '$€£¥ %‰ +−×÷= ½¼¾ #&@';
/** The Alphabet sample: the character set at a glance (board `1-wall-alphabet.png`). */
export const ALPHABET = `${LETTERS} ${NUMERALS} !?&@%`;
/** The Paragraph sample: body text (board `1-wall-paragraph.png`). */
export const PARAGRAPH =
  'Typography is the craft of endowing human language with a durable visual form. It exists to honor content, and a page set with care invites the reader in and keeps them reading.';

/** The most glyphs the Letters preset (E49.7) keeps: a card shows them all at once. */
export const LETTERS_MAX = 12;

/** The letters the empty Letters field offers to add: they tell faces apart (board 8). */
export const LETTERS_HINT = ['R', 'O', 'a', 'g', 'e'] as const;

const graphemes = new Intl.Segmenter(undefined, { granularity: 'grapheme' });

/**
 * The glyphs of the Letters preset (E49.7) in `text`: each user-perceived character once
 * (a base letter with its marks stays one), in the order typed, without spaces, at most
 * `LETTERS_MAX`. Any script: 'ROga', 'Жж', 'ặ'.
 */
export function letterGlyphs(text: string): string[] {
  const seen = new Set<string>();
  for (const { segment } of graphemes.segment(text)) {
    if (/^\s+$/u.test(segment) || seen.has(segment)) continue;
    seen.add(segment);
    if (seen.size === LETTERS_MAX) break;
  }
  return [...seen];
}

let lastLetters: { letters: string; sample: string } | undefined;

/**
 * The Letters specimen: the glyphs spaced evenly, one space between two ('R O a g').
 * Every card asks for it on every frame of a scroll; the last answer is kept.
 */
export function lettersSample(letters: string): string {
  if (lastLetters?.letters !== letters)
    lastLetters = { letters, sample: letterGlyphs(letters).join(' ') };
  return lastLetters.sample;
}

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

/** The settings a sample text is made from. `letters` is the Letters preset's glyphs. */
export interface SampleSource {
  sample: SampleKind;
  customText: string;
  language: string;
  letters?: string;
}

/**
 * The text a row shows for `face`; `pangram` gives the pangram of a language
 * code (empty while the languages load); `letters` the Letters glyphs. Empty custom text,
 * no letters and a missing pangram fall back to the font name, so a row never renders blank.
 */
export function sampleText(
  sample: SampleSource,
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
  sample: SampleSource,
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
    case 'paragraph':
      return PARAGRAPH;
    case 'fontName':
      return name;
    case 'letters':
      return lettersSample(sample.letters ?? '') || name;
  }
}

/**
 * True when `text` has a character past Basic Latin (U+0000–U+007F): then not every face
 * may render it, and the wall's count line says how many do (E43.1).
 */
export const beyondBasicLatin = (text: string) => /\P{ASCII}/u.test(text);
