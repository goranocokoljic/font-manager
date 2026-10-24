import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient, mockFaces } from '../api/mock';
import {
  FIRST_PANEL,
  MAX_PANELS,
  STUDIO_RESULTS,
  blockScope,
  layoutsOf,
  useStudioStore,
} from './studio';
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
