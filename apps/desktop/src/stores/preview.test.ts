import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it } from 'vitest';
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

function freshStore() {
  setActivePinia(createPinia());
  return usePreviewStore();
}

beforeEach(() => localStorage.clear());

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

  it('falls back to the defaults for a damaged or invalid saved entry', () => {
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
    expect(freshStore().settingsFor('all')).toEqual(DEFAULT_PREVIEW);
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
});
