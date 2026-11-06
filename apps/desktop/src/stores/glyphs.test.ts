import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { GlyphStyles, GlyphsRequest } from '../api/types';
import { GLYPH_PAGE, STYLE_COUNT_DELAY, useGlyphsStore } from './glyphs';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

let calls: GlyphsRequest[];
let client: FmClient;

beforeEach(() => {
  setActivePinia(createPinia());
  calls = [];
  const mock = createMockClient({ cjkFaces: [1] });
  client = {
    ...mock,
    listGlyphs: (req) => {
      calls.push(req);
      return mock.listGlyphs(req);
    },
  };
  setClient(client);
});
afterEach(() => setClient(undefined));

describe('glyphs store', () => {
  it('opens a face with its blocks and the first page only', async () => {
    const s = useGlyphsStore();
    await s.open(1);
    expect(s.total).toBeGreaterThan(30_000);
    expect(s.blocks.map((b) => b.name)).toContain('CJK Unified Ideographs');
    expect(s.faceBlocks).toEqual(s.blocks);
    expect(calls).toHaveLength(1);
    expect(s.item(0)?.codePoint).toBe(0x20);
    expect(s.item(GLYPH_PAGE)).toBeUndefined();
  });

  it('loads the pages a range needs, once', async () => {
    const s = useGlyphsStore();
    await s.open(1);
    await s.ensure(GLYPH_PAGE * 3 + 10, GLYPH_PAGE * 4 + 10);
    expect(calls.map((c) => c.page?.offset)).toEqual([0, GLYPH_PAGE * 3, GLYPH_PAGE * 4]);
    expect(s.item(GLYPH_PAGE * 3 + 10)).toBeDefined();
    await s.ensure(GLYPH_PAGE * 3, GLYPH_PAGE * 4);
    expect(calls).toHaveLength(3);
  });

  it('a search asks fm-core and selects the first glyph it finds', async () => {
    const s = useGlyphsStore();
    await s.open(1);
    await s.search('U+0416');
    expect(calls[1]).toMatchObject({ face: 1, query: 'U+0416', block: null });
    expect(s.total).toBe(1);
    expect(s.selected?.codePoint).toBe(0x416);
    expect(s.selectedIndex).toBe(0);
    // The picker keeps every block of the face.
    expect(s.faceBlocks.length).toBeGreaterThan(s.blocks.length);
  });

  it('a block choice lists that block only', async () => {
    const s = useGlyphsStore();
    await s.open(1);
    await s.chooseBlock('Cyrillic');
    expect(calls[1]).toMatchObject({ block: 'Cyrillic', query: null });
    expect(s.blocks.map((b) => b.name)).toEqual(['Cyrillic']);
  });

  it('a block from the coverage list shows that block with the search cleared', async () => {
    const s = useGlyphsStore();
    await s.open(1);
    await s.search('zzz');
    await s.showBlock('Latin Extended-A');
    expect(calls[calls.length - 1]).toMatchObject({ block: 'Latin Extended-A', query: null });
    expect(s.query).toBe('');
    expect(s.blocks.map((b) => b.name)).toEqual(['Latin Extended-A']);
    expect(s.total).toBe(128);
  });

  it('drops the answer to an older list', async () => {
    const s = useGlyphsStore();
    await s.open(1);
    const first = s.search('Ж');
    const second = s.search('U+0041');
    await Promise.all([first, second]);
    expect(s.selected?.codePoint).toBe(0x41);
  });

  it('another face starts over with no search', async () => {
    const s = useGlyphsStore();
    await s.open(1);
    await s.search('A');
    await s.open(2);
    expect(s.query).toBe('');
    expect(s.selected).toBeNull();
    expect(s.total).toBeLessThan(1000);
  });

  it('keeps the API error of a failed request', async () => {
    setClient({
      ...client,
      listGlyphs: () => Promise.reject({ code: 'notFound', message: 'Face 9 was not found.' }),
    });
    const s = useGlyphsStore();
    await s.open(9);
    expect(s.error?.message).toBe('Face 9 was not found.');
  });

  it('fetches the detail of the selected glyph and clears it with the selection', async () => {
    const s = useGlyphsStore();
    await s.open(1);
    const a = s.item(s.blocks[0]!.count - 1) ?? s.item(0)!;
    await s.select({ ...a, codePoint: 0x61, name: 'a' });
    expect(s.detail?.codePoint).toBe(0x61);
    expect(s.detail?.advanceWidth).toBeGreaterThan(0);
    expect(s.detail?.replacedBy.map((f) => f.tag)).toEqual(['smcp', 'ss01']);
    await s.select(null);
    expect(s.detail).toBeNull();
  });

  it('drops the detail of a glyph selected before the last one', async () => {
    let release!: () => void;
    const slow = new Promise<void>((r) => (release = r));
    setClient({
      ...client,
      getGlyph: async (req) => {
        if (req.codePoint === 0x61) await slow;
        return client.getGlyph(req);
      },
    });
    const s = useGlyphsStore();
    await s.open(1);
    const a = s.item(0)!;
    const first = s.select({ ...a, codePoint: 0x61 });
    await s.select({ ...a, codePoint: 0x62 });
    release();
    await first;
    expect(s.detail?.codePoint).toBe(0x62);
  });

  it('keeps the API error of a failed detail request', async () => {
    setClient({
      ...client,
      getGlyph: () =>
        Promise.reject({ code: 'notFound', message: 'Character U+0061 was not found.' }),
    });
    const s = useGlyphsStore();
    await s.open(1);
    await s.select({ ...s.item(0)!, codePoint: 0x61 });
    expect(s.detail).toBeNull();
    expect(s.detailError?.message).toBe('Character U+0061 was not found.');
  });
});

