import { mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it } from 'vitest';
import type { FaceSummary } from '../api/types';
import { mockFaces } from '../api/mock';
import { useCompareStore } from '../stores/compare';
import CompareToggle from './CompareToggle.vue';

const faces: FaceSummary[] = Array.from({ length: 3 }, (_, i) => ({
  ...mockFaces[i % mockFaces.length]!.face,
  id: 300 + i,
  family: 30 + i,
  style: `Style ${i}`,
}));

beforeEach(() => setActivePinia(createPinia()));

describe('compare toggle (E48.5)', () => {
  it('a card icon adds to the current set and never clears it', async () => {
    const s = useCompareStore();
    s.compareFresh(faces[0]!);
    const w = mount(CompareToggle, { props: { face: faces[1]! } });
    await w.trigger('click');
    expect(s.pins.map((p) => p.id)).toEqual([300, 301]);
    expect(s.base?.id).toBe(300);
  });

  it('the family page icon (fresh) starts a new comparison with the face as its base', async () => {
    const s = useCompareStore();
    s.compareFresh(faces[0]!);
    s.pin(faces[1]!);
    s.close();
    const w = mount(CompareToggle, { props: { face: faces[2]!, scope: 'family', fresh: true } });
    await w.trigger('click');
    expect(s.pins.map((p) => p.id)).toEqual([302]);
    expect(s.base?.id).toBe(302);
    expect(s.open).toBe(true);
    // In the set, it still removes the family (E48.4).
    await w.trigger('click');
    expect(s.pins).toEqual([]);
  });
});
