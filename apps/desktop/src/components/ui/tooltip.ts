// The timing of the app's one tooltip (E54.9), shared by every UiIconButton, so its rules are
// in one place and testable without a DOM layout:
// - pointer: the tooltip comes up after HOVER_DELAY_MS of rest on the button; while another
//   tooltip was up a moment ago (WARM_MS), the next one comes up at once, so a pass along a
//   row of buttons does not wait at every button;
// - keyboard: a focus that follows a key press shows it at once; a focus that follows a
//   pointer press (a click) does not, as the press hides it.
// Only one tooltip is up at a time: showing one closes the one before.

/** The pointer rests this long before a tooltip comes up (the criterion: within 400 ms). */
export const HOVER_DELAY_MS = 300;
/** A tooltip that closed this recently makes the next one come up without the delay. */
export const WARM_MS = 500;

let lastInput: 'pointer' | 'key' = 'pointer';
let ids = 0;

/**
 * A page-wide unique id for a tooltip, so a button's `aria-describedby` finds its own. Not
 * `useId()`: its ids repeat across Vue apps on one page (a test mounts each as its own app).
 */
export function nextTooltipId(): string {
  ids += 1;
  return `fm-tooltip-${ids}`;
}
let listening = false;
let lastClosedAt = Number.NEGATIVE_INFINITY;
let closeCurrent: (() => void) | null = null;

function onKey(): void {
  lastInput = 'key';
}
function onPointer(): void {
  lastInput = 'pointer';
}

/** Starts to watch which input came last (once per window). */
export function watchInput(): void {
  if (listening || typeof window === 'undefined') return;
  listening = true;
  window.addEventListener('keydown', onKey, true);
  window.addEventListener('pointerdown', onPointer, true);
}

/** True when the last input was a key, so a focus now is a keyboard focus. */
export function keyboardFocus(): boolean {
  return lastInput === 'key';
}

/** The wait before a hovered button's tooltip comes up. */
export function hoverDelay(now = Date.now()): number {
  return closeCurrent || now - lastClosedAt < WARM_MS ? 0 : HOVER_DELAY_MS;
}

/** A tooltip came up: the one before it closes. */
export function claim(close: () => void): void {
  if (closeCurrent && closeCurrent !== close) closeCurrent();
  closeCurrent = close;
}

/** A tooltip closed. */
export function release(close: () => void, now = Date.now()): void {
  if (closeCurrent !== close) return;
  closeCurrent = null;
  lastClosedAt = now;
}

/** For tests: forget the last tooltip and the last input. */
export function resetTooltips(): void {
  closeCurrent = null;
  lastClosedAt = Number.NEGATIVE_INFINITY;
  lastInput = 'pointer';
}
