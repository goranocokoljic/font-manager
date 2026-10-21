<script setup lang="ts">
// The layout view mode: one face of the selected family set in a body-text
// layout (a preset's list of text blocks). The face is loaded through the
// shared face loader (asset protocol), so every block is live text in the
// webview, never a rasterized image. Switching presets swaps the blocks; the
// face stays loaded. With `editable`, each block's text can be edited in place;
// the parent keeps the draft. With `bounds` and `setProps`, a size badge beside each block
// opens its properties editor (E12.3) on click or Enter; Escape closes it and returns
// focus to the badge.
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import type { FaceId, FaceSummary, Layout, LayoutBounds, TextBlock } from '../api/types';
import type { BlockProps } from '../stores/textLayouts';
import BlockProperties from './BlockProperties.vue';
import { faceFamily, useFaceLoader } from '../preview/faceLoader';
import LayoutBlock from './LayoutBlock.vue';

const props = defineProps<{
  /** The layout to show; null while the presets load. */
  layout: Layout | null;
  /** The face to set it in; null when nothing is selected. */
  face: FaceSummary | null;
  /** The blocks' text can be edited. */
  editable?: boolean;
  /** The core's block ranges; with `setProps`, each block gets a properties editor. */
  bounds?: LayoutBounds | null;
  /** Changes one block's properties; answers the block as the core clamped it. */
  setProps?: (index: number, change: BlockProps) => Promise<TextBlock | null>;
}>();

const emit = defineEmits<{ 'edit-text': [index: number, text: string] }>();

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

/** The block whose properties editor is open. */
const editing = ref<number | null>(null);
const badges = ref<HTMLButtonElement[]>([]);
const hasEditor = computed(() => Boolean(props.bounds && props.setProps));

watch(
  () => props.layout,
  () => (editing.value = null),
);

function toggle(i: number): void {
  editing.value = editing.value === i ? null : i;
}

function close(i: number): void {
  editing.value = null;
  void nextTick(() => badges.value.find((b) => b.dataset.index === String(i))?.focus());
}

/** "12 pt", without float noise. */
function sizeLabel(block: TextBlock): string {
  return `${Number((block.sizePt ?? 0).toFixed(1))} pt`;
}

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
        <div
          v-for="(block, i) in layout.blocks"
          :key="`${layout.preset ?? layout.name}-${i}`"
          class="row"
          :class="{ 'with-badge': hasEditor }"
        >
          <button
            v-if="hasEditor"
            ref="badges"
            type="button"
            class="badge"
            data-testid="block-size-badge"
            :data-index="i"
            :aria-label="`Block ${i + 1} properties, ${sizeLabel(block)}`"
            aria-haspopup="dialog"
            :aria-expanded="editing === i"
            @click="toggle(i)"
          >
            {{ sizeLabel(block) }}
          </button>
          <div class="cell">
            <BlockProperties
              v-if="hasEditor && editing === i"
              :block="block"
              :bounds="bounds!"
              :label="`Block ${i + 1}`"
              :apply="(change) => setProps!(i, change)"
              @close="close(i)"
            />
            <LayoutBlock
              :block="block"
              :font-family="fontFamily"
              :editable="editable"
              :label="`Block ${i + 1}`"
              @edit="(text) => emit('edit-text', i, text)"
            />
          </div>
        </div>
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
.row.with-badge {
  display: grid;
  grid-template-columns: 5em minmax(0, 1fr);
  gap: 8px;
  align-items: start;
}
.badge {
  justify-self: end;
  margin-top: 2px;
  padding: 1px 6px;
  border: 1px solid var(--fm-control-border);
  border-radius: 3px;
  background: var(--fm-bg);
  color: var(--fm-muted);
  font:
    11px/1.4 system-ui,
    sans-serif;
  white-space: nowrap;
  cursor: pointer;
}
.badge[aria-expanded='true'] {
  color: var(--fm-fg);
  border-color: var(--fm-accent);
}
.badge:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
.empty {
  display: grid;
  height: 100%;
  margin: 0;
  place-items: center;
  color: var(--fm-muted);
}
</style>
