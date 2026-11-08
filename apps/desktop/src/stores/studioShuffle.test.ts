import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient, mockFaces } from '../api/mock';
import type { FaceId, FaceSummary, LayoutRole, PairingSuggestion } from '../api/types';
import {
  FIRST_PANEL,
  SHUFFLE_POOL,
  TRY_COUNT,
  UNDO_DEPTH,
  blockScope,
  nextCandidate,
  pairingRoleOf,
  partnerOf,
  useStudioStore,
} from './studio';

let client: ReturnType<typeof createMockClient>;

beforeEach(() => {
  setActivePinia(createPinia());
  client = createMockClient();
  setClient(client);
});

afterEach(() => {
  vi.restoreAllMocks();
  setClient(undefined);
});

const inter = mockFaces[0]!.face;
const mono = mockFaces[1]!.face;

const MAGAZINE: LayoutRole[] = [
  { role: 'headline', blocks: [1] },
  { role: 'deck', blocks: [2] },
  { role: 'body', blocks: [3] },
  { role: 'caption', blocks: [0, 4] },
];
const magazine = { roles: MAGAZINE };
const SCOPE = 'studio-magazine';

/** A face of its own family, so the picks can be told apart. */
const faceNamed = (id: number, familyName: string): FaceSummary => ({
  ...inter,
  id: id as FaceId,
  family: id as FaceSummary['family'],
  familyName,
});
const alpha = faceNamed(901, 'Alpha');
const beta = faceNamed(902, 'Beta');
const gamma = faceNamed(903, 'Gamma');
const suggestion = (face: FaceSummary, ...reasons: string[]): PairingSuggestion => ({
  face,
  score: 0.5,
  reasons,
});

describe('Shuffle and Try helpers (E45.3)', () => {
  it('maps each block role to the pairing role of its type size', () => {
    const roles = [
      'headline',
      'title',
      'deck',
      'subtitle',
      'body',
      'caption',
      'eyebrow',
      'action',
      'footnote',
      'details',
    ] as const;
    expect(roles.map(pairingRoleOf)).toEqual([
      'headline',
      'headline',
      'headline',
      'headline',
      'body',
      'caption',
      'caption',
      'caption',
      'caption',
      'caption',
    ]);
  });

  it('pairs with the first locked role other than the one asked, else the panel face', () => {
    const faces: Record<string, FaceSummary> = { headline: mono, body: alpha };
    const faceOf = (r: LayoutRole) => faces[r.role] ?? inter;
    const locked = (set: string[]) => (r: string) => set.includes(r);
    expect(partnerOf(MAGAZINE, 'deck', faceOf, locked(['body', 'headline']), inter)).toBe(mono);
    // The role itself does not count, even when locked.
    expect(partnerOf(MAGAZINE, 'headline', faceOf, locked(['headline', 'body']), inter)).toBe(
      alpha,
    );
    expect(partnerOf(MAGAZINE, 'deck', faceOf, locked([]), gamma)).toBe(gamma);
    expect(partnerOf(MAGAZINE, 'deck', faceOf, locked([]), null)).toBeNull();
  });

  it('the next candidate skips the current family, what was shown and what is taken', () => {
    const list = [suggestion(alpha), suggestion(beta), suggestion(gamma)];
    const none = new Set<number>();
    expect(nextCandidate(list, alpha.family, none, none)?.suggestion.face).toBe(beta);
    expect(nextCandidate(list, null, new Set([alpha.family]), none)?.suggestion.face).toBe(beta);
    expect(nextCandidate(list, null, none, new Set([alpha.family, beta.family]))).toEqual({
      suggestion: list[2],
      restart: false,
    });
    // Everything shown: the round starts over with the first free one.
    const all = new Set([alpha.family, beta.family, gamma.family]);
    expect(nextCandidate(list, alpha.family, all, none)).toEqual({
      suggestion: list[1],
      restart: true,
    });
    expect(nextCandidate([suggestion(alpha)], alpha.family, none, none)).toBeNull();
    expect(nextCandidate([], null, none, none)).toBeNull();
  });
});

