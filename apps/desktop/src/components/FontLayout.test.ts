import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient } from '../api/client';
import {
  MOCK_LAYOUT_BOUNDS,
  MOCK_LAYOUT_PRESETS,
  MOCK_WATERFALL_SIZES_PT,
  createMockClient,
  mockClampLayout,
  mockFaces,
} from '../api/mock';
import type { Layout, LayoutBounds, LayoutPreset } from '../api/types';
import { useTextLayoutsStore } from '../stores/textLayouts';
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

afterEach(() => {
  setFaceLoader(undefined);
  setClient(undefined);
});

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

  it('an editable block opens its text in the same face and hands back the edit', async () => {
    const w = mount(FontLayout, {
      props: { layout: preset('quote'), face: inter, editable: true },
      attachTo: document.body,
    });
    await flushPromises();
    const second = blocks(w)[1]!;
    expect(second.attributes('aria-label')).toBe('Block 2');
    await second.trigger('dblclick');
    const box = w.find('[data-testid="layout-block-editor"]');
    expect(box.attributes('aria-label')).toBe('Block 2 text');
    expect((box.element as HTMLElement).style.fontFamily).toContain(`fm-face-${inter.id}`);
    expect(document.activeElement).toBe(box.element);
    await box.setValue('Someone else');
    await box.trigger('blur');
    expect(w.emitted('edit-text')).toEqual([[1, 'Someone else']]);

    // Escape drops the edit; an unchanged text emits nothing.
    await blocks(w)[0]!.trigger('keydown', { key: 'Enter' });
    const again = w.find('[data-testid="layout-block-editor"]');
    await again.setValue('Not kept');
    await again.trigger('keydown', { key: 'Escape' });
    expect(w.find('[data-testid="layout-block-editor"]').exists()).toBe(false);
    expect(w.emitted('edit-text')).toHaveLength(1);
    w.unmount();
  });

  it('a block that is not editable never opens a text box', async () => {
    const w = await show('book');
    await blocks(w)[0]!.trigger('dblclick');
    expect(w.find('[data-testid="layout-block-editor"]').exists()).toBe(false);
    expect(blocks(w)[0]!.attributes('tabindex')).toBeUndefined();
  });

  it('asks for a font when nothing is selected', async () => {
    const w = mount(FontLayout, { props: { layout: preset('book'), face: null } });
    expect(w.text()).toContain('Select a font');
    expect(loader.inUse).toBe(0);
  });
});

