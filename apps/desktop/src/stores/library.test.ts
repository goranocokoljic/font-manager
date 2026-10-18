import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, EXTRA_ID_BASE } from '../api/mock';
import { useLibraryStore } from './library';

let client: FmClient;

beforeEach(() => {
  setActivePinia(createPinia());
});

afterEach(() => setClient(undefined));

describe('library store', () => {
  it('loads the families, and no faces until they are asked for', async () => {
    client = createMockClient();
    const family = vi.spyOn(client, 'getFamily');
    setClient(client);
    const store = useLibraryStore();
    await store.load();
    expect(store.loaded).toBe(true);
    expect(store.families.map((f) => f.name)).toEqual([
      'Inter',
      'JetBrains Mono',
      'Legacy Grotesk',
    ]);
    expect(store.faces.size).toBe(0);
    expect(family).not.toHaveBeenCalled();
  });

  it('fetches the faces of a family once, however often they are asked for', async () => {
    client = createMockClient({ extraFaces: 2, stylesPerFamily: 4 });
    const family = vi.spyOn(client, 'getFamily');
    setClient(client);
    const store = useLibraryStore();
    await store.load();
    const id = EXTRA_ID_BASE;
    const [a, b] = await Promise.all([store.facesOf(id), store.facesOf(id)]);
    await store.facesOf(id);
    expect(a).toBe(b);
    expect(a.map((f) => f.style)).toEqual(['Regular', 'Italic', 'Bold', 'Bold Italic']);
    expect(store.faces.get(id)).toBe(a);
    expect(family).toHaveBeenCalledTimes(1);
  });

  it('remembers a family whose faces could not be fetched, until a refresh', async () => {
    client = createMockClient();
    setClient(client);
    const store = useLibraryStore();
    await store.load();
    await expect(store.facesOf(999)).rejects.toMatchObject({ code: 'notFound' });
    expect(store.failed.has(999)).toBe(true);
    await store.facesOf(1);
    await store.refresh();
    expect(store.failed.size).toBe(0);
    expect(store.faces.size).toBe(0); // faces load again after a library change
  });

  it('pages through more families than one request returns', async () => {
    client = createMockClient({ extraFaces: 1200 });
    const list = vi.spyOn(client, 'listFamilies');
    setClient(client);
    const store = useLibraryStore();
    await store.load();
    expect(store.families).toHaveLength(1203);
    expect(store.families.at(-1)!.id).toBe(EXTRA_ID_BASE + 1199);
    expect(list).toHaveBeenCalledTimes(3);
  });

  it('reloads when the library changes', async () => {
    client = createMockClient();
    setClient(client);
    const store = useLibraryStore();
    await store.load();
    const families = vi.spyOn(client, 'listFamilies');
    await client.removeSource(1); // emits libraryChanged
    await vi.waitFor(() => expect(families).toHaveBeenCalled());
    await store.stop();
  });

  it('updates activation state in place when faces are activated', async () => {
    client = createMockClient();
    setClient(client);
    const store = useLibraryStore();
    await store.load();
    const family = store.families[0]!;
    const before = await store.facesOf(family.id);
    expect(family.activation).toBe('inactive');
    const face = before[0]!;
    const getFamily = vi.spyOn(client, 'getFamily');
    await client.activate({ target: { kind: 'faces', faces: [face.id] }, mode: 'permanent' });
    await vi.waitFor(() =>
      expect(store.faces.get(family.id)?.find((f) => f.id === face.id)?.active).toBe('permanent'),
    );
    expect(store.families[0]!.activation).not.toBe('inactive');
    // Only the family holding the face is fetched again; other cached faces stay.
    expect(getFamily.mock.calls.map(([id]) => id)).toEqual([family.id]);
    await client.deactivate({ target: { kind: 'faces', faces: [face.id] } });
    await vi.waitFor(() => expect(store.families[0]!.activation).toBe('inactive'));
    expect(store.faces.get(family.id)?.find((f) => f.id === face.id)?.active).toBeNull();
    await store.stop();
  });

  it('keeps every face current when activation changes overlap', async () => {
    client = createMockClient();
    setClient(client);
    const store = useLibraryStore();
    await store.load();
    const [first, second] = store.families;
    const a = (await store.facesOf(first!.id))[0]!;
    const b = (await store.facesOf(second!.id))[0]!;
    // Two changes in different families; the second arrives before the first is applied.
    const one = client.activate({ target: { kind: 'faces', faces: [a.id] }, mode: 'session' });
    const two = client.activate({
      target: { kind: 'faces', faces: [b.id] },
      mode: 'permanent',
      onConflict: 'replace', // the mock gives this face a system clash
    });
    await Promise.all([one, two]);
    const activeOf = (family: number, face: number) =>
      store.faces.get(family)?.find((f) => f.id === face)?.active;
    await vi.waitFor(() => {
      expect(activeOf(second!.id, b.id)).toBe('permanent');
      expect(activeOf(first!.id, a.id)).toBe('session');
    });
    await store.stop();
  });

  it('keeps the error of a failed load', async () => {
    client = createMockClient();
    client.listFamilies = () =>
      Promise.reject({ code: 'io', message: 'The index could not be read.', detail: null });
    setClient(client);
    const store = useLibraryStore();
    await store.load();
    expect(store.error?.message).toBe('The index could not be read.');
    expect(store.families).toEqual([]);
    expect(store.loaded).toBe(true);
  });

  it('orders the families by the API order and keeps the faces it already has', async () => {
    client = createMockClient();
    const list = vi.spyOn(client, 'listFamilies');
    const family = vi.spyOn(client, 'getFamily');
    setClient(client);
    const store = useLibraryStore();
    await store.setOrder({ by: 'manufacturer', descending: false }); // before load: no request
    expect(list).not.toHaveBeenCalled();
    await store.load();
    expect(list).toHaveBeenLastCalledWith(
      expect.objectContaining({ order: { by: 'manufacturer', descending: false } }),
    );
    // JetBrains, Old Foundry, rsms
    expect(store.families.map((f) => f.name)).toEqual([
      'JetBrains Mono',
      'Legacy Grotesk',
      'Inter',
    ]);
    await store.facesOf(1);
    await store.setOrder({ by: 'manufacturer', descending: true });
    expect(store.families.map((f) => f.name)).toEqual([
      'Inter',
      'Legacy Grotesk',
      'JetBrains Mono',
    ]);
    expect(store.faces.has(1)).toBe(true); // a new order is not a library change
    const calls = list.mock.calls.length;
    await store.setOrder({ by: 'manufacturer', descending: true }); // same order: nothing
    expect(list.mock.calls.length).toBe(calls);
    await store.setOrder(null);
    expect(list.mock.lastCall?.[0]).not.toHaveProperty('order');
    expect(store.families[0]!.name).toBe('Inter');
    expect(family).toHaveBeenCalledTimes(1);
  });

  it('an activation reload keeps the sort order, and a new order overtakes it', async () => {
    client = createMockClient();
    setClient(client);
    const store = useLibraryStore();
    await store.setOrder({ by: 'manufacturer', descending: false });
    await store.load();
    const face = (await store.facesOf(1))[0]!;
    const list = client.listFamilies.bind(client);
    let release!: () => void;
    const gate = new Promise<void>((resolve) => (release = resolve));
    let held = false;
    // Hold the activation reload's list answer (in the old order) until a new order is set.
    client.listFamilies = async (request) => {
      const answer = await list(request);
      if (!held) {
        held = true;
        await gate;
      }
      return answer;
    };
    await client.activate({ target: { kind: 'faces', faces: [face.id] }, mode: 'session' });
    await vi.waitFor(() => expect(held).toBe(true));
    await store.setOrder({ by: 'manufacturer', descending: true });
    release();
    await vi.waitFor(() =>
      expect(store.faces.get(1)?.find((f) => f.id === face.id)?.active).toBe('session'),
    );
    expect(store.families.map((f) => f.name)).toEqual([
      'Inter',
      'Legacy Grotesk',
      'JetBrains Mono',
    ]);
    expect(store.families[0]!.activation).not.toBe('inactive');
    await store.stop();
  });

  it('a sort fetch overtaken by an activation does not bring back the old state', async () => {
    client = createMockClient();
    setClient(client);
    const store = useLibraryStore();
    await store.load();
    const face = (await store.facesOf(1))[0]!;
    const list = client.listFamilies.bind(client);
    let release!: () => void;
    const gate = new Promise<void>((resolve) => (release = resolve));
    let held = false;
    // Hold the sort fetch's answer (from before the activation) until the activation lands.
    client.listFamilies = async (request) => {
      const answer = await list(request);
      if (!held) {
        held = true;
        await gate;
      }
      return answer;
    };
    const sorting = store.setOrder({ by: 'name', descending: true });
    await vi.waitFor(() => expect(held).toBe(true));
    await client.activate({ target: { kind: 'faces', faces: [face.id] }, mode: 'session' });
    await vi.waitFor(() =>
      expect(store.families.find((f) => f.id === 1)?.activation).not.toBe('inactive'),
    );
    release();
    await sorting;
    expect(store.families.map((f) => f.name)).toEqual([
      'Legacy Grotesk',
      'JetBrains Mono',
      'Inter',
    ]);
    expect(store.families.find((f) => f.id === 1)?.activation).not.toBe('inactive');
    await store.stop();
  });

  it('a refresh overtaken by a new order still forgets the faces', async () => {
    client = createMockClient();
    setClient(client);
    const store = useLibraryStore();
    await store.load();
    await store.facesOf(1);
    const refreshing = store.refresh();
    await store.setOrder({ by: 'name', descending: true });
    await refreshing;
    expect(store.faces.size).toBe(0);
    expect(store.families[0]!.name).toBe('Legacy Grotesk');
  });
});
