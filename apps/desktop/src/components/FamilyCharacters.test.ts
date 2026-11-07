import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { FaceDetail } from '../api/types';
import { useAxesStore } from '../stores/axes';
import { useFamilyPageStore } from '../stores/familyPage';
import { STYLE_COUNT_DELAY, useGlyphsStore } from '../stores/glyphs';
import { useGlyphCompareStore } from '../stores/glyphCompare';
import { useSelectionStore } from '../stores/selection';
import FamilyCharacters from './FamilyCharacters.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
const copyText = vi.hoisted(() => vi.fn());
vi.mock('../api/clipboard', () => ({ copyText }));

async function mountWith(face: number | null) {
  const pinia = createPinia();
  setActivePinia(pinia);
  const selection = useSelectionStore();
  await selection.select(face);
  const detail = selection.detail;
  if (detail) await useFamilyPageStore().show(detail.face.family);
  const wrapper = mount(FamilyCharacters, {
    props: { detail, faceStyle: { fontFamily: "'fm-test'" } },
    global: { plugins: [pinia], stubs: { GlyphCompare: true } },
    attachTo: document.body,
  });
  await flushPromises();
  return wrapper;
}

const card = (w: Awaited<ReturnType<typeof mountWith>>) => w.get('[aria-label="Selected glyph"]');

beforeEach(() => {
  setClient(createMockClient({ cjkFaces: [1] }));
  copyText.mockReset();
  copyText.mockResolvedValue(undefined);
});
afterEach(() => {
  setClient(undefined);
  document.body.innerHTML = '';
});

