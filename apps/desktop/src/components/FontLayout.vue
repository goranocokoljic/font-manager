<script setup lang="ts">
// The layout view mode: one face of the selected family set in a body-text
// layout (a preset's list of text blocks). The face is loaded through the
// shared face loader (asset protocol), so every block is live text in the
// webview, never a rasterized image. Switching presets swaps the blocks; the
// face stays loaded. With `editable`, each block's text can be edited in place;
// the parent keeps the draft. With `bounds` and `setProps`, a size badge beside each block
// opens its properties editor (E12.3) on click or Enter; Escape closes it and returns
// focus to the badge. With the same two, each block is resized in place (E12.2) by drag,
// pinch or the arrow keys; the result goes through `setProps` like an editor change.
//
// The text is set at `width` CSS px, centered, when the layout's width is locked (E12.4);
// unlocked it follows the window. With `setWidthLock`, the Lock width button (and its
// command palette entry) toggles the lock; locking keeps the width the text has on screen.
// A color palette chosen for the face's family (E16.3) applies through `font-palette`.
//
// With `blockFaces` (Pairing Studio, E24.2), a block can be set in a face of its own; the
// other blocks keep `face`. Each block face is loaded through the same face loader. The
// `block-tools` slot puts the parent's controls for a block beside its size badge.
//
// In print (E24.4) the controls (`fm-no-print`) stay off the paper and the text keeps the
// layout's width: the locked width, or the page's when unlocked.
import { computed, nextTick, onBeforeUnmount, ref, useId, useSlots, watch } from 'vue';
import type { FaceId, FaceSummary, Layout, LayoutBounds, TextBlock } from '../api/types';
import type { BlockProps } from '../stores/textLayouts';
import BlockProperties from './BlockProperties.vue';
import { t } from '../i18n';
import { faceFamily, useFaceLoader } from '../preview/faceLoader';
import { useActions } from '../stores/commands';
import { usePalettesStore } from '../stores/palettes';
import { ALL_FONTS_VIEW } from '../stores/preview';
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
  /** A face of its own per block index; null (or no entry) sets the block in `face`. */
  blockFaces?: readonly (FaceSummary | null)[];
}>();
defineSlots<{
  /** Controls for one block, beside its size badge. */
  'block-tools'?: (props: { index: number; block: TextBlock; label: string }) => unknown;
}>();
const slots = useSlots();
const palettes = usePalettesStore();

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

// Block faces stay loaded while a block uses them; a face two blocks share loads once.
const blockHeld = new Set<FaceId>();
function holdBlocks(ids: FaceId[]): void {
  const wanted = new Set(ids);
  for (const id of blockHeld) {
    if (!wanted.has(id)) {
      blockHeld.delete(id);
      loader.release(id);
    }
  }
  for (const id of wanted) {
    if (blockHeld.has(id)) continue;
    blockHeld.add(id);
    // A face that fails to load falls back like the panel's does; the block still shows.
    loader.acquire(id).catch(() => undefined);
  }
}
watch(
  () => (props.blockFaces ?? []).flatMap((f) => (f ? [f.id] : [])),
  (ids) => holdBlocks(ids),
  { immediate: true },
);
onBeforeUnmount(() => holdBlocks([]));

/** The block whose properties editor is open. */
const editing = ref<number | null>(null);
const badges = ref<HTMLButtonElement[]>([]);
const hasEditor = computed(() => Boolean(props.bounds && props.setProps));
const hasTools = computed(() => Boolean(slots['block-tools']));
const hintId = `${useId()}-hint`;

/** A block resized in place hands its new values to the core like an editor change. */
function resize(i: number, change: BlockProps): void {
  void props.setProps?.(i, change);
}

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
  return t('blocks.sizePt', { size: Number((block.sizePt ?? 0).toFixed(1)) });
}

const blockLabel = (i: number) => t('blocks.label', { n: i + 1 });

const page = ref<HTMLDivElement | null>(null);
const locked = computed(() => props.layout?.widthLocked !== false);
const canLock = computed(() => Boolean(props.setWidthLock && props.layout && props.face));

/** Locks the width the text column has now, or unlocks it. */
async function toggleWidthLock(): Promise<void> {
  if (!props.setWidthLock || !props.layout) return;
  if (locked.value) return props.setWidthLock(false, null);
  const cell = page.value?.querySelector<HTMLElement>('.cell');
  // Floored, so the locked track (same grid as now) fits the row it was measured in.
  const now = cell ? Math.floor(cell.getBoundingClientRect().width) : null;
  return props.setWidthLock(true, now);
}

// Getters: the palette shows the title in the language chosen now.
useActions([
  {
    id: 'view.layoutWidthLock',
    get title() {
      return t('textLayout.actionWidthLock');
    },
    get group() {
      return t('textLayout.groupView');
    },
    enabled: () => canLock.value,
    run: toggleWidthLock,
  },
]);

const pageStyle = computed(() =>
  locked.value && props.width != null ? { '--fm-layout-width': `${props.width}px` } : undefined,
);

