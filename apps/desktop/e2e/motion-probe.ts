// Loaded into the page by motion.spec.ts (the dev server serves it): the real helpers,
// mounted on their own, so the browser computes what they generate.
import { createApp, h, ref, type Component } from 'vue';
import { FmFade, FmFlipGroup, FmSlide, withViewTransition } from '../src/motion';

/** The running CSS transitions, as the browser reports them. */
function transitions() {
  return document
    .getAnimations()
    .filter((a): a is CSSTransition => a instanceof CSSTransition)
    .filter((a) => (a.effect as KeyframeEffect).target?.closest('[data-probe]'))
    .map((a) => {
      const timing = a.effect!.getComputedTiming();
      const from = (a.effect as KeyframeEffect).getKeyframes()[0]!;
      return {
        property: a.transitionProperty,
        duration: timing.duration,
        easing: timing.easing,
        from: a.transitionProperty === 'transform' ? String(from.transform) : String(from.opacity),
      };
    })
    .sort((a, b) => a.property.localeCompare(b.property));
}

const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));

/** Mounts `helper` with one hidden child, shows it, and reports the transitions of the enter. */
async function enter(helper: Component, props: Record<string, unknown>) {
  const show = ref(false);
  const host = document.createElement('div');
  host.dataset.probe = '';
  document.body.append(host);
  const app = createApp({
    render: () => h(helper, props, () => (show.value ? h('p', 'child') : null)),
  });
  app.mount(host);
  show.value = true;
  // Vue swaps enter-from for enter-to on the next frame; the transition starts then.
  await frame();
  await frame();
  await frame();
  const seen = transitions();
  app.unmount();
  host.remove();
  return seen;
}

export const enterFade = (props: Record<string, unknown>) => enter(FmFade, props);
export const enterSlide = (props: Record<string, unknown>) => enter(FmSlide, props);

/** Mounts two keyed blocks in a FlipGroup, swaps them, and reports the transitions of the move. */
export async function swapInFlipGroup() {
  const items = ref(['a', 'b']);
  const host = document.createElement('div');
  host.dataset.probe = '';
  document.body.append(host);
  const app = createApp({
    render: () =>
      h(FmFlipGroup, { tag: 'div' }, () =>
        items.value.map((key) => h('p', { key, style: { height: '40px', margin: 0 } }, key)),
      ),
  });
  app.mount(host);
  items.value = ['b', 'a'];
  await frame();
  await frame();
  await frame();
  const seen = transitions();
  app.unmount();
  host.remove();
  return seen;
}

/** Runs a DOM change through withViewTransition; whether it ran, and whether a transition did. */
export async function changeInViewTransition() {
  const realStart = document.startViewTransition;
  let started = false;
  document.startViewTransition = function (update) {
    started = true;
    return realStart.call(document, update);
  } as typeof document.startViewTransition;
  let ran = false;
  await withViewTransition(() => {
    ran = true;
  });
  document.startViewTransition = realStart;
  return { ran, started };
}

/** Mounts the real UiPopover over a button, opens it, and reports the transitions of the enter. */
export async function enterPopover() {
  const { default: UiPopover } = await import('../src/components/ui/UiPopover.vue');
  const host = document.createElement('div');
  host.dataset.probe = '';
  const anchor = document.createElement('button');
  anchor.textContent = 'anchor';
  host.append(anchor);
  document.body.append(host);
  const open = ref(false);
  const app = createApp({
    render: () => h(UiPopover, { open: open.value, anchor }, () => h('p', 'menu')),
  });
  const mountAt = document.createElement('div');
  host.append(mountAt);
  app.mount(mountAt);
  open.value = true;
  await frame();
  await frame();
  await frame();
  const seen = transitionsOf('[data-ui-popover]');
  app.unmount();
  host.remove();
  return seen;
}

