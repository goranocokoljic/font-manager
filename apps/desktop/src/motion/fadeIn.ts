import { setMotionSpeed, type MotionSpeed } from './speed';

const CLASS = 'fm-fade-in';

/**
 * Fades `el` in from transparent: a page, or a page's content, that has just changed (E46.1).
 * Call it AFTER the change. The fade is a CSS animation (styles/motion.css: `fm-fade-in`) at the
 * `speed` token on `--fm-ease`, so no duration or curve lives in a script. Under reduced motion
 * it is the one fade the OS setting keeps, `--fm-motion-reduced-fade` (120 ms), not a cut.
 * Called again while it runs, the fade starts over.
 */
export function fadeIn(el: Element | null | undefined, speed: MotionSpeed = 'move'): void {
  if (!(el instanceof HTMLElement)) return;
  setMotionSpeed(el, speed);
  el.classList.remove(CLASS);
  void el.offsetWidth; // A class put back in the same frame does not restart the animation.
  el.classList.add(CLASS);
  const done = (e: AnimationEvent) => {
    if (e.target !== el) return;
    el.removeEventListener('animationend', done);
    el.removeEventListener('animationcancel', done);
    el.classList.remove(CLASS);
  };
  el.addEventListener('animationend', done);
  el.addEventListener('animationcancel', done);
}
