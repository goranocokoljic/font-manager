// The TypeScript mock's cleanup (E56.7): the same outcomes as the Rust mock's plan.
import { describe, expect, it } from 'vitest';
import { duplicateSets, type SetFile } from './mockDuplicates';
import { mockPlanCleanup } from './mockCleanup';

const file = (path: string, hash: string, face: number, name: string, system = false): SetFile => ({
  path,
  size: 10,
  hash,
  format: 'ttf',
  source: 1,
  inLibraryFolder: false,
  system,
  faces: [{ face, name, version: '1.0' }],
});

describe('mockPlanCleanup', () => {
  it('plans the keeper rule with active and system copies staying', () => {
    const files = [
      file('/a/A.ttf', 'a', 1, 'A'),
      file('/z/A.ttf', 'a', 2, 'A'),
      file('/a/B.ttf', 'b', 3, 'B'),
      file('/z/B.ttf', 'b', 4, 'B'),
      file('/sys/C.ttf', 'c', 5, 'C', true),
      file('/z/C.ttf', 'c', 6, 'C'),
    ];
    const all = duplicateSets(files, { kind: 'library' });
    const planned = mockPlanCleanup(
      all,
      { scope: { kind: 'library' } },
      new Map([['/z/B.ttf', 'session']]),
      () => true,
    );
    if (typeof planned === 'string') throw new Error(planned);
    expect(planned.sets).toBe(3);
    const of = (p: string) => {
      const r = planned.files.find((f) => f.path === p)!;
      return [r.action, r.reason];
    };
    expect(of('/a/A.ttf')).toEqual(['stays', 'keeper']);
    expect(of('/z/A.ttf')).toEqual(['trash', null]);
    expect(of('/z/B.ttf')).toEqual(['stays', 'active']);
    expect(planned.files.find((f) => f.path === '/z/B.ttf')?.activation?.to).toBe('/a/B.ttf');
    expect(of('/sys/C.ttf')).toEqual(['stays', 'system']);
    expect(of('/z/C.ttf')).toEqual(['stays', 'keeper']);
  });

  it('refuses an unknown set', () => {
    expect(
      mockPlanCleanup([], { scope: { kind: 'sets', ids: ['name:X'] } }, new Map(), () => true),
    ).toBe('name:X is not a duplicate set any more.');
  });
});