/** The running CSS transitions on the element `selector` finds, as the browser reports them. */
function transitionsOf(selector: string) {
  return document
    .getAnimations()
    .filter((a): a is CSSTransition => a instanceof CSSTransition)
    .filter((a) => (a.effect as KeyframeEffect).target?.matches(selector))
    .map((a) => {
      const timing = a.effect!.getComputedTiming();
      const from = (a.effect as KeyframeEffect).getKeyframes()[0]!;
      return {
        property: a.transitionProperty,
        duration: timing.duration,
        easing: timing.easing,
        from: a.transitionProperty === 'transform' ? String(from.transform) : String(from.opacity),
      };
    })
    .sort((a, b) => a.property.localeCompare(b.property));
}

type Seen = ReturnType<typeof transitionsOf>;
const watched = window as unknown as { fmEnter?: Promise<Record<string, Seen>> };

/**
 * Starts watching for every one of `selectors` to appear in the page (the app shows them after
 * a user action the test does next); `enterSeen` then reports the transitions of their enter.
 */
export function watchEnter(selectors: string[]) {
  watched.fmEnter = new Promise((resolve) => {
    const report = async () => {
      await frame();
      await frame();
      await frame();
      resolve(Object.fromEntries(selectors.map((s) => [s, transitionsOf(s)])));
    };
    // Shown, not only in the page: a v-show panel is there while hidden.
    const shown = (s: string) => {
      const el = document.querySelector(s);
      return !!el && el.getClientRects().length > 0;
    };
    const all = () => selectors.every(shown);
    if (all()) return void report();
    const observer = new MutationObserver(() => {
      if (!all()) return;
      observer.disconnect();
      void report();
    });
    observer.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ['style', 'class'],
    });
  });
}

export const enterSeen = () => watched.fmEnter;

/** One transition or animation that started in the page while the recorder ran (E46.4). */
export interface Moved {
  /** `transition` or `animation`. */
  kind: string;
  /** The `data-testid` of the element or its nearest marked ancestor, else ''. */
  testid: string;
  /** The element's classes. */
  classes: string;
  /** `::before` for a pseudo-element, else ''. */
  pseudo: string;
  /** The transitioned property, or the animation's name. */
  property: string;
  duration: number;
  easing: string;
  /** The first keyframe's transform and opacity, as the engine computed them. */
  from: { transform?: string; opacity?: string };
}

const recorder = window as unknown as {
  fmMoved?: Moved[];
  fmStop?: () => void;
};

function movedOf(a: Animation, kind: string): Moved {
  const effect = a.effect as KeyframeEffect;
  const el = effect.target as HTMLElement;
  const timing = effect.getComputedTiming();
  const first = effect.getKeyframes()[0] ?? {};
  return {
    kind,
    testid: (el.closest('[data-testid]') as HTMLElement | null)?.dataset.testid ?? '',
    classes: typeof el.className === 'string' ? el.className : '',
    pseudo: effect.pseudoElement ?? '',
    property: a instanceof CSSTransition ? a.transitionProperty : (a as CSSAnimation).animationName,
    duration: Number(timing.duration),
    // An animation's curve is its keyframes' (`animation-timing-function`), not the effect's.
    easing: String(a instanceof CSSTransition ? timing.easing : (first.easing ?? timing.easing)),
    from: {
      ...(first.transform !== undefined && { transform: String(first.transform) }),
      ...(first.opacity !== undefined && { opacity: String(first.opacity) }),
    },
  };
}

/** A time list entry of a computed style (`0.12s`, `200ms`) in ms. */
function ms(value: string | undefined): number {
  const m = /^\s*([\d.]+)(ms|s)\s*$/.exec(value ?? '');
  if (!m) return 0;
  return m[2] === 's' ? Number(m[1]) * 1000 : Number(m[1]);
}

/** Splits a computed list (`opacity, transform`) at its top-level commas. */
function list(value: string): string[] {
  const out: string[] = [];
  let depth = 0;
  let part = '';
  for (const ch of value) {
    if (ch === '(') depth++;
    if (ch === ')') depth--;
    if (ch === ',' && depth === 0) {
      out.push(part.trim());
      part = '';
    } else part += ch;
  }
  out.push(part.trim());
  return out;
}

/**
 * A move read from the element's computed style, for a transition or an animation whose event
 * came but whose Animation had already ended (a slow frame on a busy runner): what the
 * stylesheet gave it, without its first keyframe.
 */
