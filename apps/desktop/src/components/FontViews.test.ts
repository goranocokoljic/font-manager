// The grid and waterfall views, and switching between all four font views.
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Component } from 'vue';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, EXTRA_ID_BASE, mockLanguages, type MockOptions } from '../api/mock';
import type { PreviewRequest } from '../api/types';
import { CARD } from '../list/grid';
import { familyKey } from '../list/rows';
import { WATERFALL_SIZES, waterfallRowPx } from '../list/waterfall';
import { FRAME_MS, SETTLE_MS } from '../list/settle';
import { FaceLoader, setFaceLoader, type FontFaceSetLike } from '../preview/faceLoader';
import { useSearchStore } from '../stores/search';
import { clearFaceAxes } from '../preview/variations';
import { useFontListStore } from '../stores/fontList';
import { useLibraryStore } from '../stores/library';
import { DEFAULT_PREVIEW } from '../stores/preview';
import FontGrid from './FontGrid.vue';
import FontList from './FontList.vue';
import FontTable from './FontTable.vue';
import FontWaterfall from './FontWaterfall.vue';

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

let renders: PreviewRequest[];

/** The mock API's pangram for `code`. */
const pangramOf = (code: string) => mockLanguages.find((l) => l.code === code)!.pangram!;

beforeEach(() => {
  setActivePinia(createPinia());
  clearFaceAxes();
  renders = [];
  vi.stubGlobal('devicePixelRatio', 1);
  let n = 0;
  URL.createObjectURL = vi.fn(() => `blob:thumb-${++n}`);
  URL.revokeObjectURL = vi.fn();
  setFaceLoader(
    new FaceLoader<FakeFace>({
      fonts: new FakeSet(),
      create: (family, source) => new FakeFace(family, source),
      url: (face) => `fmfont://localhost/${face}`,
      maxIdle: 16,
    }),
  );
});

afterEach(() => {
  vi.useRealTimers();
  setFaceLoader(undefined);
  setClient(undefined);
  vi.unstubAllGlobals();
});

async function load(options: MockOptions) {
  const mock = createMockClient(options);
  const client: FmClient = {
    ...mock,
    renderPreview: (req) => {
      renders.push(structuredClone(req));
      return mock.renderPreview(req);
    },
  };
  setClient(client);
  await useLibraryStore().load();
  await useSearchStore().loadLanguages();
}

async function mountView(view: Component, props: Record<string, unknown> = {}) {
  const w = mount(view, { props, attachTo: document.body });
  await flushPromises();
  return w;
}

async function scrollTo(w: VueWrapper, selector: string, top: number) {
  const el = w.get(selector);
  (el.element as HTMLElement).scrollTop = top;
  await el.trigger('scroll');
  await flushPromises();
}

const viewportOf = (w: VueWrapper, selector: string) => w.get(selector).element as HTMLElement;

