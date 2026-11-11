import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useLibraryStore } from '../stores/library';
import { useSearchStore } from '../stores/search';
import type { SearchRequest } from '../api/types';
import { SEARCH_DEBOUNCE_MS } from '../stores/search';
import { ALL_FONTS_VIEW, usePreviewStore } from '../stores/preview';
import { useViewsStore } from '../stores/views';
import { useFontListStore } from '../stores/fontList';
import WallCountLine from './WallCountLine.vue';

let wrapper: VueWrapper | undefined;

/** The line over a library whose search finds `total` families and knows Serbian (Latin). */
async function mountLine(total = 212) {
  const pinia = createPinia();
  setActivePinia(pinia);
  const client = createMockClient();
  const search = client.search.bind(client);
  client.search = async (req) => ({ ...(await search(req)), total });
  client.listLanguages = async () => [
    { code: 'sr-Latn', name: 'Serbian (Latin)', script: 'Latn', pangram: null, sampleGlyphs: null },
  ];
  setClient(client);
  await useLibraryStore().load();
  const store = useSearchStore();
  await store.loadLanguages();
  wrapper = mount(WallCountLine, {
    props: { view: ALL_FONTS_VIEW },
    global: { plugins: [pinia] },
    attachTo: document.body,
  });
  await flushPromises();
  return { w: wrapper, store };
}

afterEach(() => {
  wrapper?.unmount();
  wrapper = undefined;
  setClient(undefined);
});

describe('the count line above the wall', () => {
  it('reads the count, the filters and the sort', async () => {
    const { w, store } = await mountLine();
    store.setFilters({
      classification: ['sans'],
      supports: { kind: 'languages', codes: ['sr-Latn'] },
    });
    await flushPromises();
    expect(w.get('[role="status"]').text()).toBe('212 families · Sans serif · Serbian (Latin)');
    expect(w.get('.sort').text().replace(/\s+/g, ' ')).toBe('Sorted by name · change');
  });

  it('with several selected, reads "N selected" and how to select (E44.1)', async () => {
    const { w } = await mountLine();
    const list = useFontListStore();
    list.click(list.rows[0]!.key);
    await flushPromises();
    // One selected: the family count stays, and the row's name and state.
    expect(w.find('[data-testid="selected-count"]').exists()).toBe(false);
    list.click(list.rows[2]!.key, 'extend');
    await flushPromises();
    expect(w.get('[data-testid="selected-count"]').text()).toBe('3 selected');
    expect(w.get('.count').text()).toBe(
      '3 selected · Shift-click or drag a box to select · Esc clears',
    );
  });

  it('has no separators and no filter names with no filter', async () => {
    const { w } = await mountLine();
    expect(w.get('[role="status"]').text()).not.toContain('·');
    expect(w.get('[role="status"]').text()).toMatch(/famil(y|ies)$/);
  });

  it('follows the view sort, and names a text search best match until one is chosen', async () => {
    const { w, store } = await mountLine();
    const sort = () => w.get('.sort span').text();
    store.setText('sample');
    store.query = 'sample';
    await flushPromises();
    expect(sort()).toBe('Sorted by best match');
    useViewsStore().setSort(ALL_FONTS_VIEW, 'added', true);
    await flushPromises();
    expect(sort()).toBe('Sorted by date added, descending');
  });

  it('offers each API key once, Style and Weight being one', async () => {
    const { w } = await mountLine();
    await w.get('button.change').trigger('click');
    await flushPromises();
    const rows = w.findAll('.menu label').map((l) => l.text());
    expect(rows.filter((r) => r === 'Weight' || r === 'Style')).toHaveLength(1);
    expect(rows).toContain('Name');
    expect(rows).not.toContain('Notes');
  });

  it('marks the Weight key current for a table sorted by the Style column', async () => {
    const { w } = await mountLine();
    useViewsStore().sortBy(ALL_FONTS_VIEW, 'weightLine');
    await w.get('button.change').trigger('click');
    await flushPromises();
    const checked = w.findAll('.menu input[type="radio"]').filter((r) => {
      return (r.element as HTMLInputElement).checked;
    });
    expect(checked).toHaveLength(1);
    expect(checked[0]!.element.closest('label')!.textContent!.trim()).toBe('Weight');
  });

  it('closes on Escape and gives focus back to change', async () => {
    const { w } = await mountLine();
    await w.get('button.change').trigger('click');
    await flushPromises();
    await w.get('.menu input').trigger('keydown', { key: 'Escape' });
    await flushPromises();
    expect(w.find('.menu').exists()).toBe(false);
    expect(document.activeElement).toBe(w.get('button.change').element);
  });
});

