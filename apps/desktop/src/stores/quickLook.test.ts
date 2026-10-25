import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it } from 'vitest';
import type { FaceSummary } from '../api/types';
import { useQuickLookStore } from './quickLook';

const face = (id: number) => ({ id, familyName: 'Inter', style: 'Regular' }) as FaceSummary;

describe('quick look store', () => {
  beforeEach(() => setActivePinia(createPinia()));

  it('toggle opens on the face, then closes', () => {
    const store = useQuickLookStore();
    store.toggle(face(1));
    expect(store.face?.id).toBe(1);
    store.toggle(face(2));
    expect(store.face).toBeNull();
  });

  it('open replaces the face; close empties it', () => {
    const store = useQuickLookStore();
    store.open(face(1));
    store.open(face(2));
    expect(store.face?.id).toBe(2);
    store.close();
    expect(store.face).toBeNull();
  });
});
