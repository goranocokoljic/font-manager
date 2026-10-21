import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useLibraryStore } from './library';
import { useTaggingStore } from './tagging';
import { useTagsStore } from './tags';

let client: FmClient;

/** Faces 1 and 6 carry "Brand"; face 1 also "Body". One extra family has 3 styles. */
async function setup() {
  client = createMockClient({
    faceTags: { 1: ['Brand', 'Body'], 6: ['Brand'] },
    extraFaces: 1,
    stylesPerFamily: 3,
  });
  setClient(client);
  const library = useLibraryStore();
  await library.load();
  const tags = useTagsStore();
  await tags.refresh();
  const family = library.families.find((f) => f.styleCount === 3)!.id;
  return { tags, tagging: useTaggingStore(), family };
}

const id = (name: string) => useTagsStore().idOf(name)!;
const tagsOf = async (face: number) => (await client.getFace(face)).tags;

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => setClient(undefined));

describe('multi-tagging panel', () => {
  it('shows all, some and none, and applies several toggles in one tag and one untag call', async () => {
    const { tagging } = await setup();
    await tagging.openFor({ faces: [1, 6, 7], families: [] });
    expect(tagging.open).toBe(true);
    expect(tagging.faces).toEqual([1, 6, 7]);
    expect(tagging.stateOf(id('Brand'))).toBe('some');
    expect(tagging.stateOf(id('Body'))).toBe('some');

    const tag = vi.spyOn(client, 'tagFaces');
    const untag = vi.spyOn(client, 'untagFaces');
    tagging.toggle(id('Brand')); // some → all
    tagging.toggle(id('Body')); // some → all
    tagging.toggle(id('Body')); // all → none
    expect(tagging.stateOf(id('Brand'))).toBe('all');
    expect(tagging.stateOf(id('Body'))).toBe('none');
    expect(tagging.pending).toBe(2);

    const brand = id('Brand');
    const body = id('Body');
    expect(await tagging.apply()).toBe(true);
    expect(tag).toHaveBeenCalledTimes(1);
    expect(tag).toHaveBeenCalledWith({ faces: [1, 6, 7], families: [], tags: ['Brand'] });
    expect(untag).toHaveBeenCalledTimes(1);
    expect(untag).toHaveBeenCalledWith({ faces: [1, 6, 7], tags: [body] });
    expect(tagging.open).toBe(false);
    expect(await tagsOf(1)).toEqual(['Brand']);
    expect(await tagsOf(7)).toEqual(['Brand']);
    expect(useTagsStore().idOf('Brand')).toBe(brand);
  });

  it('a third Space on a mixed tag goes back to what the fonts carry', async () => {
    const { tagging } = await setup();
    await tagging.openFor({ faces: [1, 7], families: [] });
    const brand = id('Brand');
    tagging.toggle(brand);
    tagging.toggle(brand);
    tagging.toggle(brand);
    expect(tagging.stateOf(brand)).toBe('some');
    expect(tagging.pending).toBe(0);
  });

  it('filters fuzzily and makes a new tag on every font for a name no tag has', async () => {
    const { tagging, tags } = await setup();
    await tagging.openFor({ faces: [6, 7], families: [] });
    tagging.query = 'bd';
    expect(tagging.rows.map((r) => r.tag.name).sort()).toEqual(['Body', 'Brand']);
    tagging.query = 'brand';
    expect(tagging.newName).toBeNull();
    tagging.query = ' Display ';
    expect(tagging.rows).toEqual([]);
    expect(tagging.newName).toBe('Display');
    expect(await tagging.create()).toBe(true);
    expect(tagging.query).toBe('');
    expect(tags.tags.find((t) => t.name === 'Display')?.faceCount).toBe(2);
    expect(tagging.stateOf(id('Display'))).toBe('all');
  });

  it('tags whole families and removes a tag from every face of them', async () => {
    const { tagging, family } = await setup();
    await tagging.openFor({ faces: [], families: [family] });
    const members = tagging.faces;
    expect(members).toHaveLength(3);
    tagging.toggle(id('Brand'));
    await tagging.apply();
    for (const face of members) expect(await tagsOf(face)).toContain('Brand');
    await tagging.openFor({ faces: [], families: [family] });
    expect(tagging.stateOf(id('Brand'))).toBe('all');
    tagging.toggle(id('Brand'));
    await tagging.apply();
    for (const face of members) expect(await tagsOf(face)).not.toContain('Brand');
  });

  it('does not open for an empty selection', async () => {
    const { tagging } = await setup();
    await tagging.openFor({ faces: [], families: [] });
    expect(tagging.open).toBe(false);
  });
});

describe('Tags group drag state', () => {
  it('unfolds while fonts are dragged, marks the tags they carry, and folds back', async () => {
    const { tagging } = await setup();
    tagging.folded = true;
    await tagging.startDrag({ faces: [6], families: [] });
    expect(tagging.folded).toBe(false);
    expect([...tagging.dragCarried]).toEqual([id('Brand')]);
    tagging.endDrag();
    expect(tagging.folded).toBe(true);
    expect(tagging.dragCarried.size).toBe(0);

    // Unfolded before: stays unfolded.
    tagging.folded = false;
    await tagging.startDrag({ faces: [1], families: [] });
    tagging.endDrag();
    expect(tagging.folded).toBe(false);
  });

  it('a drop on a tag tags every dropped font, whole families included', async () => {
    const { tagging, tags, family } = await setup();
    const members = (await client.selectionTags({ faces: [], families: [family] })).faces;
    expect(await tagging.dropOn('Body', { faces: [6], families: [family] })).toBe(true);
    expect(await tagsOf(6)).toContain('Body');
    for (const face of members) expect(await tagsOf(face)).toContain('Body');
    expect(tags.tags.find((t) => t.name === 'Body')?.faceCount).toBe(
      new Set([1, 6, ...members]).size,
    );
  });
});
