import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { CoverageRequest } from '../api/types';
import { COMPARE_SIZE } from '../glyphs/compare';
import { useGlyphCompareStore } from './glyphCompare';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

let coverageCalls: CoverageRequest[];
let client: FmClient;

beforeEach(() => {
  setActivePinia(createPinia());
  coverageCalls = [];
  const mock = createMockClient();
  client = {
    ...mock,
    checkCoverage: (req) => {
      coverageCalls.push(req);
      return mock.checkCoverage(req);
    },
  };
  setClient(client);
});
afterEach(() => setClient(undefined));

const ZHE = 0x416; // Ж: Inter and JetBrains Mono have it, Legacy Grotesk does not.

async function pick(name: string) {
  const s = useGlyphCompareStore();
  await s.search(name);
  const hit = s.results.find((h) => h.family.name === name);
  expect(hit).toBeDefined();
  await s.pickFamily(hit!);
  return s;
}

describe('glyph compare store', () => {
  it('opens on a character of the inspected face and lists the library', async () => {
    const s = useGlyphCompareStore();
    await s.start(1, ZHE);
    expect(s.open).toBe(true);
    expect(s.base?.face.familyName).toBe('Inter');
    expect(s.results.length).toBeGreaterThan(1);
    expect(s.other).toBeNull();
  });

  it('picks a family by name: its best face, its styles and its coverage from fm-core', async () => {
    await useGlyphCompareStore().start(1, ZHE);
    const s = await pick('JetBrains Mono');
    expect(s.other?.face.familyName).toBe('JetBrains Mono');
    expect(s.other?.metrics.xHeight).toBe(550);
    expect(s.styles.map((f) => f.style)).toContain('Regular');
    expect(coverageCalls).toEqual([{ face: 6, target: { kind: 'text', text: 'Ж' } }]);
    expect(s.coverage?.complete).toBe(true);
  });

  it('reports a character the second face lacks, as check_coverage answers', async () => {
    await useGlyphCompareStore().start(1, ZHE);
    const s = await pick('Legacy Grotesk');
    expect(s.coverage).toMatchObject({ face: 7, complete: false, missing: ['Ж'] });
  });

  it('swaps the second face and keeps it for the next character', async () => {
    const s = useGlyphCompareStore();
    await s.start(1, ZHE);
    await pick('Legacy Grotesk');
    await s.pickFace(6);
    expect(s.other?.face.id).toBe(6);
    expect(s.coverage?.complete).toBe(true);
    s.close();
    await s.start(1, 0x41);
    expect(s.open).toBe(true);
    expect(s.other?.face.id).toBe(6);
    expect(coverageCalls.at(-1)).toEqual({ face: 6, target: { kind: 'text', text: 'A' } });
  });

  it('drops an older pick that answers late', async () => {
    const s = useGlyphCompareStore();
    await s.start(1, ZHE);
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    const mock = client;
    setClient({
      ...mock,
      getFace: async (id) => {
        if (id === 7) await gate;
        return mock.getFace(id);
      },
    });
    const late = s.pickFace(7);
    await s.pickFace(6);
    release();
    await late;
    expect(s.other?.face.id).toBe(6);
  });

  it('keeps the size in range', () => {
    const s = useGlyphCompareStore();
    expect(s.size).toBe(COMPARE_SIZE.initial);
    s.setSize(5000);
    expect(s.size).toBe(COMPARE_SIZE.max);
  });
});