describe('FontGrid', () => {
  it('renders only the lines on screen, as cards with thumbnails from render_preview', async () => {
    await load({ extraFaces: 1997 });
    const grid = await mountView(FontGrid, { overscan: 2 });
    // 800 × 600 px fallback viewport: 4 cards per line, 5 + 1 + 2 × 2 lines.
    expect(grid.get('[role="listbox"]').attributes('data-columns')).toBe('4');
    const cards = grid.findAll('[role="option"]');
    expect(cards).toHaveLength(40);
    expect(grid.get('.spacer').attributes('style')).toContain(`height: ${500 * CARD.linePx}px`);
    // One render per card on screen, at the thumbnail size, with the card's text.
    await flushPromises();
    expect(renders).toHaveLength(40);
    const sample = pangramOf('en');
    expect(renders[0]).toMatchObject({ sizePx: 36 });
    expect(sample.startsWith(renders[0]!.text)).toBe(true);
    const thumb = cards[0]!.get('[data-testid="thumbnail"]');
    expect(thumb.attributes('data-state')).toBe('ready');
    expect(thumb.attributes('style')).toContain('mask-image: url("blob:thumb-');
  });

  it('places cards in lines and moves the cursor a line with Up/Down', async () => {
    await load({ extraFaces: 17 });
    const grid = await mountView(FontGrid);
    const list = useFontListStore();
    const cards = grid.findAll('[role="option"]');
    expect(cards[5]!.attributes('style')).toContain(`translate(100%, ${CARD.linePx}px)`);
    await cards[0]!.trigger('click');
    const box = grid.get('[role="listbox"]');
    await box.trigger('keydown', { key: 'ArrowDown' });
    expect(list.cursorIndex).toBe(4);
    await box.trigger('keydown', { key: 'ArrowRight' });
    expect(list.cursorIndex).toBe(5);
    expect(box.attributes('aria-activedescendant')).toBe(
      `fm-card-family-${list.rows[5]!.family.id}`,
    );
    // Enter on a family card expands it: its styles follow as cards.
    await box.trigger('keydown', { key: 'Enter' });
    await flushPromises();
    expect(list.expanded.size).toBe(1);
  });

  it('a failed thumbnail says so', async () => {
    await load({ extraFaces: 1 });
    const failing = useClientWithFailingRenders();
    setClient(failing);
    const grid = await mountView(FontGrid);
    await flushPromises();
    const card = grid.findAll('[role="option"]')[0]!;
    expect(card.get('[data-testid="thumbnail"]').attributes('data-state')).toBe('failed');
    expect(card.text()).toContain('Preview not available');
  });
});

describe('FontGrid outline and tracking (E16.1)', () => {
  it('cards draw live webview text with tracking or outline, thumbnails without', async () => {
    await load({ extraFaces: 5 });
    const grid = await mountView(FontGrid, { preview: { ...DEFAULT_PREVIEW } });
    const cards = () => grid.findAll('[role="option"]');
    expect(cards()[0]!.find('[data-testid="thumbnail"]').exists()).toBe(true);
    expect(grid.find('[data-testid="card-live-text"]').exists()).toBe(false);
    const thumbs = renders.length;

    await grid.setProps({ preview: { ...DEFAULT_PREVIEW, trackingEm: 0.25 } });
    await flushPromises();
    for (const card of cards()) {
      const live = card.get('[data-testid="card-live-text"]');
      expect((live.element as HTMLElement).style.letterSpacing).toBe('0.25em');
      expect(live.classes()).not.toContain('outline');
      expect(card.find('[data-testid="thumbnail"]').exists()).toBe(false);
    }

    await grid.setProps({ preview: { ...DEFAULT_PREVIEW, outline: true } });
    await flushPromises();
    for (const card of cards())
      expect(card.get('[data-testid="card-live-text"]').classes()).toContain('outline');
    // Live text needs no thumbnail render.
    expect(renders).toHaveLength(thumbs);

    await grid.setProps({ preview: { ...DEFAULT_PREVIEW } });
    await flushPromises();
    expect(grid.find('[data-testid="card-live-text"]').exists()).toBe(false);
    expect(cards()[0]!.find('[data-testid="thumbnail"]').exists()).toBe(true);
  });
});

function useClientWithFailingRenders(): FmClient {
  return {
    ...createMockClient({ extraFaces: 1 }),
    renderPreview: () => Promise.reject({ code: 'internal', message: 'x', detail: null }),
  };
}

describe('FontWaterfall', () => {
  it('shows the sample text at six sizes per family', async () => {
    await load({ extraFaces: 2 });
    const fall = await mountView(FontWaterfall, { preview: { ...DEFAULT_PREVIEW } });
    const blocks = fall.findAll('[role="treeitem"]');
    expect(blocks).toHaveLength(5);
    expect(blocks[0]!.attributes('style')).toContain(`height: ${waterfallRowPx(1.2)}px`);
    for (const block of blocks) {
      const lines = block.findAll('[data-testid="waterfall-line"]');
      expect(lines.map((l) => Number(l.attributes('data-size')))).toEqual([...WATERFALL_SIZES]);
      for (const [k, line] of lines.entries()) {
        const sample = line.get('.sample');
        expect(sample.text()).toBe(pangramOf('en'));
        expect(sample.attributes('style')).toContain(`font-size: ${WATERFALL_SIZES[k]}px`);
        expect(line.text()).toContain(`${WATERFALL_SIZES[k]} px`);
      }
    }
  });
});

