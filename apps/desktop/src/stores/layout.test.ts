import { mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it } from 'vitest';
import { nextTick } from 'vue';
import PaneDivider from '../components/PaneDivider.vue';
import { LAYOUT_STORAGE_KEY, SIDEBAR, clampWidth, useLayoutStore } from './layout';

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