describe('FontLayout block properties (E12.3)', () => {
  /**
   * The store and the component on a client whose core may have its own, narrower bounds,
   * so a test proves the editor uses the bounds the core hands out and not ranges of its own.
   */
  async function editor(bounds: LayoutBounds = MOCK_LAYOUT_BOUNDS) {
    setActivePinia(createPinia());
    const client = createMockClient();
    client.layoutBounds = async () => structuredClone(bounds);
    client.clampLayout = async (l) => mockClampLayout(l, bounds);
    setClient(client);
    const store = useTextLayoutsStore();
    await store.open('magazine');
    await store.loadBounds();
    const w = mount(FontLayout, {
      props: {
        layout: store.draft,
        face: inter,
        editable: true,
        bounds: store.bounds,
        setProps: store.setBlockProps,
      },
      attachTo: document.body,
    });
    await flushPromises();
    return { w, store };
  }

  type W = Awaited<ReturnType<typeof editor>>['w'];
  const badges = (w: W) => w.findAll('[data-testid="block-size-badge"]');

  async function setField(w: W, name: string, value: string) {
    const f = w.get(`[data-testid="block-properties"] [name="${name}"]`);
    await f.setValue(value);
    await flushPromises();
    return f.element as HTMLInputElement | HTMLSelectElement;
  }

  it("shows a size badge beside each block that opens that block's editor", async () => {
    const { w, store } = await editor();
    expect(badges(w)).toHaveLength(store.draft!.blocks.length);
    expect(badges(w)[0]!.text()).toBe(`${store.draft!.blocks[0]!.sizePt} pt`);
    await badges(w)[1]!.trigger('click');
    const dialog = w.get('[data-testid="block-properties"]');
    expect(dialog.attributes('aria-label')).toBe('Block 2 properties');
    expect(badges(w)[1]!.attributes('aria-expanded')).toBe('true');
    w.unmount();
  });

  it('sets every property and the block shows it at once', async () => {
    const { w, store } = await editor();
    await badges(w)[2]!.trigger('click');
    await setField(w, 'sizePt', '30');
    await setField(w, 'lineHeight', '1.6');
    await setField(w, 'tracking', '40');
    await setField(w, 'paragraphSpacingPt', '9');
    await setField(w, 'firstLineIndentEm', '2');
    await setField(w, 'columns', '3');
    await setField(w, 'align', 'end');
    await setField(w, 'overflow', 'clip');
    expect(store.draft!.blocks[2]).toMatchObject({
      sizePt: 30,
      lineHeight: 1.6,
      tracking: 40,
      paragraphSpacingPt: 9,
      firstLineIndentEm: 2,
      columns: 3,
      align: 'end',
      overflow: 'clip',
    });
    const block = w.findAll('[data-testid="layout-block"]')[2]!;
    const st = (block.element as HTMLElement).style;
    expect(st.fontSize).toBe('30pt');
    expect(st.lineHeight).toBe('1.6');
    expect(st.letterSpacing).toBe('0.04em');
    expect(st.textAlign).toBe('end');
    expect(st.columnCount).toBe('3');
    expect(block.attributes('data-overflow')).toBe('clip');
    const p = (block.find('p').element as HTMLElement).style;
    expect(p.marginBottom).toBe('9pt');
    expect(p.textIndent).toBe('2em');
    expect(badges(w)[2]!.text()).toBe('30 pt');
    w.unmount();
  });

  it('clamps out-of-range input to the bounds the core hands out, not its own', async () => {
    const narrow: LayoutBounds = {
      ...MOCK_LAYOUT_BOUNDS,
      sizePt: { min: 10, max: 72, default: 16, step: 2, perPx: 1 },
      columns: { min: 1, max: 2 },
    };
    const { w, store } = await editor(narrow);
    await badges(w)[0]!.trigger('click');
    const size = w.get('[data-testid="block-properties"] [name="sizePt"]');
    expect(size.attributes()).toMatchObject({ min: '10', max: '72', step: '2' });
    expect(w.findAll('[name="columns"] option').map((o) => o.text())).toEqual(['1', '2']);
    const field = await setField(w, 'sizePt', '500');
    expect(store.draft!.blocks[0]!.sizePt).toBe(72);
    expect(field.value).toBe('72');
    // Again past the edge: the block stays at the edge and the field shows it, not "900".
    expect((await setField(w, 'sizePt', '900')).value).toBe('72');
    expect((await setField(w, 'sizePt', '1')).value).toBe('10');
    expect((await setField(w, 'sizePt', '')).value).toBe('16');
    w.unmount();
  });

  it('a change affects only the edited block', async () => {
    const { w, store } = await editor();
    const others = JSON.parse(JSON.stringify(store.draft!.blocks.filter((_, i) => i !== 1)));
    await badges(w)[1]!.trigger('click');
    await setField(w, 'sizePt', '44');
    await setField(w, 'align', 'justify');
    await setField(w, 'columns', '2');
    expect(store.draft!.blocks[1]).toMatchObject({ sizePt: 44, align: 'justify', columns: 2 });
    expect(store.draft!.blocks.filter((_, i) => i !== 1)).toEqual(others);
    const sizes = w.findAll('[data-testid="layout-block"]').map((b) => b.attributes('data-size'));
    expect(sizes[1]).toBe('44');
    expect(sizes.filter((_, i) => i !== 1)).toEqual(
      others.map((b: { sizePt: number }) => String(b.sizePt)),
    );
    w.unmount();
  });

  it('labels every control; Escape closes the editor and focuses the badge again', async () => {
    const { w } = await editor();
    await badges(w)[0]!.trigger('click');
    await flushPromises();
    const dialog = w.get('[data-testid="block-properties"]');
    for (const c of dialog.findAll('input, select')) {
      const id = c.attributes('id');
      expect(id).toBeTruthy();
      expect(dialog.find(`label[for="${id}"]`).text()).not.toBe('');
    }
    expect(document.activeElement).toBe(dialog.find('input').element);
    await dialog.trigger('keydown', { key: 'Escape' });
    await flushPromises();
    expect(w.find('[data-testid="block-properties"]').exists()).toBe(false);
    expect(document.activeElement).toBe(badges(w)[0]!.element);
    w.unmount();
  });

  it('shows no badge without the core bounds', async () => {
    const w = await show('book');
    expect(w.find('[data-testid="block-size-badge"]').exists()).toBe(false);
    w.unmount();
  });
});