describe('FontWaterfall metrics, outline and tracking (E16.1)', () => {
  it('every size line draws its metrics lines, outlines and tracking', async () => {
    await load({ extraFaces: 2 });
    const preview = { ...DEFAULT_PREVIEW, metrics: true, outline: true, trackingEm: 0.1 };
    const fall = await mountView(FontWaterfall, { preview });
    const lines = fall.findAll('[data-face="1"][data-testid="waterfall-line"]');
    expect(lines).toHaveLength(WATERFALL_SIZES.length);
    for (const line of lines) {
      const sample = line.get('.sample');
      expect(sample.classes()).toContain('outline');
      expect((sample.element as HTMLElement).style.letterSpacing).toBe('0.1em');
      expect(line.findAll('[data-line]').map((l) => l.attributes('data-line'))).toEqual([
        'baseline',
        'xHeight',
        'capHeight',
      ]);
    }
    await fall.setProps({ preview: { ...DEFAULT_PREVIEW } });
    expect(fall.find('[data-line]').exists()).toBe(false);
    expect(fall.find('.sample.outline').exists()).toBe(false);
  });
});

describe('switching views', () => {
  const views: [string, Component, string, (rowIndex: number) => number][] = [
    ['list', FontList, '[role="tree"]', (i) => i * 56],
    ['table', FontTable, '[role="treegrid"]', (i) => i * 32],
    ['grid', FontGrid, '[role="listbox"]', (i) => Math.floor(i / 4) * CARD.linePx],
    ['waterfall', FontWaterfall, '[role="tree"]', (i) => i * waterfallRowPx(1.2)],
  ];

  // Builds a 1,000-family mock and mounts all four views: well under a second on a
  // fast machine, more than the 5 s default on a loaded CI runner (Windows). The
  // test is about the anchor and the selection, not speed.
  it(
    'keeps the selection and the family at the top in every view',
    { timeout: 30_000 },
    async () => {
      await load({ extraFaces: 997 });
      const list = useFontListStore();
      // Scroll the list to row 200 and select two families there.
      const first = await mountView(FontList, { rowHeight: 56, preview: { ...DEFAULT_PREVIEW } });
      await scrollTo(first, '[role="tree"]', 200 * 56);
      const top = list.rows[200]!.family.id;
      expect(list.anchor).toBe(top);
      list.click(familyKey(top));
      list.click(familyKey(top + 1), 'toggle');
      first.unmount();
      for (const [name, view, selector, topOf] of views) {
        const props = name === 'list' ? { rowHeight: 56 } : name === 'table' ? { view: 'all' } : {};
        const w = await mountView(view, props);
        expect(viewportOf(w, selector).scrollTop, name).toBe(topOf(200));
        expect(list.anchor, name).toBe(top);
        expect(list.selectedCount, name).toBe(2);
        expect(w.get(`[data-key="${familyKey(top)}"]`).attributes('aria-selected'), name).toBe(
          'true',
        );
        w.unmount();
      }
    },
  );

  it('keeps the family at the top when leaving a sorted table re-orders the rows', async () => {
    await load({ extraFaces: 997 });
    const list = useFontListStore();
    const library = useLibraryStore();
    await library.setOrder({ by: 'name', descending: true });
    const table = await mountView(FontTable, { view: 'all' });
    await scrollTo(table, '[role="treegrid"]', 200 * 32);
    const top = list.anchor!;
    expect(top).toBe(list.rows[200]!.family.id);
    table.unmount();
    // The grid mounts at the sorted row; then the default order comes back (HomeView
    // sets the order after the mode changes), so the family is at another row.
    const grid = await mountView(FontGrid);
    expect(viewportOf(grid, '[role="listbox"]').scrollTop).toBe(50 * CARD.linePx);
    await library.setOrder(null);
    await flushPromises();
    const moved = list.anchorIndex;
    expect(moved).not.toBe(200);
    expect(list.rows[moved]!.family.id).toBe(top);
    expect(viewportOf(grid, '[role="listbox"]').scrollTop).toBe(
      Math.floor(moved / 4) * CARD.linePx,
    );
    // Back to the sorted table: it mounts in the default order, then re-sorts.
    grid.unmount();
    const again = await mountView(FontTable, { view: 'all' });
    await library.setOrder({ by: 'name', descending: true });
    await flushPromises();
    expect(list.anchor).toBe(top);
    expect(viewportOf(again, '[role="treegrid"]').scrollTop).toBe(200 * 32);
  });

  it('the anchor follows a scroll in the grid by line, without drifting', async () => {
    await load({ extraFaces: 997 });
    const list = useFontListStore();
    list.setAnchor(202, 203);
    const grid = await mountView(FontGrid);
    // Row 202 sits in line 50 (rows 200–203): the grid shows that line on top.
    expect(viewportOf(grid, '[role="listbox"]').scrollTop).toBe(50 * CARD.linePx);
    await scrollTo(grid, '[role="listbox"]', 50 * CARD.linePx + 10);
    expect(list.anchor).toBe(list.rows[202]!.family.id);
    await scrollTo(grid, '[role="listbox"]', 60 * CARD.linePx);
    expect(list.anchor).toBe(list.rows[240]!.family.id);
    expect(list.rows[0]!.family.id).toBeLessThan(EXTRA_ID_BASE);
  });
});

