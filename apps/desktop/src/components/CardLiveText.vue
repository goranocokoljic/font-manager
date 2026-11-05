<script setup lang="ts">
// A grid card's text as live webview text in the face itself, for a face drawn in a
// chosen color palette (E16.3), or with tracking or as outlines (E16.1): the card's
// thumbnail is a one-color mask, so it cannot show any of them. The face is held
// through the shared face loader while the card shows it. With `defer` (the grid
// moves fast, E35.8) a card that does not hold its face yet starts no load and shows
// its empty frame (`data-state="deferred"`) until `defer` is off.
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import type { FaceId } from '../api/types';
import { faceFamily, useFaceLoader } from '../preview/faceLoader';

const props = defineProps<{
  face: FaceId;
  text: string;
  /** The CSS `font-palette` to draw in; the face's default when absent. */
  palette?: string;
  sizePx: number;
  /** Letter spacing in em. */
  trackingEm?: number;
  /** Draws the text as outlines (stroke, no fill). */
  outline?: boolean;
  color?: string | null;
  /** Starts no load for a face not held yet (the grid moves fast). */
  defer?: boolean;
  /** The width the text must fit, in CSS px; wider text shrinks so it is never cut. */
  fitWidth?: number;
  /** Wraps the text and shows at most this many whole lines (a wrapped preset, E41.2). */
  wrapLines?: number;
  /** The line height of wrapped text. */
  lineHeight?: number;
}>();

const loader = useFaceLoader();
const state = ref<'deferred' | 'loading' | 'ready' | 'failed'>('loading');
let held: FaceId | null = null;

function hold(face: FaceId | null): void {
  if (held === face) return;
  if (held !== null) loader.release(held);
  held = face;
  if (face === null) return;
  state.value = 'loading';
  loader.acquire(face).then(
    () => {
      if (held === face) state.value = 'ready';
    },
    () => {
      if (held === face) state.value = 'failed';
    },
  );
}

watch(
  () => [props.face, props.defer] as const,
  ([id, defer]) => {
    if (held === id) return;
    if (!defer) return hold(id);
    hold(null);
    state.value = 'deferred';
  },
  { immediate: true },
);
onBeforeUnmount(() => hold(null));

const line = ref<HTMLElement | null>(null);
/** The text's size as a share of `sizePx`: below 1 when the text would not fit. */
const fit = ref(1);

// Measured once the text has its face. Without layout (no width to measure) it stays 1.
watch(
  () => [props.text, props.sizePx, props.fitWidth, props.trackingEm, state.value] as const,
  () => {
    const el = line.value;
    if (!el || !props.fitWidth || !el.offsetWidth) {
      fit.value = 1;
      return;
    }
    // offsetWidth is the text at the size it has now: take that size out first.
    fit.value = Math.min(1, (props.fitWidth * fit.value) / el.offsetWidth);
  },
  { flush: 'post', immediate: true },
);

const style = computed(() => ({
  fontFamily: `"${faceFamily(props.face)}", var(--fm-preview-fallback, system-ui), sans-serif`,
  fontSize: `${props.sizePx * fit.value}px`,
  fontPalette: props.palette,
  letterSpacing: props.trackingEm ? `${props.trackingEm}em` : undefined,
  color: props.color ?? undefined,
  lineHeight: props.lineHeight,
  '--fm-wrap-lines': props.wrapLines,
}));
</script>

<template>
  <div
    class="live"
    :class="{ outline, wrap: wrapLines !== undefined }"
    dir="auto"
    data-testid="card-live-text"
    :data-state="state"
    :style="style"
  >
    <span ref="line" class="line"
      ><template v-if="state !== 'deferred'">{{ text }}</template></span
    >
  </div>
</template>

<style scoped>
.live {
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: center;
  min-height: 0;
  overflow: hidden;
  line-height: 1.2;
  white-space: nowrap;
}
.line {
  flex: none;
}
.live.wrap {
  justify-content: flex-start;
  white-space: normal;
}
.wrap .line {
  display: -webkit-box;
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  /* A run wider than the card (the capitals at a big size) wraps too, or its end is cut. */
  overflow-wrap: anywhere;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: var(--fm-wrap-lines);
}
.live.outline {
  -webkit-text-fill-color: transparent;
  -webkit-text-stroke: 1px currentColor;
}
</style>
