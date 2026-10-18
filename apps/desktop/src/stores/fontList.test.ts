import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient, EXTRA_ID_BASE } from '../api/mock';
import { faceKey, familyKey } from '../list/rows';
import { useFontListStore } from './fontList';
import { useLibraryStore } from './library';

// Three sample families, then two extra ones with four styles each.
const A = EXTRA_ID_BASE;
const B = EXTRA_ID_BASE + 1;

async function setup() {
  setClient(createMockClient({ extraFaces: 2, stylesPerFamily: 4 }));
  await useLibraryStore().load();
  return useFontListStore();
}

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => setClient(undefined));

const keysOf = (list: ReturnType<typeof useFontListStore>) => list.rows.map((r) => r.key);
const selectedOf = (list: ReturnType<typeof useFontListStore>) => [...list.selection.selected];

describe('font list store', () => {
  it('expanding a family puts its styles inline under it; collapsing removes them', async () => {
    const list = await setup();
    expect(list.rows).toHaveLength(5);
    list.expand(A);
    await flushPromises();
    expect(keysOf(list).slice(3, 9)).toEqual([
      familyKey(A),
      faceKey(A),
      faceKey(A + 2),
      faceKey(A + 1),
      faceKey(A + 3),
      familyKey(B),
    ]);
    list.toggleExpanded(A);
    expect(list.rows).toHaveLength(5);
  });

  it('Right expands then enters, Left leaves then collapses', async () => {
    const list = await setup();
    list.click(familyKey(A));
    list.expandOrEnter();
    await flushPromises();
    expect(list.expanded.has(A)).toBe(true);
    expect(list.cursor).toBe(familyKey(A));
    list.expandOrEnter();
    expect(list.cursor).toBe(faceKey(A));
    expect(list.cursorFace?.style).toBe('Regular');
    list.collapseOrLeave();
    expect(list.cursor).toBe(familyKey(A));
    list.collapseOrLeave();
    expect(list.expanded.has(A)).toBe(false);
  });

  it('collapsing a family moves a cursor on its styles to the family and drops them', async () => {
    const list = await setup();
    list.expand(A);
    await flushPromises();
    list.click(familyKey(B));
    list.click(faceKey(A + 1), 'toggle');
    list.collapse(A);
    await flushPromises();
    expect(list.cursor).toBe(familyKey(A));
    expect(selectedOf(list)).toEqual([familyKey(B)]);
  });

  it('arrows move the cursor; Shift extends; Ctrl keeps; toggle and select all', async () => {
    const list = await setup();
    list.moveBy(1); // no cursor yet: the first row
    expect(list.cursorIndex).toBe(0);
    list.moveBy(1);
    expect(selectedOf(list)).toEqual([familyKey(3)]);
    list.moveBy(2, 'extend');
    expect(selectedOf(list)).toEqual([familyKey(3), familyKey(4), familyKey(A)]);
    expect(list.selectedCount).toBe(3);
    list.moveBy(1, 'keep');
    expect(list.cursor).toBe(familyKey(B));
    list.toggleCursor();
    expect(list.selectedCount).toBe(4);
    list.selectAll();
    expect(list.selectedCount).toBe(5);
  });

  it('the cursor face of a family is its first style, once loaded', async () => {
    const list = await setup();
    list.click(familyKey(B));
    expect(list.cursorFace).toBeUndefined();
    await useLibraryStore().facesOf(B);
    expect(list.cursorFace?.id).toBe(EXTRA_ID_BASE + 4); // family B: faces 4–7, Regular first
  });

  it('keeps no selection of families a refresh removed', async () => {
    const list = await setup();
    list.click(familyKey(B));
    const client = createMockClient();
    setClient(client);
    await useLibraryStore().refresh();
    await flushPromises();
    expect(list.selectedCount).toBe(0);
    expect(list.cursor).toBeNull();
  });
});
