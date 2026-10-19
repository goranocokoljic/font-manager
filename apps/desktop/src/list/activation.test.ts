import { describe, expect, it } from 'vitest';
import { mockFaces } from '../api/mock';
import type {
  ActivationMode,
  ActivationOutcome,
  FaceSummary,
  FamilyActivation,
  FamilySummary,
  OutcomeStatus,
} from '../api/types';
import { bulkMessage, planToggle, rowActivation, undoSteps } from './activation';
import { faceKey, familyKey, type FaceRow, type FamilyRow } from './rows';

const family = (id: number, activation: FamilyActivation = 'inactive'): FamilySummary => ({
  id,
  name: `Family ${id}`,
  styleCount: 2,
  formats: ['ttf'],
  foundry: null,
  classification: null,
  isVariable: false,
  activation,
  added: '2026-10-01T00:00:00Z',
});
const familyRow = (id: number, activation?: FamilyActivation): FamilyRow => ({
  kind: 'family',
  key: familyKey(id),
  family: family(id, activation),
  position: 1,
  expanded: false,
});
const faceRow = (id: number, fam: number, active: ActivationMode | null = null): FaceRow => {
  const face: FaceSummary = { ...mockFaces[0]!.face, id, family: fam, active };
  return { kind: 'face', key: faceKey(id), face, family: family(fam), position: 1, count: 2 };
};
const out = (face: number, outcome: OutcomeStatus): ActivationOutcome => ({ face, outcome });
const done = { status: 'done' } as const;

describe('rowActivation', () => {
  it('gives a style its own state and a family its summary', () => {
    expect(rowActivation(faceRow(1, 1))).toBe('inactive');
    expect(rowActivation(faceRow(1, 1, 'session'))).toBe('active');
    expect(rowActivation(faceRow(1, 1, 'permanent'))).toBe('active');
    expect(rowActivation(familyRow(1, 'partial'))).toBe('partial');
    expect(rowActivation(familyRow(1, 'active'))).toBe('active');
  });
});

describe('planToggle', () => {
  it('is null for no rows', () => {
    expect(planToggle([])).toBeNull();
  });

  it('sends styles in one request and each family in its own', () => {
    const plan = planToggle([faceRow(10, 1), familyRow(2), faceRow(11, 1), familyRow(3)]);
    expect(plan).toEqual({
      deactivate: false,
      targets: [
        { kind: 'faces', faces: [10, 11] },
        { kind: 'family', family: 2 },
        { kind: 'family', family: 3 },
      ],
    });
  });

  it('leaves out styles whose family row is selected too', () => {
    const plan = planToggle([familyRow(2), faceRow(20, 2), faceRow(30, 3)]);
    expect(plan?.targets).toEqual([
      { kind: 'faces', faces: [30] },
      { kind: 'family', family: 2 },
    ]);
  });

  it('deactivates only when every row is fully active', () => {
    expect(planToggle([faceRow(1, 1, 'session'), familyRow(2, 'active')])?.deactivate).toBe(true);
    expect(planToggle([faceRow(1, 1, 'session'), faceRow(2, 1)])?.deactivate).toBe(false);
    expect(planToggle([familyRow(2, 'partial')])?.deactivate).toBe(false);
  });

  it('deactivates permanently only when every style is permanent', () => {
    const rows = [faceRow(1, 1, 'session'), faceRow(2, 1, 'permanent')];
    expect(planToggle(rows, 'session')?.deactivate).toBe(true);
    // Shift+A makes the session one permanent instead.
    expect(planToggle(rows, 'permanent')?.deactivate).toBe(false);
    expect(planToggle([faceRow(2, 1, 'permanent')], 'permanent')?.deactivate).toBe(true);
  });

  it('A on a mixed selection activates only the inactive faces, never demoting a permanent one', () => {
    // A permanent style and an inactive one: only the inactive one is sent.
    const mixed = planToggle([faceRow(1, 1, 'permanent'), faceRow(2, 1)], 'session');
    expect(mixed).toEqual({ deactivate: false, targets: [{ kind: 'faces', faces: [2] }] });
    // A partly active family sends its inactive faces, not the family.
    const known = new Map([[3, [faceRow(30, 3, 'permanent').face, faceRow(31, 3).face]]]);
    const partial = planToggle([familyRow(3, 'partial'), faceRow(2, 1)], 'session', known);
    expect(partial?.targets).toEqual([{ kind: 'faces', faces: [31, 2] }]);
    // Its faces unknown: it is left out rather than switched to session.
    expect(planToggle([familyRow(3, 'partial'), faceRow(2, 1)], 'session')?.targets).toEqual([
      { kind: 'faces', faces: [2] },
    ]);
    // An inactive family still goes as one family request.
    expect(planToggle([familyRow(4), faceRow(1, 1, 'permanent')], 'session')?.targets).toEqual([
      { kind: 'family', family: 4 },
    ]);
  });

  it('reads the modes of a family row from its faces for Shift+A', () => {
    const rows = [familyRow(1, 'active')];
    const faces = (a: ActivationMode, b: ActivationMode) =>
      new Map([[1, [faceRow(10, 1, a).face, faceRow(11, 1, b).face]]]);
    expect(planToggle(rows, 'permanent', faces('session', 'permanent'))?.deactivate).toBe(false);
    expect(planToggle(rows, 'permanent', faces('permanent', 'permanent'))?.deactivate).toBe(true);
    // Without its faces the summary decides.
    expect(planToggle(rows, 'permanent')?.deactivate).toBe(true);
  });
});

