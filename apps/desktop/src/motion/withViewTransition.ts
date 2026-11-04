import { prefersReducedMotion } from './useReducedMotion';

/**
 * Runs `fn`, which changes the page, inside a view transition when the webview has them.
 * Without them, or under reduced motion (the spec's "cut"), it just runs `fn`. Resolves when
 * `fn` is done and rejects if `fn` fails, the same on both paths. `finished`, if given, runs
 * once when nothing animates any more, so a caller can take back what it set up for the
 * transition (a `view-transition-name`, a class on the root).
 */
export async function withViewTransition(
  fn: () => void | Promise<void>,
  finished?: () => void,
): Promise<void> {
  if (typeof document.startViewTransition !== 'function' || prefersReducedMotion()) {
    try {
      await fn();
    } finally {
      finished?.();
    }
    return;
  }
  const transition = document.startViewTransition(fn);
  // A new transition skips a running one, which rejects `ready`. The page is still right.
  transition.ready.catch(() => {});
  if (finished) void transition.finished.then(finished, finished);
  await transition.updateCallbackDone;
}
