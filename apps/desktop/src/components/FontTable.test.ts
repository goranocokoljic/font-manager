import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, type MockOptions } from '../api/mock';
import { FaceLoader, setFaceLoader, type FontFaceSetLike } from '../preview/faceLoader';
import { useFontListStore } from '../stores/fontList';
import { useLibraryStore } from '../stores/library';
import { ALL_FONTS_VIEW, DEFAULT_PREVIEW } from '../stores/preview';
import { useViewsStore } from '../stores/views';
import FontTable from './FontTable.vue';
import PreviewStrip from './PreviewStrip.vue';
import TableColumnsMenu from './TableColumnsMenu.vue';

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

beforeEach(() => {
  localStorage.clear();
  setActivePinia(createPinia());
  setFaceLoader(
    new FaceLoader<FakeFace>({
      fonts: new FakeSet(),
      create: (family, source) => new FakeFace(family, source),
      url: (face) => `fmfont://localhost/${face}`,
    }),
  );
});

afterEach(() => {
  setFaceLoader(undefined);
  setClient(undefined);
  document.body.innerHTML = '';
});

let client: FmClient | undefined;

/** Loads a mock library (in the view's API order) and mounts the table on it. */
async function tableOf(options: MockOptions = {}) {
  client = createMockClient(options);
  setClient(client);
  const views = useViewsStore();
  views.setMode(ALL_FONTS_VIEW, 'table');
  const library = useLibraryStore();
  await library.setOrder(views.orderFor(ALL_FONTS_VIEW));
  await library.load();
  const table = mount(FontTable, { props: { view: ALL_FONTS_VIEW }, attachTo: document.body });
  await flushPromises();
  return table;
}

/** Re-applies the view's order (HomeView watches it) and waits for the families. */
async function applyOrder() {
  await useLibraryStore().setOrder(useViewsStore().orderFor(ALL_FONTS_VIEW));
  await flushPromises();
}

const headers = (t: VueWrapper) => t.findAll('[role="columnheader"]').map((h) => h.text());
const rows = (t: VueWrapper) => t.findAll('.spacer [role="row"]');
const cell = (row: VueWrapper | ReturnType<VueWrapper['find']>, column: string) =>
  row.find(`[role="gridcell"][data-column="${column}"]`);
const header = (t: VueWrapper, column: string) =>
  t.get(`[role="columnheader"][data-column="${column}"]`);
const nameOf = (row: VueWrapper | ReturnType<VueWrapper['find']>) =>
  cell(row, 'name').get('.text').text();
const names = (t: VueWrapper) => rows(t).map(nameOf);

