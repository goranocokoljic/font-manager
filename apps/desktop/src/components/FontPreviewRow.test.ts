import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient, mockFaces } from '../api/mock';
import type { FaceSummary } from '../api/types';
import { FaceLoader, setFaceLoader, type FontFaceSetLike } from '../preview/faceLoader';
import { clearFaceAxes } from '../preview/variations';
import { useFeaturesStore } from '../stores/features';
import FontPreviewRow from './FontPreviewRow.vue';

class FakeFace {
  constructor(
    readonly family: string,
    readonly source: string,
  ) {}
  load() {
    return this.source.includes('/404') ? Promise.reject(new Error('404')) : Promise.resolve(this);
  }
}

class FakeSet implements FontFaceSetLike<FakeFace> {
  readonly faces = new Set<FakeFace>();
  add(f: FakeFace) {
    this.faces.add(f);
  }
  delete(f: FakeFace) {
    return this.faces.delete(f);
  }
}

const MAX_IDLE = 16;
let fonts: FakeSet;
let loader: FaceLoader<FakeFace>;

beforeEach(() => {
  setActivePinia(createPinia());
  clearFaceAxes();
  setClient(createMockClient());
  fonts = new FakeSet();
  loader = new FaceLoader<FakeFace>({
    fonts,
    create: (family, source) => new FakeFace(family, source),
    url: (face) => `fmfont://localhost/${face}`,
    maxIdle: MAX_IDLE,
  });
  setFaceLoader(loader);
});

afterEach(() => {
  setFaceLoader(undefined);
  setClient(undefined);
});

const inter = mockFaces[0]!.face; // variable, wght 100–900, default 400
const mono = mockFaces[1]!.face; // static

describe('FontPreviewRow', () => {
  it('renders the sample in the face registered for it', async () => {
    const row = mount(FontPreviewRow, { props: { face: mono } });
    await flushPromises();
    const sample = row.get('.sample').element as HTMLElement;
    expect(sample.style.fontFamily).toContain('fm-face-6');
    expect(sample.style.fontSize).toBe('28px');
    expect(row.attributes('data-state')).toBe('ready');
    expect([...fonts.faces].map((f) => f.source)).toEqual(['url("fmfont://localhost/6")']);
    expect(row.text()).toContain('JetBrains Mono Regular');
  });

  it('sets font-variation-settings from the axes of a variable face', async () => {
    const row = mount(FontPreviewRow, { props: { face: inter } });
    await flushPromises();
    expect(row.get('.sample').attributes('style')).toContain('font-variation-settings: "wght" 400');
    await row.setProps({ variations: [{ tag: 'wght', value: 700 }] });
    expect(row.get('.sample').attributes('style')).toContain('font-variation-settings: "wght" 700');
    expect(row.text()).toContain('Variable');
  });

  it('leaves a static face at normal variation settings', async () => {
    const row = mount(FontPreviewRow, { props: { face: mono } });
    await flushPromises();
    expect(row.get('.sample').attributes('style')).toContain('font-variation-settings: normal');
    expect(row.text()).not.toContain('Variable');
  });

  it('applies the features switched for its face, live', async () => {
    const row = mount(FontPreviewRow, { props: { face: inter } });
    const other = mount(FontPreviewRow, { props: { face: mono } });
    await flushPromises();
    const style = (r: typeof row) => (r.get('.sample').element as HTMLElement).style;
    expect(style(row).fontFeatureSettings).toBe('');
    const smcp = { tag: 'smcp', name: 'Small capitals', automatic: false, defaultOn: false };
    useFeaturesStore().set(inter.id, smcp, true);
    await flushPromises();
    expect(style(row).fontFeatureSettings).toBe('"smcp" 1');
    expect(style(other).fontFeatureSettings).toBe('');
    useFeaturesStore().reset(inter.id);
    await flushPromises();
    expect(style(row).fontFeatureSettings).toBe('');
  });

  it('says so when the face cannot load', async () => {
    const face: FaceSummary = { ...mono, id: 404 };
    const row = mount(FontPreviewRow, { props: { face } });
    await flushPromises();
    expect(row.attributes('data-state')).toBe('failed');
    expect(row.text()).toContain('Preview not available');
  });

  it('shows the label it is given instead of the face name', async () => {
    const row = mount(FontPreviewRow, {
      props: { face: mono, label: 'Bold Italic' },
      slots: { detail: '<span class="count">4 styles</span>' },
    });
    await flushPromises();
    expect(row.get('.label').text()).toContain('Bold Italic');
    expect(row.get('.label').text()).not.toContain('JetBrains Mono');
    expect(row.get('.count').text()).toBe('4 styles');
  });

  it('releases its face when it unmounts or shows another face', async () => {
    const row = mount(FontPreviewRow, { props: { face: mono } });
    await flushPromises();
    await row.setProps({ face: inter });
    expect(loader.inUse).toBe(1);
    row.unmount();
    expect(loader.inUse).toBe(0);
  });

  it('shows a small fallback hint when the text is a fallback', () => {
    const plain = mount(FontPreviewRow, { props: { face: mono, text: 'Hello' } });
    expect(plain.find('[data-testid="fallback-hint"]').exists()).toBe(false);
    expect(plain.get('.sample').attributes('data-fallback')).toBeUndefined();
    const script = mount(FontPreviewRow, {
      props: {
        face: mono,
        text: 'abcdefghijklmnopqrstuvwxyz',
        fallback: { kind: 'script', script: 'Latn' },
        missing: ['Ж', 'ф'],
      },
    });
    const hint = script.get('[data-testid="fallback-hint"]');
    expect(hint.text()).toBe('Fallback');
    expect(hint.attributes('title')).toBe('This font lacks Ж ф, so it shows its own letters.');
    expect(hint.attributes('aria-label')).toBe(hint.attributes('title'));
    expect(script.get('.sample').text()).toBe('abcdefghijklmnopqrstuvwxyz');
    expect(script.get('.sample').attributes('data-fallback')).toBe('script');
    const symbols = mount(FontPreviewRow, {
      props: {
        face: mono,
        text: '★ ☆',
        fallback: { kind: 'symbols' },
        missing: 'abcdefghij'.split(''),
      },
    });
    expect(symbols.get('[data-testid="fallback-hint"]').attributes('title')).toBe(
      'This font lacks a b c d e f g h …, so it shows its own characters.',
    );
  });
});
