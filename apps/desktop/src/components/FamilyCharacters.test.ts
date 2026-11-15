import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { FaceDetail } from '../api/types';
import { setFaceLoader, type FaceLoader } from '../preview/faceLoader';
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

/** Mounts again in a fresh app that keeps the saved view preferences. */
async function mountWithSaved(face: number) {
  document.body.innerHTML = '';
  return mountWith(face);
}

type Wrapper = Awaited<ReturnType<typeof mountWith>>;
const card = (w: Wrapper) => w.get('[aria-label="Selected glyph"]');
const hasCard = (w: Wrapper) => w.find('[aria-label="Selected glyph"]').exists();

/** Opens the popover's 'More details'. */
async function openMore(w: Wrapper) {
  await card(w)
    .findAll('button')
    .find((b) => b.text() === 'More details')!
    .trigger('click');
}

/** Picks JetBrains Mono (face 6) to compare with, as the overlay's picker does. */
async function compareWithMono() {
  const compare = useGlyphCompareStore();
  await compare.start(1, 97);
  await compare.pickFace(6);
  compare.close();
  await flushPromises();
  return compare;
}

beforeEach(() => {
  setClient(createMockClient({ cjkFaces: [1] }));
  localStorage.clear();
  copyText.mockReset();
  copyText.mockResolvedValue(undefined);
});
afterEach(() => {
  setClient(undefined);
  setFaceLoader(undefined);
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

  it('fills a row with 88 px cells by default, drawn at half the cell', async () => {
    const w = await mountWith(1);
    // The fallback width (720 px) holds 8 cells of at least 88 px: 90 px each.
    expect(w.get('[role="grid"]').attributes('aria-colcount')).toBe('8');
    const row = w.findAll('[role="row"]')[0]!;
    expect(row.findAll('[role="gridcell"]')).toHaveLength(8);
    expect(row.attributes('style')).toContain('repeat(auto-fill, minmax(88px, 1fr))');
    expect(row.attributes('style')).toContain('height: 90px');
    expect(row.attributes('style')).toContain('font-size: 45px');
  });

  it('the cell slider changes the cell size, and the view remembers it', async () => {
    const w = await mountWith(1);
    const slider = w.get<HTMLInputElement>('[data-testid="glyph-cell-size"]');
    expect(slider.attributes()).toMatchObject({ min: '48', max: '140', 'aria-label': 'Cell size' });
    expect(slider.element.value).toBe('88');
    await slider.setValue(48);
    expect(w.get('[role="grid"]').attributes('aria-colcount')).toBe('15');
    const row = w.findAll('[role="row"]')[0]!;
    expect(row.findAll('[role="gridcell"]')).toHaveLength(15);
    expect(row.attributes('style')).toContain('minmax(48px, 1fr)');
    expect(row.attributes('style')).toContain('font-size: 24px');
    w.unmount();
    const again = await mountWithSaved(1);
    expect(again.get('[role="grid"]').attributes('aria-colcount')).toBe('15');
  });

  it('draws the hairline grid by default, and the Grid switch turns it off and is remembered', async () => {
    const w = await mountWith(1);
    const grid = w.get('[role="grid"]');
    const toggle = w.get('[data-testid="glyph-grid-lines"]');
    expect(grid.classes()).toContain('lines');
    expect(toggle.attributes('aria-checked')).toBe('true');
    expect(toggle.attributes('aria-labelledby')).toBe('fm-glyph-lines');
    await toggle.trigger('click');
    expect(grid.classes()).not.toContain('lines');
    expect(toggle.attributes('aria-checked')).toBe('false');
    w.unmount();
    const again = await mountWithSaved(1);
    expect(again.get('[role="grid"]').classes()).not.toContain('lines');
  });

  it("puts each cell's code point at its corner, for hover", async () => {
    const w = await mountWith(1);
    const a = w.get('[role="gridcell"][aria-label="U+0041 A"]');
    expect(a.get('.cp').text()).toBe('U+0041');
    expect(a.get('.cp').attributes('aria-hidden')).toBe('true');
  });

  it("sizes the specimen from the face's ascender and descender, with four rulers", async () => {
    const w = await mountWith(1);
    // MockApi face 1: 1000 upm, ascender 900, descender -250, x-height 520; drawn at 96 px.
    const box = w.get('[data-testid="glyph-specimen"]');
    expect(box.attributes('style')).toContain('height: 110.4px');
    expect(box.get('.strut').attributes('style')).toContain('height: 86.4px');
    expect(box.get('.specimen-text').text()).toBe('Hamburgefonstiv gjpqy');
    const rulers = box.findAll('[data-ruler]');
    expect(rulers.map((r) => [r.attributes('data-ruler'), r.attributes('style')])).toEqual([
      ['ascender', 'top: 0px;'],
      ['xHeight', 'top: 36.48px;'],
      ['baseline', 'top: 86.4px;'],
      ['descender', 'top: 110.4px;'],
    ]);
    expect(rulers.map((r) => r.text())).toEqual(['ascender', 'x-height', 'baseline', 'descender']);
  });

  it('shrinks the specimen until its whole line fits the pane, and its box with it', async () => {
    // The line draws 10 px wide per px of font size; the pane leaves it 500 px.
    const rect = vi
      .spyOn(HTMLElement.prototype, 'getBoundingClientRect')
      .mockImplementation(function (this: HTMLElement) {
        const w = this.classList.contains('specimen-text')
          ? 10 * parseFloat(this.parentElement!.style.fontSize)
          : 0;
        return { width: w, height: 0, top: 0, left: 0, right: w, bottom: 0, x: 0, y: 0 } as DOMRect;
      });
    const client = vi
      .spyOn(HTMLElement.prototype, 'clientWidth', 'get')
      .mockImplementation(function (this: HTMLElement) {
        return this.classList.contains('specimen-line') ? 500 : 0;
      });
    try {
      const w = await mountWith(1);
      await flushPromises();
      const line = w.get('.specimen-line');
      expect(line.attributes('style')).toContain('font-size: 50px');
      // 1000 upm, ascender 900, descender -250 at 50 px: a 57.5 px box, baseline at 45 px.
      expect(w.get('[data-testid="glyph-specimen"]').attributes('style')).toContain(
        'height: 57.5px',
      );
      expect(w.get('.strut').attributes('style')).toContain('height: 45px');
    } finally {
      rect.mockRestore();
      client.mockRestore();
    }
  });

  it('shows no specimen when the metrics cannot place it', async () => {
    const w = await mountWith(1);
    const detail = {
      ...w.props('detail')!,
      metrics: { unitsPerEm: 0, ascender: 0, descender: 0, xHeight: null, capHeight: null },
    };
    await w.setProps({ detail });
    expect(w.find('[data-testid="glyph-specimen"]').exists()).toBe(false);
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
    expect(a.get('.char').text()).toBe('A');
    expect(a.get('.char').attributes('style')).toContain('fm-test');
  });

  it('hides the cells until the face has loaded: never a fallback font', async () => {
    const w = await mountWith(1);
    await w.setProps({ faceStyle: undefined });
    expect(w.get('[data-code-point="65"] .char').classes()).toContain('loading');
  });

  it('the arrow keys move the cursor, Enter opens the popover and it follows', async () => {
    const w = await mountWith(1);
    const grid = w.get('[role="grid"]');
    await grid.trigger('keydown', { key: 'ArrowRight' });
    await grid.trigger('keydown', { key: 'ArrowRight' });
    await flushPromises();
    expect(grid.attributes('aria-activedescendant')).toBe('fm-glyph-1');
    expect(w.get('#fm-glyph-1').attributes('aria-selected')).toBe('true');
    expect(hasCard(w)).toBe(false);
    await grid.trigger('keydown', { key: 'Enter' });
    await flushPromises();
    expect(card(w).text()).toContain('U+0021');
    await grid.trigger('keydown', { key: 'ArrowDown' });
    await flushPromises();
    expect(grid.attributes('aria-activedescendant')).toBe('fm-glyph-9'); // one row of 8 cells down
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
    expect(card(w).get('[data-testid="glyph-code"]').text()).toBe('U+0416');
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

  it('shows no popover and keeps the whole width for the grid before a click', async () => {
    const w = await mountWith(1);
    expect(hasCard(w)).toBe(false);
    expect(w.find('[data-testid="glyph-card-empty"]').exists()).toBe(false);
  });

  it('a click opens a 360 px popover with the glyph, U+code, Unicode and glyph names and feature chips', async () => {
    const w = await mountWith(1);
    const cell = w.get('[data-code-point="97"]');
    expect(cell.attributes('aria-haspopup')).toBe('dialog');
    await cell.trigger('click');
    await flushPromises();
    const detail = card(w);
    expect(detail.attributes('role')).toBe('dialog');
    expect(detail.attributes('style')).toContain('width: 360px');
    expect(detail.get('[data-testid="glyph-big"]').text()).toBe('a');
    expect(detail.get('[data-testid="glyph-big"]').attributes('style')).toContain('fm-test');
    expect(detail.get('[data-testid="glyph-code"]').text()).toBe('U+0061');
    expect(detail.get('[data-testid="glyph-names"]').text()).toBe('LATIN SMALL LETTER A · glyph a');
    const chips = detail.get('[data-testid="glyph-features"]');
    expect(chips.text()).toContain('smcp');
    expect(chips.text()).toContain('ss01');
  });

  it('a character without a Unicode name shows its glyph name only', async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="33"]').trigger('click');
    await flushPromises();
    expect(card(w).get('[data-testid="glyph-names"]').text()).toBe('glyph uni0021');
    expect(card(w).find('[data-testid="glyph-features"]').exists()).toBe(false);
  });

  it('the close button closes the popover and focus goes back to the cell', async () => {
    const w = await mountWith(1);
    const grid = w.get('[role="grid"]');
    await w.get('[data-code-point="65"]').trigger('click');
    await flushPromises();
    (document.body as HTMLElement).focus();
    await card(w).get('[data-testid="glyph-popover-close"]').trigger('click');
    expect(hasCard(w)).toBe(false);
    expect(document.activeElement).toBe(grid.element);
    expect(grid.attributes('aria-activedescendant')).toBe(
      w.get('[data-code-point="65"]').attributes('id'),
    );
  });

  it('a search hit opens no popover with Details on click off', async () => {
    const w = await mountWith(1);
    await w.get('[data-testid="glyph-details-on-click"]').trigger('click');
    await w.get('input[type="search"]').setValue('U+0416');
    await flushPromises();
    expect(w.get('[data-code-point="1046"]').attributes('aria-selected')).toBe('true');
    expect(hasCard(w)).toBe(false);
  });

  it('a press in a dialog (the compare overlay) leaves the popover open', async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="65"]').trigger('click');
    await flushPromises();
    const overlay = document.createElement('div');
    overlay.setAttribute('role', 'dialog');
    document.body.append(overlay);
    overlay.dispatchEvent(new Event('pointerdown', { bubbles: true }));
    await flushPromises();
    expect(hasCard(w)).toBe(true);
  });

  it('a press outside the popover closes it; a press inside or on a cell does not', async () => {
    const w = await mountWith(1);
    const grid = w.get('[role="grid"]');
    await w.get('[data-code-point="65"]').trigger('click');
    await flushPromises();
    card(w).element.dispatchEvent(new Event('pointerdown', { bubbles: true }));
    await flushPromises();
    expect(hasCard(w)).toBe(true);
    w.get('[data-code-point="66"]').element.dispatchEvent(
      new Event('pointerdown', { bubbles: true }),
    );
    await flushPromises();
    expect(hasCard(w)).toBe(true);
    w.get('[data-testid="glyph-specimen"]').element.dispatchEvent(
      new Event('pointerdown', { bubbles: true }),
    );
    await flushPromises();
    expect(hasCard(w)).toBe(false);
    expect(document.activeElement).toBe(grid.element);
  });

  it("the popover's alternates draw the stylistic sets of GlyphDetail, live", async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="97"]').trigger('click');
    await flushPromises();
    await openMore(w);
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
    await openMore(w);
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
      await openMore(w);
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
      await openMore(w);
      await vi.advanceTimersByTimeAsync(STYLE_COUNT_DELAY);
      await flushPromises();
      expect(card(w).get('[aria-labelledby="fm-glyph-styles"]').text()).toBe('unknown');
    } finally {
      vi.useRealTimers();
    }
  });

  it("the popover's alternates draw both ends of each axis, live", async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="65"]').trigger('click');
    await flushPromises();
    await openMore(w);
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
    await openMore(w);
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
    await openMore(w);
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

  it("'Fonts with an a like this' asks for the wall sorted by this face's glyph (E49.9)", async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="97"]').trigger('click');
    await flushPromises();
    await openMore(w);
    const button = card(w).get('[data-testid="glyph-like"]');
    expect(button.text()).toBe('Fonts with an a like this');
    await button.trigger('click');
    const detail = useSelectionStore().detail!;
    expect(w.emitted('findLike')).toEqual([
      [{ face: 1, glyph: 'a', family: detail.face.familyName }],
    ]);
  });

  it('More details keeps the glyph id and block, and no metric', async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="97"]').trigger('click');
    await flushPromises();
    const detail = card(w);
    const more = detail.findAll('button').find((b) => b.text() === 'More details')!;
    expect(more.attributes('aria-expanded')).toBe('false');
    await more.trigger('click');
    expect(more.attributes('aria-expanded')).toBe('true');
    expect(detail.get('[aria-labelledby="fm-glyph-id"]').text()).toMatch(/^\d+$/);
    expect(detail.get('[aria-labelledby="fm-glyph-block"]').text()).toBe('Basic Latin');
    expect(detail.text()).not.toMatch(/advance|units/i);
  });

  it('a feature chip is copied by its tag and named in its tooltip', async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="97"]').trigger('click');
    await flushPromises();
    const chip = card(w)
      .findAll('.feature-chip')
      .find((c) => c.text().includes('smcp'))!;
    expect(chip.attributes('title')).toBe('Small capitals');
    await w.get('button[aria-label="Copy feature tag: ss01"]').trigger('click');
    expect(copyText).toHaveBeenCalledWith('ss01');
  });

  it('Escape in the card closes it and returns to the grid', async () => {
    const w = await mountWith(1);
    const grid = w.get('[role="grid"]');
    await w.get('[data-code-point="65"]').trigger('click');
    await flushPromises();
    await card(w).trigger('keydown', { key: 'Escape' });
    expect(hasCard(w)).toBe(false);
    expect(document.activeElement).toBe(grid.element);
  });
});