describe('FontTable', () => {
  it("shows the spec's columns with the API's values", async () => {
    const t = await tableOf();
    expect(headers(t).map((h) => h.replace(/[▲▼]/, '').trim())).toEqual([
      'Activation',
      'Name',
      'Styles',
      'Style',
      'Weight',
      'Format',
      'Version',
      'Manufacturer',
      'Location',
      'License',
      'Date added',
      'Tags',
      'Notes',
    ]);
    const inter = rows(t)[0]!;
    expect(inter.attributes('aria-label')).toBe('Inter, 1 style, not active');
    expect(cell(inter, 'activation').text()).toBe('Inactive');
    expect(cell(inter, 'styleCount').text()).toBe('1');
    expect(cell(inter, 'weightRange').text()).toBe('100–900'); // the wght axis
    // The weight line: one dot at 400, a span for the variable range.
    expect(cell(inter, 'weightLine').findAll('.mark')).toHaveLength(1);
    expect(cell(inter, 'weightLine').find('.mark').attributes('style')).toContain('left: 33.3');
    expect(cell(inter, 'weightLine').find('.span').exists()).toBe(true);
    expect(
      cell(inter, 'format')
        .findAll('.badge')
        .map((b) => b.text()),
    ).toEqual(['TT', 'Variable']);
    expect(cell(inter, 'version').text()).toBe('Version 1.000');
    expect(cell(inter, 'manufacturer').text()).toBe('rsms');
    expect(cell(inter, 'location').text()).toBe('/Users/demo/Fonts');
    expect(cell(inter, 'license').text()).toBe('OFL');
    expect(cell(inter, 'added').text()).toMatch(/2026/);
    // Legacy Grotesk: Type 1, no open license.
    const legacy = rows(t)[2]!;
    expect(cell(legacy, 'format').text()).toBe('T1');
    expect(cell(legacy, 'license').text()).toBe('');
  });

  it('shows the tags of a family, A to Z, in its own Tags cell', async () => {
    // Values no other cell on the row has, so a cell bound to another field cannot pass.
    const t = await tableOf({ faceTags: { 6: ['zeta-tag', 'Alpha-tag'] } });
    const mono = rows(t).find((r) => nameOf(r) === 'JetBrains Mono')!;
    expect(cell(mono, 'tags').text()).toBe('Alpha-tag, zeta-tag');
    expect(
      rows(t)
        .filter((r) => nameOf(r) !== 'JetBrains Mono')
        .map((r) => cell(r, 'tags').text()),
    ).toEqual(['', '']);
    expect(
      mono.findAll('[role="gridcell"]').filter((c) => c.text().includes('zeta-tag')),
    ).toHaveLength(1);
  });

  it("shows a family's own notes in the Notes cell, updated by the library event", async () => {
    const t = await tableOf();
    const mono = () => rows(t).find((r) => nameOf(r) === 'JetBrains Mono')!;
    expect(cell(mono(), 'notes').text()).toBe('');
    // The list hears of the change through `libraryChanged`; nothing polls.
    await client!.updateFamilyNotes({
      family: useLibraryStore().families.find((f) => f.name === 'JetBrains Mono')!.id,
      notes: 'Code samples in the handbook',
    });
    await flushPromises();
    expect(cell(mono(), 'notes').text()).toBe('Code samples in the handbook');
    expect(
      mono()
        .findAll('[role="gridcell"]')
        .filter((c) => c.text().includes('Code samples')),
    ).toHaveLength(1);
    expect(
      rows(t)
        .filter((r) => nameOf(r) !== 'JetBrains Mono')
        .map((r) => cell(r, 'notes').text()),
    ).toEqual(['', '']);
  });

  it('does not sort by the Notes column', async () => {
    const t = await tableOf();
    const notes = header(t, 'notes');
    expect(notes.find('button').exists()).toBe(false);
    expect(notes.attributes('aria-sort')).toBeUndefined();
    expect(notes.text()).toBe('Notes');
  });

  it('draws italic styles as hollow dots on the weight line', async () => {
    const t = await tableOf({ extraFaces: 1, stylesPerFamily: 4 });
    const sample = rows(t).find((r) => nameOf(r) === 'Sample 1')!;
    const marks = cell(sample, 'weightLine').findAll('.mark');
    expect(marks).toHaveLength(4);
    expect(marks.filter((m) => m.classes('italic'))).toHaveLength(2);
  });

  it('sorts by a header click: ascending, descending, then the default', async () => {
    const t = await tableOf();
    expect(names(t)).toEqual(['Inter', 'JetBrains Mono', 'Legacy Grotesk']);
    await header(t, 'manufacturer').get('button').trigger('click');
    await applyOrder();
    expect(header(t, 'manufacturer').attributes('aria-sort')).toBe('ascending');
    expect(names(t)).toEqual(['JetBrains Mono', 'Legacy Grotesk', 'Inter']);
    await header(t, 'manufacturer').get('button').trigger('click');
    await applyOrder();
    expect(header(t, 'manufacturer').attributes('aria-sort')).toBe('descending');
    expect(names(t)).toEqual(['Inter', 'Legacy Grotesk', 'JetBrains Mono']);
    await header(t, 'manufacturer').get('button').trigger('click');
    await applyOrder();
    expect(header(t, 'manufacturer').attributes('aria-sort')).toBe('none');
    expect(names(t)).toEqual(['Inter', 'JetBrains Mono', 'Legacy Grotesk']);
  });

  it('reorders columns by dragging a header onto another', async () => {
    const t = await tableOf();
    const data = new Map<string, string>();
    const dataTransfer = {
      setData: (k: string, v: string) => data.set(k, v),
      getData: (k: string) => data.get(k) ?? '',
      effectAllowed: 'none',
    };
    await header(t, 'tags').trigger('dragstart', { dataTransfer });
    await header(t, 'name').trigger('dragover', { dataTransfer });
    await header(t, 'name').trigger('drop', { dataTransfer });
    expect(
      headers(t)
        .slice(0, 3)
        .map((h) => h.replace(/[▲▼]/, '').trim()),
    ).toEqual(['Activation', 'Tags', 'Name']);
    // The cells follow their header.
    expect(rows(t)[0]!.findAll('[role="gridcell"]')[1]!.attributes('data-column')).toBe('tags');
  });

  it('expands a family to its styles and moves with the list keys', async () => {
    const t = await tableOf({ extraFaces: 1, stylesPerFamily: 4 });
    const grid = t.get('[role="treegrid"]');
    await grid.trigger('keydown', { key: 'End' });
    await flushPromises();
    expect(useFontListStore().cursor).toBe('family:1000');
    expect(grid.attributes('aria-activedescendant')).toBe('fm-trow-family-1000');
    await grid.trigger('keydown', { key: 'ArrowRight' });
    await flushPromises();
    const styleRows = rows(t).filter((r) => r.attributes('aria-level') === '2');
    expect(styleRows.map(nameOf)).toEqual(['Regular', 'Italic', 'Bold', 'Bold Italic']);
    expect(cell(styleRows[1]!, 'weightRange').text()).toBe('400');
    expect(cell(styleRows[1]!, 'weightLine').find('.mark').classes()).toContain('italic');
    await grid.trigger('keydown', { key: 'ArrowDown', shiftKey: true });
    expect(useFontListStore().selectedCount).toBe(2);
  });

  it('emits the cursor face for the inspector, fetching the family faces', async () => {
    const t = await tableOf();
    await rows(t)[1]!.trigger('click');
    await flushPromises();
    expect(t.emitted('cursorFace')?.at(-1)).toEqual([6]);
  });

  it('renders only a window of rows for a long library', async () => {
    const t = await tableOf({ extraFaces: 5000 });
    expect(useLibraryStore().families).toHaveLength(5003);
    expect(rows(t).length).toBeLessThan(60);
    expect(t.get('[role="treegrid"]').attributes('aria-rowcount')).toBe('5004');
  });
});

