import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { CoverageRequest, FaceCoverage, FaceId } from '../api/types';
import { useCoverageStore } from './coverage';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

let coverageCalls: CoverageRequest[];
let mock: FmClient;

beforeEach(() => {
  setActivePinia(createPinia());
  coverageCalls = [];
  mock = createMockClient();
  setClient({
    ...mock,
    checkCoverage: (req) => {
      coverageCalls.push(req);
      return mock.checkCoverage(req);
    },
  });
});
afterEach(() => setClient(undefined));

// Inter (1) has Latin, Latin Extended-A and Cyrillic; Legacy Grotesk (7) Latin only.
describe('coverage store', () => {
  it('shows the coverage list fm-core computes for the face', async () => {
    const s = useCoverageStore();
    await s.open(1);
    expect(s.coverage?.face).toBe(1);
    expect(s.coverage?.blocks.map((b) => b.name)).toEqual([
      'Basic Latin',
      'Latin-1 Supplement',
      'Latin Extended-A',
      'Cyrillic',
    ]);
    expect(s.coverage?.languages.every((l) => l.covered === l.total)).toBe(true);
    expect(s.loading).toBe(false);
  });

  it('lists the missing characters of a partial language from check_coverage, once', async () => {
    const s = useCoverageStore();
    await s.open(7);
    const pl = s.coverage?.languages.find((l) => l.code === 'pl');
    expect(pl && pl.covered < pl.total).toBe(true);
    await s.loadMissing('pl');
    await s.loadMissing('pl');
    expect(coverageCalls).toEqual([{ face: 7, target: { kind: 'languages', codes: ['pl'] } }]);
    expect(s.missing.get('pl')).toEqual(['ą', 'ć', 'ę', 'ł', 'ń', 'ś', 'ź', 'ż']);
  });

  it('drops an answer about a face no longer shown', async () => {
    let release: (c: FaceCoverage) => void = () => undefined;
    setClient({
      ...mock,
      faceCoverage: (face: FaceId) =>
        face === 7 ? new Promise<FaceCoverage>((r) => (release = r)) : mock.faceCoverage(face),
    });
    const s = useCoverageStore();
    const slow = s.open(7);
    await s.open(1);
    release({ face: 7, blocks: [], languages: [] });
    await slow;
    expect(s.coverage?.face).toBe(1);
  });

  it('reports an error of fm-core', async () => {
    const s = useCoverageStore();
    await s.open(9999);
    expect(s.coverage).toBeNull();
    expect(s.error?.code).toBe('notFound');
  });
});
