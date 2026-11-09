<script setup lang="ts">
// The drag ghost (E44.2, board `4-drag.png`, motion table "Drag to a collection"): a copy of
// the card the drag started on, lifted to 0.9 scale and tilted −3° with a shadow over
// `--fm-motion-fast`, following the pointer. More than one dragged row puts a count badge on
// its corner and "+ 2 more" under it. A drop flies it into the target's row over
// `--fm-motion-move` (shrinking and fading as it goes). Under reduced motion it follows
// without the tilt and the drop is a cut. Outside the window the webview shows no ghost.
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { formatNumber, t } from '../i18n';
import { useReducedMotion } from '../motion';
import { useFontDragStore } from '../stores/fontDrag';

const drag = useFontDragStore();
const reduced = useReducedMotion();

const body = ref<HTMLElement | null>(null);
const liftEl = ref<HTMLElement | null>(null);
const flightEl = ref<HTMLElement | null>(null);
const flyBody = ref<HTMLElement | null>(null);
/** False for the first frame, so the lift animates from the card's own size. */
const lifted = ref(false);
/** The flight has started (its end state is applied). */
const flying = ref(false);

const shown = computed(() => drag.active && drag.source !== null && drag.pointer !== null);
const more = computed(() => drag.count - 1);

/** Where the ghost's top-left corner goes: the pointer, minus where it took the card. */
const place = computed(() => {
  const p = drag.pointer;
  const s = drag.source;
  if (!p || !s) return {};
  return {
    width: `${s.width}px`,
    height: `${s.height}px`,
    transform: `translate(${p.x - s.grab.x}px, ${p.y - s.grab.y}px)`,
    transformOrigin: `${s.grab.x}px ${s.grab.y}px`,
  };
});

function mount(into: HTMLElement | null, el: HTMLElement | undefined): void {
  if (!into || !el) return;
  into.replaceChildren(el);
}

watch(
  () => [shown.value, drag.source] as const,
  async ([on, source]) => {
    lifted.value = false;
    if (!on || !source) return;
    await nextTick();
    mount(body.value, source.el);
    // The card's own size must be the computed style before the lift is set, or both land
    // in one style pass and nothing lifts (E46.4: the lift is 120 ms). Frames alone do not
    // promise that on a slow machine, so read the style now: that fixes the start state.
    if (liftEl.value) void getComputedStyle(liftEl.value).transform;
    requestAnimationFrame(() => requestAnimationFrame(() => (lifted.value = true)));
  },
);

/** The flight's start and end, both as a transform of the ghost box. */
const flightStyle = computed(() => {
  const f = drag.flight;
  if (!f) return {};
  const { from, to, source } = f;
  const start = { x: from.x - source.grab.x, y: from.y - source.grab.y };
  // The ghost's centre ends on the target's centre, at a tenth of its size.
  const end = {
    x: to.left + to.width / 2 - source.width / 2,
    y: to.top + to.height / 2 - source.height / 2,
  };
  const at = flying.value ? end : start;
  return {
    width: `${source.width}px`,
    height: `${source.height}px`,
    transform: `translate(${at.x}px, ${at.y}px)`,
  };
});

let landTimer: ReturnType<typeof setTimeout> | undefined;
watch(
  () => drag.flight,
  async (f) => {
    flying.value = false;
    clearTimeout(landTimer);
    if (!f) return;
    if (reduced.value) {
      drag.landed();
      return;
    }
    await nextTick();
    mount(flyBody.value, f.source.el);
    // The start state is the computed style before the end state is set (as for the lift).
    if (flightEl.value) void getComputedStyle(flightEl.value).transform;
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        if (drag.flight !== f) return;
        flying.value = true;
        // The move token is 200 ms; land even when no `transitionend` comes. Counted from
        // the start of the flight: two slow frames must not land it before it flies.
        landTimer = setTimeout(() => drag.landed(), 400);
      }),
    );
  },
);

function onFlightEnd(e: TransitionEvent): void {
  if (e.target === e.currentTarget && e.propertyName === 'transform' && flying.value) drag.landed();
}
onBeforeUnmount(() => clearTimeout(landTimer));
</script>

<template>
  <Teleport to="body">
    <div
      v-if="shown"
      class="ghost"
      data-testid="drag-ghost"
      :data-count="drag.count"
      aria-hidden="true"
      :style="place"
    >
      <div
        ref="liftEl"
        class="lift"
        :class="{ lifted, still: reduced }"
        data-testid="drag-ghost-lift"
      >
        <div ref="body" class="body" />
        <span v-if="drag.count > 1" class="badge" data-testid="drag-ghost-badge">{{
          formatNumber(drag.count)
        }}</span>
        <span v-if="more > 0" class="more">{{ t('drag.more', { count: more }) }}</span>
      </div>
    </div>
    <div
      v-if="drag.flight"
      ref="flightEl"
      class="ghost flight"
      data-testid="drag-ghost-flight"
      aria-hidden="true"
      :style="flightStyle"
      @transitionend="onFlightEnd"
    >
      <div class="lift lifted" :class="{ gone: flying, still: reduced }">
        <div ref="flyBody" class="body" />
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.ghost {
  position: fixed;
  top: 0;
  left: 0;
  z-index: 1000;
  pointer-events: none;
}
/* The copied card keeps its own styles; none of it may take the pointer, or the drag would
   be over the ghost instead of the target under it. */
.ghost :deep(*) {
  pointer-events: none !important;
}
.flight {
  transition: transform var(--fm-motion-move) var(--fm-ease);
}
.lift {
  position: relative;
  width: 100%;
  height: 100%;
  border-radius: var(--fm-radius-lg);
  transform-origin: inherit;
  transition:
    transform var(--fm-motion-fast) var(--fm-ease),
    box-shadow var(--fm-motion-fast) var(--fm-ease),
    opacity var(--fm-motion-move) var(--fm-ease);
}
.lifted {
  box-shadow: var(--fm-shadow-2);
  transform: scale(0.9) rotate(-3deg);
}
/* Reduced motion: the ghost follows without the tilt. */
.lifted.still {
  transform: scale(0.9);
}
.gone {
  opacity: 0;
  transform: scale(0.1) rotate(-3deg);
}
.body {
  width: 100%;
  height: 100%;
  overflow: hidden;
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-bg);
}
/* The copied card or row keeps its look but not its place in the list. */
.body > :deep(*) {
  position: static !important;
  width: 100% !important;
  height: 100% !important;
  margin: 0 !important;
  transform: none !important;
  opacity: 1 !important;
}
.badge {
  position: absolute;
  top: calc(var(--fm-space-2) * -1);
  inset-inline-end: calc(var(--fm-space-2) * -1);
  min-width: 24px;
  padding: 2px var(--fm-space-2);
  border-radius: 999px;
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
  font-size: var(--fm-text-sm);
  font-weight: 600;
  text-align: center;
}
.more {
  position: absolute;
  bottom: var(--fm-space-2);
  inset-inline-end: var(--fm-space-3);
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
</style>