describe('grid and waterfall while scrolling fast (E35.8)', () => {
  /** Scrolls `steps` frames of `px` each: two fast frames make the view moving. */
  async function frames(w: VueWrapper, selector: string, px: number, steps: number) {
    let top = viewportOf(w, selector).scrollTop;
    for (let i = 0; i < steps; i++) {
      vi.advanceTimersByTime(FRAME_MS);
      top += px;
      await scrollTo(w, selector, top);
    }
  }

  async function settle() {
    vi.advanceTimersByTime(SETTLE_MS);
    await flushPromises();
  }

  it('the grid asks for no thumbnails while it moves and shows empty frames', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'performance'] });
    await load({ extraFaces: 397 });
    const grid = await mountView(FontGrid, { overscan: 2 });
    await flushPromises();
    await frames(grid, '[role="listbox"]', CARD.linePx * 2, 2);
    const before = renders.length;
    await frames(grid, '[role="listbox"]', CARD.linePx * 3, 10);
    expect(renders).toHaveLength(before);
    const states = grid.findAll('[data-testid="thumbnail"]').map((t) => t.attributes('data-state'));
    expect(states).toHaveLength(40);
    expect(states).not.toContain('loading');
    expect(states).toContain('deferred');
    await settle();
    await flushPromises();
    expect(renders.length).toBeGreaterThan(before);
    const after = grid.findAll('[data-testid="thumbnail"]').map((t) => t.attributes('data-state'));
    expect(after.every((s) => s === 'ready')).toBe(true);
    grid.unmount();
  });

  it('the waterfall starts no face loads while it moves', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'performance'] });
    await load({ extraFaces: 200 });
    const fall = await mountView(FontWaterfall, { preview: { ...DEFAULT_PREVIEW } });
    const rowPx = waterfallRowPx(1.2);
    await frames(fall, '[role="tree"]', rowPx + 20, 4);
    const lines = fall.findAll('[data-testid="waterfall-line"]');
    expect(lines.length).toBeGreaterThan(0);
    const states = lines.map((l) => l.attributes('data-state'));
    expect(states).not.toContain('loading');
    expect(states).toContain('deferred');
    await settle();
    await flushPromises();
    const after = fall.findAll('[data-testid="waterfall-line"]');
    expect(after.every((l) => l.attributes('data-state') === 'ready')).toBe(true);
    fall.unmount();
  });
});
