import { describe, expect, it } from 'vitest';
import type { SampleText } from '../api/types';
import { missingLine, specimenOf, specimenRuns, tofuLabel } from './tofu';

const SERBIAN = 'Ljubazni fenjerdžija čađavog lica hoće da mi pokaže štos.';
const LACKS = ['ž', 'č', 'đ', 'ć', 'š'];

const answer = (over: Partial<SampleText>): SampleText => ({
  face: 1,
  text: SERBIAN,
  fallback: null,
  missing: [],
  ...over,
});

describe('specimenRuns', () => {
  it('cuts every missing character into a run of its own', () => {
    expect(specimenRuns('ažb ž', ['ž'])).toEqual([
      { text: 'a', missing: false },
      { text: 'ž', missing: true },
      { text: 'b ', missing: false },
      { text: 'ž', missing: true },
    ]);
  });

  it('keeps the text whole when nothing is missing', () => {
    expect(specimenRuns('Ag', [])).toEqual([{ text: 'Ag', missing: false }]);
    expect(specimenRuns('', [])).toEqual([]);
  });

  it('cuts by code point, so a character outside the BMP is one box', () => {
    expect(specimenRuns('a𝔸', ['𝔸'])).toEqual([
      { text: 'a', missing: false },
      { text: '𝔸', missing: true },
    ]);
  });

  it('draws every character of the text, in order', () => {
    const runs = specimenRuns(SERBIAN, LACKS);
    expect(runs.map((r) => r.text).join('')).toBe(SERBIAN);
    expect(runs.filter((r) => r.missing).map((r) => r.text)).toEqual([
      'ž',
      'č',
      'đ',
      'ć',
      'ž',
      'š',
    ]);
  });
});

describe('specimenOf', () => {
  it('shows the sample until the answer arrives', () => {
    expect(specimenOf('Ag', undefined)).toEqual({
      text: 'Ag',
      fallback: null,
      missing: [],
      tofu: [],
    });
  });

  it('shows a face that maps the sample as it is', () => {
    expect(specimenOf(SERBIAN, answer({}))).toMatchObject({ text: SERBIAN, tofu: [] });
  });

  it('draws the missing characters of a sample the API keeps as tofu', () => {
    const got = answer({ missing: LACKS });
    expect(specimenOf(SERBIAN, got)).toEqual({
      text: SERBIAN,
      fallback: null,
      missing: LACKS,
      tofu: LACKS,
    });
  });

  it('follows the API: a fallback answer is shown whole, never re-decided here', () => {
    // The sample has letters the face maps; the API still chose a fallback.
    const got = answer({
      text: 'abcdefghijklmnopqrstuvwxyz',
      fallback: { kind: 'script', script: 'Latn' },
      missing: LACKS,
    });
    expect(specimenOf(SERBIAN, got)).toEqual({
      text: 'abcdefghijklmnopqrstuvwxyz',
      fallback: { kind: 'script', script: 'Latn' },
      missing: LACKS,
      tofu: [],
    });
  });

  it('keeps the fallback text of a face that draws none of the sample (E4.5)', () => {
    const got = answer({
      text: 'абвгд',
      fallback: { kind: 'script', script: 'Cyrl' },
      missing: ['a', 'b'],
    });
    expect(specimenOf('ab ab.', got)).toEqual({
      text: 'абвгд',
      fallback: { kind: 'script', script: 'Cyrl' },
      missing: ['a', 'b'],
      tofu: [],
    });
  });

  it('with keepSample shows the sample as tofu even when the face draws none of it', () => {
    const got = answer({
      text: 'абвгд',
      fallback: { kind: 'script', script: 'Cyrl' },
      missing: ['a', 'b'],
    });
    expect(specimenOf('ab', got, true)).toMatchObject({
      text: 'ab',
      fallback: null,
      tofu: ['a', 'b'],
    });
  });
});

describe('missingLine', () => {
  it('counts the missing glyphs and lists them', () => {
    expect(missingLine(['š', 'đ', 'č'])).toBe('3 glyphs missing · š đ č');
    expect(missingLine(['ž'])).toBe('1 glyph missing · ž');
  });

  it('lists eight and an ellipsis for more', () => {
    const many = [...'abcdefghij'];
    expect(missingLine(many)).toBe('10 glyphs missing · a b c d e f g h …');
  });

  it('names one box "missing glyph ž"', () => {
    expect(tofuLabel('ž')).toBe('missing glyph ž');
  });
});
