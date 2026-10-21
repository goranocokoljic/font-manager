import { describe, expect, it } from 'vitest';
import type { FaceSummary } from '../api/types';
import { mockFaces } from '../api/mock';
import {
  ALPHABET,
  LOREM,
  NUMERALS,
  pickLanguage,
  SAMPLE_KINDS,
  sampleText as sampleTextWith,
} from './sampleText';

// Family and style differ from each other and from every preset text.
const face: FaceSummary = {
  ...mockFaces[1]!.face,
  familyName: 'Zebra Serif',
  style: 'Ultra Oblique',
};
const base = { customText: '', language: 'en' };
const PANGRAMS: Record<string, string> = { en: 'English pangram', ru: 'Съешь же ещё' };
const sampleText = (s: Parameters<typeof sampleTextWith>[0], f: FaceSummary) =>
  sampleTextWith(s, f, (code) => PANGRAMS[code] ?? '');

describe('sample texts', () => {
  it('shows the pangram of the chosen language from the lookup', () => {
    expect(sampleText({ ...base, sample: 'pangram', language: 'ru' }, face)).toBe('Съешь же ещё');
  });

  it('falls back to the face name while no pangram is known', () => {
    expect(sampleText({ ...base, sample: 'pangram', language: 'xx' }, face)).toBe(
      'Zebra Serif Ultra Oblique',
    );
  });

  it('offers every sample kind of the criterion', () => {
    expect(SAMPLE_KINDS.map((k) => k.value).sort()).toEqual(
      ['alphabet', 'custom', 'fontName', 'lorem', 'numerals', 'pangram'].sort(),
    );
  });

  it('gives each kind its own text', () => {
    expect(sampleText({ ...base, sample: 'lorem' }, face)).toBe(LOREM);
    expect(sampleText({ ...base, sample: 'alphabet' }, face)).toBe(ALPHABET);
    expect(sampleText({ ...base, sample: 'numerals' }, face)).toBe(NUMERALS);
    expect(sampleText({ ...base, sample: 'pangram' }, face)).toBe('English pangram');
    expect(sampleText({ ...base, sample: 'custom', customText: 'Hamburgefonstiv' }, face)).toBe(
      'Hamburgefonstiv',
    );
  });

  it('shows the face name for the font-name sample', () => {
    expect(sampleText({ ...base, sample: 'fontName' }, face)).toBe('Zebra Serif Ultra Oblique');
  });

  it('falls back to the face name when the custom text is blank', () => {
    expect(sampleText({ ...base, sample: 'custom', customText: '   ' }, face)).toBe(
      'Zebra Serif Ultra Oblique',
    );
  });
});

describe('pickLanguage', () => {
  const available = ['en', 'de', 'sr-Cyrl', 'sr-Latn', 'zh-Hans', 'zh-Hant', 'pt'];

  it('follows the first UI language that has a pangram', () => {
    expect(pickLanguage(['de-AT', 'en-US'], available)).toBe('de');
    expect(pickLanguage(['ko-KR', 'pt-BR'], available)).toBe('pt');
  });

  it('matches language and script, then the language alone', () => {
    expect(pickLanguage(['sr-Latn-RS'], available)).toBe('sr-Latn');
    expect(pickLanguage(['sr-RS'], available)).toBe('sr-Cyrl');
    expect(pickLanguage(['sr'], ['sr-Latn'])).toBe('sr-Latn');
  });

  it('picks the Chinese script from the region', () => {
    expect(pickLanguage(['zh-TW'], available)).toBe('zh-Hant');
    expect(pickLanguage(['zh-CN'], available)).toBe('zh-Hans');
    expect(pickLanguage(['zh-Hant-HK'], available)).toBe('zh-Hant');
  });

  it('falls back to English', () => {
    expect(pickLanguage(['ko-KR'], available)).toBe('en');
    expect(pickLanguage([], available)).toBe('en');
    expect(pickLanguage(['de'], [])).toBe('en');
  });
});
