import { describe, expect, it } from 'vitest';
import { createMockClient, mockClassification, mockWall } from './mock';
import { WALL_FAMILIES, wallDuplicates, wallFixtures, wallProblems } from './mockWall';

// The fixture files themselves (licenses, real axes) are checked in e2e/wall-library.spec.ts,
// which runs in Node.

async function familiesOf(client: ReturnType<typeof createMockClient>) {
  const res = await client.search({ query: '', filters: {}, page: { offset: 0, limit: 1000 } });
  return res.families;
}

describe('the wall library (E40.2)', () => {
  it('serves 60 families in the asked mix of classes', async () => {
    const client = createMockClient({ library: 'wall' });
    const families = await familiesOf(client);
    expect(families).toHaveLength(60);
    const faceCount = families.reduce((n, h) => n + h.family.styleCount, 0);
    expect((await client.listSources())[0]!.faceCount).toBe(faceCount);
    const buckets = new Map<string, number>();
    for (const f of WALL_FAMILIES) buckets.set(f.bucket, (buckets.get(f.bucket) ?? 0) + 1);
    expect(Object.fromEntries(buckets)).toEqual({
      variable: 6,
      serif: 12,
      sans: 16,
      slab: 6,
      script: 6,
      mono: 6,
      display: 8,
    });
    // The classes the filters count: the variable six are sans or serif.
    const lib = mockWall();
    const classes = new Map<string, number>();
    for (const f of new Map(lib.faces.map((d) => [d.face.family, d])).values()) {
      const c = mockClassification(f);
      classes.set(c, (classes.get(c) ?? 0) + 1);
    }
    expect(classes.get('serif')).toBeGreaterThanOrEqual(12);
    expect(classes.get('sans')).toBeGreaterThanOrEqual(16);
    expect([classes.get('slab'), classes.get('script'), classes.get('mono')]).toEqual([6, 6, 6]);
    expect(classes.get('display')).toBe(8);
  });

  it('has six variable families, two with 3+ axes', async () => {
    const families = await familiesOf(createMockClient({ library: 'wall' }));
    expect(families.filter((h) => h.family.isVariable)).toHaveLength(6);
    const lib = mockWall();
    const variable = lib.faces.filter((d) => d.face.isVariable);
    expect(new Set(variable.filter((d) => d.axes.length >= 3).map((d) => d.face.family)).size).toBe(
      2,
    );
  });

  it('gives every face its own id and a fixture', () => {
    const lib = mockWall();
    const ids = new Set(lib.faces.map((d) => d.face.id));
    expect(ids.size).toBe(lib.faces.length);
    // The opt-in color and Hebrew faces keep their ids.
    expect(ids.has(8) || ids.has(9)).toBe(false);
    expect(ids.has(1)).toBe(true);
    expect([...wallFixtures().keys()].sort()).toEqual([...ids].sort());
  });

  it('carries designer, foundry, year, license and varied style counts', async () => {
    const lib = mockWall();
    for (const d of lib.faces) {
      expect(d.designer).toBeTruthy();
      expect(d.foundry).toBeTruthy();
      expect(d.copyright).toMatch(/^Copyright (19|20)\d\d /);
      expect(d.license.openLicense).toMatch(/^(ofl|apache|ufl)$/);
    }
    const families = await familiesOf(createMockClient({ library: 'wall' }));
    const counts = new Set(families.map((h) => h.family.styleCount));
    expect(counts.size).toBeGreaterThanOrEqual(5);
    expect(Math.max(...counts)).toBe(6);
  });

  it('covers Cyrillic, Greek and Vietnamese on some families and leaves gaps on others', async () => {
    const client = createMockClient({ library: 'wall' });
    const lib = mockWall();
    const firstOf = (name: string) => lib.faces.find((d) => d.face.familyName === name)!.face.id;
    const missing = async (name: string, text: string) =>
      (await client.sampleTexts({ faces: [firstOf(name)], text }))[0]!.missing;
    const greek = 'Ξεσκεπάζω την ψυχοφθόρα βδελυγμία';
    const serbian = 'Љубазни фењерџија чађавог лица хоће да ми покаже штос';
    const vietnamese = 'Tôi có thể ăn thủy tinh mà không hại gì';
    const german = 'Victor jagt zwölf Boxkämpfer quer über den großen Sylter Deich';
    expect(await missing('Ubuntu', greek)).toEqual([]);
    expect(await missing('Ubuntu', serbian)).toEqual([]);
    expect(await missing('Be Vietnam Pro', vietnamese)).toEqual([]);
    // The gaps: tofu shows where these characters would be.
    expect(await missing('Crimson Pro', greek)).not.toEqual([]);
    expect(await missing('Rufina', vietnamese)).toEqual(['ể', 'ủ', 'ạ']);
    expect(await missing('Zeyada', german)).toEqual(['ö', 'ä', 'ü', 'ß']);
    expect(await missing('Zeyada', 'Hamburgefonstiv')).toEqual([]);
    expect((await client.listLanguages()).map((l) => l.code)).toContain('vi');
  });

  it('has three duplicates and two broken files', () => {
    const lib = mockWall();
    const dupes = wallDuplicates(lib);
    expect(dupes).toHaveLength(3);
    for (const g of dupes) expect(lib.faces.some((d) => d.path === g.copies[0]!.path)).toBe(true);
    const problems = wallProblems(lib);
    expect(problems.filter((p) => p.kind === 'broken')).toHaveLength(2);
    expect(problems.filter((p) => p.kind.startsWith('duplicate'))).toHaveLength(3);
  });

  it('leaves the three-family library as the default and as `small`', async () => {
    for (const client of [createMockClient(), createMockClient({ library: 'small' })]) {
      const names = (await familiesOf(client)).map((h) => h.family.name);
      expect(names).toEqual(['Inter', 'JetBrains Mono', 'Legacy Grotesk']);
      expect((await client.listLanguages()).map((l) => l.code)).not.toContain('vi');
    }
  });
});
