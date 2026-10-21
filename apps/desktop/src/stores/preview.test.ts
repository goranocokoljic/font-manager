import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import {
  ALL_FONTS_VIEW,
  CUSTOM_TEXT_MAX,
  DEFAULT_PREVIEW,
  PREVIEW_STORAGE_KEY,
  normalizePreview,
  previewRowHeight,
  usePreviewStore,
} from './preview';
import { createMockClient, mockLanguages } from '../api/mock';
import { setClient } from '../api/client';

function freshStore() {
  setActivePinia(createPinia());
  return usePreviewStore();
}

beforeEach(() => localStorage.clear());
afterEach(() => {
  vi.unstubAllGlobals();
  setClient(undefined);
});

/** The mock API's pangram for `code`. */
const pangramOf = (code: string) => mockLanguages.find((l) => l.code === code)!.pangram!;

describe('preview store', () => {
  it('starts every view at the defaults', () => {
    const store = freshStore();
    expect(store.settingsFor(ALL_FONTS_VIEW)).toEqual(DEFAULT_PREVIEW);
    expect(store.settingsFor('collection-7')).toEqual(DEFAULT_PREVIEW);
  });

  it('keeps the size between 8 and 300 px', () => {
    const store = freshStore();
    store.update('all', { sizePx: 2 });
    expect(store.settingsFor('all').sizePx).toBe(8);
    store.update('all', { sizePx: 999 });
    expect(store.settingsFor('all').sizePx).toBe(300);
    store.update('all', { sizePx: 72.4 });
    expect(store.settingsFor('all').sizePx).toBe(72);
  });

  it('keeps line height and tracking inside their bounds and steps', () => {
    const store = freshStore();
    store.update('all', { lineHeight: 1.149, trackingEm: 0.123 });
    expect(store.settingsFor('all').lineHeight).toBe(1.15);
    expect(store.settingsFor('all').trackingEm).toBe(0.12);
    store.update('all', { lineHeight: 10, trackingEm: -5 });
    expect(store.settingsFor('all').lineHeight).toBe(3);
    expect(store.settingsFor('all').trackingEm).toBe(-0.2);
  });

  it('saves settings per view and reads them back after a reload', async () => {
    const store = freshStore();
    store.update('all', { sample: 'numerals', sizePx: 120, surface: 'dark' });
    store.update('collection-7', { sample: 'fontName', align: 'center', textColor: '#AA0000' });
    await nextTick();

    const reloaded = freshStore();
    expect(reloaded.settingsFor('all')).toMatchObject({
      sample: 'numerals',
      sizePx: 120,
      surface: 'dark',
      align: 'start',
    });
    expect(reloaded.settingsFor('collection-7')).toMatchObject({
      sample: 'fontName',
      align: 'center',
      textColor: '#aa0000',
      sizePx: DEFAULT_PREVIEW.sizePx,
    });
  });

  it('resets one view without touching another', async () => {
    const store = freshStore();
    store.update('all', { sizePx: 50 });
    store.update('other', { sizePx: 60 });
    store.reset('all');
    await nextTick();
    const reloaded = freshStore();
    expect(reloaded.settingsFor('all')).toEqual(DEFAULT_PREVIEW);
    expect(reloaded.settingsFor('other').sizePx).toBe(60);
  });

  it('falls back to the defaults for a damaged or invalid saved entry', async () => {
    localStorage.setItem(PREVIEW_STORAGE_KEY, '{not json');
    expect(freshStore().settingsFor('all')).toEqual(DEFAULT_PREVIEW);
    localStorage.setItem(
      PREVIEW_STORAGE_KEY,
      JSON.stringify({
        all: {
          sample: 'poem',
          language: 'xx',
          sizePx: 'big',
          align: 'justify',
          textColor: 'red',
          backgroundColor: '#12345',
          surface: 'sepia',
        },
      }),
    );
    const store = freshStore();
    await store.loadPangrams();
    expect(store.settingsFor('all')).toEqual(DEFAULT_PREVIEW);
  });

  it('cuts very long custom text', () => {
    const s = normalizePreview({ customText: 'x'.repeat(CUSTOM_TEXT_MAX + 50) });
    expect(s.customText).toHaveLength(CUSTOM_TEXT_MAX);
  });

  it('sizes rows from the sample size and line height', () => {
    expect(previewRowHeight(DEFAULT_PREVIEW)).toBe(64);
    expect(previewRowHeight({ sizePx: 300, lineHeight: 1.5 })).toBe(480);
  });

  it('pads rows by the density and keeps the sample size the user chose', () => {
    expect(previewRowHeight(DEFAULT_PREVIEW, 'comfortable')).toBe(64);
    expect(previewRowHeight(DEFAULT_PREVIEW, 'compact')).toBe(54);
    // The sample line is the same at both densities; only the padding changes.
    const big = { sizePx: 120, lineHeight: 1.2 };
    expect(previewRowHeight(big, 'comfortable') - previewRowHeight(big, 'compact')).toBe(10);
  });

  describe('pangrams', () => {
    function uiLanguages(languages: string[]) {
      vi.stubGlobal('navigator', { ...navigator, languages, language: languages[0] });
    }

    it('lists only the languages the API returns with a pangram, in its order', async () => {
      setClient(createMockClient());
      const store = freshStore();
      await store.loadPangrams();
      const withPangram = mockLanguages.filter((l) => l.pangram);
      expect(withPangram.length).toBeLessThan(mockLanguages.length);
      expect(store.pangramLanguages.map((l) => l.code)).toEqual(withPangram.map((l) => l.code));
      expect(store.pangramText('sr-Cyrl')).toBe(pangramOf('sr-Cyrl'));
    });

    it('defaults to the UI language when it has a pangram', async () => {
      uiLanguages(['sr-RS', 'en-US']);
      const store = freshStore();
      await store.loadPangrams();
      expect(store.defaultLanguage).toBe('sr-Cyrl');
      expect(store.settingsFor(ALL_FONTS_VIEW)).toEqual({
        ...DEFAULT_PREVIEW,
        language: 'sr-Cyrl',
      });
      store.update('all', { sizePx: 40 });
      store.reset('all');
      expect(store.settingsFor('all').language).toBe('sr-Cyrl');
    });

    it('defaults to English when the UI language has no pangram', async () => {
      uiLanguages(['ko-KR']);
      const store = freshStore();
      await store.loadPangrams();
      expect(store.settingsFor(ALL_FONTS_VIEW).language).toBe('en');
    });

    it('still loads a choice saved from the old built-in list', async () => {
      uiLanguages(['de-DE']);
      localStorage.setItem(
        PREVIEW_STORAGE_KEY,
        JSON.stringify({ all: { ...DEFAULT_PREVIEW, sample: 'pangram', language: 'fr' } }),
      );
      const store = freshStore();
      expect(store.settingsFor('all').language).toBe('fr');
      await store.loadPangrams();
      expect(store.settingsFor('all').language).toBe('fr');
      expect(store.pangramText(store.settingsFor('all').language)).toBe(pangramOf('fr'));
    });

    it('shows the default language for a saved code that has no pangram', async () => {
      uiLanguages(['de-DE']);
      localStorage.setItem(
        PREVIEW_STORAGE_KEY,
        JSON.stringify({ all: { ...DEFAULT_PREVIEW, language: 'mt' } }),
      );
      const store = freshStore();
      await store.loadPangrams();
      expect(store.settingsFor('all').language).toBe('de');
      expect(store.pangramText('mt')).toBe(pangramOf('en'));
    });

    it('gives no pangram text before the languages load', () => {
      expect(freshStore().pangramText('en')).toBe('');
    });
  });
});
