import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, tauriClient, type FmClient } from '../api/client';
import { createMockClient, mockFaces } from '../api/mock';
import type { ApiError, FaceDetail, Target } from '../api/types';
import { parseFaceId, useSelectionStore } from './selection';
import { useSettingsStore } from './settings';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke, isTauri: () => false }));

let client: FmClient;

beforeEach(() => {
  setActivePinia(createPinia());
  client = createMockClient();
  setClient(client);
});

afterEach(() => {
  setClient(undefined);
  invoke.mockReset();
});

describe('parseFaceId', () => {
  it('reads a face id from the route query', () => {
    expect(parseFaceId('6')).toBe(6);
    expect(parseFaceId(['7', '1'])).toBe(7);
  });

  it('ignores anything that is not a whole non-negative number', () => {
    for (const bad of [undefined, null, '', 'abc', '-1', '1.5', '1e3', '4294967296', 3]) {
      expect(parseFaceId(bad)).toBeNull();
    }
  });
});

describe('selection store', () => {
  it('fetches the detail of the selected face', async () => {
    const store = useSelectionStore();
    await store.select(1);
    expect(store.detail?.face.postscriptName).toBe('Inter-Regular');
    expect(store.error).toBeNull();
    expect(store.loading).toBe(false);
  });

  it('clears the detail when nothing is selected', async () => {
    const store = useSelectionStore();
    await store.select(1);
    await store.select(null);
    expect(store.detail).toBeNull();
    expect(store.faceId).toBeNull();
  });

  it('shows the API error for an unknown face', async () => {
    const store = useSelectionStore();
    await store.select(999);
    expect(store.detail).toBeNull();
    expect(store.error?.code).toBe('notFound');
  });

  it('keeps the latest selection when answers arrive out of order', async () => {
    let releaseFirst: (f: FaceDetail) => void = () => {};
    const first = new Promise<FaceDetail>((resolve) => (releaseFirst = resolve));
    const real = client.getFace;
    setClient({ ...client, getFace: (id) => (id === 1 ? first : real(id)) });
    const store = useSelectionStore();
    const slow = store.select(1);
    await store.select(6);
    releaseFirst(mockFaces[0]);
    await slow;
    expect(store.detail?.face.id).toBe(6);
    expect(store.loading).toBe(false);
  });

  it('activates the selected face, then deactivates it', async () => {
    const store = useSelectionStore();
    await store.select(1);
    await store.toggleActivation();
    expect(store.outcome).toEqual({ status: 'done' });
    expect(store.detail?.face.active).toBe('session');
    await store.toggleActivation();
    expect(store.outcome).toEqual({ status: 'done' });
    expect(store.detail?.face.active).toBeNull();
  });

  it('activates permanently, upgrades a session activation and deactivates a permanent one', async () => {
    const activate = vi.spyOn(client, 'activate');
    const deactivate = vi.spyOn(client, 'deactivate');
    const store = useSelectionStore();
    await store.select(1);
    await store.toggleActivation('permanent');
    expect(activate).toHaveBeenLastCalledWith({
      target: { kind: 'faces', faces: [1] },
      mode: 'permanent',
      onConflict: 'keepExisting',
    });
    expect(store.detail?.face.active).toBe('permanent');
    await store.toggleActivation('permanent');
    expect(deactivate).toHaveBeenCalledOnce();
    expect(store.detail?.face.active).toBeNull();

    await store.toggleActivation('session');
    expect(store.detail?.face.active).toBe('session');
    await store.toggleActivation('permanent');
    expect(store.detail?.face.active).toBe('permanent');
    // A session toggle on a permanently active face deactivates it.
    await store.toggleActivation('session');
    expect(store.detail?.face.active).toBeNull();
  });

  it('can activate only a loaded, healthy face with no request in flight', async () => {
    const store = useSelectionStore();
    expect(store.canActivate).toBe(false);
    await store.select(7); // unsupported
    expect(store.canActivate).toBe(false);
    await store.select(1);
    expect(store.canActivate).toBe(true);
    const pending = store.toggleActivation();
    expect(store.canActivate).toBe(false);
    await pending;
    expect(store.canActivate).toBe(true);
  });

  it('reports a conflict and leaves the face inactive', async () => {
    const store = useSelectionStore();
    await store.select(6);
    await store.toggleActivation();
    expect(store.outcome).toMatchObject({ status: 'conflict', with: { kind: 'system' } });
    expect(store.detail?.face.active).toBeNull();
  });

  it('the Activate button uses the activation mode chosen in Settings', async () => {
    const activate = vi.spyOn(client, 'activate');
    await useSettingsStore().update({ activationMode: 'permanent' });
    const store = useSelectionStore();
    await store.select(1);
    await store.toggleActivation();
    expect(activate).toHaveBeenLastCalledWith(expect.objectContaining({ mode: 'permanent' }));
    expect(store.detail?.face.active).toBe('permanent');
    // Active in any mode, the button deactivates.
    await store.toggleActivation();
    expect(store.detail?.face.active).toBeNull();
    // The A key still means this session.
    await store.toggleActivation('session');
    expect(store.detail?.face.active).toBe('session');
  });

  it('activates over a name clash when Settings say so', async () => {
    await useSettingsStore().update({ onConflict: 'replace' });
    const store = useSelectionStore();
    await store.select(6);
    await store.toggleActivation();
    expect(store.outcome).toEqual({ status: 'done' });
    expect(store.detail?.face.active).toBe('session');
  });

  it('turns a rejected activation into a failed outcome', async () => {
    const failure: ApiError = { code: 'internal', message: 'The OS said no.', detail: null };
    setClient({ ...client, activate: () => Promise.reject(failure) });
    const store = useSelectionStore();
    await store.select(1);
    await store.toggleActivation();
    expect(store.outcome).toEqual({ status: 'failed', error: failure });
    expect(store.busy).toBe(false);
  });

  it('does nothing without a selected face', async () => {
    const activate = vi.fn();
    setClient({ ...client, activate });
    await useSelectionStore().toggleActivation();
    expect(activate).not.toHaveBeenCalled();
  });
});

describe('API client', () => {
  it('calls the get_face, activate and deactivate Tauri commands', async () => {
    invoke.mockResolvedValue([]);
    const target: Target = { kind: 'faces', faces: [1] };
    await tauriClient.getFace(1);
    await tauriClient.activate({ target });
    await tauriClient.deactivate({ target });
    expect(invoke.mock.calls).toEqual([
      ['get_face', { face: 1 }],
      ['activate', { req: { target } }],
      ['deactivate', { req: { target } }],
    ]);
  });
});
