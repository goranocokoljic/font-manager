/**
 * Called on a toast as it starts to leave (E46.3: it fades out over the next one). It is no
 * longer the toast: screen readers, the keyboard, the pointer and `data-testid` see only the
 * next one, and its ids go, so the next one's `aria-describedby` finds its own text.
 */
export function retireToast(el: Element): void {
  el.removeAttribute('data-testid');
  el.setAttribute('aria-hidden', 'true');
  (el as HTMLElement).inert = true;
  for (const named of el.querySelectorAll('[id]')) named.removeAttribute('id');
}
