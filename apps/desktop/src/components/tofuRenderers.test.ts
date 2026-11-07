// E43.4: Quick Look and the family page's style rows and rail (FamilySample) draw the
// characters a face lacks as tofu, from the API's `sampleTexts` answer.
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient, mockFaces } from '../api/mock';
import { FaceLoader, setFaceLoader, type FontFaceSetLike } from '../preview/faceLoader';
import { ALL_FONTS_VIEW, usePreviewStore } from '../stores/preview';
import FamilySample from './FamilySample.vue';
import QuickLookSheet from './QuickLookSheet.vue';

class FakeFace {
  constructor(
    readonly family: string,
    readonly source: string,
  ) {}
  load() {
    return Promise.resolve(this);
  }
}
class FakeSet implements FontFaceSetLike<FakeFace> {
  add() {}
  delete() {
    return true;
  }
}

// Legacy Grotesk maps basic Latin only; Inter maps Cyrillic too.
const legacy = mockFaces.find((f) => f.face.familyName === 'Legacy Grotesk')!.face;
const inter = mockFaces.find((f) => f.face.familyName === 'Inter')!.face;
const CYRILLIC = ['Љ', 'у', 'б', 'а', 'в'].map((c) => `missing glyph ${c}`);

beforeEach(() => {
  setActivePinia(createPinia());
  setClient(createMockClient());
  setFaceLoader(
    new FaceLoader<FakeFace>({
      fonts: new FakeSet(),
      create: (family, source) => new FakeFace(family, source),
      url: (f) => `fmfont://localhost/${f}`,
    }),
  );
});
afterEach(() => {
  setFaceLoader(undefined);
  setClient(undefined);
  document.body.innerHTML = '';
});

const labels = (el: Element) =>
  [...el.querySelectorAll('[role="img"]')].map((b) => b.getAttribute('aria-label'));

describe('Quick Look tofu (E43.4)', () => {
  it('draws the sample with boxes for the characters the face lacks, and counts them', async () => {
    usePreviewStore().update(ALL_FONTS_VIEW, { sample: 'custom', customText: 'Love Љубав' });
    const sheet = mount(QuickLookSheet, { props: { face: legacy }, attachTo: document.body });
    await flushPromises();
    const sample = sheet.get('.sample').element;
    expect(labels(sample)).toEqual(CYRILLIC);
    expect(sample.textContent?.trim()).toBe('Love');
    expect(sheet.get('[data-testid="missing-glyphs"]').text()).toBe('5 glyphs missing · Љ у б а в');
    // The alphabet line has its own boxes: the face lacks € of the symbols, not the letters.
    const alphabet = labels(sheet.get('.alphabet').element);
    expect(alphabet).toContain('missing glyph €');
    expect(alphabet.some((l) => /missing glyph [A-Za-z]$/.test(l ?? ''))).toBe(false);
    sheet.unmount();
  });

  it('draws no box and no count for a face that maps the sample', async () => {
    usePreviewStore().update(ALL_FONTS_VIEW, { sample: 'custom', customText: 'Love Љубав' });
    const sheet = mount(QuickLookSheet, { props: { face: inter }, attachTo: document.body });
    await flushPromises();
    expect(labels(sheet.get('.sample').element)).toEqual([]);
    expect(sheet.find('[data-testid="missing-glyphs"]').exists()).toBe(false);
    sheet.unmount();
  });
});

describe('FamilySample tofu (E43.4)', () => {
  it('draws the characters its face lacks as boxes', async () => {
    const row = mount(FamilySample, {
      props: { face: legacy.id, text: 'Love Љубав', variation: '' },
    });
    await flushPromises();
    expect(labels(row.element)).toEqual(CYRILLIC);
    expect(row.text()).toBe('Love');
  });

  it('draws a face that maps the text whole', async () => {
    const row = mount(FamilySample, {
      props: { face: inter.id, text: 'Love Љубав', variation: '' },
    });
    await flushPromises();
    expect(labels(row.element)).toEqual([]);
    expect(row.text()).toBe('Love Љубав');
  });
});
