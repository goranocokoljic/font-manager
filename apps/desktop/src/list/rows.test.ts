import { describe, expect, it } from 'vitest';
import { mockFaces } from '../api/mock';
import type { FaceSummary, FamilySummary } from '../api/types';
import { faceKey, familyKey, flattenRows } from './rows';

const family = (id: number, name: string): FamilySummary => ({
  id,
  name,
  styleCount: 2,
  formats: ['ttf'],
  foundry: null,
  classification: null,
  isVariable: false,
  activation: 'inactive',
  added: '2026-10-01T00:00:00Z',
});
const face = (id: number, fam: number, style: string): FaceSummary => ({
  ...mockFaces[1]!.face,
  id,
  family: fam,
  style,
});

const families = [family(1, 'Alpha'), family(2, 'Beta'), family(3, 'Gamma')];
const faces = new Map([
  [2, [face(20, 2, 'Regular'), face(21, 2, 'Bold')]],
  [3, [face(30, 3, 'Regular')]],
]);

describe('flattenRows', () => {
  it('lists one row per family when nothing is expanded', () => {
    const asked: number[] = [];
    const rows = flattenRows(families, new Set(), (id) => (asked.push(id), faces.get(id)));
    expect(rows.map((r) => r.key)).toEqual([familyKey(1), familyKey(2), familyKey(3)]);
    expect(rows.map((r) => r.position)).toEqual([1, 2, 3]);
    expect(asked).toEqual([]); // only expanded families are asked for their faces
  });

  it('puts the styles of an expanded family right under it, in order', () => {
    const rows = flattenRows(families, new Set([2]), (id) => faces.get(id));
    expect(rows.map((r) => r.key)).toEqual([
      familyKey(1),
      familyKey(2),
      faceKey(20),
      faceKey(21),
      familyKey(3),
    ]);
    const bold = rows[3]!;
    expect(bold.kind === 'face' && [bold.face.style, bold.position, bold.count]).toEqual([
      'Bold',
      2,
      2,
    ]);
    expect(rows[1]!.kind === 'family' && rows[1]!.expanded).toBe(true);
  });

  it('shows an expanded family alone until its styles have loaded', () => {
    const rows = flattenRows(families, new Set([1]), () => undefined);
    expect(rows).toHaveLength(3);
  });
});
