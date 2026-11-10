import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import {
  ALL_FONTS_VIEW,
  CUSTOM_TEXT_MAX,
  DEFAULT_PREVIEW,
  PREVIEW_STORAGE_KEY,
  RECENT_LANGUAGES_KEY,
  RECENT_LANGUAGES_MAX,
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

describe('preview presets (E41.2)', () => {
  it('each chip sets its layout and text; Word keeps the typed text', () => {
    const store = freshStore();
    store.update('all', { customText: 'Hamburg' });
    store.applyPreset('all', 'alphabet');
    expect(store.settingsFor('all')).toMatchObject({ preset: 'alphabet', sample: 'alphabet' });
    store.applyPreset('all', 'word');
    expect(store.settingsFor('all')).toMatchObject({
      preset: 'word',
      sample: 'custom',
      customText: 'Hamburg',
    });
    expect(store.settingsFor('collection-3').preset).toBe('word');
  });

  it('keeps the preset per view through a reload; an unknown one falls back', async () => {
    const store = freshStore();
    store.applyPreset('all', 'paragraph');
    store.update('other', { sizePx: 60 });
    await nextTick();
    const again = freshStore();
    expect(again.settingsFor('all').preset).toBe('paragraph');
    expect(again.settingsFor('other').preset).toBe('word');
    expect(normalizePreview({ preset: 'poster' }).preset).toBe('word');
    // A view saved before E41.2 has no preset.
    expect(normalizePreview({ sample: 'lorem' }).preset).toBe('word');
  });

  it('a chosen language goes first, once, and is remembered; the list has a limit', async () => {
    setClient(createMockClient());
    const store = freshStore();
    await store.loadPangrams();
    store.chooseLanguage('all', 'de');
    store.chooseLanguage('all', 'sr-Cyrl');
    store.chooseLanguage('all', 'de');
    expect(store.settingsFor('all')).toMatchObject({
      language: 'de',
      sample: 'pangram',
      preset: 'pangram',
    });
    expect(store.languagePicks.slice(0, 2).map((l) => l.code)).toEqual(['de', 'sr-Cyrl']);
    expect(store.languagePicks).toHaveLength(mockLanguages.length);
    expect(new Set(store.languagePicks.map((l) => l.code)).size).toBe(mockLanguages.length);

    const again = freshStore();
    await again.loadPangrams();
    expect(again.languagePicks[0]!.code).toBe('de');

    for (const code of mockLanguages.map((l) => l.code)) again.chooseLanguage('all', code);
    expect(again.recentLanguages).toHaveLength(RECENT_LANGUAGES_MAX);
  });

  it('ignores a damaged recent list and a recent language the API no longer has', async () => {
    setClient(createMockClient());
    localStorage.setItem(RECENT_LANGUAGES_KEY, '{oops');
    expect(freshStore().recentLanguages).toEqual([]);
    localStorage.setItem(RECENT_LANGUAGES_KEY, JSON.stringify(['xx', 42, 'de']));
    const store = freshStore();
    await store.loadPangrams();
    expect(store.recentLanguages).toEqual(['xx', 'de']);
    expect(store.languagePicks[0]!.code).toBe('de');
    expect(store.languagePicks.map((l) => l.code)).not.toContain('xx');
  });
});

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

  it('keeps auto size per view and reads it back after a reload', async () => {
    const store = freshStore();
    expect(store.settingsFor('all').autoSize).toBe('off');
    store.update('all', { autoSize: 'xHeight' });
    store.update('collection-2', { autoSize: 'capHeight' });
    await nextTick();
    const again = freshStore();
    expect(again.settingsFor('all').autoSize).toBe('xHeight');
    expect(again.settingsFor('collection-2').autoSize).toBe('capHeight');
    expect(again.settingsFor('collection-9').autoSize).toBe('off');
    expect(normalizePreview({ autoSize: 'ascender' }).autoSize).toBe('off');
  });

  it('keeps the metrics and outline toggles per view and reads them back after a reload', async () => {
    const store = freshStore();
    expect(store.settingsFor('all')).toMatchObject({ metrics: false, outline: false });
    store.update('all', { metrics: true });
    store.update('collection-2', { outline: true });
    await nextTick();
    const again = freshStore();
    expect(again.settingsFor('all')).toMatchObject({ metrics: true, outline: false });
    expect(again.settingsFor('collection-2')).toMatchObject({ metrics: false, outline: true });
    again.update('all', { metrics: false });
    expect(again.settingsFor('all').metrics).toBe(false);
    expect(normalizePreview({ metrics: 'yes', outline: 1 })).toMatchObject({
      metrics: false,
      outline: false,
    });
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

describe('the reading rows body (E43.1)', () => {
  it('defaults to 17 px / 1.45', () => {
    expect(normalizePreview({})).toMatchObject({ bodySizePx: 17, bodyLineHeight: 1.45 });
  });

  it('keeps a saved body size and line height inside their bounds', () => {
    expect(normalizePreview({ bodySizePx: 22, bodyLineHeight: 1.6 })).toMatchObject({
      bodySizePx: 22,
      bodyLineHeight: 1.6,
    });
    expect(normalizePreview({ bodySizePx: 4, bodyLineHeight: 9 })).toMatchObject({
      bodySizePx: 10,
      bodyLineHeight: 3,
    });
    expect(normalizePreview({ bodySizePx: 'big' }).bodySizePx).toBe(17);
  });
});

describe('the hide-missing switch (E43.3)', () => {
  it('is off by default and keeps only a boolean', () => {
    expect(normalizePreview({}).hideMissing).toBe(false);
    expect(normalizePreview({ hideMissing: true }).hideMissing).toBe(true);
    expect(normalizePreview({ hideMissing: 'yes' }).hideMissing).toBe(false);
  });

  it('is remembered with the view across a restart', async () => {
    localStorage.clear();
    setActivePinia(createPinia());
    usePreviewStore().update('all', { hideMissing: true });
    await nextTick();
    setActivePinia(createPinia());
    expect(usePreviewStore().settingsFor('all').hideMissing).toBe(true);
    expect(usePreviewStore().settingsFor('other').hideMissing).toBe(false);
  });
});

describe('the Letters preset (E49.7)', () => {
  it('sets its layout and the glyphs; the glyphs and the Word text live apart', () => {
    const store = freshStore();
    store.update('all', { customText: 'Hamburg' });
    store.applyPreset('all', 'letters');
    store.update('all', { letters: 'ROag' });
    expect(store.settingsFor('all')).toMatchObject({
      preset: 'letters',
      sample: 'letters',
      letters: 'ROag',
      customText: 'Hamburg',
    });
    store.applyPreset('all', 'word');
    expect(store.settingsFor('all')).toMatchObject({ sample: 'custom', customText: 'Hamburg' });
    store.applyPreset('all', 'letters');
    expect(store.settingsFor('all').letters).toBe('ROag');
  });

  it('keeps the glyphs and the preset per view through a reload, cleaned', async () => {
    const store = freshStore();
    store.applyPreset('all', 'letters');
    store.update('all', { letters: 'R O a g R' });
    await nextTick();
    const again = freshStore();
    expect(again.settingsFor('all')).toMatchObject({ preset: 'letters', letters: 'ROag' });
    expect(again.settingsFor('collection-3')).toMatchObject({ preset: 'word', letters: '' });
    expect(normalizePreview({ letters: 42 }).letters).toBe('');
    // A view saved before E49.7 has no letters.
    expect(normalizePreview({ preset: 'word' }).letters).toBe('');
  });

  it('the Language picker shows its pangram; Letters gets its glyphs back after', async () => {
    setClient(createMockClient());
    const store = freshStore();
    await store.loadPangrams();
    store.applyPreset('all', 'letters');
    store.update('all', { letters: 'ROag' });
    store.chooseLanguage('all', 'sr-Cyrl');
    expect(store.settingsFor('all')).toMatchObject({ preset: 'pangram', language: 'sr-Cyrl' });
    store.applyPreset('all', 'letters');
    expect(store.settingsFor('all')).toMatchObject({
      preset: 'letters',
      letters: 'ROag',
      language: 'sr-Cyrl',
    });
  });

  it('is a preview setting only: the search, its filters and order are left alone', async () => {
    const client = createMockClient();
    const search = vi.spyOn(client, 'search');
    setClient(client);
    const store = freshStore();
    store.applyPreset('all', 'letters');
    store.update('all', { letters: 'ROag' });
    await nextTick();
    expect(search).not.toHaveBeenCalled();
  });
});