describe('undoSteps', () => {
  const since = '2026-10-01T00:00:00Z';
  const active = (face: number, mode: ActivationMode) => ({ face, mode, since, document: null });

  it('deactivates the faces an activation turned on', () => {
    const steps = undoSteps([], [out(1, done), out(2, done)]);
    expect(steps).toEqual([{ kind: 'deactivate', faces: [1, 2] }]);
  });

  it('puts a face whose mode changed back in its old mode', () => {
    const steps = undoSteps([active(1, 'session')], [out(1, done), out(2, done)]);
    expect(steps).toEqual([
      { kind: 'deactivate', faces: [2] },
      { kind: 'activate', faces: [1], mode: 'session' },
    ]);
  });

  it('activates deactivated faces again, each in its old mode', () => {
    const before = [active(1, 'permanent'), active(2, 'session'), active(3, 'session')];
    const steps = undoSteps(before, [out(1, done), out(2, done), out(3, done)]);
    expect(steps).toEqual([
      { kind: 'activate', faces: [2, 3], mode: 'session' },
      { kind: 'activate', faces: [1], mode: 'permanent' },
    ]);
  });

  it('leaves the faces the change did not touch', () => {
    const steps = undoSteps(
      [],
      [
        out(1, { status: 'unchanged' }),
        out(2, { status: 'skipped', reason: 'Type 1' }),
        out(3, { status: 'conflict', with: { kind: 'system', path: '/x.ttf' } }),
      ],
    );
    expect(steps).toEqual([]);
  });
});

describe('bulkMessage', () => {
  it('counts the faces that changed, in the mode used', () => {
    expect(bulkMessage(false, 'session', [out(1, done), out(2, done)])).toBe(
      'Activated 2 fonts for this session.',
    );
    expect(bulkMessage(false, 'permanent', [out(1, done)])).toBe('Activated 1 font permanently.');
    expect(bulkMessage(true, 'session', [out(1, done), out(2, done), out(3, done)])).toBe(
      'Deactivated 3 fonts.',
    );
  });

  it('says how many faces were not changed, but not the ones already so', () => {
    const msg = bulkMessage(false, 'session', [
      out(1, done),
      out(2, { status: 'unchanged' }),
      out(3, { status: 'skipped', reason: 'Type 1' }),
    ]);
    expect(msg).toBe('Activated 1 font for this session. 1 font not activated.');
    expect(bulkMessage(true, 'session', [])).toBe('No fonts were deactivated.');
  });
});
