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

  it('asks before activating a face whose name clashes; keep leaves it inactive', async () => {
    const activate = vi.spyOn(client, 'activate');
    const store = useSelectionStore();
    await store.select(6);
    await store.toggleActivation();
    expect(activate).not.toHaveBeenCalled();
    expect(store.pending).toMatchObject({
      face: 6,
      mode: 'session',
      conflicts: [{ face: 6, with: { kind: 'system' } }],
    });
    expect(store.detail?.face.active).toBeNull();

    await store.resolveConflict('keep');
    expect(store.pending).toBeNull();
    expect(activate).toHaveBeenLastCalledWith(
      expect.objectContaining({ onConflict: 'keepExisting' }),
    );
    expect(store.outcome).toMatchObject({ status: 'conflict', with: { kind: 'system' } });
    expect(store.detail?.face.active).toBeNull();
  });

  it('replace activates a clashing face for this session only', async () => {
    const store = useSelectionStore();
    await store.select(6);
    await store.toggleActivation('permanent');
    expect(store.pending?.mode).toBe('permanent');
    await store.resolveConflict('replace');
    expect(store.outcome).toEqual({ status: 'done' });
    expect(store.detail?.face.active).toBe('session');
  });

  it('cancel activates nothing', async () => {
    const activate = vi.spyOn(client, 'activate');
    const store = useSelectionStore();
    await store.select(6);
    await store.toggleActivation();
    await store.resolveConflict('cancel');
    expect(store.pending).toBeNull();
    expect(activate).not.toHaveBeenCalled();
    expect(store.detail?.face.active).toBeNull();
  });

  it('a face without a clash activates without asking', async () => {
    const store = useSelectionStore();
    await store.select(1);
    await store.toggleActivation();
    expect(store.pending).toBeNull();
    expect(store.detail?.face.active).toBe('session');
  });

  it('a new selection drops the pending question', async () => {
    const store = useSelectionStore();
    await store.select(6);
    await store.toggleActivation();
    await store.select(1);
    expect(store.pending).toBeNull();
  });

  it('a failed clash check is a failed outcome', async () => {
    const failure: ApiError = { code: 'unsupported', message: 'No font service.', detail: null };
    setClient({ ...client, conflicts: () => Promise.reject(failure) });
    const store = useSelectionStore();
    await store.select(6);
    await store.toggleActivation();
    expect(store.outcome).toEqual({ status: 'failed', error: failure });
    expect(store.pending).toBeNull();
    expect(store.busy).toBe(false);
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

  it('replaces without asking when Settings say so', async () => {
    await useSettingsStore().update({ onConflict: 'replace', activationMode: 'permanent' });
    const store = useSelectionStore();
    await store.select(6);
    await store.toggleActivation();
    expect(store.pending).toBeNull();
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

  it('calls the conflicts and list_conflicts Tauri commands', async () => {
    invoke.mockResolvedValue([]);
    const target: Target = { kind: 'faces', faces: [6] };
    await tauriClient.conflicts(target);
    await tauriClient.listConflicts();
    expect(invoke.mock.calls).toEqual([['conflicts', { target }], ['list_conflicts']]);
  });
});

describe('declared license', () => {
  it('declares the family license and shows a refusal in the license form only', async () => {
    const store = useSelectionStore();
    await store.select(1);
    expect(
      await store.declareLicense({ licenseType: 'desktop', seats: 3, purchaseDate: '2026-03-01' }),
    ).toBe(true);
    expect(store.detail?.license.declared).toBe('desktop');
    expect(store.detail?.license.seats).toBe(3);
    expect(store.detail?.license.purchaseDate).toBe('2026-03-01');

    expect(await store.declareLicense({ licenseType: 'desktop', seats: -1 })).toBe(false);
    expect(store.licenseError?.code).toBe('invalidRequest');
    expect(store.editError).toBeNull();
    // The refused declaration changed nothing; the next good one clears the error.
    expect(store.detail?.license.seats).toBe(3);
    expect(await store.declareLicense({})).toBe(true);
    expect(store.licenseError).toBeNull();
    expect(store.detail?.license.declared).toBeNull();
  });

  it('attaches a picked receipt, and a cancelled pick changes nothing', async () => {
    const store = useSelectionStore();
    await store.select(1);
    setClient({ ...client, pickFile: async () => null });
    expect(await store.attachReceipt({ licenseType: 'web' }, 'appData')).toBe(false);
    expect(store.detail?.license.receipt ?? null).toBeNull();
    setClient({ ...client, pickFile: async () => String.raw`C:\Users\me\invoice.pdf` });
    expect(await store.attachReceipt({ licenseType: 'web' }, 'nextToFont')).toBe(true);
    expect(store.detail?.license.receipt).toBe('/fonts/.fontlibrary/receipts/invoice.pdf');
    expect(store.detail?.license.declared).toBe('web');
  });

  it('calls the declare_license Tauri command', async () => {
    invoke.mockResolvedValue(null);
    const req = { family: 1, licenseType: 'app' as const, receipt: { kind: 'clear' as const } };
    await tauriClient.declareLicense(req);
    expect(invoke.mock.calls).toEqual([['declare_license', { req }]]);
  });
});

describe('selection store, activation events (E6.9)', () => {
  const faces = (ids: number[]): Target => ({ kind: 'faces', faces: ids });

  afterEach(async () => {
    await useSelectionStore().stop();
  });

  it('reloads the selected face when another caller activates or deactivates it', async () => {
    const store = useSelectionStore();
    await store.start();
    await store.select(1);
    expect(store.detail?.face.active).toBeNull();

    await client.activate({ target: faces([1]), mode: 'session', onConflict: 'replace' });
    await vi.waitFor(() => expect(store.detail?.face.active).toBe('session'));

    await client.deactivate({ target: faces([1]) });
    await vi.waitFor(() => expect(store.detail?.face.active).toBeNull());
  });

  it('does nothing for an event about other faces', async () => {
    const store = useSelectionStore();
    await store.start();
    await store.select(1);
    const getFace = vi.spyOn(client, 'getFace');

    const [out] = await client.activate({
      target: faces([6]),
      mode: 'session',
      onConflict: 'replace',
    });
    expect(out?.outcome.status).toBe('done');
    await Promise.resolve();
    expect(getFace).not.toHaveBeenCalled();
  });

  it('does not reload while its own activation is in flight', async () => {
    const store = useSelectionStore();
    await store.start();
    await store.select(1);
    const getFace = vi.spyOn(client, 'getFace');

    store.busy = true;
    await client.activate({ target: faces([1]), mode: 'session', onConflict: 'replace' });
    await Promise.resolve();
    expect(getFace).not.toHaveBeenCalled();
  });

  it('subscribes once and stops following events after stop', async () => {
    const onEvent = vi.spyOn(client, 'onEvent');
    const store = useSelectionStore();
    await store.start();
    await store.start();
    expect(onEvent).toHaveBeenCalledTimes(1);
    await store.select(1);
    await store.stop();
    const getFace = vi.spyOn(client, 'getFace');

    await client.activate({ target: faces([1]), mode: 'session', onConflict: 'replace' });
    await Promise.resolve();
    expect(getFace).not.toHaveBeenCalled();
    expect(store.detail?.face.active).toBeNull();
  });
});
