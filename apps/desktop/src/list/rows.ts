// The rows of the font list: one row per family and, under an expanded family,
// one row per style (face) in the API's order, then its pinned virtual styles.
// Each row has a stable key, so the selection and the cursor survive scrolling,
// expanding and reloading.
import type {
  FaceId,
  FaceSummary,
  FamilyId,
  FamilySummary,
  VirtualStyle,
  VirtualStyleId,
} from '../api/types';
import { t, type MessageKey } from '../i18n';

export type RowKey = string;

export const familyKey = (id: FamilyId): RowKey => `family:${id}`;
export const faceKey = (id: FaceId): RowKey => `face:${id}`;
export const virtualKey = (id: VirtualStyleId): RowKey => `virtual:${id}`;

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
  /**
   * Set on the row of a virtual style: `face` is the variable face it is an
   * instance of, previewed at the style's coordinates.
   */
  virtual?: VirtualStyle;
}

export type ListRow = FamilyRow | FaceRow;

/** The style a face row shows: the virtual style's name, else the face's style. */
export const styleName = (row: FaceRow): string => row.virtual?.name ?? row.face.style;

/**
 * Families in order; an expanded family is followed by its styles once they are
 * loaded (`facesOf` gives `undefined` before that), then by the virtual styles
 * `virtualOf` gives for it, each with its variable face. `facesOf`
 * and `virtualOf` are only asked about expanded families.
 */
export function flattenRows(
  families: readonly FamilySummary[],
  expanded: ReadonlySet<FamilyId>,
  facesOf: (family: FamilyId) => readonly FaceSummary[] | undefined,
  virtualOf: (family: FamilyId) => readonly { style: VirtualStyle; face: FaceSummary }[] = () => [],
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
    const pinned = virtualOf(family.id);
    const count = faces.length + pinned.length;
    faces.forEach((face, k) =>
      rows.push({
        kind: 'face',
        key: faceKey(face.id),
        face,
        family,
        position: k + 1,
        count,
      }),
    );
    pinned.forEach(({ style, face }, k) =>
      rows.push({
        kind: 'face',
        key: virtualKey(style.id),
        face,
        family,
        position: faces.length + k + 1,
        count,
        virtual: style,
      }),
    );
  });
  return rows;
}

/** "1 style", "4 styles". */
export const styleCount = (n: number) => t('list.styleCount', { count: n });

const FAMILY_STATE: Record<FamilySummary['activation'], string> = {
  inactive: 'list.row.notActive',
  partial: 'list.row.partlyActive',
  active: 'list.row.active',
};

/**
 * What a screen reader announces for a row: the family and its style count, or
 * the family and style of a face, then the activation state.
 */
export function rowLabel(row: ListRow): string {
  if (row.kind === 'family') {
    const { name, styleCount: n, activation } = row.family;
    return t('list.row.family', {
      name,
      styles: styleCount(n),
      state: t(FAMILY_STATE[activation] as MessageKey),
    });
  }
  const { familyName, active } = row.face;
  const style = row.virtual
    ? t('list.row.pinnedInstance', { name: row.virtual.name })
    : row.face.style;
  const state =
    active === 'permanent'
      ? t('list.row.activePermanently')
      : active === 'session'
        ? t('list.row.activeSession')
        : t('list.row.notActive');
  return t('list.row.face', { family: familyName, style, state });
}
