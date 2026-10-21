import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MOCK_LAYOUT_BOUNDS, MOCK_LAYOUT_PRESETS, mockFaces } from '../api/mock';
import type { Layout, TextBlock } from '../api/types';
import { FaceLoader, setFaceLoader, type FontFaceSetLike } from '../preview/faceLoader';
import FontLayout from './FontLayout.vue';
import LayoutBlock from './LayoutBlock.vue';

// Animation frames run only when the test says so.
let frames: FrameRequestCallback[] = [];
const runFrame = () => {
  const due = frames;
  frames = [];
  due.forEach((f) => f(performance.now()));
};

beforeEach(() => {
  frames = [];
  vi.stubGlobal('requestAnimationFrame', (f: FrameRequestCallback) => frames.push(f));
  vi.stubGlobal('cancelAnimationFrame', (id: number) => {
    frames.splice(id - 1, 1);
  });
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

const quote = MOCK_LAYOUT_PRESETS.find((l) => l.preset === 'quote')!;
const block: TextBlock = { ...quote.blocks[0]!, sizePt: 36, lineHeight: 1.2 };

function show(b: TextBlock = block) {
  return mount(LayoutBlock, {
    props: {
      block: b,
      fontFamily: '"fm-face-1", sans-serif',
      bounds: MOCK_LAYOUT_BOUNDS,
      editable: true,
      label: 'Block 1',
    },
    attachTo: document.body,
  });
}

type W = ReturnType<typeof show>;
const blk = (w: W) => w.get('[data-testid="layout-block"]');
const el = (w: W) => blk(w).element as HTMLElement;
const badge = (w: W) => w.find('[data-testid="size-badge"]').text();
/**
 * A pointer event (jsdom has no PointerEvent; a MouseEvent with a pointerId does). The
 * primary button is held from pointerdown to pointerup unless `buttons` says otherwise.
 */
function pointerAt(
  target: Element,
  type: string,
  x: number,
  y: number,
  buttons = type === 'pointerup' ? 0 : 1,
): void {
  const e = new MouseEvent(type, { clientX: x, clientY: y, button: 0, buttons, bubbles: true });
  Object.defineProperty(e, 'pointerId', { value: 1 });
  target.dispatchEvent(e);
}
const point = async (w: W, type: string, x: number, y: number, buttons?: number) => {
  pointerAt(el(w), type, x, y, buttons);
  await w.vm.$nextTick();
};

describe('LayoutBlock direct manipulation (E12.2)', () => {
  it('a horizontal drag sets the size and a vertical drag the line height, once per frame', async () => {
    const w = show();
    await point(w, 'pointerdown', 100, 100);
    await point(w, 'pointermove', 110, 100);
    await point(w, 'pointermove', 140, 100);
    // Nothing moves until the frame; then only the latest pointer position counts.
    expect(el(w).style.fontSize).toBe('36pt');
    expect(frames).toHaveLength(1);
    runFrame();
    await w.vm.$nextTick();
    expect(el(w).style.fontSize).toBe('56pt');
    expect(badge(w)).toBe('56 pt · 1.20');
    await point(w, 'pointermove', 140, 70);
    runFrame();
    await w.vm.$nextTick();
    expect(el(w).style.lineHeight).toBe('1.5');
    expect(w.emitted('change')).toBeUndefined();
    await point(w, 'pointerup', 140, 70);
    expect(w.emitted('change')).toEqual([[{ sizePt: 56, lineHeight: 1.5 }]]);
  });

  it('a drag is clamped to the fm-core bounds', async () => {
    const w = show();
    await point(w, 'pointerdown', 0, 0);
    await point(w, 'pointermove', 100_000, 100_000);
    runFrame();
    await point(w, 'pointerup', 100_000, 100_000);
    expect(w.emitted('change')).toEqual([[{ sizePt: 600, lineHeight: 0.1 }]]);
  });

  it('Escape during a drag puts the starting values back and emits nothing', async () => {
    const w = show();
    await point(w, 'pointerdown', 100, 100);
    await point(w, 'pointermove', 300, 0);
    runFrame();
    await w.vm.$nextTick();
    expect(el(w).style.fontSize).toBe('136pt');
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await w.vm.$nextTick();
    expect(el(w).style.fontSize).toBe('36pt');
    expect(el(w).style.lineHeight).toBe('1.2');
    await point(w, 'pointermove', 400, 0);
    await point(w, 'pointerup', 400, 0);
    expect(w.emitted('change')).toBeUndefined();
  });

  it('a click without movement changes nothing', async () => {
    const w = show();
    await point(w, 'pointerdown', 100, 100);
    await point(w, 'pointermove', 101, 101);
    await point(w, 'pointerup', 101, 101);
    expect(frames).toHaveLength(0);
    expect(w.emitted('change')).toBeUndefined();
  });

  it('a Ctrl+wheel pinch (WebView2) scales the size, clamped, and commits when it rests', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    const w = show();
    // A wheel without Ctrl scrolls as usual.
    const plain = new WheelEvent('wheel', { deltaY: -50, cancelable: true });
    el(w).dispatchEvent(plain);
    expect(plain.defaultPrevented).toBe(false);
    const pinch = new WheelEvent('wheel', {
      deltaY: -Math.log(2) * 100,
      ctrlKey: true,
      cancelable: true,
    });
    el(w).dispatchEvent(pinch);
    expect(pinch.defaultPrevented).toBe(true);
    runFrame();
    await w.vm.$nextTick();
    expect(el(w).style.fontSize).toBe('72pt');
    // Pinching on keeps scaling from where it is, up to the bound.
    for (let i = 0; i < 20; i++)
      el(w).dispatchEvent(
        new WheelEvent('wheel', { deltaY: -100, ctrlKey: true, cancelable: true }),
      );
    runFrame();
    await w.vm.$nextTick();
    expect(el(w).style.fontSize).toBe('600pt');
    expect(w.emitted('change')).toBeUndefined();
    vi.advanceTimersByTime(250);
    expect(w.emitted('change')).toEqual([[{ sizePt: 600 }]]);
  });

  it('WebKit gesture events (WKWebView) scale the size from the start of the pinch', async () => {
    const w = show();
    const gesture = (type: string, scale: number) => {
      const e = Object.assign(new Event(type, { cancelable: true }), { scale });
      el(w).dispatchEvent(e);
      return e;
    };
    expect(gesture('gesturestart', 1).defaultPrevented).toBe(true);
    gesture('gesturechange', 1.5);
    gesture('gesturechange', 0.5);
    runFrame();
    await w.vm.$nextTick();
    expect(el(w).style.fontSize).toBe('18pt');
    gesture('gesturechange', 0.01);
    gesture('gestureend', 0.01);
    expect(w.emitted('change')).toEqual([[{ sizePt: 8 }]]);
  });

  it('arrow keys resize a focused block; it has an accessible name with its values', async () => {
    const w = show();
    expect(blk(w).attributes('tabindex')).toBe('0');
    expect(blk(w).attributes('role')).toBe('group');
    expect(blk(w).attributes('aria-label')).toBe('Block 1, 36 points, line height 1.20');
    await blk(w).trigger('keydown', { key: 'ArrowRight' });
    await blk(w).trigger('keydown', { key: 'ArrowUp', shiftKey: true });
    expect(w.emitted('change')).toEqual([[{ sizePt: 37 }], [{ lineHeight: 1.7 }]]);
    // Other keys (and modified arrows) are left to the app.
    await blk(w).trigger('keydown', { key: 'ArrowRight', ctrlKey: true });
    await blk(w).trigger('keydown', { key: 'x' });
    expect(w.emitted('change')).toHaveLength(2);
  });

  it('without bounds the block is read-only', async () => {
    const w = mount(LayoutBlock, { props: { block, fontFamily: 'serif', bounds: null } });
    expect(w.find('[data-testid="size-badge"]').exists()).toBe(false);
    await point(w, 'pointerdown', 0, 0);
    await point(w, 'pointermove', 200, 0);
    await point(w, 'pointerup', 200, 0);
    await blk(w).trigger('keydown', { key: 'ArrowRight' });
    expect(w.emitted('change')).toBeUndefined();
  });

  it('double-click edits the text in place: Escape cancels, Ctrl+Enter and blur keep it', async () => {
    const w = show();
    await blk(w).trigger('dblclick');
    await flushPromises();
    let field = w.find('textarea');
    expect(field.exists()).toBe(true);
    expect(document.activeElement).toBe(field.element);
    expect((field.element as HTMLTextAreaElement).value).toBe(block.text);
    await field.setValue('Draft');
    await field.trigger('keydown', { key: 'Escape' });
    expect(w.find('textarea').exists()).toBe(false);
    expect(w.text()).toContain(block.text);
    expect(w.emitted('edit')).toBeUndefined();

    await blk(w).trigger('dblclick');
    await flushPromises();
    field = w.find('textarea');
    await field.setValue('Kept\nby the keyboard');
    await field.trigger('keydown', { key: 'Enter', ctrlKey: true });
    expect(w.emitted('edit')).toEqual([['Kept\nby the keyboard']]);

    // Enter on the focused block opens the editor too; leaving it keeps the edit.
    await blk(w).trigger('keydown', { key: 'Enter' });
    await flushPromises();
    field = w.find('textarea');
    await field.setValue('Kept by a click outside');
    await field.trigger('blur');
    expect(w.emitted('edit')?.[1]).toEqual(['Kept by a click outside']);
    expect(w.find('textarea').exists()).toBe(false);
    // Editing text is not resizing.
    expect(w.emitted('change')).toBeUndefined();
  });

  it('a block that is not editable resizes but never opens a text box', async () => {
    const w = mount(LayoutBlock, {
      props: { block, fontFamily: 'serif', bounds: MOCK_LAYOUT_BOUNDS, label: 'Block 1' },
    });
    await blk(w).trigger('dblclick');
    await blk(w).trigger('keydown', { key: 'Enter' });
    await flushPromises();
    expect(w.find('textarea').exists()).toBe(false);
    await blk(w).trigger('keydown', { key: 'ArrowLeft' });
    expect(w.emitted('change')).toEqual([[{ sizePt: 35 }]]);
  });

  it('keeps the committed values on screen until the parent block shows them', async () => {
    const w = show();
    await blk(w).trigger('keydown', { key: 'ArrowRight' });
    expect(el(w).style.fontSize).toBe('37pt');
    // A second press before the core answers goes on from there.
    await blk(w).trigger('keydown', { key: 'ArrowRight' });
    expect(w.emitted('change')).toEqual([[{ sizePt: 37 }], [{ sizePt: 38 }]]);
    await w.setProps({ block: { ...block, sizePt: 38 } });
    expect(el(w).style.fontSize).toBe('38pt');
    // A different block (another layout) shows its own values.
    await w.setProps({ block: { ...block, sizePt: 20 } });
    expect(el(w).style.fontSize).toBe('20pt');
  });
});

class FakeFace {
  constructor(
    readonly family: string,
    readonly source: string,
  ) {}
  load() {
    return Promise.resolve(this);
  }
}
class FakeSet implements FontFaceSetLike<FakeFace> {
  readonly faces = new Set<FakeFace>();
  added = 0;
  add(f: FakeFace) {
    this.added++;
    this.faces.add(f);
  }
  delete(f: FakeFace) {
    return this.faces.delete(f);
  }
}

describe('FontLayout with a 10-block layout (E12.2)', () => {
  it('a drag updates only the dragged block, every frame, without loading the face again', async () => {
    setActivePinia(createPinia());
    const fonts = new FakeSet();
    const create = vi.fn((family: string, source: string) => new FakeFace(family, source));
    setFaceLoader(
      new FaceLoader<FakeFace>({
        fonts,
        create,
        url: (f) => `fmfont://localhost/${f}`,
        maxIdle: 0,
      }),
    );
    const ten: Layout = {
      preset: null,
      name: 'Ten',
      blocks: Array.from({ length: 10 }, (_, i) => ({ ...block, text: `Block ${i}` })),
    };
    const setProps = vi.fn(async () => null);
    const w = mount(FontLayout, {
      props: { layout: ten, face: mockFaces[0]!.face, bounds: MOCK_LAYOUT_BOUNDS, setProps },
      attachTo: document.body,
    });
    await flushPromises();
    const blocks = () => w.findAll('[data-testid="layout-block"]');
    expect(blocks()).toHaveLength(10);
    const families = blocks().map((b) => (b.element as HTMLElement).style.fontFamily);
    const target = blocks()[4]!;

    pointerAt(target.element, 'pointerdown', 0, 0);
    for (let f = 2; f <= 60; f++) {
      pointerAt(target.element, 'pointermove', f * 2, 0);
      runFrame();
      await w.vm.$nextTick();
      // The text follows the pointer on every frame.
      expect((target.element as HTMLElement).style.fontSize).toBe(`${36 + f}pt`);
    }
    pointerAt(target.element, 'pointerup', 120, 0);

    // One change for the whole drag, handed to the store like a properties-editor change.
    expect(setProps.mock.calls).toEqual([[4, { sizePt: 96 }]]);
    blocks().forEach((b, i) => {
      const s = (b.element as HTMLElement).style;
      if (i !== 4) expect(s.fontSize).toBe('36pt');
      expect(s.fontFamily).toBe(families[i]);
    });
    // One face, created and added once: the drag never reloaded it.
    expect(create).toHaveBeenCalledTimes(1);
    expect(fonts.added).toBe(1);
    // Every block points at the gesture help.
    const hint = w.find('.hint');
    expect(hint.text()).toMatch(/Left and Right.*size.*Up and Down.*line height/s);
    expect(target.attributes('aria-describedby')).toBe(hint.attributes('id'));
    w.unmount();
    setFaceLoader(undefined);
  });
});
