import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { defineComponent, effectScope, h, nextTick, ref, type Component } from 'vue';
import {
  FmFade,
  FmFlipGroup,
  FmSlide,
  useReducedMotion,
  withViewTransition,
  type MotionSpeed,
} from './index';

const REAL_TRANSITIONS = { transition: false, 'transition-group': false };

/** A stand-in for the OS setting: `matchMedia` for the reduced-motion query, and a way to flip it. */
function fakeOsSetting(matches: boolean) {
  const listeners = new Set<(e: MediaQueryListEvent) => void>();
  const matchMedia = vi.fn((query: string) => ({
    media: query,
    get matches() {
      return matches;
    },
    addEventListener: (_: string, l: (e: MediaQueryListEvent) => void) => listeners.add(l),
    removeEventListener: (_: string, l: (e: MediaQueryListEvent) => void) => listeners.delete(l),
  }));
  vi.stubGlobal('matchMedia', matchMedia);
  return {
    matchMedia,
    listeners,
    flip(next: boolean) {
      matches = next;
      listeners.forEach((l) => l({ matches: next } as MediaQueryListEvent));
    },
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
  Reflect.deleteProperty(document, 'startViewTransition');
});

describe('useReducedMotion', () => {
  it('reads the setting at the start and again when it changes', async () => {
    const os = fakeOsSetting(true);
    const scope = effectScope();
    const reduced = scope.run(() => useReducedMotion())!;
    expect(os.matchMedia).toHaveBeenCalledWith('(prefers-reduced-motion: reduce)');
    expect(reduced.value).toBe(true);
    os.flip(false);
    expect(reduced.value).toBe(false);
    os.flip(true);
    expect(reduced.value).toBe(true);
    scope.stop();
  });

  it('stops listening when its scope ends', () => {
    const os = fakeOsSetting(false);
    const scope = effectScope();
    scope.run(() => useReducedMotion());
    expect(os.listeners.size).toBe(1);
    scope.stop();
    expect(os.listeners.size).toBe(0);
  });

  it('is false in a webview without matchMedia', () => {
    vi.stubGlobal('matchMedia', undefined);
    expect(useReducedMotion().value).toBe(false);
  });
});

describe('withViewTransition', () => {
  /** A `startViewTransition` that runs the callback at once, as a browser does soon after. */
  function fakeViewTransitions(ready: Promise<void> = Promise.resolve()) {
    const start = vi.fn((update: () => void | Promise<void>) => ({
      updateCallbackDone: Promise.resolve().then(update),
      ready,
      finished: Promise.resolve(),
    }));
    Object.defineProperty(document, 'startViewTransition', { value: start, configurable: true });
    return start;
  }

  it('goes through document.startViewTransition when it is there', async () => {
    const start = fakeViewTransitions();
    const fn = vi.fn();
    await withViewTransition(fn);
    expect(start).toHaveBeenCalledWith(fn);
    expect(fn).toHaveBeenCalledOnce();
  });

  it('tells the caller when the transition has finished, not before', async () => {
    let finish = () => {};
    const start = vi.fn((update: () => void | Promise<void>) => ({
      updateCallbackDone: Promise.resolve().then(update),
      ready: Promise.resolve(),
      finished: new Promise<void>((resolve) => (finish = resolve)),
    }));
    Object.defineProperty(document, 'startViewTransition', { value: start, configurable: true });
    const finished = vi.fn();
    await withViewTransition(() => {}, finished);
    expect(finished).not.toHaveBeenCalled();
    finish();
    await Promise.resolve();
    await Promise.resolve();
    expect(finished).toHaveBeenCalledOnce();
  });

  it('calls finished after fn when nothing animates, also when fn fails', async () => {
    const finished = vi.fn();
    await expect(
      withViewTransition(() => Promise.reject(new Error('no')), finished),
    ).rejects.toThrow('no');
    expect(finished).toHaveBeenCalledOnce();
  });

  it('calls it as a method of the document, or a browser throws', async () => {
    const start = fakeViewTransitions();
    await withViewTransition(() => {});
    expect(start.mock.contexts[0]).toBe(document);
  });

  it('runs fn directly when the webview has no view transitions', async () => {
    expect(document.startViewTransition).toBeUndefined();
    const fn = vi.fn();
    await withViewTransition(fn);
    expect(fn).toHaveBeenCalledOnce();
  });

  it('runs fn directly under reduced motion, even with view transitions', async () => {
    const start = fakeViewTransitions();
    fakeOsSetting(true);
    const fn = vi.fn();
    await withViewTransition(fn);
    expect(start).not.toHaveBeenCalled();
    expect(fn).toHaveBeenCalledOnce();
  });

  it('uses the transition when the OS does not ask for reduced motion', async () => {
    const start = fakeViewTransitions();
    fakeOsSetting(false);
    await withViewTransition(() => {});
    expect(start).toHaveBeenCalledOnce();
  });

  it('waits for an async fn and passes on its failure, on both paths', async () => {
    const boom = () => Promise.reject(new Error('boom'));
    await expect(withViewTransition(boom)).rejects.toThrow('boom');
    fakeViewTransitions();
    await expect(withViewTransition(boom)).rejects.toThrow('boom');
  });

  it('a skipped transition is not an unhandled rejection', async () => {
    // Vitest fails the run on an unhandled rejection.
    fakeViewTransitions(Promise.reject(new DOMException('skipped', 'AbortError')));
    await expect(withViewTransition(() => {})).resolves.toBeUndefined();
    await new Promise((resolve) => setTimeout(resolve));
  });
});

