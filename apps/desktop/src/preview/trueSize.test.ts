import { afterEach, describe, expect, it } from 'vitest';
import { applyInterfaceSize } from '../api/interfaceSize';
import { interfaceFactorNow, localWidth, toTruePx } from './trueSize';

/** An element whose box reports `rect` (getBoundingClientRect) and `client` (clientWidth). */
function box(rect: number, client = rect): HTMLElement {
  const el = document.createElement('div');
  el.getBoundingClientRect = () => ({ width: rect }) as DOMRect;
  Object.defineProperty(el, 'clientWidth', { value: client });
  return el;
}

afterEach(() => applyInterfaceSize(1, document.documentElement, true));

describe('measuring inside a surface that takes the size back (E58.7)', () => {
  it('reads the factor the page set, 1 when unset', () => {
    expect(interfaceFactorNow()).toBe(1);
    applyInterfaceSize(1.5, document.documentElement, true);
    expect(interfaceFactorNow()).toBe(1.5);
  });

  it('turns interface pixels into true pixels', () => {
    expect(toTruePx(200, 1.25)).toBe(250);
    expect(toTruePx(200, 1)).toBe(200);
  });

  it("gives a width in the element's own pixels whatever space the engine reports", () => {
    // Chromium reports the zoomed space: the frame of 300 own px is 200 on the screen at 1.5.
    expect(localWidth(box(100), box(200, 300), 1.5)).toBeCloseTo(150);
    // Older WebKit reports the element's own space: the ratio is 1.
    expect(localWidth(box(150), box(300, 300), 1.5)).toBeCloseTo(150);
    // At the default size nothing changes.
    expect(localWidth(box(120), box(300), 1)).toBe(120);
  });
});
