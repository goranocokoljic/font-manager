// The motion helpers every animated screen reuses. The table they implement is
// dev-docs/redesign/MOTION.md; the tokens they read are in styles/theme.css.
export { default as FmCrossfade } from './FmCrossfade.vue';
export { default as FmFade } from './FmFade.vue';
export { default as FmFlipGroup } from './FmFlipGroup.vue';
export { default as FmSlide } from './FmSlide.vue';
export type { MotionSpeed, ReducedMotion } from './speed';
export { useReducedMotion } from './useReducedMotion';
export { viewTransitionsRun, withViewTransition } from './withViewTransition';
export { fadeIn } from './fadeIn';
export { crossfade } from './crossfade';
