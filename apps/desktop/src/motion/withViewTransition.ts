import { prefersReducedMotion } from './useReducedMotion';

/**
 * Runs `fn`, which changes the page, inside a view transition when the webview has them.
 * Without them, or under reduced motion (the spec's "cut"), it just runs `fn`. Resolves when
 * `fn` is done and rejects if `fn` fails, the same on both paths.
 */
export async function withViewTransition(fn: () => void | Promise<void>): Promise<void> {
  if (typeof document.startViewTransition !== 'function' || prefersReducedMotion()) {
    await fn();
    return;
  }
  const transition = document.startViewTransition(fn);
  // A new transition skips a running one, which rejects `ready`. The page is still right.
  transition.ready.catch(() => {});
  await transition.updateCallbackDone;
}
