import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { GlyphsRequest } from '../api/types';
import { GLYPH_PAGE, useGlyphsStore } from './glyphs';

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
});