describe('the count line with your own text (E43.1)', () => {
  const SERBIAN = 'Ljubazni fenjerdžija čađavog lica hoće da mi pokaže štos.';
  const ENGLISH = 'The quick brown fox jumps over the lazy dog, twice and then once more.';

  /** A search that finds 212 families, and 148 of them with a text-coverage rule on top. */
  async function mountOwn(rows = false) {
    // The preview settings are saved: start every test from the defaults.
    localStorage.clear();
    const pinia = createPinia();
    setActivePinia(pinia);
    const client = createMockClient();
    const search = client.search.bind(client);
    const asked: SearchRequest[] = [];
    client.search = async (req) => {
      asked.push(structuredClone(req));
      return { ...(await search(req)), total: req.rule ? 148 : 212 };
    };
    setClient(client);
    await useLibraryStore().load();
    wrapper = mount(WallCountLine, {
      props: { view: ALL_FONTS_VIEW, rows },
      global: { plugins: [pinia] },
      attachTo: document.body,
    });
    await flushPromises();
    return { w: wrapper, store: useSearchStore(), preview: usePreviewStore(), asked };
  }

  const type = async (preview: ReturnType<typeof usePreviewStore>, text: string) => {
    preview.update(ALL_FONTS_VIEW, { sample: 'custom', customText: text });
    await flushPromises();
    vi.advanceTimersByTime(SEARCH_DEBOUNCE_MS);
    await flushPromises();
  };

  afterEach(() => vi.useRealTimers());

  it('reads "N of M families render your text completely" for text past Basic Latin', async () => {
    vi.useFakeTimers();
    const { w, store, preview, asked } = await mountOwn();
    store.setFilters({ supports: { kind: 'languages', codes: ['sr-Latn'] } });
    await flushPromises();
    await type(preview, SERBIAN);
    expect(w.get('[role="status"]').text()).toBe('148 of 212 families render your text completely');
    expect(w.get('[data-testid="renders-text"]').text()).toBe('148 of 212 families');
    // The count is the same search with the text as a rule on top: the language filter stays.
    const cover = asked.filter((r) => r.rule).pop()!;
    expect(cover.filters).toEqual({ supports: { kind: 'languages', codes: ['sr-Latn'] } });
    expect(cover.rule).toEqual({
      op: 'and',
      rules: [{ kind: 'filters', filters: { supports: { kind: 'text', text: SERBIAN } } }],
    });
  });

  it('keeps the plain count for Basic Latin text, and asks for no cover count', async () => {
    vi.useFakeTimers();
    const { w, preview, asked } = await mountOwn();
    await type(preview, ENGLISH);
    expect(w.find('[data-testid="renders-text"]').exists()).toBe(false);
    expect(w.get('[role="status"]').text()).toMatch(/^\d+ families$/);
    expect(asked.filter((r) => r.rule).map((r) => JSON.stringify(r))).toEqual([]);
  });

  it('drops the cover count when the text goes back to Basic Latin', async () => {
    vi.useFakeTimers();
    const { w, preview } = await mountOwn();
    await type(preview, SERBIAN);
    expect(w.find('[data-testid="renders-text"]').exists()).toBe(true);
    await type(preview, ENGLISH);
    expect(w.find('[data-testid="renders-text"]').exists()).toBe(false);
  });

  it('counts only your own text, not a stock sample', async () => {
    vi.useFakeTimers();
    const { w, preview } = await mountOwn();
    preview.update(ALL_FONTS_VIEW, { sample: 'pangram', customText: SERBIAN });
    await flushPromises();
    vi.advanceTimersByTime(SEARCH_DEBOUNCE_MS);
    await flushPromises();
    expect(w.find('[data-testid="renders-text"]').exists()).toBe(false);
  });

  it('names the body size and line height of the reading rows, and only there', async () => {
    const { w, preview } = await mountOwn(true);
    expect(w.get('[data-testid="body-size"]').text()).toBe('Body size 17 px · line height 1.45');
    preview.update(ALL_FONTS_VIEW, { bodySizePx: 20, bodyLineHeight: 1.6 });
    await flushPromises();
    expect(w.get('[data-testid="body-size"]').text()).toBe('Body size 20 px · line height 1.6');
    await w.setProps({ rows: false });
    expect(w.find('[data-testid="body-size"]').exists()).toBe(false);
  });

  it('offers "Hide the N with missing glyphs" and hides them with a text rule (E43.3)', async () => {
    vi.useFakeTimers();
    const { w, store, preview, asked } = await mountOwn();
    store.setFilters({ supports: { kind: 'languages', codes: ['sr-Latn'] } });
    await flushPromises();
    await type(preview, SERBIAN);
    const toggle = w.get('[data-testid="hide-missing"]');
    expect(toggle.text()).toBe('Hide the 64 with missing glyphs');
    const sw = toggle.get('[role="switch"]');
    expect(sw.attributes('aria-checked')).toBe('false');

    asked.length = 0;
    await sw.trigger('click');
    await flushPromises();
    expect(sw.attributes('aria-checked')).toBe('true');
    // Remembered with the view, and the list search itself now carries the text rule.
    expect(preview.settingsFor(ALL_FONTS_VIEW).hideMissing).toBe(true);
    expect(store.hiding).toBe(true);
    const list = asked.find((r) => (r.page?.limit ?? 0) > 1)!;
    expect(list.filters).toEqual({ supports: { kind: 'languages', codes: ['sr-Latn'] } });
    expect(list.rule).toEqual({
      op: 'and',
      rules: [{ kind: 'filters', filters: { supports: { kind: 'text', text: SERBIAN } } }],
    });
    // The count still reads against every family, and the switch still names the hidden ones.
    expect(w.get('[role="status"]').text()).toBe('148 of 212 families render your text completely');
    expect(w.get('[data-testid="hide-missing"]').text()).toBe('Hide the 64 with missing glyphs');

    asked.length = 0;
    await sw.trigger('click');
    await flushPromises();
    expect(store.hiding).toBe(false);
    expect(asked.find((r) => (r.page?.limit ?? 0) > 1)?.rule).toBeUndefined();
  });

  it('shows no switch while no family misses a glyph, nor without text past Basic Latin', async () => {
    vi.useFakeTimers();
    localStorage.clear();
    const pinia = createPinia();
    setActivePinia(pinia);
    const client = createMockClient();
    const search = client.search.bind(client);
    client.search = async (req) => ({ ...(await search(req)), total: 212 });
    setClient(client);
    await useLibraryStore().load();
    wrapper = mount(WallCountLine, {
      props: { view: ALL_FONTS_VIEW },
      global: { plugins: [pinia] },
    });
    const preview = usePreviewStore();
    await type(preview, ENGLISH);
    expect(wrapper.find('[data-testid="hide-missing"]').exists()).toBe(false);
    await type(preview, SERBIAN);
    expect(wrapper.get('[data-testid="renders-text"]').text()).toBe('212 of 212 families');
    expect(wrapper.find('[data-testid="hide-missing"]').exists()).toBe(false);
  });

  it('a remembered switch hides from the start, and only while there is text to check', async () => {
    vi.useFakeTimers();
    const { store, preview, asked } = await mountOwn();
    preview.update(ALL_FONTS_VIEW, { hideMissing: true });
    await flushPromises();
    // Basic Latin: nothing to hide, no rule on the list.
    expect(store.hiding).toBe(false);
    await type(preview, SERBIAN);
    expect(store.hiding).toBe(true);
    expect(asked.filter((r) => (r.page?.limit ?? 0) > 1).pop()?.rule).toBeTruthy();
    asked.length = 0;
    await type(preview, ENGLISH);
    expect(store.hiding).toBe(false);
    expect(asked.find((r) => (r.page?.limit ?? 0) > 1)?.rule).toBeUndefined();
  });
});

