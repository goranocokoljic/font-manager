import { mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { nextTick } from 'vue';
import PaneDivider from '../components/PaneDivider.vue';
import {
  LAYOUT_STORAGE_KEY,
  MAIN_ROOM,
  SIDEBAR,
  SPLIT_LIST,
  clampWidth,
  panesShown,
  useLayoutStore,
} from './layout';

function freshStore() {
  setActivePinia(createPinia());
  return useLayoutStore();
}

beforeEach(() => localStorage.clear());

describe('layout store', () => {
  it('starts with the default width and the sidebar open', () => {
    const layout = freshStore();
    expect(layout.sidebarWidth).toBe(SIDEBAR.initial);
    expect(SIDEBAR.initial).toBe(232);
    expect(layout.sidebarOpen).toBe(true);
  });

  it('keeps the sidebar width inside its bounds', () => {
    const layout = freshStore();
    layout.setSidebarWidth(10);
    expect(layout.sidebarWidth).toBe(SIDEBAR.min);
    layout.setSidebarWidth(5000);
    expect(layout.sidebarWidth).toBe(SIDEBAR.max);
    layout.setSidebarWidth(250.6);
    expect(layout.sidebarWidth).toBe(251);
  });

  it('restores the width and the closed sidebar in a new session', async () => {
    const first = freshStore();
    first.setSidebarWidth(300);
    first.toggleSidebar();
    await nextTick();

    const second = freshStore();
    expect(second.sidebarWidth).toBe(300);
    expect(second.sidebarOpen).toBe(false);
  });

  it('reads a layout saved with the old inspector fields and drops them', async () => {
    localStorage.setItem(
      LAYOUT_STORAGE_KEY,
      JSON.stringify({ sidebarWidth: 300, inspectorWidth: 400, inspectorOpen: false }),
    );
    const layout = freshStore();
    expect(layout.sidebarWidth).toBe(300);
    layout.setSidebarWidth(310);
    await nextTick();
    expect(JSON.parse(localStorage.getItem(LAYOUT_STORAGE_KEY)!)).toEqual({
      sidebarWidth: 310,
      sidebarOpen: true,
      splitListWidth: SPLIT_LIST.initial,
    });
  });

  it('falls back to defaults when the saved layout is damaged', () => {
    localStorage.setItem(LAYOUT_STORAGE_KEY, '{not json');
    expect(freshStore().sidebarWidth).toBe(SIDEBAR.initial);

    localStorage.setItem(
      LAYOUT_STORAGE_KEY,
      JSON.stringify({ sidebarWidth: 'wide', sidebarOpen: 'yes' }),
    );
    const layout = freshStore();
    expect(layout.sidebarWidth).toBe(SIDEBAR.initial);
    expect(layout.sidebarOpen).toBe(true);
  });

  it('starts the Layout split list at 560 px and keeps it between 400 and 760 px', () => {
    const layout = freshStore();
    expect(layout.splitListWidth).toBe(560);
    layout.setSplitListWidth(100);
    expect(layout.splitListWidth).toBe(400);
    layout.setSplitListWidth(9000);
    expect(layout.splitListWidth).toBe(760);
    layout.setSplitListWidth(612.4);
    expect(layout.splitListWidth).toBe(612);
  });

  it('remembers the split list width in a new session, and clamps a saved one', async () => {
    const first = freshStore();
    first.setSplitListWidth(640);
    await nextTick();
    expect(freshStore().splitListWidth).toBe(640);

    localStorage.setItem(LAYOUT_STORAGE_KEY, JSON.stringify({ splitListWidth: 2000 }));
    expect(freshStore().splitListWidth).toBe(760);
    localStorage.setItem(LAYOUT_STORAGE_KEY, JSON.stringify({ splitListWidth: 'wide' }));
    expect(freshStore().splitListWidth).toBe(560);
  });

  it('gives the initial width for anything not a finite number', () => {
    expect(clampWidth(Number.NaN, SIDEBAR)).toBe(SIDEBAR.initial);
    expect(clampWidth(null, SIDEBAR)).toBe(SIDEBAR.initial);
  });
});

describe('pane divider', () => {
  function divider(direction: 1 | -1) {
    return mount(PaneDivider, {
      props: { label: 'Resize', controls: 'x', value: 240, min: 180, max: 400, direction },
    });
  }

  it('exposes its value to assistive technology', () => {
    const el = divider(1).get('[role="separator"]');
    expect(el.attributes('aria-valuenow')).toBe('240');
    expect(el.attributes('aria-valuemin')).toBe('180');
    expect(el.attributes('aria-valuemax')).toBe('400');
  });

  it('resizes by arrow keys in the direction of its column', async () => {
    const left = divider(1);
    await left.trigger('keydown', { key: 'ArrowRight' });
    await left.trigger('keydown', { key: 'Home' });
    expect(left.emitted('resize')).toEqual([[248], [180]]);

    const right = divider(-1);
    await right.trigger('keydown', { key: 'ArrowRight' });
    await right.trigger('keydown', { key: 'End' });
    await right.trigger('keydown', { key: 'Enter' });
    expect(right.emitted('resize')).toEqual([[232], [400]]);
  });

  it('resizes by pointer drag in the direction of its column', async () => {
    // jsdom has no PointerEvent; a MouseEvent carries the same clientX.
    const pointer = async (w: ReturnType<typeof divider>, type: string, clientX: number) => {
      w.element.dispatchEvent(new MouseEvent(type, { clientX, button: 0, bubbles: true }));
      await nextTick();
    };
    const left = divider(1);
    await pointer(left, 'pointerdown', 100);
    await pointer(left, 'pointermove', 130);
    await pointer(left, 'pointerup', 130);
    await pointer(left, 'pointermove', 200);
    expect(left.emitted('resize')).toEqual([[270]]);

    const right = divider(-1);
    await pointer(right, 'pointerdown', 100);
    await pointer(right, 'pointermove', 130);
    expect(right.emitted('resize')).toEqual([[210]]);
  });
});

describe('the panes fold by themselves (E58.6)', () => {
  const both = { sidebar: 232, inspector: 320 };

  it('shows every open pane when there is room', () => {
    expect(panesShown(232 + 320 + MAIN_ROOM, both)).toEqual({ sidebar: true, inspector: true });
  });

  it('folds the inspector first, then the sidebar', () => {
    expect(panesShown(232 + 320 + MAIN_ROOM - 1, both)).toEqual({
      sidebar: true,
      inspector: false,
    });
    expect(panesShown(232 + MAIN_ROOM, both)).toEqual({ sidebar: true, inspector: false });
    expect(panesShown(232 + MAIN_ROOM - 1, both)).toEqual({ sidebar: false, inspector: false });
  });

  it('never shows a pane the user folded, and gives its room to the others', () => {
    expect(panesShown(5000, { sidebar: null, inspector: null })).toEqual({
      sidebar: false,
      inspector: false,
    });
    // The sidebar folded by hand: the inspector alone fits where both would not.
    expect(panesShown(320 + MAIN_ROOM, { sidebar: null, inspector: 320 })).toEqual({
      sidebar: false,
      inspector: true,
    });
  });

  it('folds nothing at 1000 px and wider, whatever the sidebar width', () => {
    expect(panesShown(1000, { sidebar: SIDEBAR.max, inspector: null }).sidebar).toBe(true);
  });

  it('folds the default sidebar at 667 px and keeps it at 800 px', () => {
    const panes = { sidebar: SIDEBAR.initial, inspector: null };
    expect(panesShown(667, panes).sidebar).toBe(false);
    expect(panesShown(800, panes).sidebar).toBe(true);
  });
});

describe('the store follows the window and keeps the user choice (E58.6)', () => {
  const initial = window.innerWidth;
  async function resize(width: number) {
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: width });
    window.dispatchEvent(new Event('resize'));
    await nextTick();
  }
  afterEach(() => resize(initial));

  it('folds the sidebar without room, brings it back with room, and saves nothing', async () => {
    const layout = freshStore();
    await resize(667);
    expect(layout.sidebarShown).toBe(false);
    expect(layout.sidebarFolded).toBe(true);
    expect(layout.sidebarOpen).toBe(true);
    await resize(1000);
    expect(layout.sidebarShown).toBe(true);
    expect(layout.sidebarFolded).toBe(false);
    expect(JSON.parse(localStorage.getItem(LAYOUT_STORAGE_KEY) ?? '{}').sidebarOpen).not.toBe(
      false,
    );
  });

  it('keeps a sidebar the user folded folded when the room comes back', async () => {
    const layout = freshStore();
    layout.toggleSidebar();
    await resize(667);
    await resize(1280);
    expect(layout.sidebarOpen).toBe(false);
    expect(layout.sidebarShown).toBe(false);
  });

  it('shows a folded sidebar on request without changing the saved choice', async () => {
    const layout = freshStore();
    await resize(667);
    layout.toggleSidebar();
    expect(layout.sidebarShown).toBe(true);
    layout.toggleSidebar();
    expect(layout.sidebarShown).toBe(false);
    expect(layout.sidebarOpen).toBe(true);
    // A look ends when the room changes; with room the sidebar is back by the user's choice.
    layout.toggleSidebar();
    await resize(700);
    expect(layout.sidebarShown).toBe(false);
    await resize(1000);
    expect(layout.sidebarShown).toBe(true);
  });
});
