import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient } from '../api/client';
import type { FaceSummary } from '../api/types';
import { createMockClient, mockComparisons, mockFaces } from '../api/mock';
import { MAX_PINS, useCompareStore } from './compare';
import { useToastStore } from './toast';

/** Five distinct faces: the mock's summaries with their own ids and styles. */
const faces: FaceSummary[] = Array.from({ length: 5 }, (_, i) => ({
  ...mockFaces[i % mockFaces.length]!.face,
  id: 100 + i,
  style: `Style ${i}`,
}));

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => setClient(undefined));

describe('compare store', () => {
  it('pins up to four faces and refuses a fifth with a message', () => {
    const s = useCompareStore();
    const toast = useToastStore();
    for (const f of faces.slice(0, MAX_PINS)) expect(s.pin(f)).toBe('pinned');
    expect(toast.toast?.message).toContain('(4 of 4)');
    expect(s.pin(faces[4]!)).toBe('full');
    expect(s.pins.map((p) => p.id)).toEqual([100, 101, 102, 103]);
    expect(toast.toast?.message).toBe(
      `Compare holds 4 fonts. Remove one to pin ${faces[4]!.familyName} Style 4.`,
    );
  });

  it('does not pin a face twice', () => {
    const s = useCompareStore();
    s.pin(faces[0]!);
    expect(s.pin(faces[0]!)).toBe('already');
    expect(s.pins).toHaveLength(1);
    expect(useToastStore().toast?.message).toContain('already pinned');
  });

  it('removes a pin, which frees a place; the last removal closes the sheet', () => {
    const s = useCompareStore();
    faces.slice(0, 4).forEach((f) => s.pin(f));
    s.unpin(101);
    expect(s.pins.map((p) => p.id)).toEqual([100, 102, 103]);
    expect(s.pin(faces[4]!)).toBe('pinned');
    s.openSheet();
    for (const id of [100, 102, 103, 104]) s.unpin(id);
    expect(s.pins).toEqual([]);
    expect(s.open).toBe(false);
  });

  it('keeps the pins and the mode while the sheet is closed and reopened', () => {
    const s = useCompareStore();
    s.pin(faces[0]!);
    s.pin(faces[1]!);
    s.openSheet();
    s.setMode('overlay');
    s.close();
    expect(s.open).toBe(false);
    s.openSheet();
    expect(s.open).toBe(true);
    expect(s.pins.map((p) => p.id)).toEqual([100, 101]);
    expect(s.mode).toBe('overlay');
  });

  it('does not open with nothing pinned', () => {
    const s = useCompareStore();
    s.openSheet();
    expect(s.open).toBe(false);
  });

  it('overlays the first two pins until the user chooses; choosing the other slot swaps', () => {
    const s = useCompareStore();
    faces.slice(0, 4).forEach((f) => s.pin(f));
    const pair = () => s.overlayPair.map((f) => f?.id ?? null);
    expect(pair()).toEqual([100, 101]);
    s.setOverlay(1, 103);
    expect(pair()).toEqual([100, 103]);
    s.setOverlay(0, 103);
    expect(pair()).toEqual([103, 100]);
    // A face that is not pinned is ignored.
    s.setOverlay(0, 999);
    expect(pair()).toEqual([103, 100]);
    // A chosen face that is unpinned is replaced by another pin.
    s.unpin(103);
    expect(pair()).toEqual([101, 100]);
  });

  it('has one overlay face with one pin and none with none', () => {
    const s = useCompareStore();
    expect(s.overlayPair).toEqual([null, null]);
    s.pin(faces[0]!);
    expect(s.overlayPair.map((f) => f?.id ?? null)).toEqual([100, null]);
  });

  it('saves the pins, mode and text under a name; the same name saves over it', async () => {
    const client = createMockClient();
    setClient(client);
    const s = useCompareStore();
    const [inter, mono] = [mockFaces[0]!.face, mockFaces[1]!.face];
    s.pin(inter);
    s.pin(mono);
    s.setMode('stacked');
    expect(await s.save('  Body ', 'Sphinx of black quartz')).toBe(true);
    expect(s.current?.name).toBe('Body');
    expect(s.saved.map((c) => c.name)).toEqual(['Body']);
    expect(useToastStore().toast?.message).toBe('Comparison “Body” saved.');
    const first = s.current!.id;
    const got = await client.getComparison({ id: first });
    expect(got.faces.map((f) => f.id)).toEqual([inter.id, mono.id]);
    expect([got.mode, got.sampleText]).toEqual(['stacked', 'Sphinx of black quartz']);

    // The same name again overwrites; another name makes a new one.
    s.unpin(mono.id);
    s.pin(mockFaces[2]!.face);
    await s.save('Body', '');
    expect(s.current?.id).toBe(first);
    await s.save('Other', '');
    expect(s.saved.map((c) => c.name)).toEqual(['Body', 'Other']);
  });

  it('refuses a save with one pin and says why', async () => {
    setClient(createMockClient());
    const s = useCompareStore();
    s.pin(mockFaces[0]!.face);
    expect(await s.save('Solo', '')).toBe(false);
    expect(useToastStore().toast?.message).toBe('A comparison holds 2 to 4 fonts, not 1.');
    expect(s.current).toBeNull();
  });

  it('reopens a saved comparison with the faces left and names the missing ones', async () => {
    setClient(createMockClient({ comparisons: mockComparisons }));
    const s = useCompareStore();
    await s.loadSaved();
    expect(s.canOpen).toBe(true);
    s.pin(faces[0]!);
    await s.reopen(1);
    expect(s.open).toBe(true);
    expect(s.pins.map((p) => p.id)).toEqual([1, 6]);
    expect(s.mode).toBe('stacked');
    expect(s.text).toBe('Quarterly results');
    expect(s.missing.map((m) => m.name)).toEqual(['Retired Sans Bold']);
    expect(useToastStore().toast?.message).toBe(
      'Opened “Headline shortlist”. Not in the library now: Retired Sans Bold.',
    );
    // Removing every pin forgets the saved comparison.
    s.unpin(1);
    s.unpin(6);
    expect([s.current, s.text, s.missing]).toEqual([null, null, []]);
  });

  it('does not open a saved comparison none of whose fonts are left', async () => {
    setClient(
      createMockClient({
        comparisons: [{ ...mockComparisons[0]!, faces: [{ id: 9001, name: 'Retired Sans Bold' }] }],
      }),
    );
    const s = useCompareStore();
    await s.reopen(1);
    expect(s.open).toBe(false);
    expect(s.pins).toEqual([]);
    expect(useToastStore().toast?.message).toBe(
      'None of the fonts of “Headline shortlist” are in the library now: Retired Sans Bold.',
    );
  });
});
