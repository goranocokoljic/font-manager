/** The three durations of the motion spec (dev-docs/redesign/MOTION.md). */
export type MotionSpeed = 'fast' | 'move' | 'page';

/** Points the helper's transition at the token of `speed`; the stylesheet reads the variable. */
export function setMotionSpeed(el: Element, speed: MotionSpeed): void {
  (el as HTMLElement).style.setProperty('--fm-transition-time', `var(--fm-motion-${speed})`);
}
