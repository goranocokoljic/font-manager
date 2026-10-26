import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, mockPickedFiles, mockPickedSaves, mockSetFiles } from '../api/mock';
import { useToastStore } from './toast';
import { useSetsStore } from './sets';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

let client: FmClient;

beforeEach(() => {
  setActivePinia(createPinia());
  client = createMockClient({
    sets: [
      { id: 1, name: 'Client', faces: [1] },
      { id: 2, name: 'Book', faces: [6] },
    ],
  });
  setClient(client);
});

afterEach(() => {
  setClient(undefined);
  mockPickedFiles.length = 0;
  mockPickedSaves.length = 0;
});

describe('sets store', () => {
  it('loads the sets with no current set', async () => {
    const store = useSetsStore();
    await store.load();
    expect(store.sets.map((s) => s.name)).toEqual(['Client', 'Book']);
    expect(store.current).toBeNull();
  });

  it('picks a current set, switches it, and clears it on a second pick', async () => {
    const store = useSetsStore();
    await store.load();
    store.pick(1);
    expect(store.current?.name).toBe('Client');
    store.pick(2);
    expect(store.current?.name).toBe('Book');
    store.pick(2);
    expect(store.current).toBeNull();
  });

  it('keeps no current set when the picked set is gone after a reload', async () => {
    const store = useSetsStore();
    await store.load();
    store.pick(2);
    vi.spyOn(client, 'listSets').mockResolvedValue([{ id: 1, name: 'Client', faces: [1] }]);
    await store.load();
    expect(store.current).toBeNull();
  });

  it('reports a failed read', async () => {
    vi.spyOn(client, 'listSets').mockRejectedValue({
      code: 'io',
      message: 'The index is locked.',
      detail: null,
    });
    const store = useSetsStore();
    await store.load();
    expect(store.error?.message).toBe('The index is locked.');
    expect(store.sets).toEqual([]);
  });

  it('exports a set to the picked file, adding the set file extension', async () => {
    const store = useSetsStore();
    await store.load();
    mockPickedSaves.push('/Users/demo/Shared/Client');
    await store.exportSet(1);
    expect(mockSetFiles.get('/Users/demo/Shared/Client.fmset.json')).toEqual({
      name: 'Client',
      fonts: [{ family: 'Inter', style: 'Regular', hash: '1'.padStart(64, '0') }],
    });
    expect(useToastStore().toast?.message).toBe('Saved the set: 1 font.');
  });

  it('suggests the set name as the file name and does nothing on a cancel', async () => {
    const save = vi.spyOn(client, 'saveSetFile');
    const store = useSetsStore();
    await store.load();
    mockPickedSaves.push(null);
    await store.exportSet(2);
    expect(save).toHaveBeenCalledWith(2, 'Book.fmset.json');
    expect(useToastStore().toast?.message).toBeFalsy();
  });

  it('imports a set file, lists what is missing and reloads the sets', async () => {
    const store = useSetsStore();
    await store.load();
    await store.importSet();
    expect(store.imported?.set.name).toBe('Team brand');
    expect(store.imported?.set.faces).toEqual([1]);
    expect(store.imported?.missing.map((m) => `${m.family} ${m.style}`)).toEqual([
      'Quarto Bold',
      'Gotham Rounded Medium',
    ]);
    expect(store.sets.map((s) => s.name)).toContain('Team brand');
    store.dismissImport();
    expect(store.imported).toBeNull();
  });

  it('keeps the last import on a cancel and shows a failed read as a toast', async () => {
    const store = useSetsStore();
    await store.importSet();
    mockPickedFiles.push(null);
    await store.importSet();
    expect(store.imported?.set.name).toBe('Team brand');
    mockPickedFiles.push('/Users/demo/gone.fmset.json');
    await store.importSet();
    expect(useToastStore().toast?.message).toContain('was not found');
    expect(store.imported?.set.name).toBe('Team brand');
  });
});
