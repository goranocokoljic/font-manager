import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, tauriClient, type FmClient } from '../api/client';
import { createMockClient, mockPickedFolders } from '../api/mock';
import type { Event } from '../api/types';
import { useSourcesStore } from './sources';

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
const open = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke, isTauri: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open }));

let client: FmClient;

beforeEach(() => {
  vi.useFakeTimers();
  setActivePinia(createPinia());
  client = createMockClient({ scanStepMs: 10 });
  setClient(client);
});

afterEach(async () => {
  await useSourcesStore().stop();
  setClient(undefined);
  mockPickedFolders.length = 0;
  vi.useRealTimers();
  invoke.mockReset();
  listen.mockReset();
  open.mockReset();
});

describe('sources store', () => {
  it('lists the watched folder and the system fonts', async () => {
    const store = useSourcesStore();
    await store.load();
    expect(store.sources.map((s) => [s.path, s.kind])).toEqual([
      ['/Users/demo/Fonts', 'watched'],
      ['/System/Library/Fonts', 'system'],
    ]);
  });

  it('adds the folder the picker returns and scans it', async () => {
    const scan = vi.spyOn(client, 'scan');
    mockPickedFolders.push('/Users/demo/Brand fonts');
    const store = useSourcesStore();
    await store.load();
    await store.add();
    const added = store.sources.find((s) => s.path === '/Users/demo/Brand fonts');
    expect(added?.kind).toBe('watched');
    expect(scan).toHaveBeenCalledWith({ source: added!.id });
    expect(await client.listSources()).toContainEqual(added);
  });

  it('does nothing when the picker is cancelled', async () => {
    const addSource = vi.spyOn(client, 'addSource');
    mockPickedFolders.push(null);
    const store = useSourcesStore();
    await store.load();
    await store.add();
    expect(addSource).not.toHaveBeenCalled();
    expect(store.sources).toHaveLength(2);
    expect(store.error).toBeNull();
  });

  it('reports a folder that is already in the library', async () => {
    mockPickedFolders.push('/Users/demo/Fonts');
    const store = useSourcesStore();
    await store.load();
    await store.add();
    expect(store.error?.code).toBe('conflict');
    expect(store.sources).toHaveLength(2);
  });

  it('pauses and resumes a source', async () => {
    const store = useSourcesStore();
    await store.load();
    await store.setPaused(1, true);
    expect(store.sources[0]!.paused).toBe(true);
    expect((await client.listSources())[0]!.paused).toBe(true);
    await store.setPaused(1, false);
    expect(store.sources[0]!.paused).toBe(false);
  });

  it('removes a source', async () => {
    const store = useSourcesStore();
    await store.load();
    await store.remove(1);
    expect(store.sources.map((s) => s.id)).toEqual([2]);
    expect((await client.listSources()).map((s) => s.id)).toEqual([2]);
  });

  it('keeps the system source when its removal is refused', async () => {
    const store = useSourcesStore();
    await store.load();
    await store.remove(2);
    expect(store.error?.code).toBe('permissionDenied');
    expect(store.sources.map((s) => s.id)).toEqual([1, 2]);
  });

  it('shows the progress of a scan on its source until it finishes', async () => {
    const store = useSourcesStore();
    await store.load();
    await store.rescan(1);
    expect(store.progressOf(1)).toBeNull();
    await vi.advanceTimersByTimeAsync(30); // progress 0..3 of 7
    expect(store.progressOf(1)).toEqual({ done: 3, total: 7 });
    expect(store.otherScan).toBeNull();
    await vi.advanceTimersByTimeAsync(100);
    expect(store.progressOf(1)).toBeNull();
  });

  it('shows a scan it did not start as library progress', async () => {
    const store = useSourcesStore();
    await store.load();
    await client.scan({ source: null });
    await vi.advanceTimersByTimeAsync(20);
    expect(store.otherScan).toEqual({ done: 2, total: 7 });
    expect(store.progressOf(1)).toBeNull();
    await vi.advanceTimersByTimeAsync(100);
    expect(store.otherScan).toBeNull();
  });

  it('ignores progress that arrives after the scan ended', async () => {
    let send: (e: Event) => void = () => {};
    setClient({
      ...client,
      onEvent: async (h) => {
        send = h;
        return () => {};
      },
      // The finish is emitted before the reply, as the Rust mock does.
      scan: async () => {
        send({ type: 'scanFinished', job: 9, added: 0, removed: 0, problems: 0 });
        return 9;
      },
    });
    const store = useSourcesStore();
    await store.load();
    await store.rescan(1);
    send({ type: 'scanProgress', job: 9, done: 1, total: 2 });
    expect(store.progressOf(1)).toBeNull();
    expect(store.otherScan).toBeNull();
  });

  it('shows why a scan it started failed', async () => {
    let send: (e: Event) => void = () => {};
    setClient({
      ...client,
      onEvent: async (h) => {
        send = h;
        return () => {};
      },
      scan: async () => 5,
    });
    const store = useSourcesStore();
    await store.load();
    await store.rescan(1);
    send({
      type: 'jobFailed',
      job: 5,
      error: { code: 'io', message: 'The folder could not be read.', detail: null },
    });
    expect(store.error?.message).toBe('The folder could not be read.');
    expect(store.progressOf(1)).toBeNull();
  });

  it('reloads the sources when the library changes', async () => {
    const store = useSourcesStore();
    await store.load();
    const list = vi.spyOn(client, 'listSources');
    await client.removeSource(1);
    await vi.waitFor(() => expect(store.sources.map((s) => s.id)).toEqual([2]));
    expect(list).toHaveBeenCalled();
  });
});

describe('tauri client', () => {
  it('calls the source commands with the fm-api argument names', async () => {
    invoke.mockResolvedValue(undefined);
    await tauriClient.listSources();
    await tauriClient.addSource({ path: '/f', mode: { kind: 'indexInPlace' } });
    await tauriClient.setSourcePaused({ source: 1, paused: true });
    await tauriClient.removeSource(1);
    await tauriClient.scan({ source: 1 });
    expect(invoke.mock.calls).toEqual([
      ['list_sources'],
      ['add_source', { req: { path: '/f', mode: { kind: 'indexInPlace' } } }],
      ['set_source_paused', { req: { source: 1, paused: true } }],
      ['remove_source', { source: 1 }],
      ['scan', { req: { source: 1 } }],
    ]);
  });

  it('opens the native picker for one folder', async () => {
    open.mockResolvedValueOnce('/Users/demo/New').mockResolvedValueOnce(null);
    expect(await tauriClient.pickFolder('Add')).toBe('/Users/demo/New');
    expect(open).toHaveBeenCalledWith({ directory: true, multiple: false, title: 'Add' });
    expect(await tauriClient.pickFolder('Add')).toBeNull();
  });

  it('forwards the events of the fm://event channel', async () => {
    listen.mockResolvedValue(() => {});
    const seen: Event[] = [];
    await tauriClient.onEvent((e) => seen.push(e));
    expect(listen.mock.calls[0]![0]).toBe('fm://event');
    const e: Event = { type: 'scanProgress', job: 1, done: 1, total: 2 };
    listen.mock.calls[0]![1]({ payload: e });
    expect(seen).toEqual([e]);
  });
});
