import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient, mockFaces } from '../api/mock';
import {
  FIRST_PANEL,
  MAX_PANELS,
  STUDIO_RESULTS,
  blockScope,
  headlineBlock,
  layoutsOf,
  useStudioStore,
} from './studio';
import type { TextBlock } from '../api/types';
import { useTextLayoutsStore } from './textLayouts';

let client: ReturnType<typeof createMockClient>;

beforeEach(() => {
  setActivePinia(createPinia());
  client = createMockClient();
  setClient(client);
});

afterEach(() => setClient(undefined));

const inter = mockFaces[0]!.face;
const mono = mockFaces[1]!.face;

describe('studio store', () => {
  it('searches the library through the search command for one block', async () => {
    const search = vi.spyOn(client, 'search');
    const studio = useStudioStore();
    await studio.search('book', 0, '  mono ');
    expect(search).toHaveBeenCalledWith({
      query: 'mono',
      page: { offset: 0, limit: STUDIO_RESULTS },
    });
    expect(studio.searchFor).toBe('book#0');
    expect(studio.query).toBe('  mono ');
    expect(studio.results.map((h) => h.family.name)).toContain(mono.familyName);
  });

  it('drops the answer to an older search that comes back last', async () => {
    const real = client.search.bind(client);
    let releaseOlder!: () => void;
    const olderHeld = new Promise<void>((r) => (releaseOlder = r));
    vi.spyOn(client, 'search')
      .mockImplementationOnce(async (req) => {
        await olderHeld;
        return real(req);
      })
      .mockImplementation(real);
    const studio = useStudioStore();
    const older = studio.search('book', 0, mono.familyName);
    await studio.search('book', 1, inter.familyName);
    const newer = studio.results.map((h) => h.family.name);
    expect(newer).toContain(inter.familyName);
    expect(newer).not.toContain(mono.familyName);
    // The older answer arrives after the newer one: it must not replace it.
    releaseOlder();
    await older;
    expect(studio.results.map((h) => h.family.name)).toEqual(newer);
    expect(studio.searchFor).toBe('book#1');
  });

  it('sets a block in the best face of a family hit, per layout and index', async () => {
    const studio = useStudioStore();
    await studio.search('book', 0, mono.familyName);
    const hit = studio.results.find((h) => h.family.id === mono.family)!;
    await studio.pickFamily('book', 0, hit);
    expect(studio.faceOf('book', 0)?.id).toBe(hit.faces[0]);
    expect(studio.faceOf('book', 1)).toBeNull();
    expect(studio.faceOf('quote', 0)).toBeNull();
    expect(studio.faceOf(null, 0)).toBeNull();
  });

  it('"Use panel font" resets the block, and a pick still on its way does not come back', async () => {
    const studio = useStudioStore();
    await studio.setBlockFace('book', 0, inter.id);
    expect(studio.faceOf('book', 0)?.id).toBe(inter.id);
    const late = studio.setBlockFace('book', 0, mono.id);
    studio.resetBlock('book', 0);
    await late;
    expect(studio.faceOf('book', 0)).toBeNull();
  });

  it('a failed face lookup keeps the block as it was and reports the error', async () => {
    const studio = useStudioStore();
    await studio.setBlockFace('book', 0, 999_999);
    expect(studio.faceOf('book', 0)).toBeNull();
    expect(studio.error).not.toBeNull();
  });

  it('never writes a block font into the layout: the saved file has the same shape', async () => {
    const save = vi.spyOn(client, 'saveLayout');
    const layouts = useTextLayoutsStore();
    await layouts.open('book');
    const before = JSON.stringify(layouts.draft);
    const studio = useStudioStore();
    await studio.setBlockFace('book', 0, mono.id);
    expect(JSON.stringify(layouts.draft)).toBe(before);
    layouts.setBlockText(1, 'Changed.');
    await layouts.save();
    expect(save).toHaveBeenCalledOnce();
    expect(JSON.stringify(save.mock.calls[0])).not.toContain(String(mono.familyName));
    expect(JSON.stringify(save.mock.calls[0])).not.toMatch(/face/i);
  });
});