describe("'Details on click' (E50.9)", () => {
  it('is on by default; off, a click only selects the cell, and the view remembers it', async () => {
    let w = await mountWith(1);
    const toggle = w.get('[data-testid="glyph-details-on-click"]');
    expect(toggle.attributes('aria-checked')).toBe('true');
    await toggle.trigger('click');
    expect(toggle.attributes('aria-checked')).toBe('false');
    const cell = w.get('[data-code-point="65"]');
    expect(cell.attributes('aria-haspopup')).toBeUndefined();
    await cell.trigger('click');
    await flushPromises();
    expect(cell.attributes('aria-selected')).toBe('true');
    expect(hasCard(w)).toBe(false);
    await w.get('[role="grid"]').trigger('keydown', { key: 'Enter' });
    await flushPromises();
    expect(hasCard(w)).toBe(false);
    w = await mountWithSaved(1);
    expect(w.get('[data-testid="glyph-details-on-click"]').attributes('aria-checked')).toBe(
      'false',
    );
  });

  it('turning it off closes an open popover', async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="65"]').trigger('click');
    await flushPromises();
    await w.get('[data-testid="glyph-details-on-click"]').trigger('click');
    expect(hasCard(w)).toBe(false);
  });

  it("with it off, the cell's context menu opens the glyph detail", async () => {
    const w = await mountWith(1);
    await w.get('[data-testid="glyph-details-on-click"]').trigger('click');
    await w.get('[data-code-point="97"]').trigger('contextmenu');
    await flushPromises();
    const menu = document.querySelector('[data-testid="glyph-menu"]') as HTMLElement;
    expect(menu.getAttribute('role')).toBe('menu');
    expect(menu.getAttribute('aria-label')).toBe('Actions for U+0061');
    const items = [...menu.querySelectorAll<HTMLElement>('[role="menuitem"]')];
    expect(items.map((b) => b.textContent?.trim())).toEqual([
      'Glyph details',
      'Copy glyph',
      'Fonts with an a like this',
      'Compare with…',
    ]);
    items[0]!.click();
    await flushPromises();
    expect(card(w).get('[data-testid="glyph-code"]').text()).toBe('U+0061');
    expect(document.querySelector('[data-testid="glyph-menu"]')).toBeNull();
  });

  it('Shift+F10 opens the menu on the cursor cell; Escape closes it back to the grid', async () => {
    const w = await mountWith(1);
    const grid = w.get('[role="grid"]');
    await grid.trigger('keydown', { key: 'ArrowRight' });
    await flushPromises();
    await grid.trigger('keydown', { key: 'F10', shiftKey: true });
    await flushPromises();
    const menu = document.querySelector('[data-testid="glyph-menu"]') as HTMLElement;
    expect(menu.getAttribute('aria-label')).toBe('Actions for U+0020');
    menu.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await flushPromises();
    expect(document.querySelector('[data-testid="glyph-menu"]')).toBeNull();
    expect(document.activeElement).toBe(grid.element);
  });

  it("the menu's Copy glyph copies the character", async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="97"]').trigger('contextmenu');
    await flushPromises();
    const items = document.querySelectorAll<HTMLElement>(
      '[data-testid="glyph-menu"] [role="menuitem"]',
    );
    items[1]!.click();
    await flushPromises();
    expect(copyText).toHaveBeenCalledWith('a');
  });
});

