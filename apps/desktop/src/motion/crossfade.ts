import { prefersReducedMotion } from './useReducedMotion';
import { setMotionSpeed, type MotionSpeed } from './speed';

/** A CSS time token's value in ms: `200ms`, `0.2s`; 0 when it is not a time. */
export function tokenMs(value: string): number {
  const m = /^\s*([\d.]+)\s*(ms|s)\s*$/.exec(value);
  if (!m) return 0;
  const n = Number(m[1]);
  return m[2] === 's' ? n * 1000 : n;
}

/** Extra time after the token before the copy goes, in case `transitionend` never comes. */
const SETTLE_MS = 50;

/**
 * A crossfade of one element whose content is about to change (E45.3, the Studio's Shuffle).
 * Call it BEFORE the change: it copies the element as it looks now. Call the answer AFTER the
 * change: the copy lies over the element and fades out while the element fades in. The fade is
 * a CSS transition of opacity (styles/motion.css: `fm-crossfade`, `fm-crossfade-hidden`) at the
 * `speed` token on `--fm-ease`, so no duration or curve lives in a script. The copy is inert and
 * hidden from assistive technology and from tests (no test ids), and goes when the fade ends.
 *
 * The parent of the element must be its containing block (`position: relative`). Under reduced
 * motion or with a 0 ms token the change is a cut.
 */
export function crossfade(el: HTMLElement, speed: MotionSpeed = 'move'): () => void {
  if (prefersReducedMotion() || !el.parentElement) return () => undefined;
  const ms = tokenMs(getComputedStyle(el).getPropertyValue(`--fm-motion-${speed}`));
  if (ms <= 0) return () => undefined;
  const ghost = el.cloneNode(true) as HTMLElement;
  for (const node of [ghost, ...ghost.querySelectorAll<HTMLElement>('*')]) {
    node.removeAttribute('data-testid');
    node.removeAttribute('id');
    node.removeAttribute('tabindex');
  }
  ghost.setAttribute('aria-hidden', 'true');
  ghost.inert = true;
  const top = el.offsetTop;
  const left = el.offsetLeft;
  const width = el.offsetWidth;
  return () => {
    const parent = el.parentElement;
    if (!parent || !el.isConnected) return;
    Object.assign(ghost.style, {
      position: 'absolute',
      top: `${top}px`,
      left: `${left}px`,
      width: `${width}px`,
      margin: '0',
      pointerEvents: 'none',
    });
    setMotionSpeed(el, speed);
    setMotionSpeed(ghost, speed);
    // The start: the new content hidden, the copy shown, no transition yet.
    el.classList.remove('fm-crossfade');
    el.classList.add('fm-crossfade-hidden');
    ghost.classList.add('fm-crossfade');
    parent.appendChild(ghost);
    void el.offsetWidth; // Applies the start before the transition begins.
    el.classList.add('fm-crossfade');
    el.classList.remove('fm-crossfade-hidden');
    ghost.classList.add('fm-crossfade-hidden');
    let done = false;
    const finish = () => {
      if (done) return;
      done = true;
      ghost.remove();
      el.classList.remove('fm-crossfade');
    };
    ghost.addEventListener('transitionend', finish, { once: true });
    setTimeout(finish, ms + SETTLE_MS);
  };
}