function movedFromStyle(
  kind: 'transition' | 'animation',
  el: HTMLElement,
  pseudo: string,
  property: string,
): Moved {
  const style = getComputedStyle(el, pseudo || null);
  const names = list(kind === 'transition' ? style.transitionProperty : style.animationName);
  const durations = list(
    kind === 'transition' ? style.transitionDuration : style.animationDuration,
  );
  const easings = list(
    kind === 'transition' ? style.transitionTimingFunction : style.animationTimingFunction,
  );
  let i = names.indexOf(property);
  if (i < 0) i = names.indexOf('all');
  if (i < 0) i = 0;
  return {
    kind,
    testid: (el.closest('[data-testid]') as HTMLElement | null)?.dataset.testid ?? '',
    classes: typeof el.className === 'string' ? el.className : '',
    pseudo,
    property,
    duration: ms(durations[i % durations.length]),
    easing: easings[i % easings.length] ?? '',
    from: {},
  };
}

/** Waits until nothing runs in the page (at most `limit` ms), so a recording starts clean. */
async function settled(limit = 3000) {
  const end = performance.now() + limit;
  while (performance.now() < end) {
    const running = document
      .getAnimations()
      .some(
        (a) =>
          a.playState === 'running' &&
          Number.isFinite(Number(a.effect?.getComputedTiming().endTime)),
      );
    if (!running) return;
    await frame();
  }
}

/**
 * Records every CSS transition and animation that starts in the page from now on, until
 * `recorded()` is read. What the engine runs, not what a stylesheet says: a transition the
 * page never starts is not there. It first waits for what still runs to end (an earlier
 * step's motion is not this step's). On a slow runner a 120 ms transition can be over before
 * its `transitionrun` is read; then the move comes from the element's computed style.
 */
export async function recordMotion() {
  recorder.fmStop?.();
  await settled();
  const seen = new Set<Animation>();
  const moved: Moved[] = [];
  recorder.fmMoved = moved;
  /** Per element, per `pseudo|property`: events that came and moves recorded. */
  const counts = new WeakMap<Element, Map<string, { events: number; recorded: number }>>();
  const countOf = (el: Element, key: string) => {
    let byKey = counts.get(el);
    if (!byKey) counts.set(el, (byKey = new Map()));
    let c = byKey.get(key);
    if (!c) byKey.set(key, (c = { events: 0, recorded: 0 }));
    return c;
  };
  const take = () => {
    for (const a of document.getAnimations()) {
      if (seen.has(a)) continue;
      let m: Moved;
      if (a instanceof CSSTransition) m = movedOf(a, 'transition');
      else if (a instanceof CSSAnimation) m = movedOf(a, 'animation');
      else continue;
      seen.add(a);
      moved.push(m);
      const target = (a.effect as KeyframeEffect).target;
      if (target) countOf(target, `${m.pseudo}|${m.property}`).recorded++;
    }
  };
  const onEvent = (e: Event) => {
    take();
    const el = e.target;
    if (!(el instanceof HTMLElement)) return;
    const transition = e instanceof TransitionEvent;
    const pseudo = (e as TransitionEvent | AnimationEvent).pseudoElement ?? '';
    const property = transition
      ? (e as TransitionEvent).propertyName
      : (e as AnimationEvent).animationName;
    const c = countOf(el, `${pseudo}|${property}`);
    c.events++;
    if (c.recorded < c.events) {
      moved.push(movedFromStyle(transition ? 'transition' : 'animation', el, pseudo, property));
      c.recorded++;
    }
  };
  // A transition or animation can end before the next frame: read them as they start.
  document.addEventListener('transitionrun', onEvent, true);
  document.addEventListener('animationstart', onEvent, true);
  let next = requestAnimationFrame(function tick() {
    take();
    next = requestAnimationFrame(tick);
  });
  recorder.fmStop = () => {
    document.removeEventListener('transitionrun', onEvent, true);
    document.removeEventListener('animationstart', onEvent, true);
    cancelAnimationFrame(next);
    recorder.fmStop = undefined;
  };
}

/** What `recordMotion()` saw; the recorder stops. */
export async function recorded() {
  await frame();
  await settled();
  recorder.fmStop?.();
  return recorder.fmMoved ?? [];
}