describe('Shuffle unlocked (E45.3)', () => {
  it('asks suggestPairings per unlocked role with the locked role as partnerOf and its role', async () => {
    const studio = useStudioStore();
    studio.applySuggestion(SCOPE, MAGAZINE[0]!, suggestion(mono));
    studio.toggleLock(SCOPE, 'headline');
    const spy = vi.spyOn(client, 'suggestPairings').mockResolvedValue([suggestion(alpha)]);
    const result = await studio.shuffle(SCOPE, magazine, inter);
    expect(spy.mock.calls.map(([req]) => req)).toEqual([
      { face: inter.id, limit: SHUFFLE_POOL, role: 'headline', partnerOf: mono.id },
      { face: inter.id, limit: SHUFFLE_POOL, role: 'body', partnerOf: mono.id },
      { face: inter.id, limit: SHUFFLE_POOL, role: 'caption', partnerOf: mono.id },
    ]);
    // One family per shuffle: the first role takes Alpha, the others have nothing else.
    expect(result).toEqual({ changed: 1 });
    expect(studio.faceOf(SCOPE, 1)?.id).toBe(mono.id);
    expect(studio.faceOf(SCOPE, 2)?.id).toBe(alpha.id);
    expect(studio.faceOf(SCOPE, 3)).toBeNull();
  });

  it('gives each unlocked role its own family, crossfades them and leaves the locked one', async () => {
    const studio = useStudioStore();
    studio.toggleLock(SCOPE, 'body');
    vi.spyOn(client, 'suggestPairings').mockResolvedValue([
      suggestion(alpha),
      suggestion(beta),
      suggestion(gamma),
    ]);
    expect(await studio.shuffle(SCOPE, magazine, inter)).toEqual({ changed: 3 });
    expect(studio.faceOf(SCOPE, 1)?.id).toBe(alpha.id);
    expect(studio.faceOf(SCOPE, 2)?.id).toBe(beta.id);
    expect(studio.faceOf(SCOPE, 0)?.id).toBe(gamma.id);
    expect(studio.faceOf(SCOPE, 4)?.id).toBe(gamma.id);
    expect(studio.faceOf(SCOPE, 3)).toBeNull();
    expect([0, 1, 2, 3, 4].map((i) => studio.fadeOf(SCOPE, i))).toEqual([1, 1, 1, 0, 1]);
  });

  it('moves on press by press and does not go back to the face it started from', async () => {
    const studio = useStudioStore();
    for (const r of ['deck', 'body', 'caption'] as const) studio.toggleLock(SCOPE, r);
    // The core leaves out the current family; the list is the same otherwise.
    vi.spyOn(client, 'suggestPairings').mockImplementation(async (req) =>
      [inter, alpha, beta].filter((f) => f.id !== req.face).map((f) => suggestion(f)),
    );
    const picks: (number | undefined)[] = [];
    for (let i = 0; i < 4; i++) {
      await studio.shuffle(SCOPE, magazine, inter);
      picks.push(studio.faceOf(SCOPE, 1)?.id);
    }
    // Alpha, then Beta; then every one was shown, so the round starts over.
    expect(picks).toEqual([alpha.id, beta.id, inter.id, alpha.id]);
  });

  it('shuffles nothing when every role is locked, and asks nothing', async () => {
    const studio = useStudioStore();
    for (const r of MAGAZINE) studio.toggleLock(SCOPE, r.role);
    const spy = vi.spyOn(client, 'suggestPairings');
    expect(await studio.shuffle(SCOPE, magazine, inter)).toEqual({ none: 'allLocked' });
    expect(spy).not.toHaveBeenCalled();
    expect(studio.canUndo(SCOPE)).toBe(false);
  });

  it('asks nothing without a face to pair with', async () => {
    const studio = useStudioStore();
    const spy = vi.spyOn(client, 'suggestPairings');
    expect(await studio.shuffle(SCOPE, magazine, null)).toEqual({ none: 'noFace' });
    expect(spy).not.toHaveBeenCalled();
  });

  it('says so when the library has nothing that pairs, and changes nothing', async () => {
    const studio = useStudioStore();
    vi.spyOn(client, 'suggestPairings').mockResolvedValue([]);
    expect(await studio.shuffle(SCOPE, magazine, inter)).toEqual({ none: 'noCandidates' });
    expect(studio.blockFaces).toEqual({});
    expect(studio.canUndo(SCOPE)).toBe(false);
  });

  it('keeps the error of a failed shuffle and changes nothing', async () => {
    const studio = useStudioStore();
    vi.spyOn(client, 'suggestPairings').mockRejectedValue({ code: 'io', message: 'gone' });
    expect(await studio.shuffle(SCOPE, magazine, inter)).toEqual({ none: 'failed' });
    expect(studio.error).toEqual({ code: 'io', message: 'gone' });
    expect(studio.blockFaces).toEqual({});
  });

  it('drops the answers of an older shuffle that come back last', async () => {
    const studio = useStudioStore();
    let release!: () => void;
    const held = new Promise<void>((r) => (release = r));
    vi.spyOn(client, 'suggestPairings')
      .mockImplementationOnce(async () => {
        await held;
        return [suggestion(gamma)];
      })
      .mockResolvedValue([suggestion(alpha), suggestion(beta), suggestion(gamma)]);
    const older = studio.shuffle(SCOPE, { roles: [MAGAZINE[0]!] }, inter);
    await studio.shuffle(SCOPE, { roles: [MAGAZINE[0]!] }, inter);
    release();
    expect(await older).toEqual({ none: 'stale' });
    expect(studio.faceOf(SCOPE, 1)?.id).toBe(alpha.id);
  });

  it('a role locked while the answers were on their way is left as it is', async () => {
    const studio = useStudioStore();
    vi.spyOn(client, 'suggestPairings').mockResolvedValue([suggestion(alpha)]);
    const running = studio.shuffle(SCOPE, { roles: [MAGAZINE[0]!] }, inter);
    studio.toggleLock(SCOPE, 'headline');
    expect(await running).toEqual({ none: 'noCandidates' });
    expect(studio.faceOf(SCOPE, 1)).toBeNull();
  });

  it('suggests faces of the library only: every pick is a face the client lists', async () => {
    // Twenty sample families after the mock's own; the odd ones pair with Inter.
    client = createMockClient({ extraFaces: 20 });
    setClient(client);
    const studio = useStudioStore();
    const result = await studio.shuffle(SCOPE, magazine, inter);
    expect(result).toEqual({ changed: 4 });
    const picked = Object.values(studio.blockFaces);
    expect(picked.length).toBe(5);
    // Each is a face of the library the client indexes, as the library has it.
    for (const f of picked) expect((await client.getFace(f.id)).face).toEqual(f);
  });
});