describe('the likeness sort chip (E49.9)', () => {
  it("reads 'Sorted by R like Inter' with the glyph in the reference face, and removes it", async () => {
    const { w, store } = await mountLine();
    store.setLikeGlyph({ face: 3, codePoint: 82, glyph: 'R', family: 'Inter' });
    await flushPromises();
    expect(w.get('.sort').text().replace(/\s+/g, ' ')).toBe('Sorted by R like Inter · change');
    const glyph = w.get('[data-testid="like-glyph"]');
    expect(glyph.text()).toBe('R');
    expect(glyph.attributes('style')).toContain('fm-face-3');
    const chip = w.get('[data-testid="like-chip"]');
    expect(chip.attributes('aria-label')).toBe('Remove sort R like Inter');
    await chip.trigger('click');
    await flushPromises();
    expect(store.likeGlyph).toBeNull();
    expect(w.find('[data-testid="like-chip"]').exists()).toBe(false);
    expect(w.get('.sort').text().replace(/\s+/g, ' ')).toBe('Sorted by name · change');
    // The chip is gone: the focus is on the menu's button, not on the body.
    expect(document.activeElement).toBe(w.get('.change').element);
  });

  it('a sort chosen in the menu ends the likeness sort', async () => {
    const { w, store } = await mountLine();
    store.setLikeGlyph({ face: 3, codePoint: 82, glyph: 'R', family: 'Inter' });
    await flushPromises();
    await w.get('.change').trigger('click');
    await flushPromises();
    // No row is the current one: any of them changes the sort.
    expect(w.findAll('.menu input[type="radio"]').some((r) => r.element.matches(':checked'))).toBe(
      false,
    );
    await w.findAll('.menu input[type="radio"]')[0]!.setValue(true);
    await flushPromises();
    expect(store.likeGlyph).toBeNull();
  });
});
