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
});
