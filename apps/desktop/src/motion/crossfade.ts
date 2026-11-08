import { prefersReducedMotion } from './useReducedMotion';
import type { MotionSpeed } from './speed';

/** A CSS time token's value in ms: `200ms`, `0.2s`; 0 when it is not a time. */
export function tokenMs(value: string): number {
  const m = /^\s*([\d.]+)\s*(ms|s)\s*$/.exec(value);
  if (!m) return 0;
  const n = Number(m[1]);
  return m[2] === 's' ? n * 1000 : n;
}

/**
 * A crossfade of one element whose content is about to change (E45.3, the Studio's Shuffle).
 * Call it BEFORE the change: it copies the element as it looks now. Call the answer AFTER the
 * change: the copy lies over the element and fades out while the element fades in, at the
 * `speed` token with `--fm-ease`. The copy is inert and hidden from assistive technology and
 * from tests (no test ids), and goes when the fade ends.
 *
 * The parent of the element must be its containing block (`position: relative`). Without the
 * Web Animations API, under reduced motion or with a 0 ms token, the change is a cut.
 */
export function crossfade(el: HTMLElement, speed: MotionSpeed = 'move'): () => void {
  if (typeof el.animate !== 'function' || prefersReducedMotion()) return () => undefined;
  const style = getComputedStyle(el);
  const ms = tokenMs(style.getPropertyValue(`--fm-motion-${speed}`));
  if (ms <= 0 || !el.parentElement) return () => undefined;
  const easing = style.getPropertyValue('--fm-ease').trim() || 'ease';
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
    parent.appendChild(ghost);
    const timing = { duration: ms, easing };
    el.animate([{ opacity: 0 }, { opacity: 1 }], timing);
    const out = ghost.animate([{ opacity: 1 }, { opacity: 0 }], { ...timing, fill: 'forwards' });
    const done = () => ghost.remove();
    out.finished.then(done, done);
  };
}
