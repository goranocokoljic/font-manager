<script setup lang="ts">
// A grid card's text as live webview text in the face itself, for a face drawn in a
// chosen color palette (E16.3), or with tracking or as outlines (E16.1): the card's
// thumbnail is a one-color mask, so it cannot show any of them. The face is held
// through the shared face loader while the card shows it.
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
}>();

const loader = useFaceLoader();
const state = ref<'loading' | 'ready' | 'failed'>('loading');
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
  () => props.face,
  (id) => hold(id),
  { immediate: true },
);
onBeforeUnmount(() => hold(null));

const style = computed(() => ({
  fontFamily: `"${faceFamily(props.face)}", var(--fm-preview-fallback, system-ui), sans-serif`,
  fontSize: `${props.sizePx}px`,
  fontPalette: props.palette,
  letterSpacing: props.trackingEm ? `${props.trackingEm}em` : undefined,
  color: props.color ?? undefined,
}));
</script>

<template>
  <div
    class="live"
    :class="{ outline }"
    dir="auto"
    data-testid="card-live-text"
    :data-state="state"
    :style="style"
  >
    {{ text }}
  </div>
</template>

<style scoped>
.live {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  line-height: 1.2;
  white-space: nowrap;
}
.live.outline {
  -webkit-text-fill-color: transparent;
  -webkit-text-stroke: 1px currentColor;
}
</style>
