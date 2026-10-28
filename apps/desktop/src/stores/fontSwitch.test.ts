import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { FaceId } from '../api/types';
import { FONT_SWITCH_MIME, FONTS_MIME } from '../list/drag';
import { faceKey, type ListRow } from '../list/rows';
import { useFontSwitchStore } from './fontSwitch';
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

afterEach(() => {
  // Ends any drag a test left watching.
  document.dispatchEvent(new Event('dragend'));
  setClient(undefined);
});

/** A `DataTransfer` stand-in for jsdom, which has none. */
function transfer(): DataTransfer {
  const data = new Map<string, string>();
  return {
    effectAllowed: 'none',
    get types() {
      return [...data.keys()];
    },
    setData: (type: string, value: string) => void data.set(type, value),
    getData: (type: string) => data.get(type) ?? '',
  } as unknown as DataTransfer;
}

async function faceRow(id: FaceId): Promise<ListRow> {
  const { face } = await client.getFace(id);
  const family = (await client.getFamily(face.family)).summary;
  return { kind: 'face', key: faceKey(id), face, family, position: 1, count: 1 };
}

/** A `dragleave` on the document at (x, y) that entered no element. */
function leave(x: number, y: number): void {
  const e = new Event('dragleave');
  Object.assign(e, { clientX: x, clientY: y, relatedTarget: null });
  document.dispatchEvent(e);
}

const settle = () => new Promise((r) => setTimeout(r, 0));

describe('font switch store (E34.1)', () => {
  it('a primed row drags its payload; leaving the window activates it for the session', async () => {
    const store = useFontSwitchStore();
    const row = await faceRow(1);
    store.prime(row);
    await settle();
    const dataTransfer = transfer();
    store.startDrag({ dataTransfer } as DragEvent, [row.key], row);
    expect(dataTransfer.types).toEqual([
      FONTS_MIME,
      FONT_SWITCH_MIME,
      'text/plain',
      'text/uri-list',
    ]);
    expect(JSON.parse(dataTransfer.getData(FONT_SWITCH_MIME))).toEqual(
      await client.fontSwitchPayload(1),
    );

    leave(200, 200); // still inside the window
    await settle();
    expect(await client.listActive()).toEqual([]);

    leave(-1, 200);
    await settle();
    const active = await client.listActive();
    expect(active.map((a) => [a.face, a.mode])).toEqual([[1, 'session']]);

    // Once per drag; nothing is deactivated when it ends.
    document.dispatchEvent(new Event('dragend'));
    await settle();
    expect((await client.listActive()).map((a) => a.face)).toEqual([1]);
    expect(useToastStore().toast).toBeNull();
  });

  it('leaves an active face as it is', async () => {
    await client.activate({ target: { kind: 'faces', faces: [1] }, mode: 'permanent' });
    const store = useFontSwitchStore();
    const row = await faceRow(1);
    store.prime(row);
    await settle();
    const spy = vi.spyOn(client, 'activate');
    store.startDrag({ dataTransfer: transfer() } as DragEvent, [row.key], row);
    leave(-1, 0);
    await settle();
    expect(spy).not.toHaveBeenCalled();
    expect((await client.listActive()).map((a) => a.mode)).toEqual(['permanent']);
  });

  it('a face that cannot be activated shows a toast', async () => {
    const store = useFontSwitchStore();
    const row = await faceRow(7); // a Type 1 font: skipped
    store.prime(row);
    await settle();
    store.startDrag({ dataTransfer: transfer() } as DragEvent, [row.key], row);
    leave(-1, 0);
    await settle();
    expect(useToastStore().toast?.message).toMatch(/could not be activated for the drop/);
  });

  it('a rejected activation shows a toast', async () => {
    const store = useFontSwitchStore();
    const row = await faceRow(1);
    store.prime(row);
    await settle();
    vi.spyOn(client, 'activate').mockRejectedValue({ code: 'internal', message: 'OS said no' });
    store.startDrag({ dataTransfer: transfer() } as DragEvent, [row.key], row);
    leave(-1, 0);
    await settle();
    expect(useToastStore().toast?.message).toMatch(/could not be activated for the drop/);
  });

  it('an unprimed row drags only its ids and activates nothing', async () => {
    const store = useFontSwitchStore();
    const row = await faceRow(1);
    const spy = vi.spyOn(client, 'activate');
    const dataTransfer = transfer();
    store.startDrag({ dataTransfer } as DragEvent, [row.key], row);
    expect(dataTransfer.types).toEqual([FONTS_MIME]);
    leave(-1, 0);
    await settle();
    expect(spy).not.toHaveBeenCalled();
  });

  it('copies the payload as JSON', async () => {
    const text = await useFontSwitchStore().clipboardText(1);
    expect(JSON.parse(text)).toEqual(await client.fontSwitchPayload(1));
  });
});