describe('metrics only while comparing (E50.9)', () => {
  it('with no comparison: no metrics row and no metric anywhere in the section', async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="97"]').trigger('click');
    await flushPromises();
    await openMore(w);
    expect(w.find('[data-testid="glyph-metrics"]').exists()).toBe(false);
    expect(w.find('[data-testid="glyph-ghost"]').exists()).toBe(false);
    expect(w.text()).not.toMatch(/advance|bearing|per 1000 em|units/i);
    expect(card(w).text()).toContain('another font to see metrics side by side');
    const chip = w.get('[data-testid="glyph-compare-chip"]');
    expect(chip.text()).toBe('Compare with…');
  });

  it("the control row's Compare with… opens the overlay on the selected character", async () => {
    const w = await mountWith(1);
    await w.get('[data-code-point="97"]').trigger('click');
    await w.get('[data-testid="glyph-compare-chip"]').trigger('click');
    await flushPromises();
    const compare = useGlyphCompareStore();
    expect(compare.open).toBe(true);
    expect(compare.codePoint).toBe(97);
  });

  it("while comparing: this face's and the compared face's metrics per 1000 em, and the ghost", async () => {
    setFaceLoader({
      acquire: async (id: number) => `fm-face-${id}`,
      release: () => undefined,
    } as unknown as FaceLoader);
    const w = await mountWith(1);
    await compareWithMono();
    await w.get('[data-code-point="97"]').trigger('click');
    await flushPromises();
    const metrics = card(w).get('[data-testid="glyph-metrics"]');
    expect(metrics.text()).toContain('Metrics · per 1000 em');
    expect(metrics.text()).toContain('JetBrains Mono');
    const value = (id: string) => {
      const dd = metrics.get(`[data-metric="${id}"] dd`);
      return [dd.get('.mine').text(), dd.get('.theirs').text()];
    };
    // The mock: 'a' is 600 units wide, inked 40 in from both sides; Inter's x-height is 520,
    // JetBrains Mono's 550, both at 1000 units per em.
    expect(value('advance')).toEqual(['600', '600']);
    expect(value('leftBearing')).toEqual(['40', '40']);
    expect(value('rightBearing')).toEqual(['40', '40']);
    expect(value('xHeight')).toEqual(['520', '550']);
    expect(metrics.get('[data-metric="xHeight"] dd').attributes('aria-label')).toBe(
      'Inter: 520, JetBrains Mono: 550',
    );
    expect(metrics.get('[data-metric="xHeight"] .theirs').classes()).toContain('accent');
    const ghost = card(w).get('[data-testid="glyph-ghost"]');
    expect(ghost.text()).toBe('a');
    expect(ghost.attributes('style')).toContain('fm-face-6');
    expect(card(w).text()).toContain('Comparing with');
    expect(w.get('[data-testid="glyph-compare-chip"]').text()).toContain(
      'Compare with JetBrains Mono',
    );
  });

  it('a compared face that lacks the character shows dashes and no ghost', async () => {
    setFaceLoader({
      acquire: async (id: number) => `fm-face-${id}`,
      release: () => undefined,
    } as unknown as FaceLoader);
    const w = await mountWith(1);
    await compareWithMono();
    // Inter (face 1) maps CJK in this mock; JetBrains Mono does not.
    await useGlyphsStore().search('U+4E00');
    await flushPromises();
    await w.get('[data-code-point="19968"]').trigger('click');
    await flushPromises();
    const metrics = card(w).get('[data-testid="glyph-metrics"]');
    expect(metrics.get('[data-metric="advance"] .theirs').text()).toBe('—');
    expect(metrics.get('[data-metric="xHeight"] .theirs').text()).toBe('550');
    expect(card(w).find('[data-testid="glyph-ghost"]').exists()).toBe(false);
  });

  it('stop ends the comparison: the metrics row goes, from the popover or the chip', async () => {
    const w = await mountWith(1);
    await compareWithMono();
    await w.get('[data-code-point="97"]').trigger('click');
    await flushPromises();
    expect(card(w).find('[data-testid="glyph-metrics"]').exists()).toBe(true);
    await card(w)
      .findAll('button')
      .find((b) => b.text() === 'stop')!
      .trigger('click');
    await flushPromises();
    expect(card(w).find('[data-testid="glyph-metrics"]').exists()).toBe(false);
    await compareWithMono();
    expect(card(w).find('[data-testid="glyph-metrics"]').exists()).toBe(true);
    await w.get('[data-testid="glyph-compare-stop"]').trigger('click');
    await flushPromises();
    expect(card(w).find('[data-testid="glyph-metrics"]').exists()).toBe(false);
    expect(w.get('[data-testid="glyph-compare-chip"]').text()).toBe('Compare with…');
  });
});
