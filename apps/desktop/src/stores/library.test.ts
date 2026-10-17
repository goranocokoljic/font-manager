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
  it('loads the faces of every family', async () => {
    client = createMockClient();
    setClient(client);
    const store = useLibraryStore();
    await store.load();
    expect(store.loaded).toBe(true);
    expect(store.faces.map((f) => f.id)).toEqual([1, 6, 7]);
  });

  it('pages through more families than one request returns', async () => {
    client = createMockClient({ extraFaces: 1200 });
    const list = vi.spyOn(client, 'listFamilies');
    setClient(client);
    const store = useLibraryStore();
    await store.load();
    expect(store.faces).toHaveLength(1203);
    expect(store.faces.at(-1)!.id).toBe(EXTRA_ID_BASE + 1199);
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
    expect(store.faces).toEqual([]);
    expect(store.loaded).toBe(true);
  });
});
