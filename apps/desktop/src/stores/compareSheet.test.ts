import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it } from 'vitest';
import {
  COMPARE_SHEET_STORAGE_KEY,
  SHEET_HEIGHT,
  autoSpecimenSize,
  clampSheetHeight,
  heightAtPointer,
  useCompareSheetStore,
} from './compareSheet';

beforeEach(() => {
  localStorage.clear();
  setActivePinia(createPinia());
});

describe('compare sheet height', () => {
  it('keeps the height between 40 % and the whole window', () => {
    expect(clampSheetHeight(0.1)).toBe(0.4);
    expect(clampSheetHeight(1.5)).toBe(1);
    expect(clampSheetHeight(0.7)).toBe(0.7);
    expect(clampSheetHeight('0.7')).toBe(SHEET_HEIGHT.initial);
    expect(clampSheetHeight(Number.NaN)).toBe(SHEET_HEIGHT.initial);
  });

  it('scales the Auto specimen size from 56 px at 40 % to 80 px at full', () => {
    expect(autoSpecimenSize(0.4)).toBe(56);
    expect(autoSpecimenSize(0.7)).toBe(68);
    expect(autoSpecimenSize(1)).toBe(80);
    expect(autoSpecimenSize(0.2)).toBe(56);
  });

  it('turns a pointer position into a height measured from the bottom edge', () => {
    expect(heightAtPointer(200, 800)).toBe(0.75);
    expect(heightAtPointer(-50, 800)).toBe(1);
    expect(heightAtPointer(790, 800)).toBe(0.4);
    expect(heightAtPointer(100, 0)).toBe(SHEET_HEIGHT.initial);
  });

  it('remembers the last height and restores it in a new session', () => {
    const s = useCompareSheetStore();
    expect(s.height).toBe(SHEET_HEIGHT.initial);
    s.setHeight(0.8);
    // A drag in progress is not saved until it ends.
    expect(localStorage.getItem(COMPARE_SHEET_STORAGE_KEY)).toBeNull();
    s.save();
    setActivePinia(createPinia());
    const next = useCompareSheetStore();
    expect(next.height).toBe(0.8);
    expect(next.autoSize).toBe(72);
  });

  it('toggles full height and goes back to the last partial height', () => {
    const s = useCompareSheetStore();
    s.setHeight(0.5);
    s.toggleFull();
    expect(s.full).toBe(true);
    expect(s.height).toBe(1);
    // Full height is remembered too.
    setActivePinia(createPinia());
    const next = useCompareSheetStore();
    expect(next.full).toBe(true);
    next.toggleFull();
    expect(next.height).toBe(0.5);
  });

  it('falls back to the default height for a damaged entry', () => {
    localStorage.setItem(COMPARE_SHEET_STORAGE_KEY, '{not json');
    expect(useCompareSheetStore().height).toBe(SHEET_HEIGHT.initial);
    setActivePinia(createPinia());
    localStorage.setItem(COMPARE_SHEET_STORAGE_KEY, JSON.stringify({ height: 1, partial: 1 }));
    const s = useCompareSheetStore();
    s.toggleFull();
    expect(s.height).toBe(SHEET_HEIGHT.initial);
  });
});
