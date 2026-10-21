import { describe, expect, it } from 'vitest';
import {
  ariaSort,
  COLUMNS,
  cycleSort,
  dateText,
  defaultTable,
  formatBadges,
  licenseBadges,
  openLicenseName,
  moveColumn,
  normalizeTable,
  orderOf,
  toggleColumn,
  visibleColumns,
  weightPosition,
  weightRangeText,
} from './columns';

const ids = (cols: { id: string }[]) => cols.map((c) => c.id);

describe('detail list columns', () => {
  it("has the spec's columns in the spec's order", () => {
    expect(COLUMNS.map((c) => c.label)).toEqual([
      'Activation',
      'Name',
      'Styles',
      'Style',
      'Weight',
      'Format',
      'Version',
      'Manufacturer',
      'Location',
      'License',
      'Date added',
      'Tags',
      'Notes',
    ]);
    expect(ids(visibleColumns(defaultTable()))).toEqual(COLUMNS.map((c) => c.id));
  });

  it('sorts ascending, then descending, then back to the default order', () => {
    let t = cycleSort(defaultTable(), 'version');
    expect(orderOf(t)).toEqual({ by: 'version', descending: false });
    expect(ariaSort(t, 'version')).toBe('ascending');
    expect(ariaSort(t, 'name')).toBe('none');
    t = cycleSort(t, 'version');
    expect(orderOf(t)).toEqual({ by: 'version', descending: true });
    expect(ariaSort(t, 'version')).toBe('descending');
    t = cycleSort(t, 'version');
    expect(orderOf(t)).toBeNull();
    // Another column starts ascending again.
    t = cycleSort(cycleSort(t, 'tags'), 'added');
    expect(orderOf(t)).toEqual({ by: 'dateAdded', descending: false });
  });

  it('leaves the order alone for the Notes column, which does not sort', () => {
    const sorted = cycleSort(defaultTable(), 'version');
    expect(cycleSort(sorted, 'notes')).toEqual(sorted);
    expect(cycleSort(defaultTable(), 'notes').sort).toBeNull();
    // A saved sort by notes (hand-edited) is no sort.
    expect(normalizeTable({ sort: { column: 'notes', descending: true } }).sort).toBeNull();
    expect(orderOf({ ...defaultTable(), sort: { column: 'notes', descending: false } })).toBeNull();
  });

  it('marks only the clicked column as sorted when two share a sort key', () => {
    const t = cycleSort(defaultTable(), 'weightLine');
    expect(orderOf(t)?.by).toBe('weight');
    expect(ariaSort(t, 'weightLine')).toBe('ascending');
    expect(ariaSort(t, 'weightRange')).toBe('none');
  });

  it('moves a column to any position, clamped', () => {
    let t = moveColumn(defaultTable(), 'tags', 0);
    expect(t.order.slice(0, 2)).toEqual(['tags', 'activation']);
    t = moveColumn(t, 'tags', 99);
    expect(t.order.at(-1)).toBe('tags');
    expect(t.order).toHaveLength(COLUMNS.length);
  });

  it('hides and shows columns, but never the name', () => {
    let t = toggleColumn(defaultTable(), 'location');
    expect(ids(visibleColumns(t))).not.toContain('location');
    t = toggleColumn(t, 'name');
    expect(ids(visibleColumns(t))).toContain('name');
    t = toggleColumn(t, 'location');
    expect(ids(visibleColumns(t))).toContain('location');
  });

  it('repairs a damaged saved configuration', () => {
    const t = normalizeTable({
      order: ['tags', 'bogus', 'tags', 'name'],
      hidden: ['name', 'version', 'nope', 'version'],
      sort: { column: 'nope', descending: true },
      stripSizePx: 9999,
    });
    // The saved order stays (tags before name); missing columns come back at their default place.
    expect(t.order.indexOf('tags')).toBeLessThan(t.order.indexOf('name'));
    expect(t.order[0]).toBe('activation');
    expect([...t.order].sort()).toEqual(COLUMNS.map((c) => c.id).sort());
    expect(t.hidden).toEqual(['version']);
    expect(t.sort).toBeNull();
    expect(t.stripSizePx).toBe(300);
    expect(normalizeTable('garbage')).toEqual(defaultTable());
    expect(normalizeTable({ sort: { column: 'version' } }).sort).toEqual({
      column: 'version',
      descending: false,
    });
  });

  it('reads the API values as cells', () => {
    expect(formatBadges(['ttf', 'ttc', 'woff2'], true, true)).toEqual([
      'TT',
      'WF',
      'Variable',
      'Color',
    ]);
    expect(formatBadges(['otf'], false)).toEqual(['OT']);
    expect(licenseBadges({ declaredLicense: 'web', openLicense: 'ofl' })).toEqual([
      'Web',
      'Open license',
    ]);
    expect(licenseBadges({ declaredLicense: null, openLicense: 'apache' })).toEqual([
      'Open license',
    ]);
    expect(licenseBadges({ declaredLicense: 'trial', openLicense: null })).toEqual(['Trial']);
    expect(licenseBadges({})).toEqual([]);
    expect(openLicenseName('ufl')).toBe('Ubuntu Font Licence');
    expect(openLicenseName(null)).toBe('');
    expect(weightRangeText({ min: 100, max: 900 })).toBe('100–900');
    expect(weightRangeText({ min: 400, max: 400 })).toBe('400');
    expect(weightRangeText(null)).toBe('');
    expect([weightPosition(100), weightPosition(550), weightPosition(1000)]).toEqual([0, 50, 100]);
    expect(weightPosition(1)).toBe(0);
    expect(dateText('2026-10-01T23:30:00Z', 'en-US')).toBe('Oct 1, 2026');
    expect(dateText('not a date', 'en-US')).toBe('');
  });
});
