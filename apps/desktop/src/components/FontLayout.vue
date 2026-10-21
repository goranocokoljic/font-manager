<script setup lang="ts">
// The layout view mode: one face of the selected family set in a body-text
// layout (a preset's list of text blocks). The face is loaded through the
// shared face loader (asset protocol), so every block is live text in the
// webview, never a rasterized image. Switching presets swaps the blocks; the
// face stays loaded. With `editable`, each block's text can be edited in place;
// the parent keeps the draft. With `bounds` and `setProps`, a size badge beside each block
// opens its properties editor (E12.3) on click or Enter; Escape closes it and returns
// focus to the badge.
//
// The text is set at `width` CSS px, centered, when the layout's width is locked (E12.4);
// unlocked it follows the window. With `setWidthLock`, the Lock width button (and its
// command palette entry) toggles the lock; locking keeps the width the text has on screen.
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import type { FaceId, FaceSummary, Layout, LayoutBounds, TextBlock } from '../api/types';
import type { BlockProps } from '../stores/textLayouts';
import BlockProperties from './BlockProperties.vue';
import { faceFamily, useFaceLoader } from '../preview/faceLoader';
import { useActions } from '../stores/commands';
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
  /** The width in CSS px the text is set at; null follows the window. */
  width?: number | null;
  /** Locks the width (at `widthPx`, the text's width on screen now) or unlocks it. */
  setWidthLock?: (locked: boolean, widthPx: number | null) => Promise<void>;
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

const page = ref<HTMLDivElement | null>(null);
const locked = computed(() => props.layout?.widthLocked !== false);
const canLock = computed(() => Boolean(props.setWidthLock && props.layout && props.face));

/** Locks the width the text column has now, or unlocks it. */
async function toggleWidthLock(): Promise<void> {
  if (!props.setWidthLock || !props.layout) return;
  if (locked.value) return props.setWidthLock(false, null);
  const cell = page.value?.querySelector<HTMLElement>('.cell');
  const now = cell ? Math.round(cell.getBoundingClientRect().width) : null;
  return props.setWidthLock(true, now);
}

useActions([
  {
    id: 'view.layoutWidthLock',
    title: 'Lock or unlock the layout width',
    group: 'View',
    enabled: () => canLock.value,
    run: toggleWidthLock,
  },
]);

const pageStyle = computed(() =>
  locked.value && props.width != null ? { '--fm-layout-width': `${props.width}px` } : undefined,
);

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
        <button
          v-if="setWidthLock"
          type="button"
          class="lock"
          data-testid="layout-width-lock"
          :aria-pressed="locked"
          :title="
            locked
              ? 'The text keeps its width; press to follow the window'
              : 'The text follows the window; press to keep its width'
          "
          @click="toggleWidthLock"
        >
          Lock width
        </button>
      </header>
      <div
        ref="page"
        class="page"
        :class="{ locked: pageStyle !== undefined }"
        data-testid="layout-page"
        :style="pageStyle"
      >
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
  padding: 24px 32px;
}
.row.with-badge {
  display: grid;
  grid-template-columns: 5em minmax(0, 1fr);
  gap: 8px;
  align-items: start;
}
/* Locked: the text column keeps its width and is centered in the window. */
.page.locked .row {
  max-width: var(--fm-layout-width);
  margin-inline: auto;
}
.page.locked .row.with-badge {
  max-width: none;
  grid-template-columns: 5em minmax(0, var(--fm-layout-width)) 5em;
  justify-content: center;
}
.lock {
  margin-left: auto;
  padding: 1px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: 3px;
  background: var(--fm-bg);
  color: var(--fm-fg);
  font:
    11px/1.4 system-ui,
    sans-serif;
  cursor: pointer;
}
.lock[aria-pressed='true'] {
  border-color: var(--fm-accent);
  color: var(--fm-accent);
}
.lock:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
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
