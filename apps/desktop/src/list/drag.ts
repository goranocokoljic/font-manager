// Dragging fonts from the list or the table onto a collection in the sidebar.
// The drag carries the ids of the rows, as a family or a face each; dragging a
// selected row drags the whole selection.
import type { FontPick } from '../stores/collections';
import type { RowKey } from './rows';

/** The `DataTransfer` type a font drag carries its JSON `FontPick` in. */
export const FONTS_MIME = 'application/x-fm-fonts';

/** The rows a drag that starts on `key` carries: the selection if `key` is in it, else `key`. */
export function dragKeys(selected: ReadonlySet<RowKey>, key: RowKey): RowKey[] {
  return selected.has(key) ? [...selected] : [key];
}

/** Family rows become families, style rows faces; anything else is dropped. */
export function pickOf(keys: Iterable<RowKey>): FontPick {
  const pick: FontPick = { faces: [], families: [] };
  for (const key of keys) {
    const m = /^(family|face):(\d+)$/.exec(key);
    if (!m) continue;
    (m[1] === 'family' ? pick.families : pick.faces).push(Number(m[2]));
  }
  return pick;
}

/** Starts a font drag of `keys` on `e`. */
export function startFontDrag(e: DragEvent, keys: RowKey[]): void {
  if (!e.dataTransfer) return;
  e.dataTransfer.effectAllowed = 'copy';
  e.dataTransfer.setData(FONTS_MIME, JSON.stringify(pickOf(keys)));
}

/** True when `e` drags fonts (the payload itself is only readable on drop). */
export function isFontDrag(e: DragEvent): boolean {
  return !!e.dataTransfer?.types.includes(FONTS_MIME);
}

/** The fonts a drop carries, or null when it carries none. */
export function readFontDrop(e: DragEvent): FontPick | null {
  const raw = e.dataTransfer?.getData(FONTS_MIME);
  if (!raw) return null;
  try {
    const v = JSON.parse(raw) as Partial<FontPick>;
    const ids = (a: unknown) =>
      Array.isArray(a) ? a.filter((x): x is number => Number.isSafeInteger(x)) : [];
    return { faces: ids(v.faces), families: ids(v.families) };
  } catch {
    return null;
  }
}