describe('studio panels', () => {
  it('starts with the first panel only, on the library store', () => {
    const studio = useStudioStore();
    expect(studio.panels).toEqual([FIRST_PANEL]);
    expect(layoutsOf(FIRST_PANEL)).toBe(useTextLayoutsStore());
  });

  it('adds a panel as a copy of the current one: layout, unsaved text and block fonts', async () => {
    const first = useTextLayoutsStore();
    await first.open('book');
    first.setBlockText(0, 'Unsaved heading');
    const studio = useStudioStore();
    await studio.setBlockFace(blockScope(FIRST_PANEL, 'book'), 0, mono.id);

    const panel = studio.addPanel(FIRST_PANEL);
    expect(panel).not.toBeNull();
    expect(studio.panels).toEqual([FIRST_PANEL, panel]);
    const copy = layoutsOf(panel!);
    expect(copy).not.toBe(first);
    expect(copy.saved?.id).toBe('book');
    expect(copy.draft?.blocks[0]!.text).toBe('Unsaved heading');
    expect(copy.dirty).toBe(true);
    expect(copy.entries.map((e) => e.id)).toEqual(first.entries.map((e) => e.id));
    expect(studio.faceOf(blockScope(panel!, 'book'), 0)?.id).toBe(mono.id);
    expect(studio.faceOf(blockScope(panel!, 'book'), 1)).toBeNull();
  });

  it('keeps each panel independent: text, layout and block fonts', async () => {
    const first = useTextLayoutsStore();
    await first.open('book');
    const studio = useStudioStore();
    const panel = studio.addPanel(FIRST_PANEL)!;
    const other = layoutsOf(panel);

    other.setBlockText(0, 'Only in panel 2');
    expect(first.draft?.blocks[0]!.text).toBe('Chapter One');
    first.setBlockText(1, 'Only in panel 1');
    expect(other.draft?.blocks[1]!.text).not.toBe('Only in panel 1');

    await studio.setBlockFace(blockScope(panel, 'book'), 0, inter.id);
    expect(studio.faceOf(blockScope(FIRST_PANEL, 'book'), 0)).toBeNull();
    await studio.setBlockFace(blockScope(FIRST_PANEL, 'book'), 0, mono.id);
    expect(studio.faceOf(blockScope(panel, 'book'), 0)?.id).toBe(inter.id);

    await other.open('quote');
    expect(other.saved?.id).toBe('quote');
    expect(first.saved?.id).toBe('book');
  });

  it('copies the panel it is given, not always the first', async () => {
    await useTextLayoutsStore().open('book');
    const studio = useStudioStore();
    const second = studio.addPanel(FIRST_PANEL)!;
    await layoutsOf(second).open('quote');
    const third = studio.addPanel(second)!;
    expect(layoutsOf(third).saved?.id).toBe('quote');
    expect(studio.panels).toEqual([FIRST_PANEL, second, third]);
  });

  it('opens at most four panels', async () => {
    await useTextLayoutsStore().open('book');
    const studio = useStudioStore();
    for (let i = 1; i < MAX_PANELS; i++) expect(studio.addPanel(FIRST_PANEL)).not.toBeNull();
    expect(studio.panels).toHaveLength(MAX_PANELS);
    expect(studio.addPanel(FIRST_PANEL)).toBeNull();
    expect(studio.panels).toHaveLength(MAX_PANELS);
    expect(studio.addPanel(999)).toBeNull();
  });

  it('"Close panel" removes one panel with its block fonts; the first panel stays', async () => {
    await useTextLayoutsStore().open('book');
    const studio = useStudioStore();
    const second = studio.addPanel(FIRST_PANEL)!;
    const third = studio.addPanel(FIRST_PANEL)!;
    await studio.setBlockFace(blockScope(second, 'book'), 0, inter.id);
    await studio.setBlockFace(blockScope(third, 'book'), 0, mono.id);
    layoutsOf(second).setBlockText(0, 'Gone with panel 2');

    studio.closePanel(second);
    expect(studio.panels).toEqual([FIRST_PANEL, third]);
    expect(studio.faceOf(blockScope(second, 'book'), 0)).toBeNull();
    expect(studio.faceOf(blockScope(third, 'book'), 0)?.id).toBe(mono.id);
    // The closed panel's store is gone: asking for it again gives a fresh one.
    expect(layoutsOf(second).draft).toBeNull();

    studio.closePanel(FIRST_PANEL);
    expect(studio.panels).toEqual([FIRST_PANEL, third]);
  });

  it('"Print..." marks the panel as the print target before it opens the print dialog', async () => {
    const studio = useStudioStore();
    const second = studio.addPanel(FIRST_PANEL)!;
    let targetWhenPrinted: number | null = null;
    const printPage = vi.spyOn(client, 'printPage').mockImplementation(async () => {
      targetWhenPrinted = studio.printPanel;
    });
    await studio.print(second);
    expect(printPage).toHaveBeenCalledTimes(1);
    expect(targetWhenPrinted).toBe(second);
    expect(studio.printError).toBeNull();

    // Another panel's print moves the target; closing the target clears it.
    await studio.print(FIRST_PANEL);
    expect(studio.printPanel).toBe(FIRST_PANEL);
    await studio.print(second);
    studio.closePanel(second);
    expect(studio.printPanel).toBeNull();

    // A panel that is not open prints nothing.
    await studio.print(second);
    expect(printPage).toHaveBeenCalledTimes(3);
  });

  it('keeps the error of a print dialog that could not open, for its panel', async () => {
    const studio = useStudioStore();
    const failure = { code: 'io' as const, message: 'No dialog.', detail: 'x' };
    vi.spyOn(client, 'printPage').mockRejectedValueOnce(failure);
    await studio.print(FIRST_PANEL);
    expect(studio.printError).toEqual({ panel: FIRST_PANEL, error: failure });
    // The next print that opens clears it.
    vi.spyOn(client, 'printPage').mockResolvedValueOnce(undefined);
    await studio.print(FIRST_PANEL);
    expect(studio.printError).toBeNull();
  });

  it('scopes no two panels the same, and leaves the first panel keys as they were', () => {
    expect(blockScope(FIRST_PANEL, 'book')).toBe('book');
    expect(blockScope(2, 'book')).not.toBe(blockScope(3, 'book'));
    expect(blockScope(2, 'book')).not.toBe('book');
  });
});