/** A helper with one child behind a v-if, and the switch for it. */
function mountToggle(helper: Component, props: Record<string, unknown> = {}) {
  const show = ref(false);
  const wrapper = mount(
    defineComponent({
      setup: () => () => h(helper, props, () => (show.value ? h('p', { id: 'child' }) : null)),
    }),
    { global: { stubs: REAL_TRANSITIONS }, attachTo: document.body },
  );
  const child = () => wrapper.find<HTMLElement>('#child').element;
  return { wrapper, show, child };
}

describe('FmFade and FmSlide', () => {
  it.each([
    ['FmFade', FmFade, 'fm-fade'],
    ['FmSlide', FmSlide, 'fm-slide'],
  ] as const)('%s runs a Vue transition with its classes, in and out', async (_, helper, name) => {
    const { wrapper, show, child } = mountToggle(helper);
    show.value = true;
    await nextTick();
    expect(child().classList).toContain(`${name}-enter-active`);
    const el = child();
    show.value = false;
    await nextTick();
    expect(el.classList).toContain(`${name}-leave-active`);
    wrapper.unmount();
  });

  it.each<[string, Component, MotionSpeed | undefined, string]>([
    ['FmFade', FmFade, undefined, 'move'],
    ['FmFade', FmFade, 'fast', 'fast'],
    ['FmSlide', FmSlide, undefined, 'move'],
    ['FmSlide', FmSlide, 'page', 'page'],
  ])('%s at speed %s reads the %s token', async (_, helper, speed, token) => {
    const { wrapper, show, child } = mountToggle(helper, speed ? { speed } : {});
    show.value = true;
    await nextTick();
    expect(child().style.getPropertyValue('--fm-transition-time')).toBe(
      `var(--fm-motion-${token})`,
    );
    wrapper.unmount();
  });

  it.each([
    ['end', 'calc(24px * 1)', '0px'],
    ['start', 'calc(24px * -1)', '0px'],
    ['top', '0px', 'calc(24px * -1)'],
    ['bottom', '0px', 'calc(24px * 1)'],
  ])('FmSlide from %s moves on the right axis', async (from, inline, block) => {
    const { wrapper, show, child } = mountToggle(FmSlide, { from });
    show.value = true;
    await nextTick();
    expect(child().style.getPropertyValue('--fm-slide-inline')).toBe(inline);
    expect(child().style.getPropertyValue('--fm-slide-block')).toBe(block);
    wrapper.unmount();
  });

  it('FmSlide takes the distance the spec names for the moment', async () => {
    const { wrapper, show, child } = mountToggle(FmSlide, { from: 'bottom', distance: '16px' });
    show.value = true;
    await nextTick();
    expect(child().style.getPropertyValue('--fm-slide-block')).toBe('calc(16px * 1)');
    wrapper.unmount();
  });
});

