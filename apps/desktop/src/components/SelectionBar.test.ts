// The selection bar (E44.1): its name, and that every action takes the whole selection.
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import type { DragFilesRequest } from '../api/types';
import { createMockClient, EXTRA_ID_BASE, MOCK_NO_FILE_DRAG } from '../api/mock';
import { faceKey, familyKey } from '../list/rows';
import { useActivationStore } from '../stores/activation';
import { useAppStore } from '../stores/app';
import { useCollectionsStore } from '../stores/collections';
import { MAX_PINS, useCompareStore } from '../stores/compare';
import { useFontListStore } from '../stores/fontList';
import { useLibraryStore } from '../stores/library';
import { useTaggingStore } from '../stores/tagging';
import { useToastStore } from '../stores/toast';
import SelectionBar from './SelectionBar.vue';

const push = vi.fn();
vi.mock('vue-router', () => ({ useRouter: () => ({ push }) }));

let wrapper: VueWrapper | undefined;

const A = EXTRA_ID_BASE;
const B = EXTRA_ID_BASE + 1;

async function setup(
  opts: { fileDrag?: 'off' | 'on'; onFileDrag?: (req: DragFilesRequest) => void } = {},
) {
  const pinia = createPinia();
  setActivePinia(pinia);
  setClient(createMockClient({ extraFaces: 7, stylesPerFamily: 2, ...opts }));
  await useLibraryStore().load();
  await useAppStore().load();
  const list = useFontListStore();
  wrapper = mount(SelectionBar, { global: { plugins: [pinia] }, attachTo: document.body });
  await flushPromises();
  return { w: wrapper, list };
}

beforeEach(() => push.mockReset());
afterEach(() => {
  wrapper?.unmount();
  wrapper = undefined;
  setClient(undefined);
  vi.restoreAllMocks();
});

