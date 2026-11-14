import { mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { useLayoutStore } from '../stores/layout';
import LayoutSplit from './LayoutSplit.vue';

/** A window whose width matches the split's collapse query or not; `set` changes it later. */
function fakeWindowWidth(narrow: boolean) {
  const listeners = new Set<() => void>();
  const media = {
    matches: narrow,
    media: '',
    addEventListener: (_: string, fn: () => void) => listeners.add(fn),
    removeEventListener: (_: string, fn: () => void) => listeners.delete(fn),
  };
  vi.stubGlobal('matchMedia', (query: string) => {
    media.media = query;
    return media as unknown as MediaQueryList;
  });
  return {
    media,
    set(n: boolean) {
      media.matches = n;
      listeners.forEach((fn) => fn());
    },
  };
}

function show() {
  return mount(LayoutSplit, {
    attachTo: document.body,
    slots: {
      list: '<div role="tree" tabindex="0" data-testid="the-list">rows</div>',
      head: '<span data-testid="the-head">head</span>',
      default: '<div data-testid="the-pane">pane</div>',
    },
  });
}

beforeEach(() => {
  localStorage.clear();
  setActivePinia(createPinia());
});
// jsdom has no matchMedia; each test installs its own.
afterEach(() => vi.unstubAllGlobals());

describe('LayoutSplit (E50.3)', () => {
  it('collapses below 1100 px of window', () => {
    const win = fakeWindowWidth(false);
    show().unmount();
    expect(win.media.media).toBe('(max-width: 1099.98px)');
  });

  it('shows the list at 560 px, a divider, and the pane with its head', () => {
    fakeWindowWidth(false);
    const w = show();
    const list = w.get('[data-testid="split-list"]');
    expect((list.element as HTMLElement).style.width).toBe('560px');
    expect(list.find('[data-testid="the-list"]').exists()).toBe(true);
    const divider = w.get('[role="separator"]');
    expect(divider.attributes('aria-valuemin')).toBe('400');
    expect(divider.attributes('aria-valuemax')).toBe('760');
    expect(divider.attributes('aria-controls')).toBe(list.attributes('id'));
    expect(w.get('[role="toolbar"]').find('[data-testid="the-head"]').exists()).toBe(true);
    expect(w.find('[data-testid="the-pane"]').exists()).toBe(true);
    expect(w.text()).not.toContain('Fonts');
    w.unmount();
  });

  it('the divider resizes the list inside 400–760 px and the store keeps it', async () => {
    fakeWindowWidth(false);
    const w = show();
    const divider = w.get('[role="separator"]');
    await divider.trigger('keydown', { key: 'ArrowRight' });
    expect(useLayoutStore().splitListWidth).toBe(568);
    expect((w.get('[data-testid="split-list"]').element as HTMLElement).style.width).toBe('568px');
    await divider.trigger('keydown', { key: 'End' });
    expect(useLayoutStore().splitListWidth).toBe(760);
    await divider.trigger('keydown', { key: 'Home' });
    expect(useLayoutStore().splitListWidth).toBe(400);
    w.unmount();
  });

  it('narrow: the pane only, and a Fonts button slides the list over it and back', async () => {
    fakeWindowWidth(true);
    const w = show();
    expect(w.find('[data-testid="split-list"]').exists()).toBe(false);
    expect(w.find('[role="separator"]').exists()).toBe(false);
    expect(w.find('[data-testid="the-pane"]').exists()).toBe(true);
    const fonts = w.get('button.fonts');
    expect(fonts.text()).toBe('Fonts');
    expect(fonts.attributes('aria-expanded')).toBe('false');

    await fonts.trigger('click');
    await nextTick();
    const list = w.get('[data-testid="split-list"]');
    expect(list.classes()).toContain('overlay');
    expect(fonts.attributes('aria-expanded')).toBe('true');
    expect(fonts.attributes('aria-controls')).toBe(list.attributes('id'));
    // The keys go to the list at once.
    expect(document.activeElement).toBe(list.get('[data-testid="the-list"]').element);

    // Escape in the list slides it out and gives the focus back to the button.
    await list.get('[data-testid="the-list"]').trigger('keydown', { key: 'Escape' });
    await nextTick();
    await nextTick();
    expect(fonts.attributes('aria-expanded')).toBe('false');
    expect(document.activeElement).toBe(fonts.element);
    w.unmount();
  });

  it('a window that widens again shows the split, not the slid-in list', async () => {
    const win = fakeWindowWidth(true);
    const w = show();
    await w.get('button.fonts').trigger('click');
    win.set(false);
    await nextTick();
    expect(w.find('button.fonts').exists()).toBe(false);
    expect(w.get('[data-testid="split-list"]').classes()).not.toContain('overlay');
    expect(w.find('[role="separator"]').exists()).toBe(true);
    win.set(true);
    await nextTick();
    expect(w.find('[data-testid="split-list"]').exists()).toBe(false);
    w.unmount();
  });
});
