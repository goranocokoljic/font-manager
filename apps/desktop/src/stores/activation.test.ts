import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { FamilySummary } from '../api/types';
import { faceKey, familyKey, type ListRow } from '../list/rows';
import { useActivationStore } from './activation';
import { useToastStore } from './toast';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

let client: FmClient;

beforeEach(() => {
  setActivePinia(createPinia());
  client = createMockClient();
  setClient(client);
});

afterEach(async () => {
  await useActivationStore().stop();
  setClient(undefined);
});

describe('activation store', () => {
  it('counts the active faces and follows activation events', async () => {
    const store = useActivationStore();
    await store.load();
    expect(store.count).toBe(0);

    const listActive = vi.spyOn(client, 'listActive');
    await client.activate({ target: { kind: 'faces', faces: [1, 6] }, onConflict: 'replace' });
    await vi.waitFor(() => expect(store.count).toBe(2));
    expect(listActive).toHaveBeenCalledTimes(1);

    await client.deactivate({ target: { kind: 'faces', faces: [1] } });
    await vi.waitFor(() => expect(store.count).toBe(1));
  });

  it('stops following events after stop', async () => {
    const store = useActivationStore();
    await store.load();
    await store.stop();
    await client.activate({ target: { kind: 'faces', faces: [1] } });
    await Promise.resolve();
    expect(store.count).toBe(0);
  });

  it('keeps the count when the API fails', async () => {
    const store = useActivationStore();
    await store.load();
    await client.activate({ target: { kind: 'faces', faces: [1] } });
    await vi.waitFor(() => expect(store.count).toBe(1));
    vi.spyOn(client, 'listActive').mockRejectedValue({ code: 'io', message: 'x', detail: null });
    await store.refresh();
    expect(store.count).toBe(1);
  });

  const familyRow = (family: FamilySummary): ListRow => ({
    kind: 'family',
    key: familyKey(family.id),
    family,
    position: 1,
    expanded: false,
  });
  const activeModes = async () =>
    Object.fromEntries((await client.listActive()).map((a) => [a.face, a.mode]));

  it('activates selected families and styles in one action, with a toast that undoes it', async () => {
    client = createMockClient({ extraFaces: 2, stylesPerFamily: 3 });
    setClient(client);
    const store = useActivationStore();
    const toasts = useToastStore();
    const families = (await client.listFamilies({})).items.filter((f) => f.styleCount === 3);
    expect(families).toHaveLength(2);
    const [first, second] = families as [FamilySummary, FamilySummary];
    const styles = (await client.getFamily(second.id)).faces;
    const face = { ...styles[0]!, active: null };
    const rows: ListRow[] = [
      familyRow(first),
      { kind: 'face', key: faceKey(face.id), face, family: second, position: 1, count: 3 },
    ];

    await store.toggleRows(rows, 'session');
    const firstIds = (await client.getFamily(first.id)).faces.map((f) => f.id);
    expect(await activeModes()).toEqual({
      ...Object.fromEntries(firstIds.map((id) => [id, 'session'])),
      [face.id]: 'session',
    });
    expect(toasts.toast?.message).toBe('Activated 4 fonts for this session.');
    expect(toasts.toast?.action?.label).toBe('Undo');

    toasts.act();
    await vi.waitFor(() => expect(toasts.toast?.message).toBe('Undone.'));
    expect(await activeModes()).toEqual({});
  });

  it('A on a partly permanent family activates the rest and keeps the permanent style', async () => {
    client = createMockClient({ extraFaces: 1, stylesPerFamily: 3 });
    setClient(client);
    const store = useActivationStore();
    const toasts = useToastStore();
    const fam = (await client.listFamilies({})).items.find((f) => f.styleCount === 3)!;
    const [kept, ...rest] = (await client.getFamily(fam.id)).faces;
    await client.activate({ target: { kind: 'faces', faces: [kept!.id] }, mode: 'permanent' });
    const partial = (await client.getFamily(fam.id)).summary;
    expect(partial.activation).toBe('partial');

    await store.toggleRows([familyRow(partial)], 'session');
    expect(await activeModes()).toEqual({
      [kept!.id]: 'permanent',
      ...Object.fromEntries(rest.map((f) => [f.id, 'session'])),
    });
    expect(toasts.toast?.message).toBe('Activated 2 fonts for this session.');
  });

  it('deactivates all and undoes it in the old modes', async () => {
    const store = useActivationStore();
    const toasts = useToastStore();
    await store.load();
    await client.activate({ target: { kind: 'faces', faces: [1] }, mode: 'permanent' });
    await client.activate({ target: { kind: 'faces', faces: [6] }, onConflict: 'replace' });

    await store.deactivateAll();
    expect(await activeModes()).toEqual({});
    expect(toasts.toast?.message).toBe('Deactivated 2 fonts.');
    await vi.waitFor(() => expect(store.count).toBe(0));

    toasts.act();
    await vi.waitFor(() => expect(toasts.toast?.message).toBe('Undone.'));
    expect(await activeModes()).toEqual({ 1: 'permanent', 6: 'session' });
  });

  it('deactivates everything not in a set and undoes it', async () => {
    client = createMockClient({ sets: [{ id: 1, name: 'Client', faces: [1] }] });
    setClient(client);
    const store = useActivationStore();
    const toasts = useToastStore();
    await client.activate({ target: { kind: 'faces', faces: [1, 6] }, onConflict: 'replace' });

    await store.deactivateAllExcept(1);
    expect(await activeModes()).toEqual({ 1: 'session' });
    expect(toasts.toast?.message).toBe('Deactivated 1 font.');

    toasts.act();
    await vi.waitFor(() => expect(toasts.toast?.message).toBe('Undone.'));
    expect(await activeModes()).toEqual({ 1: 'session', 6: 'session' });
  });

  it('keeps everything active when the set is gone', async () => {
    const store = useActivationStore();
    const toasts = useToastStore();
    await client.activate({ target: { kind: 'faces', faces: [1] } });
    await store.deactivateAllExcept(99);
    expect(await activeModes()).toEqual({ 1: 'session' });
    expect(toasts.toast?.message).toBe('Set 99 was not found.');
  });

  it('offers no undo when nothing changed', async () => {
    const toasts = useToastStore();
    await useActivationStore().deactivateAll();
    expect(toasts.toast?.message).toBe('No fonts were deactivated.');
    expect(toasts.toast?.action).toBeUndefined();
  });

  it('runs one bulk action at a time and reports a failure in the toast', async () => {
    const store = useActivationStore();
    const toasts = useToastStore();
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    const deactivate = vi.spyOn(client, 'deactivate').mockImplementation(async () => {
      await gate;
      throw { code: 'io', message: 'The OS refused.', detail: null };
    });
    const first = store.deactivateAll();
    expect(store.busy).toBe(true);
    await store.deactivateAll();
    release();
    await first;
    expect(deactivate).toHaveBeenCalledTimes(1);
    expect(store.busy).toBe(false);
    expect(toasts.toast?.message).toBe('The OS refused.');
  });
});