// A color palette chosen for the family applies to every block (`font-palette` inherits).
const fontPaletteStyle = computed(() => {
  const value = props.face ? palettes.fontPaletteOf(ALL_FONTS_VIEW, props.face) : undefined;
  return value ? { fontPalette: value } : undefined;
});

const familyOf = (face: FaceSummary) =>
  `"${faceFamily(face.id)}", var(--fm-preview-fallback, system-ui), sans-serif`;
const fontFamily = computed(() => (props.face ? familyOf(props.face) : 'inherit'));

/** The `font-family` of block `i`: its own face, or the layout's. */
function blockFamily(i: number): string {
  const own = props.blockFaces?.[i];
  return own ? familyOf(own) : fontFamily.value;
}
</script>

<template>
  <section
    class="layout"
    tabindex="0"
    :aria-label="
      layout ? t('textLayout.regionNamed', { name: layout.name }) : t('textLayout.region')
    "
    :data-preset="layout?.preset ?? undefined"
    :data-state="face ? state : undefined"
  >
    <template v-if="face && layout">
      <header class="head">
        <span class="name">{{ face.familyName }}</span>
        <span class="style">{{ face.style }}</span>
        <span v-if="state === 'failed'" class="failed" role="status">
          {{ t('textLayout.loadFailed') }}
        </span>
        <button
          v-if="setWidthLock"
          type="button"
          class="lock fm-no-print"
          data-testid="layout-width-lock"
          :aria-pressed="locked"
          :title="locked ? t('textLayout.lockedTitle') : t('textLayout.unlockedTitle')"
          @click="toggleWidthLock"
        >
          {{ t('textLayout.lockWidth') }}
        </button>
      </header>
      <div
        ref="page"
        class="page"
        :class="{ locked: pageStyle !== undefined, 'with-tools': hasTools }"
        data-testid="layout-page"
        :style="[pageStyle, fontPaletteStyle]"
      >
        <div
          v-for="(block, i) in layout.blocks"
          :key="`${layout.preset ?? layout.name}-${i}`"
          class="row"
          :class="{ 'with-badge': hasEditor || hasTools }"
        >
          <div v-if="hasEditor || hasTools" class="tools fm-no-print">
            <button
              v-if="hasEditor"
              ref="badges"
              type="button"
              class="badge"
              data-testid="block-size-badge"
              :data-index="i"
              :aria-label="t('textLayout.badgeLabel', { n: i + 1, size: sizeLabel(block) })"
              aria-haspopup="dialog"
              :aria-expanded="editing === i"
              @click="toggle(i)"
            >
              {{ sizeLabel(block) }}
            </button>
            <slot name="block-tools" :index="i" :block="block" :label="blockLabel(i)" />
          </div>
          <div class="cell">
            <BlockProperties
              v-if="hasEditor && editing === i"
              class="fm-no-print"
              :block="block"
              :bounds="bounds!"
              :label="blockLabel(i)"
              :apply="(change) => setProps!(i, change)"
              @close="close(i)"
            />
            <LayoutBlock
              :block="block"
              :font-family="blockFamily(i)"
              :editable="editable"
              :label="blockLabel(i)"
              :bounds="hasEditor ? bounds : null"
              :describedby="hasEditor ? hintId : undefined"
              @edit="(text) => emit('edit-text', i, text)"
              @change="(change) => resize(i, change)"
            />
          </div>
        </div>
      </div>
      <p v-if="hasEditor" :id="hintId" class="hint fm-no-print">
        {{ t('textLayout.hint') }}
        <template v-if="editable">{{ t('textLayout.hintEdit') }}</template>
      </p>
    </template>
    <p v-else-if="!face" class="empty">{{ t('textLayout.empty') }}</p>
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
  --fm-tools-width: 5em;
  padding: 24px 32px;
}
/* Room for the parent's block controls (a font picker) beside the badge. */
.page.with-tools {
  --fm-tools-width: 11em;
}
.row.with-badge {
  display: grid;
  /* An empty column opposite the badges keeps the text centered and gives the locked and
     unlocked grids the same tracks, so locking keeps the width the text has on screen. */
  grid-template-columns: var(--fm-tools-width) minmax(0, 1fr) var(--fm-tools-width);
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
  grid-template-columns:
    var(--fm-tools-width) minmax(0, var(--fm-layout-width))
    var(--fm-tools-width);
  justify-content: center;
}
.lock {
  margin-inline-start: auto;
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
.tools {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 4px;
  min-width: 0;
  margin-top: 2px;
}
.badge {
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
.hint {
  margin: 0;
  padding: 0 32px 16px;
  color: var(--fm-muted);
  font-size: 12px;
}
@media print {
  .layout {
    overflow: visible;
    background: none;
  }
  /* No badge column on paper: each row is its text, at the layout's width. */
  .page {
    padding: 0;
  }
  .row.with-badge {
    display: block;
  }
  .page.locked .row,
  .page.locked .row.with-badge {
    max-width: min(var(--fm-layout-width), 100%);
    margin-inline: auto;
  }
}
.empty {
  display: grid;
  height: 100%;
  margin: 0;
  place-items: center;
  color: var(--fm-muted);
}
</style>
