<script setup lang="ts">
// A virtualized list of preview rows: only the rows on screen (plus a few
// above and below) exist, so only their faces are loaded. A row that scrolls
// away unmounts and releases its face to the face loader, which unloads it.
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import type { FaceSummary } from '../api/types';
import FontPreviewRow from './FontPreviewRow.vue';

const props = withDefaults(
  defineProps<{
    faces: FaceSummary[];
    rowHeight?: number;
    /** Rows rendered beyond each edge of the viewport. */
    overscan?: number;
  }>(),
  { rowHeight: 64, overscan: 4 },
);

/** Viewport height before the first measurement (and where there is no layout). */
const FALLBACK_HEIGHT = 600;

const viewport = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const height = ref(FALLBACK_HEIGHT);
let observer: ResizeObserver | undefined;

function measure(): void {
  const el = viewport.value;
  if (!el) return;
  scrollTop.value = el.scrollTop;
  if (el.clientHeight > 0) height.value = el.clientHeight;
}

onMounted(() => {
  measure();
  if (typeof ResizeObserver !== 'undefined' && viewport.value) {
    observer = new ResizeObserver(measure);
    observer.observe(viewport.value);
  }
});
onBeforeUnmount(() => observer?.disconnect());

const range = computed(() => {
  const first = Math.floor(scrollTop.value / props.rowHeight) - props.overscan;
  const last = Math.ceil((scrollTop.value + height.value) / props.rowHeight) + props.overscan;
  return {
    start: Math.max(0, first),
    end: Math.min(props.faces.length, Math.max(0, last)),
  };
});

const rows = computed(() =>
  props.faces
    .slice(range.value.start, range.value.end)
    .map((face, i) => ({ face, index: range.value.start + i })),
);
</script>

<template>
  <div
    ref="viewport"
    class="viewport"
    role="region"
    tabindex="0"
    aria-label="Font list"
    @scroll="measure"
  >
    <div
      class="spacer"
      role="list"
      aria-label="Font previews"
      :style="{ height: `${faces.length * rowHeight}px` }"
    >
      <div
        v-for="row in rows"
        :key="row.face.id"
        class="slot"
        role="listitem"
        :aria-setsize="faces.length"
        :aria-posinset="row.index + 1"
        :style="{ height: `${rowHeight}px`, transform: `translateY(${row.index * rowHeight}px)` }"
      >
        <FontPreviewRow :face="row.face" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.viewport {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  contain: strict;
}
.viewport:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: -2px;
}
.spacer {
  position: relative;
}
.slot {
  position: absolute;
  top: 0;
  right: 0;
  left: 0;
}
</style>
