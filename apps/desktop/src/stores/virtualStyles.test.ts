import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient, useClient } from '../api/client';
import { createMockClient, mockFaces } from '../api/mock';
import { faceKey, familyKey, rowLabel, styleName, virtualKey } from '../list/rows';
import { useCollectionsStore } from './collections';
import { useFontListStore } from './fontList';
import { useLibraryStore } from './library';
import { useSearchStore } from './search';
import { useVirtualStylesStore } from './virtualStyles';

const INTER = mockFaces[0]!.face; // variable, family 1
const MONO = mockFaces[1]!.face; // static, another family

async function setup() {
  setClient(createMockClient());
  await useLibraryStore().load();
  const client = useClient();
  const brand = await client.createCollection({ name: 'Brand' });
  await client.addToCollection({ collection: brand.id, faces: [MONO.id] });
  await useCollectionsStore().refresh();
  return { brand, list: useFontListStore(), pinned: useVirtualStylesStore() };
}

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => setClient(undefined));

describe('virtual styles in the font list', () => {
  it('shows none outside a collection', async () => {
    const { brand, pinned } = await setup();
    await useClient().pinInstance({ face: INTER.id, name: 'Wide', collection: brand.id });
    await pinned.refresh();
    expect(pinned.styles).toEqual([]);
  });

  it('lists a pinned style as its own row in its collection, at its coordinates', async () => {
    const { brand, list, pinned } = await setup();
    useCollectionsStore().show(brand.id);
    await flushPromises();
    // Only the static family is a member: Inter is not listed yet.
    expect(list.families.map((f) => f.id)).toEqual([MONO.family]);

    const style = await pinned.pin({
      face: INTER.id,
      name: 'Semibold Pin',
      coordinates: [{ tag: 'wght', value: 620 }],
      collection: brand.id,
    });
    await flushPromises();
    // The family held only through the pin is listed, and shows only the pin.
    expect(list.families.map((f) => f.id)).toEqual([MONO.family, INTER.family]);
    list.expand(INTER.family);
    await flushPromises();
    const keys = list.rows.map((r) => r.key);
    expect(keys).toEqual([familyKey(MONO.family), familyKey(INTER.family), virtualKey(style.id)]);
    expect(keys).not.toContain(faceKey(INTER.id));
    const row = list.rows[2]!;
    expect(row.kind).toBe('face');
    if (row.kind !== 'face') return;
    expect(row.face.id).toBe(INTER.id);
    expect(styleName(row)).toBe('Semibold Pin');
    expect(row.virtual?.coordinates).toEqual([{ tag: 'wght', value: 620 }]);
    expect(rowLabel(row)).toContain('Semibold Pin, pinned instance');
    // The family row previews the pinned face.
    expect(list.previewFace(INTER.family)?.id).toBe(INTER.id);
  });

  it('puts a pinned style after the matching styles of a member family', async () => {
    const { brand, list, pinned } = await setup();
    await useClient().addToCollection({ collection: brand.id, faces: [INTER.id] });
    const style = await useClient().pinInstance({
      face: INTER.id,
      name: 'Heavy',
      coordinates: [{ tag: 'wght', value: 850 }],
      collection: brand.id,
    });
    useCollectionsStore().show(brand.id);
    await flushPromises();
    list.expand(INTER.family);
    await flushPromises();
    const keys = list.rows.map((r) => r.key);
    const at = keys.indexOf(familyKey(INTER.family));
    expect(keys.slice(at, at + 3)).toEqual([
      familyKey(INTER.family),
      faceKey(INTER.id),
      virtualKey(style.id),
    ]);
    const row = list.rows[at + 2]!;
    expect(row.kind === 'face' && [row.position, row.count]).toEqual([2, 2]);
    expect(pinned.byFamily.get(INTER.family)?.map((p) => p.style.name)).toEqual(['Heavy']);
  });

  it('drops the rows when the collection closes', async () => {
    const { brand, list, pinned } = await setup();
    await useClient().pinInstance({ face: INTER.id, name: 'Wide', collection: brand.id });
    useCollectionsStore().show(brand.id);
    await flushPromises();
    expect(pinned.styles).toHaveLength(1);
    useSearchStore().clear();
    await flushPromises();
    expect(pinned.styles).toEqual([]);
    expect(list.rows.some((r) => r.key.startsWith('virtual:'))).toBe(false);
  });

  it('removes a pinned style through the API and drops its row', async () => {
    const { brand, list, pinned } = await setup();
    const style = await useClient().pinInstance({
      face: INTER.id,
      name: 'Wide',
      collection: brand.id,
    });
    useCollectionsStore().show(brand.id);
    await flushPromises();
    list.expand(INTER.family);
    await flushPromises();
    expect(list.rows.map((r) => r.key)).toContain(virtualKey(style.id));
    await pinned.remove(style.id);
    await flushPromises();
    expect(await useClient().listVirtualStyles({ collection: brand.id })).toEqual([]);
    expect(list.rows.map((r) => r.key)).not.toContain(virtualKey(style.id));
  });
});
