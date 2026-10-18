import { describe, expect, it } from 'vitest';
import type { FaceSummary } from '../api/types';
import { mockFaces } from '../api/mock';
import {
  ALPHABET,
  LOREM,
  NUMERALS,
  PANGRAM_LANGUAGES,
  PANGRAMS,
  SAMPLE_KINDS,
  sampleText,
} from './sampleText';

// Family and style differ from each other and from every preset text.
const face: FaceSummary = {
  ...mockFaces[1]!.face,
  familyName: 'Zebra Serif',
  style: 'Ultra Oblique',
};
const base = { customText: '', language: 'en' as const };

describe('sample texts', () => {
  it('offers pangrams in 10 languages, each a different text', () => {
    expect(PANGRAM_LANGUAGES).toHaveLength(10);
    const texts = new Set(PANGRAM_LANGUAGES.map((l) => PANGRAMS[l].text));
    expect(texts.size).toBe(10);
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
    expect(sampleText({ ...base, sample: 'pangram', language: 'ru' }, face)).toBe(PANGRAMS.ru.text);
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
