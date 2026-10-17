import { mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it } from 'vitest';
import { nextTick } from 'vue';
import PaneDivider from '../components/PaneDivider.vue';
import {
  INSPECTOR,
  LAYOUT_STORAGE_KEY,
  SIDEBAR,
  clampWidth,
  isInspectorShortcut,
  useLayoutStore,
} from './layout';

function freshStore() {
  setActivePinia(createPinia());
  return useLayoutStore();
}

function key(init: KeyboardEventInit) {
  return new KeyboardEvent('keydown', init);
}

beforeEach(() => localStorage.clear());

describe('layout store', () => {
  it('starts with the default widths and the inspector open', () => {
    const layout = freshStore();
    expect(layout.sidebarWidth).toBe(SIDEBAR.initial);
    expect(layout.inspectorWidth).toBe(INSPECTOR.initial);
    expect(layout.inspectorVisible).toBe(true);
  });

  it('keeps column widths inside their bounds', () => {
    const layout = freshStore();
    layout.setSidebarWidth(10);
    expect(layout.sidebarWidth).toBe(SIDEBAR.min);
    layout.setInspectorWidth(5000);
    expect(layout.inspectorWidth).toBe(INSPECTOR.max);
    layout.setSidebarWidth(250.6);
    expect(layout.sidebarWidth).toBe(251);
  });

  it('hides the inspector on a narrow window and reveals it with the toggle', () => {
    const layout = freshStore();
    layout.setNarrow(true);
    expect(layout.inspectorVisible).toBe(false);
    layout.toggleInspector();
    expect(layout.inspectorVisible).toBe(true);
    // The session reveal does not change the saved wide-window choice.
    expect(layout.inspectorOpen).toBe(true);
    layout.toggleInspector();
    expect(layout.inspectorVisible).toBe(false);
  });

  it('collapses again when the window turns narrow after a reveal', () => {
    const layout = freshStore();
    layout.setNarrow(true);
    layout.toggleInspector();
    layout.setNarrow(false);
    layout.setNarrow(true);
    expect(layout.inspectorVisible).toBe(false);
  });

  it('returns to the saved choice when the window widens', () => {
    const layout = freshStore();
    layout.toggleInspector();
    expect(layout.inspectorVisible).toBe(false);
    layout.setNarrow(true);
    layout.toggleInspector();
    layout.setNarrow(false);
    expect(layout.inspectorVisible).toBe(false);
  });

  it('restores widths and inspector state in a new session', async () => {
    const first = freshStore();
    first.setSidebarWidth(300);
    first.setInspectorWidth(400);
    first.toggleInspector();
    await nextTick();

    const second = freshStore();
    expect(second.sidebarWidth).toBe(300);
    expect(second.inspectorWidth).toBe(400);
    expect(second.inspectorOpen).toBe(false);
  });

  it('falls back to defaults when the saved layout is damaged', () => {
    localStorage.setItem(LAYOUT_STORAGE_KEY, '{not json');
    expect(freshStore().sidebarWidth).toBe(SIDEBAR.initial);

    localStorage.setItem(
      LAYOUT_STORAGE_KEY,
      JSON.stringify({ sidebarWidth: 'wide', inspectorWidth: 9999, inspectorOpen: 'yes' }),
    );
    const layout = freshStore();
    expect(layout.sidebarWidth).toBe(SIDEBAR.initial);
    expect(layout.inspectorWidth).toBe(INSPECTOR.max);
    expect(layout.inspectorOpen).toBe(true);
  });

  it('gives the initial width for anything not a finite number', () => {
    expect(clampWidth(Number.NaN, SIDEBAR)).toBe(SIDEBAR.initial);
    expect(clampWidth(null, SIDEBAR)).toBe(SIDEBAR.initial);
  });
});

describe('inspector shortcut', () => {
  it('is Ctrl+I or Cmd+I', () => {
    expect(isInspectorShortcut(key({ key: 'i', ctrlKey: true }))).toBe(true);
    expect(isInspectorShortcut(key({ key: 'I', metaKey: true }))).toBe(true);
  });

  it('ignores other keys and extra modifiers', () => {
    expect(isInspectorShortcut(key({ key: 'i' }))).toBe(false);
    expect(isInspectorShortcut(key({ key: 'I', ctrlKey: true, shiftKey: true }))).toBe(false);
    expect(isInspectorShortcut(key({ key: 'i', ctrlKey: true, altKey: true }))).toBe(false);
    expect(isInspectorShortcut(key({ key: 'k', ctrlKey: true }))).toBe(false);
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
