// Where the focus goes back to when a sheet (Compare, Quick look) closes: the
// element it was opened from (WCAG 2.4.3). A sheet opened from the command palette
// mounts while the focus is still in the palette, or already nowhere; it then goes
// back to the element the palette was opened from.

let paletteOrigin: HTMLElement | null = null;

const usable = (el: Element | null): el is HTMLElement =>
  el instanceof HTMLElement &&
  el.isConnected &&
  el !== document.body &&
  !el.closest('[role="dialog"]');

/** The palette records the element it was opened from, and clears it once it has closed. */
export function setPaletteOrigin(el: Element | null): void {
  paletteOrigin = usable(el) ? el : null;
}

/** The element a sheet that opens now returns the focus to; the list column as a last resort. */
export function sheetReturnTarget(): HTMLElement | null {
  const now = document.activeElement;
  if (usable(now)) return now;
  if (usable(paletteOrigin)) return paletteOrigin;
  return document.querySelector<HTMLElement>('main');
}