describe('TableColumnsMenu', () => {
  it('hides, shows and moves columns by keyboard-operable controls', async () => {
    const t = await tableOf();
    const menu = mount(TableColumnsMenu, {
      props: { view: ALL_FONTS_VIEW },
      attachTo: document.body,
    });
    await menu.get('button').trigger('click');
    const box = menu.get(
      'input[type="checkbox"]#' + CSS.escape(menu.findAll('input')[8]!.element.id),
    );
    expect(menu.findAll('label')[8]!.text()).toBe('Location');
    await box.setValue(false);
    await flushPromises();
    expect(headers(t).join()).not.toContain('Location');
    // The name column cannot be hidden.
    expect(menu.findAll('input')[1]!.attributes('disabled')).toBeDefined();
    await menu.get('button[aria-label="Move Tags up"]').trigger('click');
    await flushPromises();
    const order = useViewsStore().tableFor(ALL_FONTS_VIEW).order;
    expect(order.indexOf('tags')).toBe(order.length - 3);
    expect(
      menu.get('button[aria-label="Move Activation up"]').attributes('disabled'),
    ).toBeDefined();
    await menu.get('.reset').trigger('click');
    expect(useViewsStore().tableFor(ALL_FONTS_VIEW).hidden).toEqual([]);
    await menu.get('.panel').trigger('keydown', { key: 'Escape' });
    expect(menu.find('.panel').exists()).toBe(false);
  });
});

describe('PreviewStrip', () => {
  async function stripOf(sizePx = 28) {
    await tableOf();
    return mount(PreviewStrip, {
      props: { view: ALL_FONTS_VIEW, preview: { ...DEFAULT_PREVIEW, sizePx } },
      attachTo: document.body,
    });
  }

  it('asks for a selection until there is a cursor', async () => {
    const strip = await stripOf();
    expect(strip.text()).toContain('Select a family to preview it here.');
  });

  it('shows the cursor family at the toolbar size, then at its own size', async () => {
    const strip = await stripOf(40);
    useFontListStore().click('family:3');
    await useLibraryStore().facesOf(3);
    await flushPromises();
    expect(strip.text()).toContain('JetBrains Mono');
    const slider = strip.get('input[type="range"]');
    expect((slider.element as HTMLInputElement).value).toBe('40');
    expect(strip.get('[style*="font-size"]').attributes('style')).toContain('40px');
    expect(strip.find('.reset').exists()).toBe(false);

    await slider.setValue('96');
    await flushPromises();
    expect(useViewsStore().tableFor(ALL_FONTS_VIEW).stripSizePx).toBe(96);
    expect(strip.get('output').text()).toBe('96 px');
    expect(strip.get('[style*="font-size"]').attributes('style')).toContain('96px');

    await strip.setProps({ preview: { ...DEFAULT_PREVIEW, sizePx: 20 } });
    expect(strip.get('output').text()).toBe('96 px'); // its own size, not the toolbar's

    await strip.get('.reset').trigger('click');
    expect(strip.get('output').text()).toBe('20 px');
  });
});
