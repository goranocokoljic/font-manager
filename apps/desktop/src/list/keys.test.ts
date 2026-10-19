import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useFontListStore } from '../stores/fontList';
import { useLibraryStore } from '../stores/library';
import { handleListKey } from './keys';

const key = (k: string, init: KeyboardEventInit = {}) =>
  new KeyboardEvent('keydown', { key: k, ...init });

async function setup() {
  setClient(createMockClient({ extraFaces: 17 }));
  await useLibraryStore().load();
  const list = useFontListStore();
  list.moveCursor(0);
  return list;
}

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => setClient(undefined));

describe('list keys', () => {
  const open = () => undefined;

  it('in one column, Up/Down move a row and Right/Left expand and collapse', async () => {
    const list = await setup();
    expect(handleListKey(key('ArrowDown'), list, { page: 5, count: 20, open })).toBe(true);
    expect(list.cursorIndex).toBe(1);
    handleListKey(key('ArrowRight'), list, { page: 5, count: 20, open });
    expect(list.cursorIndex).toBe(1);
    expect(list.expanded.size).toBe(1);
  });

  it('in a grid, Up/Down move a line of cards and Left/Right one card', async () => {
    const list = await setup();
    const opts = { page: 12, count: 20, open, columns: 4 };
    handleListKey(key('ArrowDown'), list, opts);
    expect(list.cursorIndex).toBe(4);
    handleListKey(key('ArrowRight'), list, opts);
    expect(list.cursorIndex).toBe(5);
    handleListKey(key('ArrowLeft'), list, opts);
    handleListKey(key('ArrowLeft'), list, opts);
    expect(list.cursorIndex).toBe(3);
    handleListKey(key('ArrowUp'), list, opts);
    expect(list.cursorIndex).toBe(0);
    expect(list.expanded.size).toBe(0);
    // Shift extends the selection along the cards.
    handleListKey(key('ArrowRight', { shiftKey: true }), list, opts);
    expect(list.selectedCount).toBe(2);
    handleListKey(key('PageDown'), list, opts);
    expect(list.cursorIndex).toBe(13);
  });
});
