<script setup lang="ts">
// A virtualized list of preview rows: only the rows on screen (plus a few
// above and below) exist, so only their faces are loaded. A row that scrolls
// away unmounts and releases its face to the face loader, which unloads it.
// The view's preview settings apply to every row; the row height follows the
// sample size and line height, so all rows stay the same height.
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import type { FaceSummary } from '../api/types';
import { sampleText } from '../preview/sampleText';
import { DEFAULT_PREVIEW, previewRowHeight, type PreviewSettings } from '../stores/preview';
import FontPreviewRow from './FontPreviewRow.vue';

const props = withDefaults(
  defineProps<{
    faces: FaceSummary[];
    preview?: PreviewSettings;
    /** Fixed row height; derived from `preview` when absent. */
    rowHeight?: number;
    /** Rows rendered beyond each edge of the viewport. */
    overscan?: number;
  }>(),
  { preview: () => ({ ...DEFAULT_PREVIEW }), rowHeight: undefined, overscan: 4 },
);

const rowPx = computed(() => props.rowHeight ?? previewRowHeight(props.preview));

const surfaceClass = computed(() =>
  props.preview.surface === 'auto' ? undefined : `surface-${props.preview.surface}`,
);
const viewportStyle = computed(() => ({
  backgroundColor: props.preview.backgroundColor ?? undefined,
}));

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
  const first = Math.floor(scrollTop.value / rowPx.value) - props.overscan;
  const last = Math.ceil((scrollTop.value + height.value) / rowPx.value) + props.overscan;
  return {
    start: Math.max(0, first),
    end: Math.min(props.faces.length, Math.max(0, last)),
  };
});

const rows = computed(() =>
  props.faces.slice(range.value.start, range.value.end).map((face, i) => ({
    face,
    index: range.value.start + i,
    text: sampleText(props.preview, face),
  })),
);
</script>

<template>
  <div
    ref="viewport"
    class="viewport"
    :class="surfaceClass"
    :style="viewportStyle"
    :data-surface="preview.surface"
    role="region"
    tabindex="0"
    aria-label="Font list"
    @scroll="measure"
  >
    <div
      class="spacer"
      role="list"
      aria-label="Font previews"
      :style="{ height: `${faces.length * rowPx}px` }"
    >
      <div
        v-for="row in rows"
        :key="row.face.id"
        class="slot"
        role="listitem"
        :aria-setsize="faces.length"
        :aria-posinset="row.index + 1"
        :style="{ height: `${rowPx}px`, transform: `translateY(${row.index * rowPx}px)` }"
      >
        <FontPreviewRow
          :face="row.face"
          :text="row.text"
          :size-px="preview.sizePx"
          :line-height="preview.lineHeight"
          :tracking-em="preview.trackingEm"
          :align="preview.align"
          :color="preview.textColor"
        />
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
  background: var(--fm-bg);
  color: var(--fm-fg);
}
/* A preview surface other than `auto` overrides the theme inside the list only. */
.surface-light {
  color-scheme: light;
  --fm-bg: #ffffff;
  --fm-fg: #1a1a1a;
  --fm-muted: #595959;
  --fm-border: #d4d4d4;
  --fm-accent: #0a5fd6;
  --fm-warning: #8a4b00;
}
.surface-dark {
  color-scheme: dark;
  --fm-bg: #1c1c1e;
  --fm-fg: #ececec;
  --fm-muted: #a6a6a6;
  --fm-border: #3a3a3c;
  --fm-accent: #6ea8ff;
  --fm-warning: #ffb74d;
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
