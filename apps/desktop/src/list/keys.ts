// The keyboard model of the font list, shared by the list and the table views:
// arrows, Home/End, PageUp/PageDown move the cursor; Right/Left expand/collapse;
// Enter toggles a family or opens a style; Shift extends the selection,
// Ctrl/Cmd moves without selecting, Ctrl/Cmd+Space toggles a row, Ctrl/Cmd+A
// selects all. Plain Space is the app's quick-look key, so it is left alone.
import type { FaceId } from '../api/types';
import type { useFontListStore } from '../stores/fontList';
import type { SelectMode } from './selection';

type FontListStore = ReturnType<typeof useFontListStore>;

/** The selection mode of a key or click: Shift extends, Ctrl/Cmd keeps (key) or toggles (click). */
export function modeOf(e: KeyboardEvent | MouseEvent): SelectMode {
  if (e.shiftKey) return 'extend';
  if (e.ctrlKey || e.metaKey) return e instanceof KeyboardEvent ? 'keep' : 'toggle';
  return 'replace';
}

/**
 * Applies `e` to the list. Returns true when the key was the list's (the caller
 * then prevents the default and reveals the cursor).
 */
export function handleListKey(
  e: KeyboardEvent,
  list: FontListStore,
  opts: { page: number; count: number; open: (face: FaceId) => void },
): boolean {
  if (e.altKey) return false;
  const mod = e.ctrlKey || e.metaKey;
  const row = list.cursorRow;
  switch (e.key) {
    case 'ArrowDown':
      list.moveBy(1, modeOf(e));
      return true;
    case 'ArrowUp':
      list.moveBy(-1, modeOf(e));
      return true;
    case 'PageDown':
      list.moveBy(opts.page, modeOf(e));
      return true;
    case 'PageUp':
      list.moveBy(-opts.page, modeOf(e));
      return true;
    case 'Home':
      list.moveCursor(0, modeOf(e));
      return true;
    case 'End':
      list.moveCursor(opts.count - 1, modeOf(e));
      return true;
    case 'ArrowRight':
      if (mod || e.shiftKey) return false;
      list.expandOrEnter();
      return true;
    case 'ArrowLeft':
      if (mod || e.shiftKey) return false;
      list.collapseOrLeave();
      return true;
    case 'Enter':
      if (mod || e.shiftKey || !row) return false;
      if (row.kind === 'family') list.toggleExpanded(row.family.id);
      else opts.open(row.face.id);
      return true;
    case ' ':
      if (!mod) return false;
      list.toggleCursor();
      return true;
    default:
      if (mod && !e.shiftKey && e.key.toLowerCase() === 'a') {
        list.selectAll();
        return true;
      }
      return false;
  }
}
