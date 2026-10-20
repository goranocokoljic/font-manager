import { defineStore } from 'pinia';
import { computed, ref, shallowReactive } from 'vue';
import { useClient } from '../api/client';
import type {
  ApiError,
  FaceId,
  GlyphBlock,
  GlyphInfo,
  GlyphPage,
  GlyphsRequest,
  Health,
} from '../api/types';

/** Characters fetched per request: fm-api's largest page. */
export const GLYPH_PAGE = 500;

/**
 * The inspector's glyph map of one face (the Glyphs tab), as fm-core lists it:
 * the blocks of the list with their counts, and the characters page by page as
 * the grid scrolls to them. A search or a block choice asks fm-core for a new
 * list; the first character a search finds is selected.
 */
export const useGlyphsStore = defineStore('glyphs', () => {
  const face = ref<FaceId | null>(null);
  const query = ref('');
  /** Only this block, or every block. */
  const block = ref<string | null>(null);
  /** The blocks of the list, in order, with their counts. */
  const blocks = ref<GlyphBlock[]>([]);
  /** Every block of the face, for the block picker. */
  const faceBlocks = ref<GlyphBlock[]>([]);
  const total = ref(0);
  const health = ref<Health>({ kind: 'ok' });
  const unicodeVersion = ref('');
  const error = ref<ApiError | null>(null);
  const loading = ref(false);
  const selected = ref<GlyphInfo | null>(null);
  /** Pages of the current list by page number. */
  const pages = shallowReactive(new Map<number, GlyphInfo[]>());
  /** Bumped when a new list arrives, so the grid can scroll to its selection. */
  const generation = ref(0);

  // Answers to an older list are dropped.
  let request = 0;
  const pending = new Set<number>();

  const req = (page: number): GlyphsRequest => ({
    face: face.value!,
    block: block.value,
    query: query.value.trim() || null,
    page: { offset: page * GLYPH_PAGE, limit: GLYPH_PAGE },
  });

  async function load(): Promise<void> {
    if (face.value === null) return;
    const mine = ++request;
    pending.clear();
    loading.value = true;
    const filtered = !!query.value.trim() || block.value !== null;
    let page: GlyphPage;
    try {
      page = await useClient().listGlyphs(req(0));
    } catch (e) {
      if (mine !== request) return;
      error.value = e as ApiError;
      loading.value = false;
      return;
    }
    if (mine !== request) return;
    error.value = null;
    loading.value = false;
    pages.clear();
    pages.set(0, page.items);
    blocks.value = page.blocks;
    if (!filtered) faceBlocks.value = page.blocks;
    total.value = page.total;
    health.value = page.health;
    unicodeVersion.value = page.unicodeVersion;
    if (query.value.trim()) selected.value = page.items[0] ?? null;
    generation.value += 1;
  }

  /** Shows the glyph map of `id`; a new face starts with no search and every block. */
  async function open(id: FaceId): Promise<void> {
    if (id === face.value) return;
    face.value = id;
    query.value = '';
    block.value = null;
    selected.value = null;
    faceBlocks.value = [];
    blocks.value = [];
    total.value = 0;
    pages.clear();
    await load();
  }

  async function search(text: string): Promise<void> {
    query.value = text;
    await load();
  }

  async function chooseBlock(name: string | null): Promise<void> {
    block.value = name;
    await load();
  }

  /** Fetches the pages holding list indexes `start` (inclusive) to `end` (exclusive). */
  async function ensure(start: number, end: number): Promise<void> {
    if (face.value === null || end <= start) return;
    const mine = request;
    const wanted: number[] = [];
    for (let p = Math.floor(start / GLYPH_PAGE); p * GLYPH_PAGE < Math.min(end, total.value); p++)
      if (!pages.has(p) && !pending.has(p)) wanted.push(p);
    await Promise.all(
      wanted.map(async (p) => {
        pending.add(p);
        try {
          const page = await useClient().listGlyphs(req(p));
          if (mine === request) pages.set(p, page.items);
        } catch (e) {
          if (mine === request) error.value = e as ApiError;
        } finally {
          if (mine === request) pending.delete(p);
        }
      }),
    );
  }

  /** The character at list index `i`, or undefined while its page loads. */
  function item(i: number): GlyphInfo | undefined {
    return pages.get(Math.floor(i / GLYPH_PAGE))?.[i % GLYPH_PAGE];
  }

  function select(g: GlyphInfo | null): void {
    selected.value = g;
  }

  /** The list index of the selected character, when its page is loaded. */
  const selectedIndex = computed(() => {
    const s = selected.value;
    if (!s) return -1;
    for (const [p, items] of pages) {
      const i = items.findIndex((g) => g.codePoint === s.codePoint);
      if (i >= 0) return p * GLYPH_PAGE + i;
    }
    return -1;
  });

  return {
    face,
    query,
    block,
    blocks,
    faceBlocks,
    total,
    health,
    unicodeVersion,
    error,
    loading,
    selected,
    selectedIndex,
    generation,
    open,
    search,
    chooseBlock,
    ensure,
    item,
    select,
  };
});
