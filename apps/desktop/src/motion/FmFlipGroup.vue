<script setup lang="ts">
/**
 * Children that change place glide to the new place; children that come or go do not
 * animate. Keyed children, as for any `<TransitionGroup>`; other props (`tag`) pass through.
 * Vue measures the move and animates `transform` only, so nothing lays out again per frame.
 *
 * With `glide` set, the group is a plain element and measures its children only on an
 * update made while `glide` is true: a `<TransitionGroup>` reads every child's box on
 * every update, and a scrolled wall updates on each line and thumbnail that comes in
 * (E49.7 scroll budget). The move is the same FLIP: the old box is read before the update,
 * the child starts at it with a transform, and the `fm-flip-move` transition takes it home.
 */
import { onBeforeUpdate, onUpdated, ref } from 'vue';

// The explicit `undefined` default: Vue would turn an absent boolean prop into `false`.
const props = withDefaults(
  defineProps<{
    tag?: string;
    /** When set, children glide only on an update made while it is true. */
    glide?: boolean;
  }>(),
  { tag: undefined, glide: undefined },
);

const MOVE = 'fm-flip-move';
const root = ref<HTMLElement | null>(null);
/** The children's boxes before a gliding update. */
let before: Map<Element, DOMRect> | null = null;

onBeforeUpdate(() => {
  before = null;
  if (!props.glide || !root.value) return;
  before = new Map();
  for (const el of root.value.children) before.set(el, el.getBoundingClientRect());
});

onUpdated(() => {
  const from = before;
  before = null;
  if (!from || !root.value) return;
  // All boxes are read first, then written: a read after a write would lay out again.
  const moved: { el: HTMLElement; dx: number; dy: number }[] = [];
  for (const el of root.value.children) {
    const old = from.get(el);
    if (!old || !(el instanceof HTMLElement)) continue;
    const now = el.getBoundingClientRect();
    const dx = old.left - now.left;
    const dy = old.top - now.top;
    if (dx || dy) moved.push({ el, dx, dy });
  }
  // Without the move class a child has no transform transition: it jumps back at once.
  for (const { el, dx, dy } of moved) {
    el.classList.remove(MOVE);
    el.style.transform = `translate(${dx}px, ${dy}px)`;
  }
  if (!moved.length) return;
  // Starts every child at its old place before the transition takes it home.
  void root.value.offsetHeight;
  for (const { el } of moved) {
    el.classList.add(MOVE);
    el.style.transform = '';
    const done = (e: TransitionEvent) => {
      if (e.target !== el || !e.propertyName.endsWith('transform')) return;
      el.removeEventListener('transitionend', done);
      el.classList.remove(MOVE);
    };
    el.addEventListener('transitionend', done);
  }
});
</script>

<template>
  <TransitionGroup v-if="glide === undefined" name="fm-flip" :tag="tag">
    <slot />
  </TransitionGroup>
  <component :is="tag ?? 'div'" v-else ref="root">
    <slot />
  </component>
</template>

<!-- Not scoped: the class lands on the slotted children, which belong to the parent. -->
<style>
.fm-flip-move {
  transition: transform var(--fm-motion-move) var(--fm-ease);
}
</style>
