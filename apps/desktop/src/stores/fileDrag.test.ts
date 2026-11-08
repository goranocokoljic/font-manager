// Dragging font files out of the window (E44.4): the edge test, and what the user is told
// when files were left out or the drag failed.
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useAppStore } from './app';
import { pastEdge, useFileDragStore } from './fileDrag';
import { useToastStore } from './toast';

let client: FmClient;

async function store(over: Partial<FmClient> = {}) {
  client = { ...createMockClient({ fileDrag: 'on' }), ...over };
  setClient(client);
  await useAppStore().load();
  return useFileDragStore();
}

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => setClient(undefined));

describe('pastEdge', () => {
  it('is true on or past every edge of the viewport, false inside it', () => {
    expect(pastEdge(500, 400, 1280, 800)).toBe(false);
    expect(pastEdge(1, 1, 1280, 800)).toBe(false);
    expect(pastEdge(0, 400, 1280, 800)).toBe(true);
    expect(pastEdge(-20, 400, 1280, 800)).toBe(true);
    expect(pastEdge(500, 0, 1280, 800)).toBe(true);
    expect(pastEdge(1279, 400, 1280, 800)).toBe(true);
    expect(pastEdge(500, 812, 1280, 800)).toBe(true);
  });
});

describe('useFileDragStore', () => {
  it('drags only while held, once, and says nothing when every file went', async () => {
    const start = vi.fn(async () => ({ paths: ['/Fonts/A.ttf'], missing: 0 }));
    const drag = await store({ startFileDrag: start });
    const show = vi.spyOn(useToastStore(), 'show');
    await drag.dragOut({ faces: [1], families: [] });
    expect(start).not.toHaveBeenCalled();
    expect(drag.hold()).toBe(true);
    await drag.dragOut({ faces: [1], families: [2] });
    expect(start).toHaveBeenCalledWith({ faces: [1], families: [2] });
    expect(drag.holding).toBe(false);
    expect(drag.outgoing).toBe(false);
    expect(show).not.toHaveBeenCalled();
  });

  it('says how many files were left out because they are gone', async () => {
    const drag = await store({
      startFileDrag: async () => ({ paths: ['/Fonts/A.ttf'], missing: 2 }),
    });
    const show = vi.spyOn(useToastStore(), 'show');
    drag.hold();
    await drag.dragOut({ faces: [1, 2, 3], families: [] });
    expect(show).toHaveBeenCalledWith('2 font files are not on disk any more and were left out.');
  });

  it('says so when none of the files is on disk', async () => {
    const drag = await store({ startFileDrag: async () => ({ paths: [], missing: 1 }) });
    const show = vi.spyOn(useToastStore(), 'show');
    drag.hold();
    await drag.dragOut({ faces: [1], families: [] });
    expect(show).toHaveBeenCalledWith('None of these font files are on disk any more.');
  });

  it('a failed drag is a toast with the reason, and the grip can be held again', async () => {
    const drag = await store({
      startFileDrag: async () => {
        throw { code: 'internal', message: 'no window to drag from', detail: null };
      },
    });
    const show = vi.spyOn(useToastStore(), 'show');
    drag.hold();
    await drag.dragOut({ faces: [1], families: [] });
    expect(show).toHaveBeenCalledWith(
      'The font files could not be dragged out: no window to drag from',
    );
    expect(drag.hold()).toBe(true);
  });

  it('cannot be held where the system cannot drag files out', async () => {
    client = createMockClient();
    setClient(client);
    await useAppStore().load();
    const drag = useFileDragStore();
    expect(drag.available).toBe(false);
    expect(drag.reason).toBe('Dragging font files out needs the desktop app.');
    expect(drag.hold()).toBe(false);
  });
});
