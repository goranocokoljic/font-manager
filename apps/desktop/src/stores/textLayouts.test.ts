import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useTextLayoutsStore } from './textLayouts';

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => setClient(undefined));

function fresh() {
  const client = createMockClient();
  setClient(client);
  return { client, store: useTextLayoutsStore() };
}

describe('text layouts store', () => {
  it('lists the files, presets first, with the broken one and its reason', async () => {
    const { store } = fresh();
    await store.loadList();
    expect(store.entries.map((e) => e.id)).toEqual([
      'book',
      'columns',
      'magazine',
      'quote',
      'characters',
      'waterfall',
      'draft',
    ]);
    expect(store.entries.at(-1)?.broken).toMatch(/Not valid JSON/);
    expect(await store.open('draft')).toBe(false);
    expect(store.error?.code).toBe('invalidRequest');
    expect(store.draft).toBeNull();
  });

  it('edits the draft, saves it through the client and keeps the old one as a version', async () => {
    const { client, store } = fresh();
    const spy = vi.spyOn(client, 'saveLayout');
    await store.open('book');
    expect(store.dirty).toBe(false);
    store.setBlockText(0, 'Chapter Two');
    expect(store.dirty).toBe(true);
    expect(store.saved?.layout.blocks[0].text).toBe('Chapter One');
    await store.save();
    expect(spy).toHaveBeenCalledWith(
      expect.objectContaining({ id: 'book', layout: expect.objectContaining({ name: 'Book' }) }),
    );
    expect(store.dirty).toBe(false);
    expect(store.saved?.layout.blocks[0].text).toBe('Chapter Two');
    await store.loadHistory();
    expect(store.history.map((v) => v.number)).toEqual([1]);
  });

  it('reverts unsaved changes to the saved file', async () => {
    const { store } = fresh();
    await store.open('quote');
    store.setBlockText(1, 'Someone else');
    await store.revert();
    expect(store.dirty).toBe(false);
    expect(store.draft?.blocks[1].text).toBe('Matthew Carter');
  });

  it('duplicates into a new file and opens the copy; the original stays', async () => {
    const { client, store } = fresh();
    await store.open('book');
    store.setBlockText(0, 'Not saved');
    const copy = await store.duplicate();
    expect(copy?.id).toBe('book-copy');
    expect(store.saved?.id).toBe('book-copy');
    expect(store.draft?.blocks[0].text).toBe('Chapter One');
    expect(store.entries.some((e) => e.id === 'book-copy')).toBe(true);
    expect((await client.getLayout({ id: 'book' })).layout.blocks[0].text).toBe('Chapter One');
    // The original's unsaved change is still there when the user goes back to it.
    await store.open('book');
    expect(store.draft?.blocks[0].text).toBe('Not saved');
  });

  it('restores a version and resets a preset to its shipped original', async () => {
    const { store } = fresh();
    await store.open('columns');
    store.setBlockText(0, 'Edited');
    await store.save();
    await store.resetToOriginal();
    expect(store.saved?.layout.blocks[0].text).toBe('Chapter One');
    await store.loadHistory();
    expect(store.history.map((v) => v.number)).toEqual([2, 1]);
    await store.restore(2);
    expect(store.saved?.layout.blocks[0].text).toBe('Edited');
    expect(store.history.map((v) => v.number)).toEqual([3, 2, 1]);
  });

  it('keeps an unsaved draft when the user switches to another layout and back', async () => {
    const { store } = fresh();
    await store.open('book');
    store.setBlockText(0, 'Draft heading');
    await store.open('quote');
    expect(store.dirty).toBe(false);
    await store.open('book');
    expect(store.dirty).toBe(true);
    expect(store.draft?.blocks[0].text).toBe('Draft heading');
    await store.revert();
    await store.open('quote');
    await store.open('book');
    expect(store.dirty).toBe(false);
  });

  it('drops an open that answers after a later one', async () => {
    const { client, store } = fresh();
    let release: () => void = () => undefined;
    const slow = new Promise<void>((r) => (release = r));
    const real = client.getLayout.bind(client);
    vi.spyOn(client, 'getLayout').mockImplementationOnce(async (req) => {
      await slow;
      return real(req);
    });
    const first = store.open('book');
    await store.open('quote');
    release();
    expect(await first).toBe(false);
    expect(store.saved?.id).toBe('quote');
  });

  it('changes properties of one block through the core clamp; the other blocks keep theirs', async () => {
    const { client, store } = fresh();
    const clamp = vi.spyOn(client, 'clampLayout');
    await store.open('book');
    const before = JSON.parse(JSON.stringify(store.draft!.blocks[0]));
    const block = await store.setBlockProps(1, { sizePt: 5000, columns: 9, align: 'center' });
    expect(clamp).toHaveBeenCalledOnce();
    // Only the edited block went to the core, with the change applied.
    expect(clamp.mock.calls[0]![0].blocks).toHaveLength(1);
    expect(clamp.mock.calls[0]![0].blocks[0]).toMatchObject({ sizePt: 5000, columns: 9 });
    expect(block).toMatchObject({ sizePt: 600, columns: 3, align: 'center' });
    expect(store.draft!.blocks[1]).toMatchObject({ sizePt: 600, columns: 3, align: 'center' });
    expect(store.draft!.blocks[0]).toEqual(before);
    expect(store.dirty).toBe(true);
    // A field the user emptied is not a number: the core gives it the default.
    await store.setBlockProps(1, { tracking: Number.NaN });
    expect(store.draft!.blocks[1]!.tracking).toBe(0);
  });

  it('loads the core bounds once', async () => {
    const { client, store } = fresh();
    const spy = vi.spyOn(client, 'layoutBounds');
    expect((await store.loadBounds())?.sizePt.max).toBe(600);
    await store.loadBounds();
    expect(spy).toHaveBeenCalledOnce();
    expect(store.bounds?.columns).toEqual({ min: 1, max: 3 });
  });

  it('drops a property change that answers after a later one to the same block', async () => {
    const { client, store } = fresh();
    await store.open('book');
    let release: () => void = () => undefined;
    const slow = new Promise<void>((r) => (release = r));
    const real = client.clampLayout.bind(client);
    vi.spyOn(client, 'clampLayout').mockImplementationOnce(async (req) => {
      await slow;
      return real(req);
    });
    const first = store.setBlockProps(0, { sizePt: 40 });
    await store.setBlockProps(0, { sizePt: 50 });
    release();
    await first;
    expect(store.draft!.blocks[0]!.sizePt).toBe(50);
  });

  it('keeps text typed while a property change was on its way', async () => {
    const { client, store } = fresh();
    await store.open('book');
    let release: () => void = () => undefined;
    const slow = new Promise<void>((r) => (release = r));
    const real = client.clampLayout.bind(client);
    vi.spyOn(client, 'clampLayout').mockImplementationOnce(async (req) => {
      await slow;
      return real(req);
    });
    const change = store.setBlockProps(0, { sizePt: 30 });
    store.setBlockText(0, 'Typed meanwhile');
    release();
    await change;
    expect(store.draft!.blocks[0]).toMatchObject({ sizePt: 30, text: 'Typed meanwhile' });
  });

  it('starts locked at the core width of the body block; unlocking follows the window', async () => {
    const { store } = fresh();
    await store.open('book');
    await vi.waitFor(() => expect(store.width).not.toBeNull());
    expect(store.widthLocked).toBe(true);
    // Book's body is 11 pt: 33 em at 4/3 px per point.
    expect(store.width).toBeCloseTo(11 * (96 / 72) * 33, 3);
    await store.setWidthLock(false);
    expect(store.draft!.widthLocked).toBe(false);
    await vi.waitFor(() => expect(store.width).toBeNull());
    expect(store.dirty).toBe(true);
  });

  it('locks again at the width given, clamped by the core, and keeps it with the saved file', async () => {
    const { client, store } = fresh();
    await store.open('book');
    await store.setWidthLock(false);
    await store.setWidthLock(true, 1234);
    expect(store.draft).toMatchObject({ widthLocked: true, widthPx: 1234 });
    await vi.waitFor(() => expect(store.width).toBe(1234));
    await store.save();
    const saved = await client.getLayout({ id: 'book' });
    expect(saved.layout).toMatchObject({ widthLocked: true, widthPx: 1234 });
    await store.setWidthLock(false);
    await store.setWidthLock(true, 5);
    expect(store.draft!.widthPx).toBe(160);
  });

  it('a larger body block gives a wider readable width', async () => {
    const { store } = fresh();
    await store.open('book');
    await vi.waitFor(() => expect(store.width).not.toBeNull());
    const before = store.width!;
    await store.setBlockProps(1, { sizePt: 22 });
    await vi.waitFor(() => expect(store.width).toBeCloseTo(before * 2, 3));
  });
});
