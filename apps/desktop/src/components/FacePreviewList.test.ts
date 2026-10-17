import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient, EXTRA_ID_BASE, mockFaces } from '../api/mock';
import type { FaceSummary } from '../api/types';
import { FaceLoader, setFaceLoader, type FontFaceSetLike } from '../preview/faceLoader';
import { NUMERALS, PANGRAMS } from '../preview/sampleText';
import { clearFaceAxes } from '../preview/variations';
import { DEFAULT_PREVIEW, type PreviewSettings } from '../stores/preview';
import FacePreviewList from './FacePreviewList.vue';
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
  clearFaceAxes();
  setClient(createMockClient({ extraFaces: 1000 }));
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

  it('says so when the face cannot load', async () => {
    const face: FaceSummary = { ...mono, id: 404 };
    const row = mount(FontPreviewRow, { props: { face } });
    await flushPromises();
    expect(row.attributes('data-state')).toBe('failed');
    expect(row.text()).toContain('Preview not available');
  });

  it('releases its face when it unmounts or shows another face', async () => {
    const row = mount(FontPreviewRow, { props: { face: mono } });
    await flushPromises();
    await row.setProps({ face: inter });
    expect(loader.inUse).toBe(1);
    row.unmount();
    expect(loader.inUse).toBe(0);
  });
});

describe('FacePreviewList', () => {
  async function listOf1000() {
    const client = createMockClient({ extraFaces: 1000 });
    const details = await Promise.all(
      Array.from({ length: 1000 }, (_, i) => client.getFace(EXTRA_ID_BASE + i)),
    );
    const faces = details.map((d) => d.face);
    return mount(FacePreviewList, { props: { faces, rowHeight: 50, overscan: 2 } });
  }

  it('renders only the rows on screen', async () => {
    const list = await listOf1000();
    await flushPromises();
    const rows = list.findAll('[role="listitem"]');
    // 600 px fallback viewport / 50 px rows + 2 overscan below.
    expect(rows.length).toBe(14);
    expect(rows[0]!.attributes('aria-setsize')).toBe('1000');
    expect(list.get('.spacer').attributes('style')).toContain('height: 50000px');
  });

  it('does not leak font faces while 1,000 rows scroll past', async () => {
    const list = await listOf1000();
    const viewport = list.get('.viewport').element as HTMLElement;
    let most = 0;
    const seen = new Set<string>();
    for (let top = 0; top < 50000; top += 250) {
      viewport.scrollTop = top;
      await list.get('.viewport').trigger('scroll');
      await flushPromises();
      const rendered = list.findAll('[role="listitem"]').length;
      // Only the rows on screen hold a face; at most MAX_IDLE released ones stay loaded.
      expect(loader.inUse).toBe(rendered);
      expect(fonts.faces.size - loader.inUse).toBeLessThanOrEqual(MAX_IDLE);
      most = Math.max(most, fonts.faces.size);
      for (const f of fonts.faces) seen.add(f.family);
    }
    const rows = list.findAll('[role="listitem"]');
    expect(rows.at(-1)!.attributes('aria-posinset')).toBe('1000');
    expect(seen.size).toBe(1000);
    // 12 rows on screen + 2 overscan on each side.
    expect(most).toBeLessThanOrEqual(16 + MAX_IDLE);
    list.unmount();
    expect(loader.inUse).toBe(0);
    loader.trim();
    expect(fonts.faces.size).toBe(0);
  });
});

describe('FacePreviewList preview settings', () => {
  const faces = [inter, mono];
  const settings = (patch: Partial<PreviewSettings>): PreviewSettings => ({
    ...DEFAULT_PREVIEW,
    ...patch,
  });

  it('applies the sample, size, spacing, alignment and colors to every row', async () => {
    const list = mount(FacePreviewList, {
      props: {
        faces,
        preview: settings({
          sample: 'numerals',
          sizePx: 96,
          lineHeight: 1.5,
          trackingEm: 0.05,
          align: 'end',
          textColor: '#aa0000',
          backgroundColor: '#00ff00',
          surface: 'dark',
        }),
      },
    });
    await flushPromises();
    const samples = list.findAll('.sample');
    expect(samples).toHaveLength(2);
    for (const sample of samples) {
      const style = (sample.element as HTMLElement).style;
      expect(sample.text()).toBe(NUMERALS);
      expect(style.fontSize).toBe('96px');
      expect(style.lineHeight).toBe('1.5');
      expect(style.letterSpacing).toBe('0.05em');
      expect(style.textAlign).toBe('end');
      expect(style.color).toBe('rgb(170, 0, 0)');
    }
    const viewport = list.get('.viewport');
    expect((viewport.element as HTMLElement).style.backgroundColor).toBe('rgb(0, 255, 0)');
    expect(viewport.classes()).toContain('surface-dark');
    // 96 px × 1.5 + label and padding.
    expect(list.get('[role="listitem"]').attributes('style')).toContain('height: 174px');
  });

  it('updates all rows live when the settings change', async () => {
    const list = mount(FacePreviewList, { props: { faces, preview: settings({}) } });
    await flushPromises();
    expect(list.get('.viewport').classes()).not.toContain('surface-light');
    await list.setProps({
      preview: settings({ sample: 'pangram', language: 'el', sizePx: 12, surface: 'light' }),
    });
    for (const sample of list.findAll('.sample')) {
      expect(sample.text()).toBe(PANGRAMS.el.text);
      expect((sample.element as HTMLElement).style.fontSize).toBe('12px');
    }
    expect(list.get('.viewport').classes()).toContain('surface-light');
  });

  it('shows each face its own name for the font-name sample', async () => {
    const list = mount(FacePreviewList, {
      props: { faces, preview: settings({ sample: 'fontName' }) },
    });
    await flushPromises();
    const texts = list.findAll('.sample').map((s) => s.text());
    expect(texts).toEqual([
      `${inter.familyName} ${inter.style}`,
      `${mono.familyName} ${mono.style}`,
    ]);
    expect(texts[0]).not.toBe(texts[1]);
  });
});
