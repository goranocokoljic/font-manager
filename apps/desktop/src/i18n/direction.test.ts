import { afterEach, describe, expect, it } from 'vitest';
import {
  FALLBACK_LOCALE,
  LOCALES,
  PSEUDO_RTL,
  direction,
  inlineKey,
  locale,
  resolveLocale,
  setLocale,
  t,
  textDirection,
} from '.';

afterEach(() => setLocale(FALLBACK_LOCALE));

describe('textDirection', () => {
  it('is rtl for languages written right to left, by language or by script', () => {
    for (const tag of ['ar', 'ar-EG', 'he', 'he-IL', 'iw', 'fa', 'ur-PK', 'yi', 'ckb', PSEUDO_RTL])
      expect(textDirection(tag), tag).toBe('rtl');
    for (const tag of ['en', 'de-AT', 'ja', 'zh-Hant-TW', 'tr', 'sr-Cyrl'])
      expect(textDirection(tag), tag).toBe('ltr');
    // The script subtag wins: Azerbaijani in Arabic letters, Punjabi in Gurmukhi.
    expect(textDirection('az-Arab')).toBe('rtl');
    expect(textDirection('ku-Latn')).toBe('ltr');
    expect(textDirection('pa_Arab_PK')).toBe('rtl');
  });

  it('every shipped catalog is left to right, so the pseudo-locale is how RTL is checked', () => {
    expect(LOCALES.map(textDirection)).toEqual(LOCALES.map(() => 'ltr'));
    expect(LOCALES).not.toContain(PSEUDO_RTL);
  });
});

describe('the right-to-left pseudo-locale', () => {
  it('is chosen only when the OS prefers it, and shows English in rtl chrome', () => {
    expect(resolveLocale('system', ['ar-xb', 'de'])).toBe(PSEUDO_RTL);
    expect(resolveLocale('de', [PSEUDO_RTL])).toBe('de');
    // Arabic has no catalog yet: English, left to right.
    expect(resolveLocale('system', ['ar-EG'])).toBe('en');
    setLocale(PSEUDO_RTL);
    expect(locale()).toBe(PSEUDO_RTL);
    expect(direction()).toBe('rtl');
    expect(t('common.close')).toBe('Close');
    setLocale('de');
    expect(direction()).toBe('ltr');
  });
});

describe('inlineKey', () => {
  it('swaps Left and Right in rtl chrome only', () => {
    expect(inlineKey('ArrowLeft')).toBe('ArrowLeft');
    expect(inlineKey('ArrowRight')).toBe('ArrowRight');
    setLocale(PSEUDO_RTL);
    expect(inlineKey('ArrowLeft')).toBe('ArrowRight');
    expect(inlineKey('ArrowRight')).toBe('ArrowLeft');
    expect(inlineKey('ArrowUp')).toBe('ArrowUp');
    expect(inlineKey('Home')).toBe('Home');
  });
});
