// Dragging fonts from the list or the table onto a collection in the sidebar.
// The drag carries the ids of the rows, as a family or a face each; dragging a
// selected row drags the whole selection.
//
// A drag that knows the Font Switch payload of the row's face (E34.1) also
// carries it for a design app outside the window: as JSON, the PostScript name
// as plain text and the font file as a URI list. See ADR 0017 for what each
// platform's webview carries out of the window.
import type { FontSwitchPayload } from '../api/types';
import type { FontPick } from '../stores/collections';
import type { RowKey } from './rows';

/** The `DataTransfer` type a font drag carries its JSON `FontPick` in. */
export const FONTS_MIME = 'application/x-fm-fonts';

/** The `DataTransfer` type a font drag carries its JSON `FontSwitchPayload` in. */
export const FONT_SWITCH_MIME = 'application/x-fm-font-switch';

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

/**
 * Starts a font drag of `keys` on `e`; with `fontSwitch`, the drag also carries
 * it for another app (`FONT_SWITCH_MIME`, `text/plain`, `text/uri-list`).
 */
export function startFontDrag(e: DragEvent, keys: RowKey[], fontSwitch?: FontSwitchPayload): void {
  if (!e.dataTransfer) return;
  e.dataTransfer.effectAllowed = 'copy';
  e.dataTransfer.setData(FONTS_MIME, JSON.stringify(pickOf(keys)));
  if (!fontSwitch) return;
  e.dataTransfer.setData(FONT_SWITCH_MIME, JSON.stringify(fontSwitch));
  e.dataTransfer.setData('text/plain', fontSwitch.postscriptName);
  e.dataTransfer.setData('text/uri-list', fontSwitch.fileUrl);
}

/**
 * True when a `dragleave` on the document means the drag left the window: no
 * element it entered, and the pointer on or past an edge of the `width` ×
 * `height` viewport.
 */
export function leftWindow(e: DragEvent, width: number, height: number): boolean {
  if (e.relatedTarget) return false;
  return e.clientX <= 0 || e.clientY <= 0 || e.clientX >= width || e.clientY >= height;
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
