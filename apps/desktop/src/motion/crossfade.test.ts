import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { crossfade, tokenMs } from './crossfade';

/** A block in a parent, with the motion token. */
function setup(ms = '200ms') {
  const parent = document.createElement('div');
  const el = document.createElement('div');
  el.dataset.testid = 'layout-block';
  el.innerHTML = '<p id="inner" data-testid="inner" tabindex="0" style="font-family: Old">Old</p>';
  el.style.setProperty('--fm-motion-move', ms);
  parent.appendChild(el);
  document.body.appendChild(parent);
  return { parent, el };
}

beforeEach(() => vi.useFakeTimers());

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  document.body.innerHTML = '';
});

describe('crossfade', () => {
  it('reads a time token in ms or s, and anything else as 0', () => {
    expect(tokenMs('200ms')).toBe(200);
    expect(tokenMs(' 0.28s ')).toBe(280);
    expect(tokenMs('0ms')).toBe(0);
    expect(tokenMs('')).toBe(0);
    expect(tokenMs('fast')).toBe(0);
  });

  it('lays a copy of the old block over the new one; CSS fades one out, the other in', () => {
    const { parent, el } = setup();
    const play = crossfade(el);
    el.innerHTML = '<p style="font-family: New">New</p>';
    play();
    const ghost = parent.lastElementChild as HTMLElement;
    expect(ghost).not.toBe(el);
    expect(ghost.textContent).toBe('Old');
    expect(ghost.style.position).toBe('absolute');
    expect(ghost.style.pointerEvents).toBe('none');
    expect(ghost.getAttribute('aria-hidden')).toBe('true');
    // No test ids, ids or tab stops: the copy is not a second block.
    expect(ghost.querySelector('[data-testid], [id], [tabindex]')).toBeNull();
    expect(ghost.hasAttribute('data-testid')).toBe(false);
    // Both run the opacity transition of styles/motion.css at the move token; the copy ends
    // hidden, the block shown.
    expect([...ghost.classList]).toEqual(['fm-crossfade', 'fm-crossfade-hidden']);
    expect([...el.classList]).toEqual(['fm-crossfade']);
    expect(el.style.getPropertyValue('--fm-transition-time')).toBe('var(--fm-motion-move)');
    // The copy goes when the transition ends.
    ghost.dispatchEvent(new Event('transitionend'));
    expect(parent.children).toHaveLength(1);
    expect(el.classList.contains('fm-crossfade')).toBe(false);
  });

  it('removes the copy after the token even when no transitionend comes', () => {
    const { parent, el } = setup('200ms');
    crossfade(el)();
    expect(parent.children).toHaveLength(2);
    vi.advanceTimersByTime(199);
    expect(parent.children).toHaveLength(2);
    vi.advanceTimersByTime(100);
    expect(parent.children).toHaveLength(1);
  });

  it('cuts with a 0 ms token (reduced motion sets the tokens to 0)', () => {
    const { parent, el } = setup('0ms');
    crossfade(el)();
    expect(parent.children).toHaveLength(1);
    expect(el.classList.length).toBe(0);
  });

  it('cuts when the OS asks for reduced motion', () => {
    const { parent, el } = setup();
    vi.stubGlobal('matchMedia', () => ({ matches: true }));
    crossfade(el)();
    expect(parent.children).toHaveLength(1);
  });

  it('cuts for an element without a parent', () => {
    const el = document.createElement('div');
    expect(() => crossfade(el)()).not.toThrow();
  });
});
