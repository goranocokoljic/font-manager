import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient, mockFaces } from '../api/mock';
import { STUDIO_RESULTS, useStudioStore } from './studio';
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

  it('drops the answer to an older search', async () => {
    const studio = useStudioStore();
    const older = studio.search('book', 0, 'mono');
    await studio.search('book', 1, 'inter');
    await older;
    expect(studio.searchFor).toBe('book#1');
    expect(studio.results.map((h) => h.family.name)).not.toContain(mono.familyName);
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
