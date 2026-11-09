/** The three durations of the motion spec (dev-docs/redesign/MOTION.md). */
export type MotionSpeed = 'fast' | 'move' | 'page';

/** Points the helper's transition at the token of `speed`; the stylesheet reads the variable. */
export function setMotionSpeed(el: Element, speed: MotionSpeed): void {
  (el as HTMLElement).style.setProperty('--fm-transition-time', `var(--fm-motion-${speed})`);
}

/**
 * What a helper does under reduced motion: `cut` (the default) shows and hides at once; `fade`
 * keeps a plain fade at `--fm-motion-reduced-fade` and drops any movement (styles/motion.css).
 */
export type ReducedMotion = 'cut' | 'fade';

/** Marks `el` for the reduced-motion rule of styles/motion.css that keeps the fade. */
export function setReducedMotion(el: Element, reduced: ReducedMotion): void {
  const { dataset } = el as HTMLElement;
  if (reduced === 'fade') dataset.fmReduced = 'fade';
  else delete dataset.fmReduced;
}
