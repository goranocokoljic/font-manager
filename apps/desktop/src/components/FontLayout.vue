<script setup lang="ts">
// The layout view mode: one face of the selected family set in a body-text
// layout (a preset's list of text blocks). The face is loaded through the
// shared face loader (asset protocol), so every block is live text in the
// webview, never a rasterized image. Switching presets swaps the blocks; the
// face stays loaded.
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import type { FaceId, FaceSummary, Layout } from '../api/types';
import { faceFamily, useFaceLoader } from '../preview/faceLoader';
import LayoutBlock from './LayoutBlock.vue';

const props = defineProps<{
  /** The layout to show; null while the presets load. */
  layout: Layout | null;
  /** The face to set it in; null when nothing is selected. */
  face: FaceSummary | null;
}>();

type LoadState = 'loading' | 'ready' | 'failed';

const loader = useFaceLoader();
const state = ref<LoadState>('loading');
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
  () => props.face?.id ?? null,
  (id) => hold(id),
  { immediate: true },
);
onBeforeUnmount(() => hold(null));

const fontFamily = computed(() =>
  props.face
    ? `"${faceFamily(props.face.id)}", var(--fm-preview-fallback, system-ui), sans-serif`
    : 'inherit',
);
</script>

<template>
  <section
    class="layout"
    tabindex="0"
    :aria-label="layout ? `${layout.name} layout` : 'Layout'"
    :data-preset="layout?.preset ?? undefined"
    :data-state="face ? state : undefined"
  >
    <template v-if="face && layout">
      <header class="head">
        <span class="name">{{ face.familyName }}</span>
        <span class="style">{{ face.style }}</span>
        <span v-if="state === 'failed'" class="failed" role="status">
          This font could not be loaded.
        </span>
      </header>
      <div class="page">
        <LayoutBlock
          v-for="(block, i) in layout.blocks"
          :key="`${layout.preset ?? layout.name}-${i}`"
          :block="block"
          :font-family="fontFamily"
        />
      </div>
    </template>
    <p v-else-if="!face" class="empty">Select a font to see it in a layout.</p>
  </section>
</template>

<style scoped>
.layout {
  flex: 1;
  min-height: 0;
  overflow: auto;
  background: var(--fm-bg);
  color: var(--fm-fg);
}
.layout:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: -2px;
}
.head {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--fm-border);
  font-size: 12px;
}
.name {
  font-weight: 600;
}
.style {
  color: var(--fm-muted);
}
.failed {
  color: var(--fm-warning);
}
.page {
  max-width: 72em;
  padding: 24px 32px;
}
.empty {
  display: grid;
  height: 100%;
  margin: 0;
  place-items: center;
  color: var(--fm-muted);
}
</style>
