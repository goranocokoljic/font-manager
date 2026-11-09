import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient, mockFaces } from '../api/mock';
import {
  FIRST_PANEL,
  MAX_PANELS,
  STUDIO_RESULTS,
  blockScope,
  blocksOf,
  headlineBlock,
  layoutsOf,
  namedAfter,
  roleOf,
  useStudioStore,
} from './studio';
import type { LayoutRole, TextBlock } from '../api/types';
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
    await studio.loadPairing('book', { blocks: [block(12), block(36), block(10)] }, inter.id, null);
    expect(studio.faceOf('book', 1)?.id).toBe(inter.id);
    expect(studio.faceOf('book', 0)).toBeNull();
    expect(studio.faceOf('book', 2)).toBeNull();
  });

  it('a layout without blocks loads nothing', async () => {
    const studio = useStudioStore();
    await studio.loadPairing('book', { blocks: [] }, inter.id, null);
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
    expect(path).toBe(`/mock/exports/${mono.familyName}-Inter-magazine.png`);
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

describe('the roles rail (E45.2)', () => {
  const MAGAZINE: LayoutRole[] = [
    { role: 'headline', blocks: [1] },
    { role: 'deck', blocks: [2] },
    { role: 'body', blocks: [3] },
    { role: 'caption', blocks: [0, 4] },
  ];
  const magazine = {
    blocks: [block(8), block(40), block(15), block(9.5), block(8)],
    roles: MAGAZINE,
  };

  it('finds the role of a block and the blocks of a role', () => {
    expect(roleOf(MAGAZINE, 4)?.role).toBe('caption');
    expect(roleOf(MAGAZINE, 9)).toBeNull();
    expect(blocksOf(magazine, 'caption')).toEqual([0, 4]);
    expect(blocksOf(magazine, 'subtitle')).toEqual([]);
    // Without roles, the headline is the largest block and there is no body.
    expect(blocksOf({ blocks: magazine.blocks }, 'headline')).toEqual([1]);
    expect(blocksOf({ blocks: magazine.blocks }, 'body')).toEqual([]);
  });

  it('a pairing fills the Headline and Body roles and puts the rest on the panel face', async () => {
    const studio = useStudioStore();
    await studio.setBlockFace('studio-magazine', 0, inter.id);
    await studio.loadPairing('studio-magazine', magazine, inter.id, mono.id);
    expect(studio.faceOf('studio-magazine', 1)?.id).toBe(inter.id);
    expect(studio.faceOf('studio-magazine', 3)?.id).toBe(mono.id);
    for (const i of [0, 2, 4]) expect(studio.faceOf('studio-magazine', i)).toBeNull();
  });

  it('a pairing without a body face fills the Headline alone', async () => {
    const studio = useStudioStore();
    await studio.loadPairing('studio-magazine', magazine, mono.id, null);
    expect(studio.faceOf('studio-magazine', 1)?.id).toBe(mono.id);
    expect(studio.faceOf('studio-magazine', 3)).toBeNull();
  });

  it('a role picker searches for its card alone and sets every block of the role', async () => {
    const getFace = vi.spyOn(client, 'getFace');
    const studio = useStudioStore();
    await studio.searchRole('studio-magazine', 'caption', 'mono');
    expect(studio.searchFor).toBe('studio-magazine@caption');
    const hit = studio.results[0]!;
    await studio.pickRoleFamily('studio-magazine', MAGAZINE[3]!, hit);
    // One face lookup for the two blocks of the role.
    expect(getFace).toHaveBeenCalledTimes(1);
    expect(studio.faceOf('studio-magazine', 0)?.id).toBe(hit.faces[0]);
    expect(studio.faceOf('studio-magazine', 4)?.id).toBe(hit.faces[0]);
    expect(studio.roleFace('studio-magazine', MAGAZINE[3]!)?.id).toBe(hit.faces[0]);
    expect(studio.faceOf('studio-magazine', 1)).toBeNull();
  });

  it('a later pick for one block of a role wins over the role pick still on its way', async () => {
    const studio = useStudioStore();
    const role = studio.setBlocksFace('studio-magazine', [0, 4], inter.id);
    await studio.setBlockFace('studio-magazine', 4, mono.id);
    await role;
    expect(studio.faceOf('studio-magazine', 0)?.id).toBe(inter.id);
    expect(studio.faceOf('studio-magazine', 4)?.id).toBe(mono.id);
  });

  it('locks a role per layout and panel, and a closed panel forgets its locks', () => {
    const studio = useStudioStore();
    expect(studio.isLocked('studio-magazine', 'headline')).toBe(false);
    studio.toggleLock('studio-magazine', 'headline');
    expect(studio.isLocked('studio-magazine', 'headline')).toBe(true);
    expect(studio.isLocked('studio-magazine', 'body')).toBe(false);
    expect(studio.isLocked('studio-poster', 'headline')).toBe(false);
    expect(studio.isLocked(null, 'headline')).toBe(false);
    const second = studio.addPanel(FIRST_PANEL)!;
    const scope = blockScope(second, 'studio-magazine');
    studio.toggleLock(scope, 'body');
    expect(studio.isLocked(scope, 'body')).toBe(true);
    studio.closePanel(second);
    expect(studio.locks).toEqual({ 'studio-magazine@headline': true });
    studio.toggleLock('studio-magazine', 'headline');
    expect(studio.isLocked('studio-magazine', 'headline')).toBe(false);
  });
});

describe('compare, save and export (E45.4)', () => {
  it('exports at 1x when asked; 2x stays the default', async () => {
    const exportPng = vi.spyOn(client, 'exportLayoutPng');
    const studio = useStudioStore();
    await layoutsOf(FIRST_PANEL).open('studio-magazine');
    await studio.exportPng(FIRST_PANEL, inter.id, 1);
    expect(exportPng).toHaveBeenLastCalledWith(expect.objectContaining({ scale: 1 }));
    await studio.exportPng(FIRST_PANEL, inter.id);
    expect(exportPng).toHaveBeenLastCalledWith(expect.objectContaining({ scale: 2 }));
  });

  it('compared pairings share their text, not their roles', async () => {
    const studio = useStudioStore();
    const first = layoutsOf(FIRST_PANEL);
    await first.open('studio-magazine');
    studio.toggleCompare();
    const panel = studio.panels[1]!;
    const second = layoutsOf(panel);
    const head = blocksOf(first.draft!, 'headline')[0]!;

    studio.setText(panel, head, 'Shared headline');
    expect(second.draft!.blocks[head]!.text).toBe('Shared headline');
    expect(first.draft!.blocks[head]!.text).toBe('Shared headline');
    studio.setText(FIRST_PANEL, head, 'From the first');
    expect(second.draft!.blocks[head]!.text).toBe('From the first');

    // A face or lock in one panel stays there.
    await studio.setBlockFace(blockScope(panel, 'studio-magazine'), head, mono.id);
    studio.toggleLock(blockScope(panel, 'studio-magazine'), 'headline');
    expect(studio.faceOf(blockScope(FIRST_PANEL, 'studio-magazine'), head)).toBeNull();
    expect(studio.isLocked(blockScope(FIRST_PANEL, 'studio-magazine'), 'headline')).toBe(false);

    // A panel on another layout file keeps its own text.
    await second.open('studio-web-hero');
    const before = second.draft!.blocks[0]!.text;
    studio.setText(FIRST_PANEL, 0, 'Only the magazine');
    expect(first.draft!.blocks[0]!.text).toBe('Only the magazine');
    expect(second.draft!.blocks[0]!.text).toBe(before);
  });

  it('each compared pairing keeps its own "Try for" list', async () => {
    const studio = useStudioStore();
    const first = layoutsOf(FIRST_PANEL);
    await first.open('studio-magazine');
    studio.toggleCompare();
    const panel = studio.panels[1]!;
    const roles = first.draft!.roles!;
    const headline = roles.find((r) => r.role === 'headline')!;
    const body = roles.find((r) => r.role === 'body')!;
    const one = blockScope(FIRST_PANEL, 'studio-magazine');
    const two = blockScope(panel, 'studio-magazine');
    await Promise.all([
      studio.loadTries(one, first.draft!, headline, inter),
      studio.loadTries(two, layoutsOf(panel).draft!, body, inter),
    ]);
    expect(studio.triesOf(one)?.key).toBe(studio.roleKey(one, 'headline'));
    expect(studio.triesOf(two)?.key).toBe(studio.roleKey(two, 'body'));
    expect(studio.triesOf(one)?.loading).toBe(false);
    studio.closePanel(panel);
    expect(studio.triesOf(two)).toBeNull();
    expect(studio.triesOf(one)).not.toBeNull();
  });

  it('names each pairing after its own Headline and Body faces', async () => {
    const studio = useStudioStore();
    await layoutsOf(FIRST_PANEL).open('studio-magazine');
    studio.toggleCompare();
    const panel = studio.panels[1]!;
    const head = blocksOf(layoutsOf(panel).draft!, 'headline');
    await studio.setBlocksFace(blockScope(panel, 'studio-magazine'), head, mono.id);
    const names = (p: number) => {
      const f = studio.pairingOf(p, inter);
      return f && [f.headline.familyName, f.body.familyName];
    };
    expect(names(FIRST_PANEL)).toEqual(['Inter', 'Inter']);
    expect(names(panel)).toEqual([mono.familyName, 'Inter']);
    expect(studio.pairingOf(panel, null)).toBeNull();
  });

  it('closing the first pairing keeps the second in its place, with its roles', async () => {
    const studio = useStudioStore();
    const first = layoutsOf(FIRST_PANEL);
    await first.open('studio-magazine');
    studio.toggleCompare();
    const panel = studio.panels[1]!;
    const second = layoutsOf(panel);
    await second.open('studio-web-hero');
    const scope = blockScope(panel, 'studio-web-hero');
    const head = blocksOf(second.draft!, 'headline');
    await studio.setBlocksFace(scope, head, mono.id);
    studio.toggleLock(scope, 'headline');
    second.setBlockText(0, 'Kept text');

    expect(studio.promote()).toBe('studio-web-hero');
    expect(studio.panels).toEqual([FIRST_PANEL]);
    expect(first.saved?.id).toBe('studio-web-hero');
    expect(first.draft!.blocks[0]!.text).toBe('Kept text');
    const own = blockScope(FIRST_PANEL, 'studio-web-hero');
    expect(studio.faceOf(own, head[0]!)?.id).toBe(mono.id);
    expect(studio.isLocked(own, 'headline')).toBe(true);
    // Nothing of the closed panel is left behind.
    expect(Object.keys(studio.blockFaces).some((k) => k.startsWith(`p${panel}/`))).toBe(false);
    expect(Object.keys(studio.locks).some((k) => k.startsWith(`p${panel}/`))).toBe(false);
    // Alone, the first panel has nothing to give way to.
    expect(studio.promote()).toBeNull();
  });

  it("copies a layout scope's roles to another; the first keeps its own", async () => {
    const studio = useStudioStore();
    await studio.setBlockFace('studio-magazine', 0, mono.id);
    studio.toggleLock('studio-magazine', 'headline');
    await studio.setBlockFace('lora-inter', 3, inter.id);
    studio.copyScope('studio-magazine', 'lora-inter');
    expect(studio.faceOf('lora-inter', 0)?.id).toBe(mono.id);
    expect(studio.faceOf('lora-inter', 3)).toBeNull();
    expect(studio.isLocked('lora-inter', 'headline')).toBe(true);
    expect(studio.faceOf('studio-magazine', 0)?.id).toBe(mono.id);
    expect(studio.isLocked('studio-magazine', 'headline')).toBe(true);
  });

  it('a layout is named after a pairing by its name or that name with a number', () => {
    expect(namedAfter('Lora + Inter', 'Lora + Inter')).toBe(true);
    expect(namedAfter('lora + inter 2', 'Lora + Inter')).toBe(true);
    expect(namedAfter('Lora + Inter copy', 'Lora + Inter')).toBe(false);
    expect(namedAfter('Lora + Inter Display', 'Lora + Inter')).toBe(false);
    expect(namedAfter('Magazine page', 'Lora + Inter')).toBe(false);
  });
});