describe('the Characters section', () => {
  it('says so when no face has loaded', async () => {
    expect((await mountWith(null)).text()).toContain('Select a face');
  });

  it('renders the rows on screen only, for a face with 30k+ characters', async () => {
    const w = await mountWith(1);
    expect(useGlyphsStore().total).toBeGreaterThan(30_000);
    const cells = w.findAll('[role="gridcell"]');
    expect(cells.length).toBeGreaterThan(0);
    expect(cells.length).toBeLessThan(200);
    expect(Number(w.get('[role="grid"]').attributes('aria-rowcount'))).toBeGreaterThan(2500);
  });

  it('always lays the characters out in twelve columns', async () => {
    const w = await mountWith(1);
    expect(w.get('[role="grid"]').attributes('aria-colcount')).toBe('12');
    expect(w.findAll('[role="row"]')[0]!.findAll('[role="gridcell"]')).toHaveLength(12);
  });

  it('has a chip with its count for every block of the face', async () => {
    const w = await mountWith(1);
    const chips = w.get('[role="group"][aria-label="Blocks"]').findAll('button');
    const latin = chips.find((c) => c.text().startsWith('Basic Latin'));
    expect(latin?.text()).toBe(
      `Basic Latin ${useGlyphsStore().faceBlocks.find((b) => b.name === 'Basic Latin')?.count}`,
    );
    expect(chips.some((c) => c.text().startsWith('Hangul Syllables'))).toBe(true);
    expect(chips.every((c) => c.attributes('aria-pressed') === 'false')).toBe(true);
  });

  it('a chip lists its block only, and a second press lists every block again', async () => {
    const w = await mountWith(1);
    const glyphs = useGlyphsStore();
    const all = glyphs.total;
    const chip = () => w.findAll('.chip').find((c) => c.text().startsWith('Cyrillic'))!;
    await chip().trigger('click');
    await flushPromises();
    expect(glyphs.block).toBe('Cyrillic');
    expect(glyphs.total).toBeLessThan(all);
    expect(chip().attributes('aria-pressed')).toBe('true');
    // The other chips stay, with their counts of the whole face.
    expect(w.findAll('.chip').some((c) => c.text().startsWith('Basic Latin'))).toBe(true);
    await chip().trigger('click');
    await flushPromises();
    expect(glyphs.block).toBeNull();
    expect(glyphs.total).toBe(all);
    expect(chip().attributes('aria-pressed')).toBe('false');
  });

  it('labels each cell with its code point and name and draws it in the face', async () => {
    const w = await mountWith(1);
    const a = w.get('[data-code-point="65"]');
    expect(a.attributes('aria-label')).toBe('U+0041 A');
    expect(a.text()).toBe('A');
    expect(a.get('.char').attributes('style')).toContain('fm-test');
  });

  it('hides the cells until the face has loaded: never a fallback font', async () => {
    const w = await mountWith(1);
    await w.setProps({ faceStyle: undefined });
    expect(w.get('[data-code-point="65"] .char').classes()).toContain('loading');
  });

  it('the arrow keys move the cursor and the card follows', async () => {
    const w = await mountWith(1);
    const grid = w.get('[role="grid"]');
    await grid.trigger('keydown', { key: 'ArrowRight' });
    await grid.trigger('keydown', { key: 'ArrowRight' });
    await flushPromises();
    expect(grid.attributes('aria-activedescendant')).toBe('fm-glyph-1');
    expect(w.get('#fm-glyph-1').attributes('aria-selected')).toBe('true');
    expect(card(w).text()).toContain('U+0021');
    await grid.trigger('keydown', { key: 'ArrowDown' });
    await flushPromises();
    expect(grid.attributes('aria-activedescendant')).toBe('fm-glyph-13');
    await grid.trigger('keydown', { key: 'ArrowUp' });
    await flushPromises();
    expect(grid.attributes('aria-activedescendant')).toBe('fm-glyph-1');
    expect(card(w).text()).toContain('U+0021');
  });

  it('Enter selects the cursor glyph again after Escape closed the card', async () => {
    const w = await mountWith(1);
    const grid = w.get('[role="grid"]');
    await grid.trigger('keydown', { key: 'ArrowRight' });
    await flushPromises();
    await grid.trigger('keydown', { key: 'Escape' });
    expect(w.find('[aria-label="Selected glyph"]').exists()).toBe(false);
    await grid.trigger('keydown', { key: 'Enter' });
    await flushPromises();
    expect(w.find('[aria-label="Selected glyph"]').exists()).toBe(true);
  });

  it('End reaches a character on a page still on its way, and selects it', async () => {
    // Answers take real time, as over IPC: a mock that answers at once hides the race.
    const mock = createMockClient({ cjkFaces: [1] });
    setClient({
      ...mock,
      listGlyphs: async (req) => {
        await new Promise((resolve) => setTimeout(resolve, 5));
        return mock.listGlyphs(req);
      },
    });
    const w = await mountWith(1);
    const glyphs = useGlyphsStore();
    await vi.waitFor(() => expect(glyphs.item(0)).toBeDefined());
    await w.get('[role="grid"]').trigger('keydown', { key: 'End' });
    await vi.waitFor(() => expect(glyphs.selected).not.toBeNull());
    expect(glyphs.selected).toEqual(glyphs.item(glyphs.total - 1));
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

  it('keeps the card column with a hint before anything is selected', async () => {
    const w = await mountWith(1);
    expect(w.find('[aria-label="Selected glyph"]').exists()).toBe(false);
    expect(w.get('[data-testid="glyph-card-empty"]').text()).toContain('Select a character');
  });

  it('the card shows the glyph large, its name and its code point', async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="97"]').trigger('click');
    await flushPromises();
    const detail = card(w);
    expect(detail.get('[data-testid="glyph-big"]').text()).toBe('a');
    expect(detail.get('[data-testid="glyph-big"]').attributes('style')).toContain('fm-test');
    expect(detail.get('[aria-labelledby="fm-glyph-cp"]').text()).toBe('U+0061');
    expect(detail.get('[aria-labelledby="fm-glyph-name"]').text()).toBe('a');
  });

  it("the card's alternates draw the stylistic sets of GlyphDetail, live", async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="97"]').trigger('click');
    await flushPromises();
    const alternates = w.findAll('[data-testid="glyph-alternate"]');
    const set = alternates.find((a) => a.text().includes('Stylistic set 1'))!;
    expect(set.get('.alternate-char').text()).toBe('a');
    const style = set.get('.alternate-char').attributes('style');
    expect(style).toContain('ss01');
    expect(style).toContain('fm-test');
    // Small capitals replaces it too, but is not a stylistic alternate.
    expect(w.text()).not.toContain('Small capitals');
  });

  it('says so when a glyph has no alternates', async () => {
    const w = await mountWith(6);
    await w.get('[data-code-point="65"]').trigger('click');
    await flushPromises();
    expect(w.findAll('[data-testid="glyph-alternate"]')).toHaveLength(0);
    expect(card(w).text()).toContain('none');
  });

  it.each([
    [{ have: 2, total: 3 }, '2 of 3'],
    [{ have: 3, total: 3 }, 'all 3'],
    [{ have: 1, total: 1 }, '1 of 1'],
    [{ have: 0, total: 3 }, '0 of 3'],
  ])("the card's 'in styles' shows fm-core's answer %j as %s", async (count, text) => {
    setClient({ ...createMockClient({ cjkFaces: [1] }), glyphStyles: async () => count });
    const w = await mountWith(1);
    vi.useFakeTimers();
    try {
      await w.get('[data-code-point="65"]').trigger('click');
      const shown = () => card(w).get('[aria-labelledby="fm-glyph-styles"]').text();
      expect(shown()).toBe('loading…');
      await vi.advanceTimersByTimeAsync(STYLE_COUNT_DELAY);
      await flushPromises();
      expect(shown()).toBe(text);
    } finally {
      vi.useRealTimers();
    }
  });

  it("the card's 'in styles' says unknown when fm-core cannot answer", async () => {
    setClient({
      ...createMockClient({ cjkFaces: [1] }),
      glyphStyles: () => Promise.reject({ code: 'notFound', message: 'gone' }),
    });
    const w = await mountWith(1);
    vi.useFakeTimers();
    try {
      await w.get('[data-code-point="65"]').trigger('click');
      await vi.advanceTimersByTimeAsync(STYLE_COUNT_DELAY);
      await flushPromises();
      expect(card(w).get('[aria-labelledby="fm-glyph-styles"]').text()).toBe('unknown');
    } finally {
      vi.useRealTimers();
    }
  });

  it("the card's alternates draw both ends of each axis, live", async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="65"]').trigger('click');
    await flushPromises();
    const alternates = w.findAll('[data-testid="glyph-alternate"]');
    const end = (label: string) =>
      alternates.find((a) => a.text().includes(label))!.get('.alternate-char');
    expect(end('Weight 100').attributes('style')).toContain('"wght" 100');
    expect(end('Weight 900').attributes('style')).toContain('"wght" 900');
    expect(end('Weight 100').text()).toBe('A');
    expect(end('Weight 100').attributes('style')).toContain('fm-test');
  });

  it('an axis end keeps the other axes where the axis row moved them', async () => {
    const w = await mountWith(1);
    const detail = JSON.parse(JSON.stringify(useSelectionStore().detail!)) as FaceDetail;
    const opsz = { tag: 'opsz', name: 'Optical', min: 9, default: 14, max: 144 };
    detail.axes.push(opsz);
    await w.setProps({ detail });
    useAxesStore().set(1, opsz, 30);
    await w.get('[data-code-point="65"]').trigger('click');
    await flushPromises();
    const heavy = w
      .findAll('[data-testid="glyph-alternate"]')
      .find((a) => a.text().includes('Weight 900'))!;
    const style = heavy.get('.alternate-char').attributes('style');
    expect(style).toContain('"wght" 900');
    expect(style).toContain('"opsz" 30');
  });

  it('Copy glyph copies the character itself', async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="97"]').trigger('click');
    await flushPromises();
    const button = card(w)
      .findAll('button')
      .find((b) => b.text() === 'Copy glyph')!;
    await button.trigger('click');
    await flushPromises();
    expect(copyText).toHaveBeenCalledWith('a');
    expect(card(w).text()).toContain('Copied');
  });

  it('Compare with… opens the existing overlay on the glyph', async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="97"]').trigger('click');
    await flushPromises();
    await card(w)
      .findAll('button')
      .find((b) => b.text() === 'Compare with…')!
      .trigger('click');
    await flushPromises();
    const compare = useGlyphCompareStore();
    expect(compare.open).toBe(true);
    expect(compare.codePoint).toBe(97);
  });

  it('More details keeps the glyph id, advance width, block and features', async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="97"]').trigger('click');
    await flushPromises();
    const detail = card(w);
    expect(detail.find('[aria-labelledby="fm-glyph-advance"]').exists()).toBe(false);
    const more = detail.findAll('button').find((b) => b.text() === 'More details')!;
    expect(more.attributes('aria-expanded')).toBe('false');
    await more.trigger('click');
    expect(more.attributes('aria-expanded')).toBe('true');
    expect(detail.get('[aria-labelledby="fm-glyph-advance"]').text()).toMatch(/^\d[\d,]* units$/);
    expect(detail.get('[aria-labelledby="fm-glyph-block"]').text()).toBe('Basic Latin');
    const replaced = detail.get('ul[aria-labelledby="fm-glyph-replaced"]');
    expect(replaced.text()).toContain('smcp');
    expect(replaced.text()).toContain('Small capitals');
    expect(detail.text()).toContain('No feature produces this glyph.');
  });

  it('a feature in the details is copied by its tag', async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="97"]').trigger('click');
    await flushPromises();
    await card(w)
      .findAll('button')
      .find((b) => b.text() === 'More details')!
      .trigger('click');
    await w.get('button[aria-label="Copy feature tag: ss01"]').trigger('click');
    expect(copyText).toHaveBeenCalledWith('ss01');
  });

  it('Escape in the card closes it and returns to the grid', async () => {
    const w = await mountWith(1);
    const grid = w.get('[role="grid"]');
    await w.get('[data-code-point="65"]').trigger('click');
    await flushPromises();
    await card(w).trigger('keydown', { key: 'Escape' });
    expect(w.find('[aria-label="Selected glyph"]').exists()).toBe(false);
    expect(document.activeElement).toBe(grid.element);
  });
});
