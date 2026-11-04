import { getCurrentScope, onScopeDispose, readonly, ref, type Ref } from 'vue';

const QUERY = '(prefers-reduced-motion: reduce)';

/** Whether the OS asks for reduced motion right now. */
export function prefersReducedMotion(): boolean {
  return window.matchMedia?.(QUERY).matches ?? false;
}

/**
 * Whether the OS asks for reduced motion, kept current when the setting changes. CSS needs
 * no help (the duration tokens go to 0ms); this is for motion that scripts decide on.
 */
export function useReducedMotion(): Readonly<Ref<boolean>> {
  const reduced = ref(prefersReducedMotion());
  const media = window.matchMedia?.(QUERY);
  if (media) {
    const onChange = (e: MediaQueryListEvent) => {
      reduced.value = e.matches;
    };
    media.addEventListener('change', onChange);
    if (getCurrentScope()) onScopeDispose(() => media.removeEventListener('change', onChange));
  }
  return readonly(reduced);
}
