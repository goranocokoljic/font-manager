import { afterEach, describe, expect, it, vi } from 'vitest';
import { crossfade, tokenMs } from './crossfade';

type Frames = Keyframe[];

/** A block in a parent, with the motion token, and `animate` recorded. */
function setup(ms = '200ms') {
  const parent = document.createElement('div');
  const el = document.createElement('div');
  el.dataset.testid = 'layout-block';
  el.innerHTML = '<p id="inner" data-testid="inner" tabindex="0" style="font-family: Old">Old</p>';
  el.style.setProperty('--fm-motion-move', ms);
  el.style.setProperty('--fm-ease', 'linear');
  parent.appendChild(el);
  document.body.appendChild(parent);
  const calls: { target: Element; frames: Frames; options: KeyframeAnimationOptions }[] = [];
  let finish!: () => void;
  const finished = new Promise<void>((r) => (finish = r));
  const animate = vi.fn(function (
    this: Element,
    frames: Frames,
    options: KeyframeAnimationOptions,
  ) {
    calls.push({ target: this, frames, options });
    return { finished } as unknown as Animation;
  });
  Object.defineProperty(HTMLElement.prototype, 'animate', { value: animate, configurable: true });
  return { parent, el, calls, finish };
}

afterEach(() => {
  Reflect.deleteProperty(HTMLElement.prototype, 'animate');
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

  it('lays a copy of the old block over the new one and fades one out, the other in', async () => {
    const { parent, el, calls, finish } = setup();
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
    expect(calls.map((c) => [c.target === el ? 'new' : 'old', c.frames])).toEqual([
      ['new', [{ opacity: 0 }, { opacity: 1 }]],
      ['old', [{ opacity: 1 }, { opacity: 0 }]],
    ]);
    expect(calls[0]!.options).toMatchObject({ duration: 200, easing: 'linear' });
    finish();
    await Promise.resolve();
    await Promise.resolve();
    expect(parent.children).toHaveLength(1);
  });

  it('cuts with a 0 ms token (reduced motion sets the tokens to 0)', () => {
    const { parent, el, calls } = setup('0ms');
    crossfade(el)();
    expect(calls).toHaveLength(0);
    expect(parent.children).toHaveLength(1);
  });

  it('cuts when the OS asks for reduced motion', () => {
    const { parent, el, calls } = setup();
    vi.stubGlobal('matchMedia', () => ({ matches: true }));
    crossfade(el)();
    expect(calls).toHaveLength(0);
    expect(parent.children).toHaveLength(1);
  });

  it('cuts without the Web Animations API', () => {
    const el = document.createElement('div');
    document.body.appendChild(el);
    expect(() => crossfade(el)()).not.toThrow();
    expect(document.body.children).toHaveLength(1);
  });
});
