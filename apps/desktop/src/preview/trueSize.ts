// Measuring inside a surface that takes the interface size back (`.fm-true-size`,
// E58.7, ADR 0029). Its content is laid out in true pixels while the chrome around it
// is in interface pixels (true pixels times the factor). Code that fits a specimen to
// the room the chrome gives it converts between the two here, so the size the user set
// is what is drawn.
//
// `getBoundingClientRect` does not report the same space on every engine for an
// element under a CSS zoom: Chromium reports the zoomed space, older WebKit the
// element's own. `localWidth` measures in the element's own CSS pixels on both.

import { readonly, ref, type Ref } from 'vue';
import { INTERFACE_FACTOR_VAR as FACTOR_VAR } from '../api/interfaceSize';

/** The interface factor on the root now (1, 1.25 or 1.5); 1 when unset or unreadable. */
export function interfaceFactorNow(): number {
  if (typeof document === 'undefined' || typeof getComputedStyle === 'undefined') return 1;
  const raw = getComputedStyle(document.documentElement).getPropertyValue(FACTOR_VAR);
  const factor = Number.parseFloat(raw);
  return Number.isFinite(factor) && factor > 0 ? factor : 1;
}

/** Interface pixels (the chrome's) as true pixels (a specimen's). */
export function toTruePx(interfacePx: number, factor = interfaceFactorNow()): number {
  return interfacePx * factor;
}

/**
 * The exact width of `el` in the CSS pixels of `frame`'s own layout (`frame` is laid out
 * in the same space as `el`, and has a width). `getBoundingClientRect` keeps fractions,
 * `clientWidth` gives the space: their ratio is 1, the factor or its inverse.
 */
export function localWidth(el: Element, frame: HTMLElement, factor = interfaceFactorNow()): number {
  const width = el.getBoundingClientRect().width;
  const box = frame.getBoundingClientRect().width;
  const own = frame.clientWidth;
  if (!width || !box || !own) return width;
  const ratio = box / own;
  const scales = [1, 1 / factor, factor];
  const scale = scales.reduce((best, s) =>
    Math.abs(s - ratio) < Math.abs(best - ratio) ? s : best,
  );
  return width / scale;
}

let shared: Ref<number> | undefined;

/**
 * The interface factor as a ref that follows the root: a change of size (the variable on
 * the root's style) updates it at once, so a layout computed from it follows too.
 */
export function useInterfaceFactor(): Readonly<Ref<number>> {
  if (!shared) {
    const now = ref(interfaceFactorNow());
    shared = now;
    if (typeof MutationObserver !== 'undefined' && typeof document !== 'undefined') {
      new MutationObserver(() => {
        const next = interfaceFactorNow();
        if (next !== now.value) now.value = next;
      }).observe(document.documentElement, { attributes: true, attributeFilter: ['style'] });
    }
  }
  return readonly(shared);
}
