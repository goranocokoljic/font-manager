import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, EXTRA_ID_BASE } from '../api/mock';
import { useFamilyPageStore, parseFamilyId } from './familyPage';
import { useFontListStore } from './fontList';
import { useLibraryStore } from './library';
import { useSearchStore } from './search';

let client: FmClient;

async function setup() {
  client = createMockClient({ extraFaces: 3, stylesPerFamily: 3 });
  setClient(client);
  const library = useLibraryStore();
  await library.load();
  return { page: useFamilyPageStore(), list: useFontListStore(), library };
}

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => setClient(undefined));

describe('the family page store', () => {
  it('reads only whole numbers as a family id', () => {
    expect(parseFamilyId('7')).toBe(7);
    expect(parseFamilyId('abc')).toBeNull();
    expect(parseFamilyId('-1')).toBeNull();
  });

  it('knows the place of the family and its neighbours in the list', async () => {
    const { page, list } = await setup();
    const ids = list.families.map((f) => f.id);
    await page.show(ids[1]!);
    expect(page.index).toBe(1);
    expect(page.total).toBe(ids.length);
    expect(page.previous).toBe(ids[0]);
    expect(page.next).toBe(ids[2]);
    expect(page.summary?.name).toBe(list.families[1]!.name);
    expect(page.faces.length).toBe(list.families[1]!.styleCount);
  });

  it('has no neighbour past either end', async () => {
    const { page, list } = await setup();
    const ids = list.families.map((f) => f.id);
    await page.show(ids[0]!);
    expect(page.previous).toBeNull();
    expect(page.next).toBe(ids[1]);
    await page.show(ids[ids.length - 1]!);
    expect(page.next).toBeNull();
    expect(page.previous).toBe(ids[ids.length - 2]);
  });

  it('follows the current filtered order', async () => {
    const { page, list } = await setup();
    const search = useSearchStore();
    const all = list.families;
    const kept = all[all.length - 1]!;
    await page.show(kept.id);
    expect(page.index).toBe(all.length - 1);
    search.query = kept.name;
    await search.run();
    await flushPromises();
    expect(list.families.map((f) => f.id)).toContain(kept.id);
    expect(list.families.length).toBeLessThan(all.length);
    expect(page.total).toBe(list.families.length);
    expect(page.index).toBe(list.families.findIndex((f) => f.id === kept.id));
  });

  it('keeps showing a family the list hides, without a place or neighbours', async () => {
    const { page, list } = await setup();
    const search = useSearchStore();
    const hidden = list.families[0]!;
    const other = list.families[list.families.length - 1]!;
    search.query = other.name;
    await search.run();
    await flushPromises();
    expect(list.families.map((f) => f.id)).not.toContain(hidden.id);
    await page.show(hidden.id);
    expect(page.index).toBe(-1);
    expect(page.previous).toBeNull();
    expect(page.next).toBeNull();
    expect(page.summary?.name).toBe(hidden.name);
  });

  it('drops the answer for a family the user already left', async () => {
    const { page, list } = await setup();
    const [first, second] = list.families;
    const real = client.getFamily.bind(client);
    let release = () => {};
    const gate = new Promise<void>((resolve) => (release = resolve));
    vi.spyOn(client, 'getFamily').mockImplementation(async (id) => {
      if (id === first!.id) await gate;
      return real(id);
    });
    const slow = page.show(first!.id);
    await page.show(second!.id);
    release();
    await slow;
    expect(page.detail?.summary.id).toBe(second!.id);
  });

  it('reports a family that does not exist', async () => {
    const { page } = await setup();
    await page.show(EXTRA_ID_BASE + 9_999_999);
    expect(page.error).not.toBeNull();
    expect(page.detail).toBeNull();
  });

  it('activates a family and then deactivates it once it is fully active', async () => {
    const { page, list } = await setup();
    const family = list.families[list.families.length - 1]!;
    await page.show(family.id);
    await page.toggleActivation('session');
    let active = await client.listActive();
    expect(page.faces.every((f) => active.some((a) => a.face === f.id))).toBe(true);
    await useLibraryStore().refresh();
    expect(page.summary?.activation).toBe('active');
    await page.toggleActivation('session');
    active = await client.listActive();
    expect(page.faces.some((f) => active.some((a) => a.face === f.id))).toBe(false);
  });

  it('keeps a permanent face permanent when A activates the rest of a family', async () => {
    const { page, list } = await setup();
    const family = list.families[list.families.length - 1]!;
    await page.show(family.id);
    const [first, ...rest] = page.faces;
    await client.activate({
      target: { kind: 'faces', faces: [first!.id] },
      mode: 'permanent',
      onConflict: 'replace',
    });
    await useLibraryStore().refresh();
    expect(page.summary?.activation).toBe('partial');
    await page.toggleActivation('session');
    const active = await client.listActive();
    expect(active.find((a) => a.face === first!.id)?.mode).toBe('permanent');
    for (const f of rest) expect(active.find((a) => a.face === f.id)?.mode).toBe('session');
  });

  it('favorites every face of the family, and takes it back', async () => {
    const { page, list } = await setup();
    await page.show(list.families[list.families.length - 1]!.id);
    expect(page.favorite).toBe(false);
    await page.toggleFavorite();
    expect(page.favorite).toBe(true);
    for (const f of page.faces) expect((await client.getFace(f.id)).favorite).toBe(true);
    await page.toggleFavorite();
    expect(page.favorite).toBe(false);
    for (const f of page.faces) expect((await client.getFace(f.id)).favorite).toBe(false);
  });

  it('keeps the state when the API refuses the favorite', async () => {
    const { page, list } = await setup();
    await page.show(list.families[0]!.id);
    vi.spyOn(client, 'updateUserMeta').mockRejectedValue({ kind: 'internal', message: 'no' });
    await page.toggleFavorite();
    expect(page.favorite).toBe(false);
  });

  it('keeps the section when the family changes', async () => {
    const { page, list } = await setup();
    await page.show(list.families[0]!.id);
    page.section = 'features';
    await page.show(list.families[1]!.id);
    expect(page.section).toBe('features');
  });
});

describe('the library hand-over', () => {
  it('does not forget the faces when it loads while still subscribed', async () => {
    const { library, list } = await setup();
    const id = list.families[0]!.id;
    await library.facesOf(id);
    const listFamilies = vi.spyOn(client, 'listFamilies');
    await library.load();
    expect(listFamilies).not.toHaveBeenCalled();
    expect(library.faces.has(id)).toBe(true);
  });

  it('loads again after a stop', async () => {
    const { library } = await setup();
    await library.stop();
    const listFamilies = vi.spyOn(client, 'listFamilies');
    await library.load();
    expect(listFamilies).toHaveBeenCalled();
  });

  it('tries again when the last list failed', async () => {
    const { library } = await setup();
    vi.spyOn(client, 'listFamilies').mockRejectedValueOnce({ kind: 'internal', message: 'no' });
    await library.refresh();
    expect(library.error).not.toBeNull();
    await library.load();
    expect(library.error).toBeNull();
  });
});
