// The grid and waterfall views, and switching between all four font views.
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Component } from 'vue';
import { setClient, useClient, type FmClient } from '../api/client';
import { createMockClient, EXTRA_ID_BASE, mockLanguages, type MockOptions } from '../api/mock';
import type { PreviewRequest } from '../api/types';
import {
  rowsCardHeight,
  rowsColumns,
  rowsLines,
  specimenBox,
  WALL,
  wallCardWidth,
  wallColumns,
  wallLinePx,
} from '../list/grid';
import { familyKey, type ListRow } from '../list/rows';
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

/** Views still mounted: a view left alive keeps its timers and asks the next test's client. */
const mounted: VueWrapper[] = [];

afterEach(() => {
  for (const w of mounted.splice(0)) if (w.exists()) w.unmount();
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
  mounted.push(w);
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

/** A line at the default size. In the 800 px fallback viewport that size has 2 cards per line. */
const LINE = wallLinePx(DEFAULT_PREVIEW.sizePx);
const FALLBACK_WIDTH = 800;

/** The width in a card's `mask-size`, in px. */
const maskWidth = (card: { get(s: string): { attributes(n: string): string | undefined } }) =>
  Number(
    /mask-size: ([\d.]+)px/.exec(card.get('[data-testid="thumbnail"]').attributes('style')!)?.[1],
  );

describe('FontGrid', () => {
  it('renders only the lines on screen, as cards with thumbnails from render_preview', async () => {
    await load({ extraFaces: 1997 });
    const grid = await mountView(FontGrid, { overscan: 2 });
    // 800 × 600 px fallback viewport: 2 cards per line, 5 + 1 + 2 × 2 lines.
    expect(grid.get('[role="listbox"]').attributes('data-columns')).toBe('2');
    const cards = grid.findAll('[role="option"]');
    expect(cards).toHaveLength(20);
    expect(grid.get('.spacer').attributes('style')).toContain(
      `height: ${WALL.gap + 1000 * LINE}px`,
    );
    // One render per card on screen, at the render size, with the card's text.
    await flushPromises();
    expect(renders).toHaveLength(20);
    const sample = pangramOf('en');
    expect(renders[0]).toMatchObject({ sizePx: WALL.renderPx });
    expect(sample.startsWith(renders[0]!.text)).toBe(true);
    const thumb = cards[0]!.get('[data-testid="thumbnail"]');
    expect(thumb.attributes('data-state')).toBe('ready');
    expect(thumb.attributes('style')).toContain('mask-image: url("blob:thumb-');
  });

  it('places cards in lines and moves the cursor a line with Up/Down', async () => {
    await load({ extraFaces: 17, stylesPerFamily: 3 });
    const grid = await mountView(FontGrid);
    const list = useFontListStore();
    const cards = grid.findAll('[role="option"]');
    // The sixth card: second column, third line.
    const place = (cards[5]!.element as HTMLElement).style;
    expect(place.top).toBe(`${WALL.gap + 2 * LINE}px`);
    expect(place.transform).toBe('');
    await cards[0]!.trigger('click');
    const box = grid.get('[role="listbox"]');
    await box.trigger('keydown', { key: 'ArrowDown' });
    expect(list.cursorIndex).toBe(2);
    await box.trigger('keydown', { key: 'ArrowRight' });
    expect(list.cursorIndex).toBe(3);
    expect(box.attributes('aria-activedescendant')).toBe(
      `fm-card-family-${list.rows[3]!.family.id}`,
    );
    // Enter on a family card asks for its family page and leaves the card folded.
    const at = list.rows.findIndex((r) => r.kind === 'family' && r.family.styleCount > 1);
    list.moveCursor(at);
    await box.trigger('keydown', { key: 'Enter' });
    await flushPromises();
    expect(list.expanded.size).toBe(0);
    expect((grid.emitted('open') as [ListRow][]).map(([row]) => row.key)).toEqual([
      list.rows[at]!.key,
    ]);
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

  it('the slider keeps each card, scales its mask and starts no render', async () => {
    // The text is re-chosen (and rendered) once the slider rests for SETTLE_MS; a real timer
    // would fire on a slow machine before the check below.
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    await load({ extraFaces: 20 });
    const grid = await mountView(FontGrid, { preview: { ...DEFAULT_PREVIEW, sizePx: 30 } });
    await flushPromises();
    const first = grid.findAll('[role="option"]')[0]!;
    const el = first.element;
    const before = maskWidth(first);
    expect(before).toBeGreaterThan(0);
    const asked = renders.length;

    await grid.setProps({ preview: { ...DEFAULT_PREVIEW, sizePx: 36 } });
    await flushPromises();
    const after = grid.findAll('[role="option"]')[0]!;
    expect(after.element).toBe(el);
    expect(maskWidth(after)).toBeGreaterThan(before);
    expect(renders).toHaveLength(asked);
    // Cards are as tall as the size asks.
    expect((after.element as HTMLElement).style.height).toBe(`${wallLinePx(36) - WALL.gap}px`);
  });

  it('during a drag the specimen follows every step and the cards reflow at most every 100 ms', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'performance'] });
    await load({ extraFaces: 20 });
    const grid = await mountView(FontGrid, { preview: { ...DEFAULT_PREVIEW, sizePx: 30 } });
    await flushPromises();
    const size = (sizePx: number) => grid.setProps({ preview: { ...DEFAULT_PREVIEW, sizePx } });
    const card = () => grid.findAll('[role="option"]')[0]!;
    const height = () => (card().element as HTMLElement).style.height;
    const tall = (sizePx: number) => `${wallLinePx(sizePx) - WALL.gap}px`;
    expect(height()).toBe(tall(30));

    // The first step reflows at once.
    await size(32);
    expect(height()).toBe(tall(32));
    const first = maskWidth(card());
    // The next steps, 10 ms apart: the mask grows, the card waits.
    vi.advanceTimersByTime(10);
    await size(34);
    expect(height()).toBe(tall(32));
    expect(maskWidth(card())).toBeGreaterThan(first);
    vi.advanceTimersByTime(10);
    await size(36);
    expect(height()).toBe(tall(32));
    // 100 ms after the last reflow: the card takes the latest size, not an older one.
    vi.advanceTimersByTime(WALL.reflowMs - 20);
    await flushPromises();
    expect(height()).toBe(tall(36));
  });

  it('under reduced motion the cards follow every step of a drag', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'performance'] });
    vi.stubGlobal('matchMedia', (query: string) => ({
      matches: query.includes('reduce'),
      media: query,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }));
    await load({ extraFaces: 20 });
    const grid = await mountView(FontGrid, { preview: { ...DEFAULT_PREVIEW, sizePx: 30 } });
    await flushPromises();
    const height = () => (grid.findAll('[role="option"]')[0]!.element as HTMLElement).style.height;
    await grid.setProps({ preview: { ...DEFAULT_PREVIEW, sizePx: 32 } });
    vi.advanceTimersByTime(10);
    await grid.setProps({ preview: { ...DEFAULT_PREVIEW, sizePx: 34 } });
    expect(height()).toBe(`${wallLinePx(34) - WALL.gap}px`);
  });

  it('after the slider rests, a card holds its old mask until the new text is rendered', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    await load({ extraFaces: 20 });
    const grid = await mountView(FontGrid, { preview: { ...DEFAULT_PREVIEW, sizePx: 24 } });
    await flushPromises();
    const asked = renders.length;
    const texts = () => renders.slice(asked).map((r) => r.text);
    await grid.setProps({ preview: { ...DEFAULT_PREVIEW, sizePx: 60 } });
    await flushPromises();
    // Still moving: the old text at the new scale.
    expect(texts()).toEqual([]);
    vi.advanceTimersByTime(SETTLE_MS);
    await grid.vm.$nextTick();
    // Rested: the shorter text is asked for, and the old mask is still up.
    expect(texts().length).toBeGreaterThan(0);
    expect(renders.at(-1)!.text.length).toBeLessThan(renders[0]!.text.length);
    const thumb = grid.findAll('[role="option"]')[0]!.get('[data-testid="thumbnail"]');
    expect(thumb.attributes('data-state')).toBe('ready');
  });

  it('a word wider than its card shrinks to fit; one that fits stays at its size', async () => {
    await load({ extraFaces: 4 });
    // The mock renders 1 px images: here a character is 60 px wide at the render size.
    const mock = useClient();
    setClient({
      ...mock,
      renderPreview: async (req) => {
        renders.push(structuredClone(req));
        return { ...(await mock.renderPreview(req)), width: req.text.length * 60, height: 140 };
      },
    });
    const preview = (customText: string) => ({
      ...DEFAULT_PREVIEW,
      sample: 'custom' as const,
      customText,
      sizePx: 120,
    });
    const room = specimenBox(
      wallCardWidth(FALLBACK_WIDTH, wallColumns(FALLBACK_WIDTH, 120)),
      120,
    ).width;
    const long = 'Supercalifragilisticexpialidocious';
    const grid = await mountView(FontGrid, { preview: preview(long) });
    await flushPromises();
    const card = () => grid.findAll('[role="option"]')[0]!;
    expect(renders.at(-1)!.text).toBe(long);
    expect(maskWidth(card())).toBeCloseTo(room, 3);
    await grid.setProps({ preview: preview('Ag') });
    // The thumbnail of a typed text is asked for once the keys rest.
    await vi.waitFor(() => expect(maskWidth(card())).toBeLessThan(room));
  });

  it('a typed text shows live at once, asks for no render per key, then takes its thumbnail', async () => {
    await load({ extraFaces: 4 });
    const preview = (customText: string) => ({
      ...DEFAULT_PREVIEW,
      sample: 'custom' as const,
      customText,
    });
    const grid = await mountView(FontGrid, { preview: preview('Ag') });
    await vi.waitFor(() =>
      expect(grid.findAll('[data-testid="thumbnail"][data-state="ready"]').length).toBeGreaterThan(
        0,
      ),
    );
    renders.length = 0;
    const card = () => grid.findAll('[role="option"]')[0]!;

    await grid.setProps({ preview: preview('Ham') });
    // In the same tick: the old text is gone, the new one is on the card.
    expect(card().get('[data-testid="card-live-text"]').text()).toBe('Ham');
    await grid.setProps({ preview: preview('Hamb') });
    await flushPromises();
    expect(card().get('[data-testid="card-live-text"]').text()).toBe('Hamb');
    expect(renders).toHaveLength(0);

    await vi.waitFor(() => expect(renders.some((r) => r.text === 'Hamb')).toBe(true));
    expect(renders.some((r) => r.text === 'Ham')).toBe(false);
    await vi.waitFor(() =>
      expect(card().get('[data-testid="thumbnail"]').attributes('data-state')).toBe('ready'),
    );
    expect(card().find('[data-testid="card-live-text"]').exists()).toBe(false);
  });

  it('a card with no text typed waits for its thumbnail; it loads no face', async () => {
    await load({ extraFaces: 4 });
    // Thumbnails that never arrive: only a typed text may turn a waiting card live.
    setClient({ ...useClient(), renderPreview: () => new Promise(() => undefined) });
    const grid = await mountView(FontGrid, { preview: { ...DEFAULT_PREVIEW } });
    expect(grid.find('[data-testid="card-live-text"]').exists()).toBe(false);
    // The slider changes no text.
    await grid.setProps({ preview: { ...DEFAULT_PREVIEW, sizePx: 60 } });
    expect(grid.find('[data-testid="card-live-text"]').exists()).toBe(false);
    await grid.setProps({
      preview: { ...DEFAULT_PREVIEW, sizePx: 60, customText: 'A', sample: 'custom' },
    });
    expect(grid.find('[data-testid="card-live-text"]').exists()).toBe(true);
  });

  it('a wrapped preset shows live wrapped text at the fixed body size and asks for no render', async () => {
    await load({ extraFaces: 4 });
    renders.length = 0;
    const grid = await mountView(FontGrid, {
      preview: { ...DEFAULT_PREVIEW, preset: 'paragraph', sample: 'paragraph', sizePx: 120 },
    });
    await flushPromises();
    const live = grid.findAll('[data-testid="card-live-text"]')[0]!;
    expect(live.classes()).toContain('wrap');
    expect((live.element as HTMLElement).style.fontSize).toBe('17px');
    expect((live.element as HTMLElement).style.lineHeight).toBe('1.45');
    expect(live.text()).toMatch(/^Typography is the craft.*keeps them reading\.$/);
    expect(renders).toHaveLength(0);
  });

  it('a one-line preset scales the thumbnail by the preset, not by the slider alone', async () => {
    await load({ extraFaces: 4 });
    const mock = useClient();
    setClient({
      ...mock,
      renderPreview: async (req) => ({
        ...(await mock.renderPreview(req)),
        width: req.text.length * 8,
        height: 140,
      }),
    });
    const widthAt = async (preset: 'word' | 'numerals') => {
      const grid = await mountView(FontGrid, {
        preview: { ...DEFAULT_PREVIEW, preset, sample: 'numerals', sizePx: 40 },
      });
      await vi.waitFor(() =>
        expect(maskWidth(grid.findAll('[role="option"]')[0]!)).toBeGreaterThan(0),
      );
      const w = maskWidth(grid.findAll('[role="option"]')[0]!);
      grid.unmount();
      return w;
    };
    expect(await widthAt('numerals')).toBeCloseTo((await widthAt('word')) * 0.75, 3);
  });

  it('a long line on a HiDPI display renders inside the canvas and keeps its true size', async () => {
    vi.stubGlobal('devicePixelRatio', 2);
    await load({ extraFaces: 4 });
    const mock = useClient();
    // A thin face: 0.1 em a character, so this line fits its card at size 24.
    setClient({
      ...mock,
      renderPreview: async (req) => {
        renders.push(structuredClone(req));
        return {
          ...(await mock.renderPreview(req)),
          width: Math.round([...req.text].length * 0.1 * req.sizePx!),
          height: Math.round(req.sizePx! * 1.2),
        };
      },
    });
    const long = 'W'.repeat(30);
    const grid = await mountView(FontGrid, {
      preview: { ...DEFAULT_PREVIEW, sample: 'custom' as const, customText: long, sizePx: 24 },
    });
    await flushPromises();
    const asked = renders.at(-1)!;
    expect(asked.text).toBe(long);
    expect(30 * 1.5 * asked.sizePx!).toBeLessThanOrEqual(8192);
    // 30 × 0.1 em at 24 px: scaled from the size it rendered at, not from `WALL.renderPx`.
    expect(maskWidth(grid.findAll('[role="option"]')[0]!)).toBeCloseTo(72, 0);
  });

  it('the footer says Variable on a variable face and the style count otherwise', async () => {
    await load({ library: 'wall' });
    const grid = await mountView(FontGrid);
    await flushPromises();
    const library = useLibraryStore();
    const cards = grid.findAll('[role="option"]');
    const faces = await Promise.all(
      useFontListStore()
        .rows.slice(0, cards.length)
        .map((r) => library.facesOf(r.family.id)),
    );
    await flushPromises();
    const isVariable = faces.map((f) => f[0]!.isVariable);
    expect(isVariable).toContain(true);
    expect(isVariable).toContain(false);
    grid.findAll('[role="option"]').forEach((card, i) => {
      const detail = card.get('.detail').text();
      if (isVariable[i]) expect(detail).toBe('Variable');
      else expect(detail).toMatch(/^\d+ styles?/);
    });
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

describe('paragraph mode (E43.1)', () => {
  /** 86 characters, past Basic Latin. */
  const LONG =
    'Ljubazni fenjerdžija čađavog lica hoće da mi pokaže štos, a onda i drugi štos za kraj.';
  const own = (text: string, extra: Partial<typeof DEFAULT_PREVIEW> = {}) => ({
    ...DEFAULT_PREVIEW,
    sample: 'custom' as const,
    customText: text,
    ...extra,
  });
  /** The rows' line in the 800 px fallback viewport, for `text` at `px` / `lh`. */
  const rowsLine = (text: string, px = 17, lh = 1.45) => {
    const cols = rowsColumns(FALLBACK_WIDTH);
    const lines = rowsLines(text, wallCardWidth(FALLBACK_WIDTH, cols) - 2 * WALL.paddingPx, px);
    return {
      cols,
      height: rowsCardHeight(lines, px, lh),
      line: rowsCardHeight(lines, px, lh) + WALL.gap,
    };
  };

  it('your own text past 80 characters lays the wall out as reading rows at the body size', async () => {
    expect([...LONG].length).toBeGreaterThan(80);
    await load({ extraFaces: 4 });
    renders.length = 0;
    const grid = await mountView(FontGrid, { preview: own(LONG) });
    const viewport = grid.get('[role="listbox"]');
    expect(viewport.attributes('data-layout')).toBe('rows');
    expect(viewport.classes()).toContain('rows');
    const { cols, height } = rowsLine(LONG);
    expect(viewport.attributes('data-columns')).toBe(String(cols));
    const card = grid.findAll('[role="option"]')[0]!;
    expect(card.attributes('style')).toContain(`height: ${height}px`);
    const live = card.get('[data-testid="card-live-text"]');
    expect(live.classes()).toContain('wrap');
    expect((live.element as HTMLElement).style.fontSize).toBe('17px');
    expect((live.element as HTMLElement).style.lineHeight).toBe('1.45');
    expect(live.text()).toBe(LONG);
    // The name and its meta are in the card with the text.
    expect(card.get('[data-family-name]').text()).not.toBe('');
    expect(renders).toHaveLength(0);
  });

  it('keeps the wall for 80 characters of your own text', async () => {
    await load({ extraFaces: 4 });
    const grid = await mountView(FontGrid, { preview: own('ž'.repeat(80)) });
    expect(grid.get('[role="listbox"]').attributes('data-layout')).toBe('wall');
    expect(grid.get('[role="listbox"]').classes()).not.toContain('rows');
  });

  it('the Paragraph preset is the reading rows too', async () => {
    await load({ extraFaces: 4 });
    const grid = await mountView(FontGrid, {
      preview: { ...DEFAULT_PREVIEW, preset: 'paragraph', sample: 'paragraph' },
    });
    expect(grid.get('[role="listbox"]').attributes('data-layout')).toBe('rows');
  });

  it('the Preview options body size and line height apply', async () => {
    await load({ extraFaces: 4 });
    const grid = await mountView(FontGrid, {
      preview: own(LONG, { bodySizePx: 22, bodyLineHeight: 1.6, sizePx: 90 }),
    });
    const card = grid.findAll('[role="option"]')[0]!;
    const live = card.get('[data-testid="card-live-text"]').element as HTMLElement;
    expect(live.style.fontSize).toBe('22px');
    expect(live.style.lineHeight).toBe('1.6');
    expect(card.attributes('style')).toContain(`height: ${rowsLine(LONG, 22, 1.6).height}px`);
  });

  it('switching back lands the wall on the pixel it left', async () => {
    await load({ extraFaces: 200 });
    const list = useFontListStore();
    const wall = await mountView(FontGrid, { preview: { ...DEFAULT_PREVIEW } });
    const left = 10 * LINE + 37;
    await scrollTo(wall, '[role="listbox"]', left);
    const top = list.anchor;
    wall.unmount();
    const rows = await mountView(FontGrid, { preview: own(LONG) });
    // The rows open at the wall's family, one row a line.
    expect(viewportOf(rows, '[role="listbox"]').scrollTop).toBe(20 * rowsLine(LONG).line);
    expect(list.anchor).toBe(top);
    rows.unmount();
    const back = await mountView(FontGrid, { preview: { ...DEFAULT_PREVIEW } });
    expect(viewportOf(back, '[role="listbox"]').scrollTop).toBe(left);
  });

  it('a scroll in the rows moves the wall to the family now at the top instead', async () => {
    await load({ extraFaces: 200 });
    const wall = await mountView(FontGrid, { preview: { ...DEFAULT_PREVIEW } });
    await scrollTo(wall, '[role="listbox"]', 10 * LINE + 37);
    wall.unmount();
    const rows = await mountView(FontGrid, { preview: own(LONG) });
    await scrollTo(rows, '[role="listbox"]', 60 * rowsLine(LONG).line);
    rows.unmount();
    const back = await mountView(FontGrid, { preview: { ...DEFAULT_PREVIEW } });
    expect(viewportOf(back, '[role="listbox"]').scrollTop).toBe(30 * LINE);
  });
});

describe('switching views', () => {
  const views: [string, Component, string, (rowIndex: number) => number][] = [
    ['list', FontList, '[role="tree"]', (i) => i * 56],
    ['table', FontTable, '[role="treegrid"]', (i) => i * 32],
    ['grid', FontGrid, '[role="listbox"]', (i) => Math.floor(i / 2) * LINE],
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
    expect(viewportOf(grid, '[role="listbox"]').scrollTop).toBe(100 * LINE);
    await library.setOrder(null);
    await flushPromises();
    const moved = list.anchorIndex;
    expect(moved).not.toBe(200);
    expect(list.rows[moved]!.family.id).toBe(top);
    expect(viewportOf(grid, '[role="listbox"]').scrollTop).toBe(Math.floor(moved / 2) * LINE);
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
    // Row 202 sits in line 101 (rows 202–203): the grid shows that line on top.
    expect(viewportOf(grid, '[role="listbox"]').scrollTop).toBe(101 * LINE);
    await scrollTo(grid, '[role="listbox"]', 101 * LINE + 10);
    expect(list.anchor).toBe(list.rows[202]!.family.id);
    await scrollTo(grid, '[role="listbox"]', 120 * LINE);
    expect(list.anchor).toBe(list.rows[240]!.family.id);
    expect(list.rows[0]!.family.id).toBeLessThan(EXTRA_ID_BASE);
  });

  it('the anchor family stays on top when the slider changes the lines', async () => {
    await load({ extraFaces: 997 });
    const list = useFontListStore();
    list.setAnchor(202, 203);
    const grid = await mountView(FontGrid);
    expect(viewportOf(grid, '[role="listbox"]').scrollTop).toBe(101 * LINE);
    await grid.setProps({ preview: { ...DEFAULT_PREVIEW, sizePx: 40 } });
    await flushPromises();
    expect(viewportOf(grid, '[role="listbox"]').scrollTop).toBe(101 * wallLinePx(40));
    // The anchor's card is rendered, not a window worked out from the old scroll position.
    const keys = grid.findAll('[role="option"]').map((c) => c.attributes('data-key'));
    expect(keys).toContain(list.rows[202]!.key);
    // The slider's move is the app's, so it does not change the anchor.
    expect(list.anchor).toBe(list.rows[202]!.family.id);
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
    await frames(grid, '[role="listbox"]', LINE * 2, 2);
    const before = renders.length;
    // Only this grid's first ten lines: two cards each, nothing from another test.
    expect(before).toBe(20);
    await frames(grid, '[role="listbox"]', LINE * 3, 10);
    expect(renders).toHaveLength(before);
    const states = grid.findAll('[data-testid="thumbnail"]').map((t) => t.attributes('data-state'));
    expect(states).toHaveLength(20);
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
