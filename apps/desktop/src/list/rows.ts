// The rows of the font list: one row per family and, under an expanded family,
// one row per style (face) in the API's order. Each row has a stable key, so the
// selection and the cursor survive scrolling, expanding and reloading.
import type { FaceId, FaceSummary, FamilyId, FamilySummary } from '../api/types';

export type RowKey = string;

export const familyKey = (id: FamilyId): RowKey => `family:${id}`;
export const faceKey = (id: FaceId): RowKey => `face:${id}`;

export interface FamilyRow {
  kind: 'family';
  key: RowKey;
  family: FamilySummary;
  /** 1-based position among the families. */
  position: number;
  expanded: boolean;
}

export interface FaceRow {
  kind: 'face';
  key: RowKey;
  face: FaceSummary;
  family: FamilySummary;
  /** 1-based position among the family's styles. */
  position: number;
  /** Styles in the family. */
  count: number;
}

export type ListRow = FamilyRow | FaceRow;

/**
 * Families in order; an expanded family is followed by its styles once they are
 * loaded (`facesOf` gives `undefined` before that). `facesOf` is only asked about
 * expanded families.
 */
export function flattenRows(
  families: readonly FamilySummary[],
  expanded: ReadonlySet<FamilyId>,
  facesOf: (family: FamilyId) => readonly FaceSummary[] | undefined,
): ListRow[] {
  const rows: ListRow[] = [];
  families.forEach((family, i) => {
    const open = expanded.has(family.id);
    rows.push({
      kind: 'family',
      key: familyKey(family.id),
      family,
      position: i + 1,
      expanded: open,
    });
    if (!open) return;
    const faces = facesOf(family.id) ?? [];
    faces.forEach((face, k) =>
      rows.push({
        kind: 'face',
        key: faceKey(face.id),
        face,
        family,
        position: k + 1,
        count: faces.length,
      }),
    );
  });
  return rows;
}

/** "1 style", "4 styles". */
export const styleCount = (n: number) => (n === 1 ? '1 style' : `${n} styles`);

const FAMILY_STATE: Record<FamilySummary['activation'], string> = {
  inactive: 'not active',
  partial: 'partly active',
  active: 'active',
};

/**
 * What a screen reader announces for a row: the family and its style count, or
 * the family and style of a face, then the activation state.
 */
export function rowLabel(row: ListRow): string {
  if (row.kind === 'family') {
    const { name, styleCount: n, activation } = row.family;
    return `${name}, ${styleCount(n)}, ${FAMILY_STATE[activation]}`;
  }
  const { familyName, style, active } = row.face;
  const state =
    active === 'permanent'
      ? 'active permanently'
      : active === 'session'
        ? 'active for this session'
        : 'not active';
  return `${familyName} ${style}, ${state}`;
}
