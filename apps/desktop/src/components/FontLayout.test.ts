import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { MOCK_LAYOUT_PRESETS, MOCK_WATERFALL_SIZES_PT, mockFaces } from '../api/mock';
import type { Layout, LayoutPreset } from '../api/types';
import { FaceLoader, setFaceLoader, type FontFaceSetLike } from '../preview/faceLoader';
import FontLayout from './FontLayout.vue';

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
  readonly faces = new Set<FakeFace>();
  add(f: FakeFace) {
    this.faces.add(f);
  }
  delete(f: FakeFace) {
    return this.faces.delete(f);
  }
}

let fonts: FakeSet;
let loader: FaceLoader<FakeFace>;

beforeEach(() => {
  fonts = new FakeSet();
  loader = new FaceLoader<FakeFace>({
    fonts,
    create: (family, source) => new FakeFace(family, source),
    url: (face) => `fmfont://localhost/${face}`,
    maxIdle: 0,
  });
  setFaceLoader(loader);
});

afterEach(() => setFaceLoader(undefined));

const inter = mockFaces[0]!.face;
const mono = mockFaces[1]!.face;
const preset = (p: LayoutPreset): Layout => MOCK_LAYOUT_PRESETS.find((l) => l.preset === p)!;

async function show(p: LayoutPreset, face = inter) {
  const w = mount(FontLayout, { props: { layout: preset(p), face }, attachTo: document.body });
  await flushPromises();
  return w;
}

const blocks = (w: Awaited<ReturnType<typeof show>>) => w.findAll('[data-testid="layout-block"]');
const style = (w: Awaited<ReturnType<typeof show>>, i: number) =>
  (blocks(w)[i]!.element as HTMLElement).style;

describe('FontLayout', () => {
  it('sets every block as live text in the face from the face loader, not an image', async () => {
    const w = await show('book');
    expect(w.attributes('data-state')).toBe('ready');
    expect(loader.inUse).toBe(1);
    expect([...fonts.faces].map((f) => f.source)).toEqual([
      `url("fmfont://localhost/${inter.id}")`,
    ]);
    for (let i = 0; i < blocks(w).length; i++)
      expect(style(w, i).fontFamily).toContain(`fm-face-${inter.id}`);
    expect(w.find('img').exists()).toBe(false);
    expect(w.find('canvas').exists()).toBe(false);
    expect(w.text()).toContain(inter.familyName);
    w.unmount();
    expect(loader.inUse).toBe(0);
  });

  it('Book: a centered heading and justified paragraphs with a first-line indent', async () => {
    const w = await show('book');
    expect(blocks(w)).toHaveLength(2);
    expect(style(w, 0).textAlign).toBe('center');
    expect(style(w, 0).fontSize).toBe('24pt');
    expect(style(w, 1).textAlign).toBe('justify');
    const paragraphs = blocks(w)[1]!.findAll('p');
    expect(paragraphs.length).toBeGreaterThan(1);
    expect((paragraphs[0]!.element as HTMLElement).style.textIndent).toBe('1.5em');
  });

  it('Columns: body text flows into 2–3 balanced columns', async () => {
    const w = await show('columns');
    const body = blocks(w)[1]!;
    const columns = Number(body.attributes('data-columns'));
    expect(columns).toBeGreaterThanOrEqual(2);
    expect(columns).toBeLessThanOrEqual(3);
    expect((body.element as HTMLElement).style.columnCount).toBe(String(columns));
    expect(body.findAll('p').length).toBeGreaterThan(1);
    // The heading spans one column.
    expect(style(w, 0).columnCount).toBe('');
  });

  it('Magazine: a large tight headline, a standfirst and two-column body text', async () => {
    const w = await show('magazine');
    expect(blocks(w)).toHaveLength(3);
    expect(style(w, 0).fontSize).toBe('48pt');
    expect(style(w, 0).letterSpacing).toBe('-0.02em');
    expect(style(w, 1).fontSize).toBe('16pt');
    expect(style(w, 2).columnCount).toBe('2');
  });

  it('Quote: a large centered quote and a tracked attribution', async () => {
    const w = await show('quote');
    expect(blocks(w)).toHaveLength(2);
    expect(style(w, 0).fontSize).toBe('36pt');
    expect(style(w, 0).textAlign).toBe('center');
    expect(style(w, 1).letterSpacing).toBe('0.1em');
    expect(blocks(w)[1]!.text()).toBe('Matthew Carter');
  });

  it('Characters: the upper case, lower case and figures as separate blocks', async () => {
    const w = await show('characters');
    expect(blocks(w).map((b) => b.text())).toEqual([
      'ABCDEFGHIJKLMNOPQRSTUVWXYZ',
      'abcdefghijklmnopqrstuvwxyz',
      '0123456789 &@#%?!',
    ]);
    expect((blocks(w)[0]!.find('p').element as HTMLElement).style.marginBottom).toBe('12pt');
  });

  it('Waterfall: the same text at 6–8 falling sizes, each clipped to one line', async () => {
    const w = await show('waterfall');
    const sizes = blocks(w).map((b) => Number(b.attributes('data-size')));
    expect(sizes.length).toBeGreaterThanOrEqual(6);
    expect(sizes.length).toBeLessThanOrEqual(8);
    expect(sizes).toEqual(MOCK_WATERFALL_SIZES_PT);
    expect(new Set(blocks(w).map((b) => b.text())).size).toBe(1);
    for (let i = 0; i < sizes.length; i++) expect(style(w, i).fontSize).toBe(`${sizes[i]}pt`);
  });

  it('a clip block hides overflow on one line; a wrap block wraps', async () => {
    const w = await show('waterfall');
    const clip = blocks(w)[0]!;
    expect(clip.attributes('data-overflow')).toBe('clip');
    expect(clip.classes()).toContain('clip');
    const book = await show('book');
    const wrap = blocks(book)[1]!;
    expect(wrap.attributes('data-overflow')).toBe('wrap');
    expect(wrap.classes()).not.toContain('clip');
  });

  it('switching the preset keeps the face loaded; a new face swaps it', async () => {
    const w = await show('book');
    await w.setProps({ layout: preset('waterfall') });
    await flushPromises();
    expect(loader.inUse).toBe(1);
    expect(w.attributes('data-preset')).toBe('waterfall');
    expect(style(w, 0).fontFamily).toContain(`fm-face-${inter.id}`);
    await w.setProps({ face: mono });
    await flushPromises();
    expect(loader.inUse).toBe(1);
    expect(style(w, 0).fontFamily).toContain(`fm-face-${mono.id}`);
  });

  it('asks for a font when nothing is selected', async () => {
    const w = mount(FontLayout, { props: { layout: preset('book'), face: null } });
    expect(w.text()).toContain('Select a font');
    expect(loader.inUse).toBe(0);
  });
});