describe('Undo and Try (E45.3)', () => {
  it('Undo takes back the last shuffle, then the one before, with a crossfade', async () => {
    const studio = useStudioStore();
    studio.toggleLock(SCOPE, 'body');
    vi.spyOn(client, 'suggestPairings').mockImplementation(async (req) =>
      [alpha, beta].filter((f) => f.id !== req.face).map((f) => suggestion(f)),
    );
    const two = { roles: [MAGAZINE[0]!, MAGAZINE[2]!] };
    await studio.shuffle(SCOPE, two, inter);
    await studio.shuffle(SCOPE, two, inter);
    expect(studio.faceOf(SCOPE, 1)?.id).toBe(beta.id);
    expect(studio.undo(SCOPE)).toBe(true);
    expect(studio.faceOf(SCOPE, 1)?.id).toBe(alpha.id);
    expect(studio.fadeOf(SCOPE, 1)).toBe(3);
    expect(studio.undo(SCOPE)).toBe(true);
    // Back on the panel face: the block has no face of its own again.
    expect(studio.faceOf(SCOPE, 1)).toBeNull();
    expect(studio.canUndo(SCOPE)).toBe(false);
    expect(studio.undo(SCOPE)).toBe(false);
    expect(studio.undo(null)).toBe(false);
  });

  it('the Try list asks for three candidates of the role with the locked partner', async () => {
    const studio = useStudioStore();
    studio.applySuggestion(SCOPE, MAGAZINE[2]!, suggestion(mono));
    studio.toggleLock(SCOPE, 'body');
    const got = [suggestion(alpha, 'higher contrast for headline'), suggestion(beta)];
    const spy = vi.spyOn(client, 'suggestPairings').mockResolvedValue(got);
    await studio.loadTries(SCOPE, magazine, MAGAZINE[0]!, inter);
    expect(spy).toHaveBeenCalledWith({
      face: inter.id,
      limit: TRY_COUNT,
      role: 'headline',
      partnerOf: mono.id,
    });
    expect(studio.tries).toEqual({
      key: `${SCOPE}@headline`,
      role: 'headline',
      suggestions: got,
      loading: false,
      error: null,
    });
  });

  it('the Try list keeps an error, and drops an older answer', async () => {
    const studio = useStudioStore();
    let release!: () => void;
    const held = new Promise<void>((r) => (release = r));
    vi.spyOn(client, 'suggestPairings')
      .mockImplementationOnce(async () => {
        await held;
        return [suggestion(gamma)];
      })
      .mockRejectedValueOnce({ code: 'io', message: 'gone' });
    const older = studio.loadTries(SCOPE, magazine, MAGAZINE[0]!, inter);
    await studio.loadTries(SCOPE, magazine, MAGAZINE[2]!, inter);
    release();
    await older;
    expect(studio.tries?.role).toBe('body');
    expect(studio.tries?.suggestions).toEqual([]);
    expect(studio.tries?.error).toEqual({ code: 'io', message: 'gone' });
  });

  it('a Try pick sets that role alone, and Undo takes it back', () => {
    const studio = useStudioStore();
    studio.applySuggestion(SCOPE, MAGAZINE[3]!, suggestion(alpha));
    expect(studio.faceOf(SCOPE, 0)?.id).toBe(alpha.id);
    expect(studio.faceOf(SCOPE, 4)?.id).toBe(alpha.id);
    for (const i of [1, 2, 3]) expect(studio.faceOf(SCOPE, i)).toBeNull();
    expect(studio.fadeOf(SCOPE, 0)).toBe(1);
    expect(studio.undo(SCOPE)).toBe(true);
    expect(studio.blockFaces).toEqual({});
  });

  it('keeps the last UNDO_DEPTH steps; the oldest ones drop off', () => {
    const studio = useStudioStore();
    const faces = [alpha, beta, gamma];
    for (let i = 0; i < UNDO_DEPTH + 2; i++)
      studio.applySuggestion(SCOPE, MAGAZINE[0]!, suggestion(faces[i % 3]!));
    let undone = 0;
    while (studio.undo(SCOPE)) undone++;
    expect(undone).toBe(UNDO_DEPTH);
    // The two oldest steps are gone: block 1 keeps the face of the second pick.
    expect(studio.faceOf(SCOPE, 1)?.id).toBe(beta.id);
  });

  it('a closed panel forgets its fades, undo steps and Try list', async () => {
    const studio = useStudioStore();
    const second = studio.addPanel(FIRST_PANEL)!;
    const scope = blockScope(second, SCOPE);
    studio.applySuggestion(scope, MAGAZINE[0]!, suggestion(alpha));
    vi.spyOn(client, 'suggestPairings').mockResolvedValue([suggestion(beta)]);
    await studio.loadTries(scope, magazine, MAGAZINE[0]!, inter);
    studio.closePanel(second);
    expect(studio.fades).toEqual({});
    expect(studio.canUndo(scope)).toBe(false);
    expect(studio.tries).toBeNull();
  });
});