const block = (sizePt: number | null): TextBlock => ({
  text: 'words',
  sizePt,
  lineHeight: null,
  tracking: null,
  paragraphSpacingPt: null,
  firstLineIndentEm: null,
  columns: 1,
  align: 'start',
  overflow: 'wrap',
});

describe('studio pairings (E42.7)', () => {
  it('the headline block is the largest one, the first of equals', () => {
    expect(headlineBlock([block(12), block(48), block(24)])).toBe(1);
    expect(headlineBlock([block(30), block(30)])).toBe(0);
    expect(headlineBlock([block(null), block(9)])).toBe(1);
    expect(headlineBlock([])).toBeNull();
  });

  it('sets the headline block in the headline face and puts the others back on the panel face', async () => {
    const studio = useStudioStore();
    await studio.setBlockFace('book', 0, mono.id);
    await studio.setBlockFace('book', 2, mono.id);
    await studio.loadPairing('book', [block(12), block(36), block(10)], inter.id);
    expect(studio.faceOf('book', 1)?.id).toBe(inter.id);
    expect(studio.faceOf('book', 0)).toBeNull();
    expect(studio.faceOf('book', 2)).toBeNull();
  });

  it('a layout without blocks loads nothing', async () => {
    const studio = useStudioStore();
    await studio.loadPairing('book', [], inter.id);
    expect(studio.blockFaces).toEqual({});
  });
});

describe('the Studio toolbar (E45.1)', () => {
  it('"Compare 2 pairings" adds a copy of the first panel, and pressed again closes it', async () => {
    const studio = useStudioStore();
    await layoutsOf(FIRST_PANEL).open('studio-magazine');
    studio.toggleCompare();
    expect(studio.panels).toHaveLength(2);
    const second = studio.panels[1]!;
    expect(layoutsOf(second).saved?.id).toBe('studio-magazine');
    studio.toggleCompare();
    expect(studio.panels).toEqual([FIRST_PANEL]);
  });

  it('"Compare 2 pairings" closes every other panel, so it never opens a third', () => {
    const studio = useStudioStore();
    studio.addPanel(FIRST_PANEL);
    studio.addPanel(FIRST_PANEL);
    studio.toggleCompare();
    expect(studio.panels).toEqual([FIRST_PANEL]);
  });

  it('exports the saved file in the panel face, each block with a font of its own in it', async () => {
    const exportPng = vi.spyOn(client, 'exportLayoutPng');
    const studio = useStudioStore();
    const layouts = layoutsOf(FIRST_PANEL);
    await layouts.open('studio-magazine');
    const scope = blockScope(FIRST_PANEL, 'studio-magazine');
    await studio.setBlockFace(scope, 1, mono.id);
    const path = await studio.exportPng(FIRST_PANEL, inter.id);
    expect(path).toBe('/mock/exports/studio-magazine-paper-2x.png');
    const font = (face: number) => ({ face, variations: [], features: [] });
    expect(exportPng).toHaveBeenCalledWith({
      layout: 'studio-magazine',
      scale: 2,
      surface: 'paper',
      font: font(inter.id),
      blockFonts: [null, font(mono.id), null, null, null],
    });
    expect(studio.exported).toEqual({ path });
  });

  it('keeps the export error and answers null', async () => {
    vi.spyOn(client, 'exportLayoutPng').mockRejectedValue({
      code: 'io',
      message: 'The export folder is full.',
      detail: null,
    });
    const studio = useStudioStore();
    await layoutsOf(FIRST_PANEL).open('book');
    expect(await studio.exportPng(FIRST_PANEL, inter.id)).toBeNull();
    expect(studio.exported).toEqual({
      error: { code: 'io', message: 'The export folder is full.', detail: null },
    });
  });

  it('exports nothing for a panel without a saved layout', async () => {
    const exportPng = vi.spyOn(client, 'exportLayoutPng');
    const studio = useStudioStore();
    expect(await studio.exportPng(FIRST_PANEL, inter.id)).toBeNull();
    expect(exportPng).not.toHaveBeenCalled();
  });
});
