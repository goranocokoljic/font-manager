import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useGlyphsStore } from '../stores/glyphs';
import { useSelectionStore } from '../stores/selection';
import InspectorGlyphs from './InspectorGlyphs.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

async function mountWith(face: number | null) {
  const pinia = createPinia();
  setActivePinia(pinia);
  await useSelectionStore().select(face);
  const wrapper = mount(InspectorGlyphs, { global: { plugins: [pinia] }, attachTo: document.body });
  await flushPromises();
  return wrapper;
}

beforeEach(() => setClient(createMockClient({ cjkFaces: [1] })));
afterEach(() => {
  setClient(undefined);
  document.body.innerHTML = '';
});

describe('inspector glyphs tab', () => {
  it('says so when nothing is selected', async () => {
    expect((await mountWith(null)).text()).toContain('Select a face');
  });

  it('renders the rows on screen only, for a face with 30k+ characters', async () => {
    const w = await mountWith(1);
    expect(useGlyphsStore().total).toBeGreaterThan(30_000);
    const cells = w.findAll('[role="gridcell"]');
    expect(cells.length).toBeGreaterThan(0);
    expect(cells.length).toBeLessThan(200);
    expect(Number(w.get('[role="grid"]').attributes('aria-rowcount'))).toBeGreaterThan(5000);
  });

  it('shows block headings and offers every block in the picker', async () => {
    const w = await mountWith(1);
    expect(w.get('[role="rowheader"]').text()).toContain('Basic Latin');
    const options = w.findAll('select option').map((o) => o.text());
    expect(options[0]).toBe('All blocks');
    expect(options.some((o) => o.startsWith('Hangul Syllables'))).toBe(true);
  });

  it('a block from the picker lists that block only', async () => {
    const w = await mountWith(1);
    await w.get('select').setValue('Cyrillic');
    await flushPromises();
    expect(w.findAll('[role="rowheader"]').map((h) => h.text())).toEqual([
      expect.stringContaining('Cyrillic'),
    ]);
  });

  it('labels each cell with its code point and name', async () => {
    const w = await mountWith(1);
    const a = w.get('[data-code-point="65"]');
    expect(a.attributes('aria-label')).toBe('U+0041 A');
    expect(a.text()).toBe('A');
  });

  it('arrow keys move the cursor and Enter selects the glyph', async () => {
    const w = await mountWith(1);
    const grid = w.get('[role="grid"]');
    await grid.trigger('keydown', { key: 'ArrowRight' });
    await grid.trigger('keydown', { key: 'ArrowRight' });
    expect(grid.attributes('aria-activedescendant')).toBe('fm-glyph-1');
    await grid.trigger('keydown', { key: 'ArrowDown' });
    const below = grid.attributes('aria-activedescendant');
    expect(below).not.toBe('fm-glyph-1');
    await grid.trigger('keydown', { key: 'ArrowUp' });
    expect(grid.attributes('aria-activedescendant')).toBe('fm-glyph-1');
    await grid.trigger('keydown', { key: 'Enter' });
    await flushPromises();
    expect(w.get('#fm-glyph-1').attributes('aria-selected')).toBe('true');
    expect(w.get('[aria-label="Selected glyph"]').text()).toContain('U+0021');
  });

  it('a click selects the glyph', async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="65"]').trigger('click');
    expect(w.get('[data-code-point="65"]').attributes('aria-selected')).toBe('true');
    expect(useGlyphsStore().selected?.name).toBe('A');
  });

  it('a search by code point selects the glyph it finds', async () => {
    const w = await mountWith(1);
    await w.get('input[type="search"]').setValue('U+0416');
    await flushPromises();
    const zhe = w.get('[data-code-point="1046"]');
    expect(zhe.attributes('aria-selected')).toBe('true');
    expect(w.get('[role="grid"]').attributes('aria-activedescendant')).toBe(zhe.attributes('id'));
  });

  it('says why when the face is broken', async () => {
    setClient({
      ...createMockClient(),
      listGlyphs: async () => ({
        items: [],
        total: 0,
        blocks: [],
        health: { kind: 'broken', reason: 'The cmap table is malformed.' },
        unicodeVersion: '17.0.0',
      }),
    });
    const w = await mountWith(1);
    expect(w.text()).toContain('The cmap table is malformed.');
  });
});
