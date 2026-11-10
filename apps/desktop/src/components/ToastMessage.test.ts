// The toasts (E48.6): top right under the toolbar, a stack of three with the newest on top,
// click-through except for an action button, and Escape on a focused toast takes it away.
import { mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { TOAST_MS, useToastStore } from '../stores/toast';
import ToastMessage from './ToastMessage.vue';
import SOURCE from './ToastMessage.vue?raw';

let wrapper: VueWrapper | undefined;
let header: HTMLElement | undefined;

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  wrapper?.unmount();
  wrapper = undefined;
  header?.remove();
  header = undefined;
  vi.useRealTimers();
});

function setup() {
  const pinia = createPinia();
  setActivePinia(pinia);
  wrapper = mount(ToastMessage, { global: { plugins: [pinia] }, attachTo: document.body });
  return { w: wrapper, toasts: useToastStore() };
}

/** The page's top bar, `bottom` px from the top of the window. */
function topBar(bottom: number): void {
  // A sidebar's own header comes first in the page; the toast goes by the marked top bar.
  const side = document.createElement('header');
  side.getBoundingClientRect = () => ({ top: 0, bottom: 300, height: 300 }) as DOMRect;
  header = document.createElement('div');
  header.append(side);
  const bar = document.createElement('header');
  bar.dataset.topBar = '';
  bar.getBoundingClientRect = () =>
    ({ top: 0, bottom, height: bottom, left: 0, right: 800, width: 800 }) as DOMRect;
  header.append(bar);
  document.body.prepend(header);
}

const messages = (w: VueWrapper) => w.findAll('.toast').map((t) => t.find('.message').text());

/** The declarations of one rule of the component's style. */
function rule(selector: string): string {
  const css = SOURCE.match(/<style[^>]*>([\s\S]*?)<\/style>/)?.[1] ?? '';
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return css.match(new RegExp(`(^|\\n)${escaped} \\{([^}]*)\\}`))?.[2] ?? '';
}

describe('ToastMessage (E48.6)', () => {
  it('stacks up to three, newest on top', async () => {
    const { w, toasts } = setup();
    for (const m of ['One.', 'Two.', 'Three.', 'Four.']) toasts.show(m);
    await nextTick();
    expect(messages(w)).toEqual(['Four.', 'Three.', 'Two.']);
    // The test id names the newest; the ones under it are stacked.
    expect(w.findAll('.toast').map((t) => t.attributes('data-testid'))).toEqual([
      'toast',
      'toast-stacked',
      'toast-stacked',
    ]);
  });

  it('sits at the top right, just under the page top bar', async () => {
    topBar(90);
    const { w, toasts } = setup();
    toasts.show('Done.');
    await nextTick();
    vi.advanceTimersToNextFrame();
    const region = w.find<HTMLElement>('.region').element;
    await nextTick();
    expect(region.style.top).toBe('98px');
    expect(rule('.region')).toMatch(/position: fixed;/);
    expect(rule('.region')).toMatch(/inset-inline-end: var\(--fm-space-4\);/);
    expect(rule('.region')).not.toMatch(/bottom:/);
  });

  it('follows the top bar when it grows while a toast is up', async () => {
    // A stand-in observer: the test says when the bar's size changed.
    const watched: { cb: () => void; els: Element[] }[] = [];
    vi.stubGlobal(
      'ResizeObserver',
      class {
        private readonly entry: { cb: () => void; els: Element[] };
        constructor(cb: () => void) {
          this.entry = { cb, els: [] };
          watched.push(this.entry);
        }
        observe(el: Element) {
          this.entry.els.push(el);
        }
        disconnect() {
          this.entry.els = [];
        }
      },
    );
    try {
      topBar(56);
      const { w, toasts } = setup();
      toasts.show('Done.');
      await nextTick();
      vi.advanceTimersToNextFrame();
      await nextTick();
      const region = w.find<HTMLElement>('.region').element;
      expect(region.style.top).toBe('64px');
      // The bar grows to 65 px (a control came into it): the stack moves down with it.
      const bar = document.querySelector<HTMLElement>('[data-top-bar]')!;
      bar.getBoundingClientRect = () =>
        ({ top: 0, bottom: 65, height: 65, left: 0, right: 800, width: 800 }) as DOMRect;
      const live = watched.filter((o) => o.els.includes(bar));
      expect(live).toHaveLength(1);
      live[0]!.cb();
      await nextTick();
      expect(region.style.top).toBe('73px');
      // The last toast gone: the bar is no longer watched.
      toasts.dismiss();
      await nextTick();
      expect(watched.some((o) => o.els.length > 0)).toBe(false);
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('starts under a default bar height when the page has no top bar', async () => {
    const { w, toasts } = setup();
    toasts.show('Done.');
    await nextTick();
    vi.advanceTimersToNextFrame();
    await nextTick();
    expect(w.find<HTMLElement>('.region').element.style.top).toBe('56px');
  });

  it('is click-through; only the action button takes the pointer', () => {
    expect(rule('.region')).toMatch(/pointer-events: none;/);
    expect(rule('.toast')).toMatch(/pointer-events: none;/);
    expect(rule('.action')).toMatch(/pointer-events: auto;/);
    // No other control in a toast: the close button is gone, so nothing else takes the pointer.
    const template = SOURCE.match(/<template>([\s\S]*)<\/template>/)?.[1] ?? '';
    expect(template.match(/<button\b/g)).toHaveLength(1);
  });

  it('keeps the toast off the middle of the window, where the sheet handle is', () => {
    expect(rule('.toast')).toMatch(/max-width: min\(420px, calc\(50vw - 48px\)\);/);
  });

  it('runs the action of its own toast and takes that toast away', async () => {
    const { w, toasts } = setup();
    const run = vi.fn();
    toasts.show('Activated 2 fonts.', { label: 'Undo', run });
    toasts.show('Newer.');
    await nextTick();
    const undoable = w.findAll('.toast')[1]!;
    expect(undoable.find('button').text()).toBe('Undo');
    await undoable.find('button').trigger('click');
    expect(run).toHaveBeenCalledTimes(1);
    expect(toasts.toasts.map((t) => t.message)).toEqual(['Newer.']);
  });

  it('stays while the pointer is on its button, and Escape there takes it away', async () => {
    const { w, toasts } = setup();
    toasts.show('Changed.', { label: 'Undo', run: () => undefined });
    await nextTick();
    const button = w.find('.action');
    await button.trigger('pointerenter');
    vi.advanceTimersByTime(TOAST_MS * 2);
    expect(toasts.toast?.message).toBe('Changed.');
    await button.trigger('pointerleave');
    await button.trigger('keydown', { key: 'Escape' });
    expect(toasts.toasts).toEqual([]);
  });

  it('keeps the live region in the page with no toast', () => {
    const { w } = setup();
    const region = w.find('.region');
    expect(region.attributes('role')).toBe('status');
    expect(region.attributes('aria-live')).toBe('polite');
  });
});