describe('glyphs store: in how many styles', () => {
  /** Answers `glyphStyles` with `answer`, or fails; returns the code points asked for. */
  function answer(got: GlyphStyles | null) {
    const asked: number[] = [];
    setClient({
      ...client,
      glyphStyles: async (req) => {
        asked.push(req.codePoint);
        if (!got) throw { code: 'notFound', message: 'gone' };
        return got;
      },
    });
    return asked;
  }

  beforeEach(() => {
    vi.useFakeTimers();
    return useGlyphsStore().open(1);
  });
  afterEach(() => vi.useRealTimers());

  const rest = () => vi.advanceTimersByTimeAsync(STYLE_COUNT_DELAY);

  it('shows what fm-core answers for the open face', async () => {
    const asked = answer({ have: 2, total: 3 });
    const s = useGlyphsStore();
    s.countStyles(0x17d);
    await rest();
    expect(asked).toEqual([0x17d]);
    expect(s.styleCount).toEqual({ have: 2, total: 3 });
  });

  it('waits for the selection to rest and asks for the last character only', async () => {
    const asked = answer({ have: 1, total: 1 });
    const s = useGlyphsStore();
    s.countStyles(0x41);
    s.countStyles(0x42);
    expect(asked).toEqual([]);
    expect(s.styleCount).toBeNull();
    await rest();
    expect(asked).toEqual([0x42]);
  });

  it('drops the answer to a character selected before the last one', async () => {
    let answerOld: (got: GlyphStyles) => void = () => undefined;
    setClient({
      ...client,
      glyphStyles: () => new Promise((resolve) => (answerOld = resolve)),
    });
    const s = useGlyphsStore();
    s.countStyles(0x41);
    await rest();
    s.countStyles(null);
    answerOld({ have: 1, total: 1 });
    await rest();
    expect(s.styleCount).toBeNull();
  });

  it('clears the count with the selection, and asks nothing', async () => {
    const asked = answer({ have: 1, total: 1 });
    const s = useGlyphsStore();
    s.countStyles(0x41);
    await rest();
    expect(s.styleCount).not.toBeNull();
    asked.length = 0;
    s.countStyles(null);
    await rest();
    expect(s.styleCount).toBeNull();
    expect(asked).toEqual([]);
  });

  it('says it failed when fm-core cannot answer, and a new selection starts over', async () => {
    answer(null);
    const s = useGlyphsStore();
    s.countStyles(0x41);
    await rest();
    expect(s.styleCountFailed).toBe(true);
    expect(s.styleCount).toBeNull();
    answer({ have: 1, total: 1 });
    s.countStyles(0x41);
    expect(s.styleCountFailed).toBe(false);
    await rest();
    expect(s.styleCount).toEqual({ have: 1, total: 1 });
  });
});

describe('glyphs store: pages on their way', () => {
  it('ensure waits for a page another caller already asked for', async () => {
    const s = useGlyphsStore();
    await s.open(1);
    let release: () => void = () => undefined;
    const mock = client.listGlyphs;
    setClient({
      ...client,
      listGlyphs: async (req) => {
        await new Promise<void>((resolve) => (release = resolve));
        return mock(req);
      },
    });
    const last = s.total - 1;
    void s.ensure(last, last + 1);
    let done = false;
    void s.ensure(last, last + 1).then(() => (done = true));
    await new Promise((resolve) => setTimeout(resolve));
    expect(done).toBe(false);
    release();
    await new Promise((resolve) => setTimeout(resolve));
    expect(done).toBe(true);
    expect(s.item(last)).toBeDefined();
  });
});
