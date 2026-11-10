<script setup lang="ts">
// A grid card's text as live webview text in the face itself, for a face drawn in a
// chosen color palette (E16.3), or with tracking or as outlines (E16.1): the card's
// thumbnail is a one-color mask, so it cannot show any of them. The face is held
// through the shared face loader while the card shows it. With `defer` (the grid
// moves fast, E35.8) a card that does not hold its face yet starts no load and shows
// its empty frame (`data-state="deferred"`) until `defer` is off. The characters in
// `missing` are drawn as tofu (E43.4), never in a fallback font.
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import type { FaceId } from '../api/types';
import { faceFamily, useFaceLoader } from '../preview/faceLoader';
import SpecimenText from './SpecimenText';

const props = defineProps<{
  face: FaceId;
  text: string;
  /** Characters of `text` the face lacks: drawn as tofu. */
  missing?: readonly string[];
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

/** Checks after the first measure; each takes at least 1 % off, so this is enough. */
const MAX_REFITS = 8;
let refits = 0;

/** The text's exact width: `offsetWidth` rounds, and half a pixel is enough to cut it. */
const lineWidth = () => line.value?.getBoundingClientRect().width ?? 0;

// Measured once the text has its face. Without layout (no width to measure) it stays 1.
watch(
  () => [props.text, props.sizePx, props.fitWidth, props.trackingEm, state.value] as const,
  () => {
    refits = 0;
    // A deferred card draws no text, a loading one draws it in the fallback: nothing worth
    // measuring until the face is in (or failed). Each read forces a layout, and a fast
    // scroll mounts a line of cards every few frames (E49.7 scroll budget).
    if (state.value === 'deferred' || state.value === 'loading') {
      fit.value = 1;
      return;
    }
    const width = lineWidth();
    if (!props.fitWidth || !width) {
      fit.value = 1;
      return;
    }
    // The width is the text at the size it has now: take that size out first.
    fit.value = Math.min(1, (props.fitWidth * fit.value) / width);
  },
  { flush: 'post', immediate: true },
);

// Text is not always in proportion to its size: without subpixel positioning (Chromium on
// Linux) each glyph's advance is a whole pixel. So the new size is measured too, and
// shrinks until the text fits.
watch(
  fit,
  () => {
    const width = lineWidth();
    if (!props.fitWidth || width <= props.fitWidth || refits >= MAX_REFITS) return;
    refits += 1;
    fit.value *= Math.min(0.99, props.fitWidth / width);
  },
  { flush: 'post' },
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
      ><SpecimenText v-if="state !== 'deferred'" :text="text" :missing="missing ?? []"
    /></span>
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
