import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { ghostCopy, sameTarget, useFontDragStore } from './fontDrag';

let client: FmClient;

/** A `dragstart` at (x, y) whose drag image the test can see. */
function dragStart(x = 40, y = 50) {
  const setDragImage = vi.fn();
  const e = { clientX: x, clientY: y, dataTransfer: { setDragImage } } as unknown as DragEvent;
  return { e, setDragImage };
}

function card(): HTMLElement {
  const el = document.createElement('div');
  el.id = 'fm-row-1';
  el.setAttribute('role', 'option');
  el.setAttribute('tabindex', '0');
  el.innerHTML =
    '<span id="inner" tabindex="-1">Hamburg</span><span data-ghost="hide">6 styles</span>';
  el.getBoundingClientRect = () => ({ left: 10, top: 20, width: 300, height: 200 }) as DOMRect;
  document.body.append(el);
  return el;
}

const settle = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  setActivePinia(createPinia());
  client = createMockClient({ faceTags: { 1: ['Display'] } });
  setClient(client);
});
afterEach(() => {
  useFontDragStore().end();
  setClient(undefined);
  document.body.replaceChildren();
});

describe('a font drag', () => {
  it('fades the dragged rows, hides the webview image and follows the pointer', () => {
    const drag = useFontDragStore();
    const { e, setDragImage } = dragStart();
    drag.start(e, ['face:1', 'face:2'], card());
    expect(drag.active).toBe(true);
    expect(drag.isOrigin('face:1')).toBe(true);
    expect(drag.isOrigin('face:3')).toBe(false);
    expect(drag.count).toBe(2);
    expect(setDragImage).toHaveBeenCalledTimes(1);
    expect((setDragImage.mock.calls[0]![0] as HTMLImageElement).width).toBe(1);
    // The pointer took the card 30 px in from its left, 30 px down from its top.
    expect(drag.source?.grab).toEqual({ x: 30, y: 30 });
    expect(drag.source?.width).toBe(300);
    document.dispatchEvent(new MouseEvent('dragover', { clientX: 200, clientY: 120 }));
    expect(drag.pointer).toEqual({ x: 200, y: 120 });
    // `drag` on the source moves it too; one at (0, 0) carries no position.
    document.dispatchEvent(new MouseEvent('drag', { clientX: 260, clientY: 140 }));
    expect(drag.pointer).toEqual({ x: 260, y: 140 });
    document.dispatchEvent(new MouseEvent('drag', { clientX: 0, clientY: 0 }));
    expect(drag.pointer).toEqual({ x: 260, y: 140 });
  });

  it('copies the card for the ghost without ids, roles or tab stops', () => {
    const copy = ghostCopy(card());
    expect(copy.id).toBe('');
    expect(copy.querySelector('[id]')).toBeNull();
    expect(copy.querySelector('[tabindex]')).toBeNull();
    expect(copy.hasAttribute('role')).toBe(false);
    expect(copy.getAttribute('aria-hidden')).toBe('true');
    expect(copy.textContent).toBe('Hamburg');
  });

  it('asks fm-api for every target count and shows a target its own', async () => {
    const spy = vi.spyOn(client, 'dropCounts');
    const made = await client.createCollection({
      name: 'Editorial',
      parent: null,
      rules: null,
      ruleGroup: null,
      color: null,
    });
    await client.addToCollection({ collection: made.id, faces: [1], families: [] });
    const display = (await client.listTags()).find((t) => t.name === 'Display')!.id;
    const drag = useFontDragStore();
    drag.start(dragStart().e, ['face:1', 'face:6'], card());
    await settle();
    expect(spy).toHaveBeenCalledWith({ faces: [1, 6], families: [] });
    const collection = { kind: 'collection' as const, id: made.id };
    // Face 1 is in it already: 1 → 2. Face 1 carries Display: 1 → 2.
    expect(drag.countOf(collection)).toEqual({ before: 1, after: 2 });
    expect(drag.countOf({ kind: 'tag', id: display })).toEqual({ before: 1, after: 2 });
    expect(drag.countOf({ kind: 'studio' })).toBeNull();

    expect(drag.isTarget(collection)).toBe(false);
    drag.enter(collection);
    expect(drag.isTarget(collection)).toBe(true);
    expect(drag.isTarget({ kind: 'tag', id: display })).toBe(false);
    drag.leave({ kind: 'tag', id: display });
    expect(drag.isTarget(collection)).toBe(true);
    drag.leave(collection);
    expect(drag.isTarget(collection)).toBe(false);
  });

  it('drops a late answer that belongs to an ended drag', async () => {
    let answer!: (v: Awaited<ReturnType<FmClient['dropCounts']>>) => void;
    vi.spyOn(client, 'dropCounts').mockReturnValue(new Promise((r) => (answer = r)));
    const drag = useFontDragStore();
    drag.start(dragStart().e, ['face:1'], card());
    drag.end();
    answer({ faces: [1], collections: [{ collection: 1, before: 0, after: 1 }], tags: [] });
    await settle();
    expect(drag.counts).toBeNull();
  });

  it('lands a drop: the ghost flies to the target and the drag ends', () => {
    const drag = useFontDragStore();
    drag.start(dragStart().e, ['face:1'], card());
    const row = document.createElement('li');
    row.getBoundingClientRect = () => ({ left: 0, top: 300, width: 200, height: 30 }) as DOMRect;
    const drop = { clientX: 60, clientY: 310 } as DragEvent;
    expect(drag.land(row, drop)).toBe(true);
    expect(drag.active).toBe(false);
    expect(drag.isOrigin('face:1')).toBe(false);
    expect(drag.flight?.from).toEqual({ x: 60, y: 310 });
    expect(drag.flight?.to.top).toBe(300);
    drag.landed();
    expect(drag.flight).toBeNull();
  });

  it('Esc cancels: the drag ends and its drop drops nothing', () => {
    const drag = useFontDragStore();
    drag.start(dragStart().e, ['face:1'], card());
    const esc = new KeyboardEvent('keydown', { key: 'Escape', cancelable: true });
    window.dispatchEvent(esc);
    expect(esc.defaultPrevented).toBe(true);
    expect(drag.active).toBe(false);
    expect(drag.isOrigin('face:1')).toBe(false);
    expect(drag.land(document.body, { clientX: 0, clientY: 0 } as DragEvent)).toBe(false);
    expect(drag.flight).toBeNull();
    // The next drag drops again.
    drag.start(dragStart().e, ['face:1'], card());
    expect(drag.land(document.body, { clientX: 0, clientY: 0 } as DragEvent)).toBe(true);
  });

  it('a key other than Esc, or Esc with no drag, is left alone', () => {
    const drag = useFontDragStore();
    const idle = new KeyboardEvent('keydown', { key: 'Escape', cancelable: true });
    window.dispatchEvent(idle);
    expect(idle.defaultPrevented).toBe(false);
    drag.start(dragStart().e, ['face:1'], card());
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'a' }));
    expect(drag.active).toBe(true);
  });

  it('ends on dragend and on a pointer move with no button down', () => {
    const drag = useFontDragStore();
    drag.start(dragStart().e, ['face:1'], card());
    document.dispatchEvent(new Event('dragend'));
    expect(drag.active).toBe(false);
    drag.start(dragStart().e, ['face:1'], card());
    window.dispatchEvent(new MouseEvent('pointermove', { buttons: 1 }));
    expect(drag.active).toBe(true);
    window.dispatchEvent(new MouseEvent('pointermove', { buttons: 0 }));
    expect(drag.active).toBe(false);
  });

  it('a drop of a drag that started elsewhere lands without a flight', () => {
    const drag = useFontDragStore();
    expect(drag.land(document.body, { clientX: 0, clientY: 0 } as DragEvent)).toBe(true);
    expect(drag.flight).toBeNull();
  });

  it('hides the ghost while the pointer is outside the window', () => {
    const drag = useFontDragStore();
    drag.start(dragStart().e, ['face:1'], card());
    const out = new MouseEvent('dragleave', { clientX: -1, clientY: 10 });
    document.dispatchEvent(out);
    expect(drag.pointer).toBeNull();
    expect(drag.active).toBe(true);
  });
});

describe('sameTarget', () => {
  it('matches kind and id', () => {
    expect(sameTarget({ kind: 'studio' }, { kind: 'studio' })).toBe(true);
    expect(sameTarget({ kind: 'tag', id: 1 }, { kind: 'tag', id: 1 })).toBe(true);
    expect(sameTarget({ kind: 'tag', id: 1 }, { kind: 'collection', id: 1 })).toBe(false);
    expect(sameTarget({ kind: 'tag', id: 1 }, { kind: 'tag', id: 2 })).toBe(false);
    expect(sameTarget(null, { kind: 'studio' })).toBe(false);
  });
});
