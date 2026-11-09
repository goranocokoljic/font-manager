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
    const all = () => selectors.every((s) => document.querySelector(s));
    if (all()) return void report();
    const observer = new MutationObserver(() => {
      if (!all()) return;
      observer.disconnect();
      void report();
    });
    observer.observe(document.body, { childList: true, subtree: true });
  });
}

export const enterSeen = () => watched.fmEnter;