describe('SelectionBar', () => {
  it('is a toolbar named for its selection, every action a Tab stop', async () => {
    const { w, list } = await setup();
    list.click(familyKey(A));
    list.click(familyKey(B), 'toggle');
    await flushPromises();
    const bar = w.get('[role="toolbar"]');
    expect(bar.attributes('aria-label')).toBe('Selection');
    expect(w.get('[data-testid="selection-bar-name"]').text()).toBe('2 families');
    const stops = bar.findAll('button');
    expect(stops.map((b) => b.text() || b.attributes('aria-label'))).toEqual([
      'Activate',
      'Collection',
      'Tag',
      'Compare',
      'Studio',
      'Files',
      'Clear the selection',
    ]);
    for (const b of stops) expect(b.attributes('tabindex')).not.toBe('-1');
    // A style among them: they are fonts, not families.
    list.expand(A);
    await flushPromises();
    list.click(faceKey(list.shownFaces(A)![0]!.id), 'toggle');
    await flushPromises();
    expect(w.get('[data-testid="selection-bar-name"]').text()).toBe('3 fonts');
  });

  it('Activate toggles the whole selection, as the A key does', async () => {
    const { w, list } = await setup();
    const activation = useActivationStore();
    const toggle = vi.spyOn(activation, 'toggleRows').mockResolvedValue();
    list.click(familyKey(A));
    list.click(familyKey(B), 'toggle');
    await w.get('[data-testid="bar-activate"]').trigger('click');
    expect(toggle).toHaveBeenCalledTimes(1);
    expect(toggle.mock.calls[0]![0].map((r) => r.key)).toEqual([familyKey(A), familyKey(B)]);
  });

  it('Collection adds the whole selection to the collection picked', async () => {
    const { w, list } = await setup();
    const collections = useCollectionsStore();
    await collections.create('Editorial');
    const add = vi.spyOn(collections, 'add');
    list.click(familyKey(A));
    list.click(familyKey(B), 'toggle');
    await flushPromises();
    const button = w.get('[data-testid="bar-collection"]');
    await button.trigger('click');
    await flushPromises();
    const option = [...document.querySelectorAll<HTMLElement>('[role="option"]')].find(
      (o) => o.textContent?.trim() === 'Editorial',
    )!;
    option.click();
    await flushPromises();
    expect(add).toHaveBeenCalledWith(expect.any(Number), { faces: [], families: [A, B] });
    // Nothing opens after the pick: the focus goes back to the button.
    expect(document.activeElement).toBe(button.element);
  });

  it('Tag opens the tagging panel for the whole selection', async () => {
    const { w, list } = await setup();
    const open = vi.spyOn(useTaggingStore(), 'openFor').mockResolvedValue();
    list.click(familyKey(A));
    list.click(familyKey(B), 'toggle');
    await w.get('[data-testid="bar-tag"]').trigger('click');
    expect(open).toHaveBeenCalledWith({ faces: [], families: [A, B] });
  });

  it('Compare pins the faces of the selection, up to the sheet’s four, and opens it', async () => {
    const { w, list } = await setup();
    const compare = useCompareStore();
    list.selectAll();
    expect(list.selectedCount).toBeGreaterThan(MAX_PINS);
    await w.get('[data-testid="bar-compare"]').trigger('click');
    await flushPromises();
    expect(compare.pins).toHaveLength(MAX_PINS);
    const firstFamily = list.rows[0]!.family.id;
    expect(compare.pins[0]!.family).toBe(firstFamily);
    expect(compare.open).toBe(true);
  });

  it('Studio opens the first face, the second as the headline', async () => {
    const { w, list } = await setup();
    list.click(familyKey(A));
    list.click(familyKey(B), 'toggle');
    await w.get('[data-testid="bar-studio"]').trigger('click');
    await flushPromises();
    const faceA = list.previewFace(A)!.id;
    const faceB = list.previewFace(B)!.id;
    expect(push).toHaveBeenCalledWith({
      name: 'studio',
      query: { face: String(faceA), headline: String(faceB) },
    });
  });

  it('× and Esc clear the selection and say so', async () => {
    const { w, list } = await setup();
    list.click(familyKey(A));
    await w.get('[data-testid="bar-clear"]').trigger('click');
    expect(list.selectedCount).toBe(0);
    list.click(familyKey(A));
    await w.get('[data-testid="bar-tag"]').trigger('keydown', { key: 'Escape' });
    expect(list.selectedCount).toBe(0);
    expect(w.emitted('cleared')).toHaveLength(2);
  });

  describe('Files (E44.4): drag the font files out of the window', () => {
    const ptr = async (el: Element, type: string, clientX: number, clientY = 300) => {
      el.dispatchEvent(new MouseEvent(type, { clientX, clientY, button: 0, bubbles: true }));
      await flushPromises();
    };

    it('starts the native drag of the whole selection only once the pointer reaches the window edge', async () => {
      const drags: DragFilesRequest[] = [];
      const { w, list } = await setup({ fileDrag: 'on', onFileDrag: (r) => drags.push(r) });
      list.click(familyKey(A));
      list.click(familyKey(B), 'toggle');
      await flushPromises();
      const files = w.get('[data-testid="bar-files"]');
      expect(files.attributes('aria-disabled')).toBe('false');
      expect(files.attributes('title')).toBe(
        'Hold and drag past the window edge to drop the font files into another app',
      );
      await ptr(files.element, 'pointerdown', 400);
      expect(files.classes()).toContain('holding');
      await ptr(files.element, 'pointermove', 200);
      expect(drags).toEqual([]);
      await ptr(files.element, 'pointermove', -3);
      expect(drags).toEqual([{ faces: [], families: [A, B] }]);
      expect(files.classes()).not.toContain('holding');
      // Moving on outside the window does not start a second drag.
      await ptr(files.element, 'pointermove', -40);
      expect(drags).toHaveLength(1);
    });

    it('lets go inside the window without dragging anything', async () => {
      const drags: DragFilesRequest[] = [];
      const { w, list } = await setup({ fileDrag: 'on', onFileDrag: (r) => drags.push(r) });
      list.click(familyKey(A));
      await flushPromises();
      const files = w.get('[data-testid="bar-files"]');
      await ptr(files.element, 'pointerdown', 400);
      await ptr(files.element, 'pointerup', 400);
      await ptr(files.element, 'pointermove', -3);
      expect(drags).toEqual([]);
    });

    it('a click says how the grip works', async () => {
      const { w, list } = await setup({ fileDrag: 'on' });
      list.click(familyKey(A));
      await flushPromises();
      const show = vi.spyOn(useToastStore(), 'show');
      await w.get('[data-testid="bar-files"]').trigger('click');
      expect(show).toHaveBeenCalledWith(
        'Hold and drag past the window edge to drop the font files into another app',
      );
    });

    it('is disabled with the reason as its tooltip where the system cannot drag files out', async () => {
      const drags: DragFilesRequest[] = [];
      const { w, list } = await setup({ onFileDrag: (r) => drags.push(r) });
      list.click(familyKey(A));
      await flushPromises();
      const files = w.get('[data-testid="bar-files"]');
      expect(files.attributes('aria-disabled')).toBe('true');
      expect(files.attributes('title')).toBe(MOCK_NO_FILE_DRAG);
      const show = vi.spyOn(useToastStore(), 'show');
      await ptr(files.element, 'pointerdown', 400);
      await ptr(files.element, 'pointermove', -3);
      await files.trigger('click');
      expect(files.classes()).not.toContain('holding');
      expect(drags).toEqual([]);
      expect(show).not.toHaveBeenCalled();
    });
  });
});
