import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, mockLanguages } from '../api/mock';
import type { FaceLanguageCoverage } from '../api/types';
import { completeLanguages, RAIL_PAIRINGS, useFamilyRailStore, withLanguage } from './familyRail';

let client: FmClient;

const lang = (code: string, covered: number, total = 10): FaceLanguageCoverage => ({
  code,
  name: code,
  covered,
  total,
  hasLocl: false,
});

function setup(languages?: FaceLanguageCoverage[]) {
  client = createMockClient({ extraFaces: 20 });
  setClient(client);
  if (languages)
    vi.spyOn(client, 'faceCoverage').mockImplementation(async (face) => ({
      face,
      blocks: [],
      languages,
    }));
  return useFamilyRailStore();
}

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => setClient(undefined));

describe('family rail', () => {
  it('keeps only the languages the face covers in full, six chips before "+N more"', async () => {
    const codes = ['en', 'de', 'fr', 'pl', 'cs', 'hr', 'sk', 'sl'];
    const rail = setup([...codes.map((c) => lang(c, 10)), lang('el', 9), lang('ru', 0)]);
    await rail.load(1);
    expect(rail.complete.map((l) => l.code)).toEqual(codes);
    expect(rail.shown.map((l) => l.code)).toEqual(codes.slice(0, 6));
    expect(rail.more).toBe(2);
    rail.expanded = true;
    expect(rail.shown).toHaveLength(8);
    expect(rail.more).toBe(0);
  });

  it('a preset with no characters is not complete', () => {
    expect(completeLanguages([lang('xx', 0, 0), lang('en', 3, 3)]).map((l) => l.code)).toEqual([
      'en',
    ]);
  });

  it('asks for the top two pairings', async () => {
    const rail = setup();
    const spy = vi.spyOn(client, 'suggestPairings');
    await rail.load(1);
    expect(spy).toHaveBeenCalledWith({ face: 1, limit: RAIL_PAIRINGS });
    expect(RAIL_PAIRINGS).toBe(2);
    expect(rail.pairings).toHaveLength(2);
  });

  it("a chip previews the language's pangram; again, or back to my text, restores the user's", async () => {
    const rail = setup([lang('pl', 10), lang('de', 10)]);
    await rail.load(1);
    expect(rail.textOf(mockLanguages)).toBeNull();
    rail.choose('pl');
    expect(rail.textOf(mockLanguages)).toBe(mockLanguages.find((p) => p.code === 'pl')!.pangram);
    rail.choose('pl');
    expect(rail.language).toBeNull();
    expect(rail.textOf(mockLanguages)).toBeNull();
    rail.choose('de');
    rail.choose(null);
    expect(rail.textOf(mockLanguages)).toBeNull();
  });

  it('a language the next face does not cover in full goes back to the user text', async () => {
    const rail = setup();
    let covers = true;
    vi.spyOn(client, 'faceCoverage').mockImplementation(async (face) => ({
      face,
      blocks: [],
      languages: [lang('pl', covers ? 10 : 9)],
    }));
    await rail.load(1);
    rail.choose('pl');
    rail.filterWall = true;
    await rail.load(2);
    expect(rail.language).toBe('pl');
    covers = false;
    await rail.load(3);
    expect(rail.language).toBeNull();
    expect(rail.filterWall).toBe(false);
  });

  it('the wall filter is taken once, and only with a language chosen', async () => {
    const rail = setup([lang('pl', 10)]);
    await rail.load(1);
    rail.filterWall = true;
    expect(rail.takeWallLanguage()).toBeNull();
    rail.choose('pl');
    rail.filterWall = true;
    expect(rail.takeWallLanguage()).toBe('pl');
    expect(rail.takeWallLanguage()).toBeNull();
  });

  it('withLanguage adds the language to the supports filter once, keeping the others', () => {
    expect(withLanguage({ variable: true }, 'pl')).toEqual({
      variable: true,
      supports: { kind: 'languages', codes: ['pl'] },
    });
    const both = withLanguage({ supports: { kind: 'languages', codes: ['de'] } }, 'pl');
    expect(both.supports).toEqual({ kind: 'languages', codes: ['de', 'pl'] });
    expect(withLanguage(both, 'pl')).toBe(both);
  });

  it('a coverage error shows no chips but keeps the pairings', async () => {
    const rail = setup();
    vi.spyOn(client, 'faceCoverage').mockRejectedValue({
      code: 'internal',
      message: 'x',
      detail: null,
    });
    await rail.load(1);
    expect(rail.complete).toEqual([]);
    expect(rail.error?.code).toBe('internal');
    expect(rail.pairings).toHaveLength(2);
  });
});
