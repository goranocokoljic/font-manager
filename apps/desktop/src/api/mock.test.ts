import { describe, expect, it } from 'vitest';
import type { FmClient } from './client';
import { createMockClient } from './mock';
import type { CollectionId, FaceId, Filters, Rule } from './types';

/** The faces a search with `filters` finds, sorted. */
async function facesOf(client: FmClient, filters: Filters): Promise<FaceId[]> {
  const res = await client.search({ query: '', filters, page: { offset: 0, limit: 1000 } });
  return res.families.flatMap((h) => h.faces).sort((a, b) => a - b);
}

const leaf = (filters: Filters): Rule => ({ kind: 'filters', filters });

/** A collection's count in the list, and the faces a search by it finds. */
async function live(client: FmClient, id: CollectionId) {
  const c = (await client.listCollections()).find((x) => x.id === id)!;
  return { count: c.faceCount, faces: await facesOf(client, { collection: id }) };
}

describe('MockApi smart collections (E14.2)', () => {
  it('reads a phase-1 rule as one AND group and counts it live', async () => {
    const client = createMockClient();
    const c = await client.createCollection({ name: 'Variable', rules: { variable: true } });
    expect(c.rules).toEqual({ variable: true });
    expect(c.ruleGroup).toEqual({ op: 'and', rules: [leaf({ variable: true })] });
    const variable = await facesOf(client, { variable: true });
    expect(variable.length).toBeGreaterThan(0);
    expect(await live(client, c.id)).toEqual({ count: variable.length, faces: variable });
  });

  it('combines AND/OR groups, nests them and goes through other collections', async () => {
    const client = createMockClient();
    const italic = await facesOf(client, { slant: 'italic' });
    const variable = await facesOf(client, { variable: true });
    const either = await client.createCollection({
      name: 'Italic or variable',
      ruleGroup: { op: 'or', rules: [leaf({ slant: 'italic' }), leaf({ variable: true })] },
    });
    // A phase-1 client still sees it as smart.
    expect(either.rules).toEqual({});
    const union = [...new Set([...italic, ...variable])].sort((a, b) => a - b);
    expect(await live(client, either.id)).toEqual({ count: union.length, faces: union });

    const nested = await client.createCollection({
      name: 'Nested',
      ruleGroup: {
        op: 'and',
        rules: [
          leaf({ collection: either.id }),
          { kind: 'group', group: { op: 'or', rules: [leaf({ variable: false })] } },
        ],
      },
    });
    const staticItalic = union.filter((f) => !variable.includes(f));
    expect(await live(client, nested.id)).toEqual({
      count: staticItalic.length,
      faces: staticItalic,
    });

    const all = await client.createCollection({ name: 'All', ruleGroup: { op: 'or', rules: [] } });
    expect((await live(client, all.id)).count).toBe((await facesOf(client, {})).length);
  });

  it('follows tag and rating changes with no refresh', async () => {
    const client = createMockClient();
    const [a, b] = await facesOf(client, {});
    const [tag] = await client.tagFaces({ faces: [a!], tags: ['later'] });
    const tagged = await client.createCollection({ name: 'Tagged', rules: { tags: [tag!.id] } });
    expect(await live(client, tagged.id)).toEqual({ count: 1, faces: [a] });
    await client.tagFaces({ faces: [b!], tags: ['later'] });
    expect(await live(client, tagged.id)).toEqual({ count: 2, faces: [a, b] });
    await client.untagFaces({ faces: [a!], tags: [tag!.id] });
    expect(await live(client, tagged.id)).toEqual({ count: 1, faces: [b] });
    const rated = await client.createCollection({ name: 'Rated', rules: { minRating: 3 } });
    expect((await live(client, rated.id)).count).toBe(0);
    await client.updateUserMeta({ faces: [a!], rating: 4 });
    expect(await live(client, rated.id)).toEqual({ count: 1, faces: [a] });
  });

  it('filters by date added', async () => {
    const client = createMockClient();
    const every = await facesOf(client, {});
    expect(await facesOf(client, { addedWithinDays: 30 })).toEqual(every);
    expect(await facesOf(client, { addedWithinDays: 7 })).toEqual([]);
  });

  it('filters by last activation, active now or not (E14.3)', async () => {
    const client = createMockClient();
    expect(await facesOf(client, { activatedWithinDays: 30 })).toEqual([]);
    await client.activate({ target: { kind: 'faces', faces: [1, 6] }, onConflict: 'replace' });
    await client.deactivate({ target: { kind: 'faces', faces: [1] } });
    expect(await facesOf(client, { activatedWithinDays: 30 })).toEqual([1, 6]);
  });

  it('refuses a rule that refers to itself, an unknown collection, or both rule kinds', async () => {
    const client = createMockClient();
    const manual = await client.createCollection({ name: 'Manual' });
    const next = manual.id + 1;
    await expect(
      client.createCollection({
        name: 'Itself',
        ruleGroup: { op: 'and', rules: [leaf({ collection: next })] },
      }),
    ).rejects.toMatchObject({ code: 'invalidRequest' });
    expect(await client.listCollections()).toHaveLength(1);
    await expect(
      client.createCollection({
        name: 'Unknown',
        ruleGroup: { op: 'and', rules: [leaf({ collection: 999 })] },
      }),
    ).rejects.toMatchObject({ code: 'notFound' });
    await expect(
      client.createCollection({
        name: 'Both',
        rules: {},
        ruleGroup: { op: 'and', rules: [] },
      }),
    ).rejects.toMatchObject({ code: 'invalidRequest' });
  });
});

describe('MockApi layouts (E12.1)', () => {
  it('lists the six presets in picker order, each already inside the ranges', async () => {
    const api = createMockClient();
    const presets = await api.layoutPresets();
    expect(presets.map((l) => l.preset)).toEqual([
      'book',
      'columns',
      'magazine',
      'quote',
      'characters',
      'waterfall',
    ]);
    for (const l of presets) expect(await api.clampLayout(l)).toEqual(l);
  });

  it('clamps out-of-range values and gives the default for a value that is not a number', async () => {
    const api = createMockClient();
    const [book] = await api.layoutPresets();
    const wild = {
      ...book!,
      blocks: [
        { ...book!.blocks[0]!, sizePt: 2, lineHeight: Number.NaN, columns: 7, tracking: 5000 },
      ],
    };
    const [b] = (await api.clampLayout(wild)).blocks;
    expect(b).toMatchObject({ sizePt: 8, lineHeight: 1.3, columns: 3, tracking: 1000 });
  });

  it('hands out copies, so editing one does not change the presets', async () => {
    const api = createMockClient();
    (await api.layoutPresets())[0]!.blocks[0]!.sizePt = 99;
    expect((await api.layoutPresets())[0]!.blocks[0]!.sizePt).toBe(24);
  });
});
