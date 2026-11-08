// The keyboard model of the font list, shared by the list and the table views:
// arrows, Home/End, PageUp/PageDown move the cursor; Right/Left expand/collapse;
// Enter opens the row's family page; Shift extends the selection,
// Ctrl/Cmd moves without selecting, Ctrl/Cmd+Space toggles a row, Ctrl/Cmd+A
// selects all, Escape clears the selection (E44.1). Plain Space is the app's quick-look
// key, so it is left alone.
// The grid view lays rows out in `columns` per line: Up/Down then move by a
// line of cards and Left/Right by one card (Alt+Right / Alt+Left opens and closes a family).
// Left/Right follow what the user sees: in right-to-left chrome they swap.
import { inlineKey } from '../i18n';
import type { useFontListStore } from '../stores/fontList';
import type { ListRow } from './rows';
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
  opts: { page: number; count: number; open: (row: ListRow) => void; columns?: number },
): boolean {
  const mod = e.ctrlKey || e.metaKey;
  const row = list.cursorRow;
  const columns = Math.max(1, opts.columns ?? 1);
  if (e.altKey) {
    // In the grid plain Left/Right move between cards, so Alt+Right/Left open and close a family.
    if (columns === 1 || mod || e.shiftKey) return false;
    const key = inlineKey(e.key);
    if (key === 'ArrowRight') list.expandOrEnter();
    else if (key === 'ArrowLeft') list.collapseOrLeave();
    else return false;
    return true;
  }
  switch (inlineKey(e.key)) {
    case 'ArrowDown':
      list.moveBy(columns, modeOf(e));
      return true;
    case 'ArrowUp':
      list.moveBy(-columns, modeOf(e));
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
      if (columns > 1) {
        list.moveBy(1, modeOf(e));
        return true;
      }
      if (mod || e.shiftKey) return false;
      list.expandOrEnter();
      return true;
    case 'ArrowLeft':
      if (columns > 1) {
        list.moveBy(-1, modeOf(e));
        return true;
      }
      if (mod || e.shiftKey) return false;
      list.collapseOrLeave();
      return true;
    case 'Enter':
      if (mod || e.shiftKey || !row) return false;
      opts.open(row);
      return true;
    case 'Escape':
      // Nothing selected: the key is not the list's, so a sheet or a field above can use it.
      if (mod || e.shiftKey || !list.selectedCount) return false;
      list.clearSelection();
      return true;
    case ' ':
      if (!mod) return false;
      list.toggleCursor();
      return true;
    case 'j':
    case 'k':
      if (mod || e.shiftKey) return false;
      list.moveBy(e.key === 'j' ? columns : -columns, 'replace');
      return true;
    default:
      if (mod && !e.shiftKey && e.key.toLowerCase() === 'a') {
        list.selectAll();
        return true;
      }
      return false;
  }
}
