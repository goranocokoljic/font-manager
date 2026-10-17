// The sample texts a preview row can show. These are display strings only:
// nothing here inspects a font. The font-name sample uses the face's names as
// the API reports them.
import type { FaceSummary } from '../api/types';

/** Which text the preview rows show. */
export type SampleKind = 'custom' | 'pangram' | 'lorem' | 'alphabet' | 'numerals' | 'fontName';

export const SAMPLE_KINDS: { value: SampleKind; label: string }[] = [
  { value: 'pangram', label: 'Pangram' },
  { value: 'custom', label: 'Custom text' },
  { value: 'lorem', label: 'Lorem ipsum' },
  { value: 'alphabet', label: 'Alphabet' },
  { value: 'numerals', label: 'Numerals' },
  { value: 'fontName', label: 'Font name' },
];

/** Pangrams by BCP 47 language tag. */
export const PANGRAMS = {
  en: { label: 'English', text: 'The quick brown fox jumps over the lazy dog' },
  de: { label: 'German', text: 'Victor jagt zwölf Boxkämpfer quer über den großen Sylter Deich' },
  fr: { label: 'French', text: 'Portez ce vieux whisky au juge blond qui fume' },
  es: { label: 'Spanish', text: 'El veloz murciélago hindú comía feliz cardillo y kiwi' },
  pl: { label: 'Polish', text: 'Pchnąć w tę łódź jeża lub ośm skrzyń fig' },
  cs: { label: 'Czech', text: 'Příliš žluťoučký kůň úpěl ďábelské ódy' },
  ru: { label: 'Russian', text: 'Съешь же ещё этих мягких французских булок, да выпей чаю' },
  el: { label: 'Greek', text: 'Ξεσκεπάζω την ψυχοφθόρα βδελυγμία' },
  he: { label: 'Hebrew', text: 'דג סקרן שט בים מאוכזב ולפתע מצא חברה' },
  ja: {
    label: 'Japanese',
    text: 'いろはにほへと ちりぬるを わかよたれそ つねならむ うゐのおくやま けふこえて あさきゆめみし ゑひもせす',
  },
} as const;

export type PangramLanguage = keyof typeof PANGRAMS;

export const PANGRAM_LANGUAGES = Object.keys(PANGRAMS) as PangramLanguage[];

export const LOREM =
  'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.';
export const ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz';
export const NUMERALS = '0123456789 $€£¥ %‰ +−×÷= ½¼¾ #&@';

export function isPangramLanguage(value: unknown): value is PangramLanguage {
  return typeof value === 'string' && Object.hasOwn(PANGRAMS, value);
}

/** The face's name as the preview's font-name sample shows it. */
export function fontNameSample(face: FaceSummary): string {
  return `${face.familyName} ${face.style}`;
}

/**
 * The text a row shows for `face`. Empty custom text falls back to the font
 * name, so a row never renders blank.
 */
export function sampleText(
  sample: { sample: SampleKind; customText: string; language: PangramLanguage },
  face: FaceSummary,
): string {
  switch (sample.sample) {
    case 'custom':
      return sample.customText.trim() ? sample.customText : fontNameSample(face);
    case 'pangram':
      return PANGRAMS[sample.language].text;
    case 'lorem':
      return LOREM;
    case 'alphabet':
      return ALPHABET;
    case 'numerals':
      return NUMERALS;
    case 'fontName':
      return fontNameSample(face);
  }
}
