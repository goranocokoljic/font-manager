import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { PreviewImage, PreviewRequest } from '../api/types';
import { useLibraryStore } from './library';
import { MAX_RENDERS, MAX_THUMBNAILS, useThumbnailsStore } from './thumbnails';

let calls: PreviewRequest[];
let pending: { req: PreviewRequest; done: (img: PreviewImage) => void; fail: () => void }[];
let urls: number;
let revoked: string[];

/** Answers the oldest pending render: `w`×`h` device px. */
function answer(w = 400, h = 80) {
  pending.shift()!.done({ width: w, height: h, png: [137, 80, 78, 71] });
}

beforeEach(() => {
  setActivePinia(createPinia());
  calls = [];
  pending = [];
  urls = 0;
  revoked = [];
  vi.stubGlobal('devicePixelRatio', 2);
  URL.createObjectURL = vi.fn(() => `blob:thumb-${++urls}`);
  URL.revokeObjectURL = vi.fn((url: string) => void revoked.push(url));
  const client: FmClient = {
    ...createMockClient(),
    renderPreview: (req) => {
      calls.push(structuredClone(req));
      return new Promise((done, fail) => pending.push({ req, done, fail: () => fail('x') }));
    },
  };
  setClient(client);
});

afterEach(() => {
  setClient(undefined);
  vi.unstubAllGlobals();
});

describe('thumbnails store', () => {
  it('asks the API once per face, text and size, at the device pixel ratio', async () => {
    const thumbs = useThumbnailsStore();
    thumbs.want(1, 'Ag', 36);
    thumbs.want(1, 'Ag', 36);
    thumbs.want(1, '', 36);
    expect(calls).toEqual([{ face: 1, text: 'Ag', sizePx: 72 }]);
    expect(thumbs.get(1, 'Ag', 36)).toBeUndefined();
    answer(400, 80);
    await flushPromises();
    // CSS px: the device px over the ratio.
    expect(thumbs.get(1, 'Ag', 36)).toEqual({ url: 'blob:thumb-1', width: 200, height: 40 });
    thumbs.want(1, 'Ag', 36);
    expect(calls).toHaveLength(1);
  });

  it(`runs at most ${MAX_RENDERS} renders at once; the rest wait their turn`, async () => {
    const thumbs = useThumbnailsStore();
    for (let face = 1; face <= MAX_RENDERS + 3; face++) thumbs.want(face, 'Ag', 36);
    expect(calls).toHaveLength(MAX_RENDERS);
    answer();
    answer();
    await flushPromises();
    expect(calls).toHaveLength(MAX_RENDERS + 2);
  });

  it('marks a failed render and does not ask again', async () => {
    const thumbs = useThumbnailsStore();
    thumbs.want(1, 'Ag', 36);
    pending.shift()!.fail();
    await flushPromises();
    expect(thumbs.hasFailed(1, 'Ag', 36)).toBe(true);
    thumbs.want(1, 'Ag', 36);
    expect(calls).toHaveLength(1);
  });

  it(`keeps ${MAX_THUMBNAILS} thumbnails and revokes the oldest beyond`, async () => {
    const thumbs = useThumbnailsStore();
    for (let face = 1; face <= MAX_THUMBNAILS + 1; face++) {
      thumbs.want(face, 'Ag', 36);
      answer();
      await flushPromises();
    }
    expect(revoked).toEqual(['blob:thumb-1']);
    expect(thumbs.get(1, 'Ag', 36)).toBeUndefined();
    expect(thumbs.get(2, 'Ag', 36)).toBeDefined();
    // A dropped thumbnail is asked for again when a card needs it.
    thumbs.want(1, 'Ag', 36);
    expect(calls).toHaveLength(MAX_THUMBNAILS + 2);
  });

  it('drops every thumbnail when the library changes, and ignores late answers', async () => {
    const thumbs = useThumbnailsStore();
    thumbs.want(1, 'Ag', 36);
    answer();
    thumbs.want(2, 'Ag', 36);
    await flushPromises();
    await useLibraryStore().load();
    await flushPromises();
    expect(revoked).toEqual(['blob:thumb-1']);
    expect(thumbs.get(1, 'Ag', 36)).toBeUndefined();
    answer();
    await flushPromises();
    expect(thumbs.get(2, 'Ag', 36)).toBeUndefined();
    thumbs.want(1, 'Ag', 36);
    expect(calls.at(-1)).toEqual({ face: 1, text: 'Ag', sizePx: 72 });
  });
});
