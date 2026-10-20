// Activation from the font list: the state of a row (its dot), what A / Shift+A
// do to the selected rows, and how a bulk change is undone. The API does the
// activating; this only turns rows into its requests and its answers into a toast.
import type {
  ActivationMode,
  ActivationOutcome,
  ActiveFace,
  FaceId,
  FaceSummary,
  FamilyId,
  Target,
} from '../api/types';
import type { ListRow } from './rows';

export type RowActivation = 'active' | 'partial' | 'inactive';

/** A style row is active or not; a family row can also be partly active. */
export function rowActivation(row: ListRow): RowActivation {
  return row.kind === 'face' ? (row.face.active ? 'active' : 'inactive') : row.family.activation;
}

/** What a toggle does to the rows: one request per target, all the same way. */
export interface TogglePlan {
  deactivate: boolean;
  targets: Target[];
}

/**
 * A / Shift+A on the selected rows. Every row fully active (for Shift+A: every
 * selected style permanently active) → deactivate them all; else activate them
 * all, so a mixed selection ends up active. A family row acts on the whole
 * family; a style row whose family row is also selected is covered by it. For
 * Shift+A a family row is permanent when every face in `familyFaces` is (the
 * summary does not tell the modes apart). A session activation (A) leaves
 * active faces alone, so it never turns a permanent font into a session one: a
 * partly active family sends only its inactive faces from `familyFaces`, and is
 * left out when they are not known. Null when nothing is selected.
 */
export function planToggle(
  rows: readonly ListRow[],
  mode?: ActivationMode,
  familyFaces: ReadonlyMap<FamilyId, readonly FaceSummary[]> = new Map(),
): TogglePlan | null {
  if (rows.length === 0) return null;
  const permanent = mode === 'permanent';
  const deactivate = rows.every((r) => {
    if (r.kind === 'face')
      return permanent ? r.face.active === 'permanent' : r.face.active !== null;
    const known = familyFaces.get(r.family.id);
    if (permanent && known?.length) return known.every((f) => f.active === 'permanent');
    return r.family.activation === 'active';
  });
  // Deactivating, or activating permanently (which may promote a session face):
  // every selected face. Activating for the session: only the inactive ones.
  const all = deactivate || permanent;
  const families = new Set(rows.filter((r) => r.kind === 'family').map((r) => r.family.id));
  const faces: FaceId[] = [];
  const targets: Target[] = [];
  for (const row of rows) {
    if (row.kind === 'face') {
      if (!families.has(row.family.id) && (all || row.face.active === null))
        faces.push(row.face.id);
    } else if (all || row.family.activation === 'inactive') {
      targets.push({ kind: 'family', family: row.family.id });
    } else {
      const known = familyFaces.get(row.family.id) ?? [];
      faces.push(...known.filter((f) => f.active === null).map((f) => f.id));
    }
  }
  if (faces.length) targets.unshift({ kind: 'faces', faces });
  return { deactivate, targets };
}

/** One request that puts faces back the way they were. */
export type UndoStep =
  | { kind: 'activate'; faces: FaceId[]; mode: ActivationMode }
  | { kind: 'deactivate'; faces: FaceId[] };

/**
 * The requests that undo a bulk change, from the active faces before it and its
 * outcomes: a face it deactivated is activated again in its old mode; a face it
 * activated is deactivated, or set back to its old mode when it was active before.
 * Faces the change did not touch (unchanged, conflict, skipped, failed) stay.
 */
export function undoSteps(
  before: readonly ActiveFace[],
  outcomes: readonly ActivationOutcome[],
): UndoStep[] {
  const was = new Map(before.map((a) => [a.face, a.mode]));
  const done = [...new Set(outcomes.filter((o) => o.outcome.status === 'done').map((o) => o.face))];
  const off: FaceId[] = [];
  const back: Record<ActivationMode, FaceId[]> = { session: [], permanent: [] };
  for (const face of done) {
    const mode = was.get(face);
    if (mode) back[mode].push(face);
    else off.push(face);
  }
  const steps: UndoStep[] = [];
  if (off.length) steps.push({ kind: 'deactivate', faces: off });
  for (const mode of ['session', 'permanent'] as const)
    if (back[mode].length) steps.push({ kind: 'activate', faces: back[mode], mode });
  return steps;
}

const fonts = (n: number) => (n === 1 ? '1 font' : `${n} fonts`);

/** The toast after a bulk change: how many faces changed, and how many did not. */
export function bulkMessage(
  deactivate: boolean,
  mode: ActivationMode,
  outcomes: readonly ActivationOutcome[],
): string {
  const done = new Set(outcomes.filter((o) => o.outcome.status === 'done').map((o) => o.face));
  const missed = new Set(
    outcomes.filter((o) => !['done', 'unchanged'].includes(o.outcome.status)).map((o) => o.face),
  );
  const verb = deactivate ? 'Deactivated' : 'Activated';
  const how = deactivate ? '' : mode === 'permanent' ? ' permanently' : ' for this session';
  const head =
    done.size === 0
      ? deactivate
        ? 'No fonts were deactivated.'
        : 'No fonts were activated.'
      : `${verb} ${fonts(done.size)}${how}.`;
  if (missed.size === 0) return head;
  const not = deactivate ? 'not deactivated' : 'not activated';
  return `${head} ${fonts(missed.size)} ${not}.`;
}