describe('FmFlipGroup', () => {
  it('is a transition group with the fm-flip classes that passes its tag through', async () => {
    const items = ref(['a']);
    const wrapper = mount(
      defineComponent({
        setup: () => () =>
          h(FmFlipGroup, { tag: 'ul' }, () => items.value.map((key) => h('li', { key, id: key }))),
      }),
      { global: { stubs: REAL_TRANSITIONS }, attachTo: document.body },
    );
    expect(wrapper.element.tagName).toBe('UL');
    items.value = ['a', 'b'];
    await nextTick();
    expect(wrapper.find('#b').classes()).toContain('fm-flip-enter-active');
    wrapper.unmount();
  });
});

// Criterion 3: the style blocks are what the helpers generate on the page.
const SOURCES = import.meta.glob<string>('./*.vue', {
  query: '?raw',
  import: 'default',
  eager: true,
});
const styleOf = (file: string) =>
  [...(SOURCES[file] ?? '').matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)]
    .map((m) => m[1]!.replace(/\/\*[\s\S]*?\*\//g, ''))
    .join('\n');

/** The CSS rules of a stylesheet with no nesting, as selector and declarations. */
function rules(css: string): { selector: string; declarations: [string, string][] }[] {
  return [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({
    selector: m[1]!.trim(),
    declarations: m[2]!
      .split(';')
      .map((d) => d.trim())
      .filter(Boolean)
      .map((d) => {
        const at = d.indexOf(':');
        return [d.slice(0, at).trim(), d.slice(at + 1).trim()] as [string, string];
      }),
  }));
}

/** What a stylesheet animates: each `transition` part's property, and whether it animates by keyframes. */
function animated(css: string): { properties: string[]; usesKeyframes: boolean } {
  const properties: string[] = [];
  let usesKeyframes = /@keyframes/.test(css);
  for (const { declarations } of rules(css)) {
    for (const [property, value] of declarations) {
      if (property === 'transition-property') {
        properties.push(...value.split(',').map((v) => v.trim()));
      } else if (property === 'transition') {
        properties.push(...value.split(',').map((part) => part.trim().split(/\s+/)[0]!));
      } else if (property.startsWith('animation')) usesKeyframes = true;
    }
  }
  return { properties, usesKeyframes };
}

/** What the states a transition runs from and to set (`-enter-from`, `-leave-to`). */
const statesSet = (css: string) =>
  rules(css)
    .filter((r) => /-(enter|leave)-(from|to)\b/.test(r.selector))
    .flatMap((r) => r.declarations.map(([property]) => property));

describe('what the helpers animate', () => {
  it('finds what a stylesheet animates, so the checks below can fail', () => {
    const css = `.a { transition: width 1s, opacity 1s; } .b { transition-property: height; }
      .c-enter-from { left: 0; }`;
    expect(animated(css)).toEqual({
      properties: ['width', 'opacity', 'height'],
      usesKeyframes: false,
    });
    expect(statesSet(css)).toEqual(['left']);
    expect(animated('.a { animation: rise 1s; }').usesKeyframes).toBe(true);
  });

  it('every helper is covered', () => {
    expect(Object.keys(SOURCES).sort()).toEqual([
      './FmFade.vue',
      './FmFlipGroup.vue',
      './FmSlide.vue',
    ]);
  });

  it.each([
    ['./FmFade.vue', ['opacity'], ['opacity']],
    ['./FmSlide.vue', ['transform', 'opacity'], ['opacity', 'transform', 'transform']],
    ['./FmFlipGroup.vue', ['transform'], []],
  ])('%s animates only transform and opacity', (file, properties, states) => {
    const css = styleOf(file);
    expect(animated(css)).toEqual({ properties, usesKeyframes: false });
    expect(statesSet(css)).toEqual(states);
  });
});
