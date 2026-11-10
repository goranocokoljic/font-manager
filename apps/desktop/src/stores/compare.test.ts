import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient } from '../api/client';
import type { FaceSummary } from '../api/types';
import { createMockClient, mockComparisons, mockFaces } from '../api/mock';
import { setLocale } from '../i18n';
import { COMPARE_MODES, MAX_PINS, useCompareStore } from './compare';
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

  it('the compare icon: a face is in the set when pinned, a family when any of its faces is (E48.4)', () => {
    const s = useCompareStore();
    const a1 = { ...faces[0]!, family: 7 };
    const a2 = { ...faces[1]!, family: 7 };
    const b = { ...faces[2]!, family: 8 };
    s.pin(a1);
    expect(s.inSet(a1)).toBe(true);
    expect(s.inSet(a2)).toBe(false);
    expect(s.inSet(a2, 'family')).toBe(true);
    expect(s.inSet(b, 'family')).toBe(false);
  });

  it('toggle adds outside the set and removes inside it, a family with all its faces (E48.4)', () => {
    const s = useCompareStore();
    const toast = useToastStore();
    const a1 = { ...faces[0]!, family: 7, familyName: 'Alpha' };
    const a2 = { ...faces[1]!, family: 7, familyName: 'Alpha' };
    const b = { ...faces[2]!, family: 8, familyName: 'Beta' };
    expect(s.toggle(a1)).toBe('pinned');
    expect(s.toggle(a2)).toBe('pinned');
    expect(s.toggle(b, 'family')).toBe('pinned');
    expect(s.toggle(a2)).toBe('removed');
    expect(s.pins.map((p) => p.id)).toEqual([a1.id, b.id]);
    expect(toast.toast?.message).toBe('Alpha Style 1 removed from Compare.');
    s.pin(a2);
    expect(s.toggle(a1, 'family')).toBe('removed');
    expect(s.pins.map((p) => p.id)).toEqual([b.id]);
    expect(toast.toast?.message).toBe('Alpha removed from Compare.');
    setLocale('de');
    s.toggle(b, 'family');
    expect(toast.toast?.message).toBe('Beta aus dem Vergleich entfernt.');
    setLocale('en');
  });

  it('pinMany adds up to four, skips the rest and says both in one toast (E48.4)', () => {
    const s = useCompareStore();
    const toast = useToastStore();
    s.pin(faces[0]!);
    const got = s.pinMany(faces, 3);
    expect(got).toEqual({ added: 3, already: 1, skipped: 4 });
    expect(s.pins.map((p) => p.id)).toEqual([100, 101, 102, 103]);
    expect(toast.toast?.message).toBe(
      '3 fonts added to Compare (4 of 4). 4 skipped: Compare holds 4 fonts.',
    );
    expect(s.pinMany([faces[4]!])).toEqual({ added: 0, already: 0, skipped: 1 });
    expect(toast.toast?.message).toBe('1 skipped: Compare holds 4 fonts.');
    expect(s.pinMany([faces[0]!])).toEqual({ added: 0, already: 1, skipped: 0 });
    expect(toast.toast?.message).toBe('These fonts are already in Compare.');
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

  it('offers Stacked, Grid and Overlay with icons, in that order (en, de)', () => {
    try {
      expect(COMPARE_MODES.map((m) => [m.value, m.icon, m.label])).toEqual([
        ['stacked', 'stacked', 'Stacked'],
        ['sideBySide', 'grid', 'Grid'],
        ['overlay', 'overlay', 'Overlay'],
      ]);
      setLocale('de');
      expect(COMPARE_MODES.map((m) => m.label)).toEqual(['Untereinander', 'Raster', 'Überlagert']);
    } finally {
      setLocale('en');
    }
  });

  it('opens a new comparison Stacked, also after the last pin of the previous one goes', () => {
    const s = useCompareStore();
    expect(s.mode).toBe('stacked');
    s.pin(faces[0]!);
    s.pin(faces[1]!);
    s.setMode('overlay');
    s.unpin(100);
    // One pin is left: the comparison and its mode stay.
    expect(s.mode).toBe('overlay');
    s.unpin(101);
    expect(s.mode).toBe('stacked');
  });

  it('stores Grid as sideBySide, so old Side by side saves reopen as Grid', async () => {
    const client = createMockClient();
    setClient(client);
    const s = useCompareStore();
    s.pin(mockFaces[0]!.face);
    s.pin(mockFaces[1]!.face);
    s.setMode('sideBySide');
    expect(await s.save('Grid pair', '')).toBe(true);
    const id = s.current!.id;
    expect((await client.getComparison({ id })).mode).toBe('sideBySide');
    s.setMode('overlay');
    await s.reopen(id);
    expect(s.mode).toBe('sideBySide');
    expect(COMPARE_MODES.find((m) => m.value === s.mode)?.label).toBe('Grid');
  });
});
