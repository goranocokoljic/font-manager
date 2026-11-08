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
//
// A layout with a page (a Pairing Studio context, E45.1) is set on a sheet of the page's size in
// CSS pixels, at 100 %, on a neutral ground with a soft shadow; the text keeps the page's inner
// width (`width`, from the core) and the width lock has nothing to do. Safe-area guides are drawn
// as dashed frames. A block's controls sit outside the sheet's edge and show while the pointer
// or the focus is on the block, so the page reads like the page.
//
// With `roleMarks` (the Studio's roles, E45.2), a block with a role carries no mark at rest; on
// hover its role's name shows in the margin and a hairline ring around it. A press or the focus
// selects it (`select`); the `selected` block keeps its mark and gets the accent ring.
import { computed, nextTick, onBeforeUnmount, ref, useId, useSlots, watch, watchEffect } from 'vue';
import type { FaceId, FaceSummary, Layout, LayoutBounds, TextBlock } from '../api/types';
import type { BlockProps } from '../stores/textLayouts';
import BlockProperties from './BlockProperties.vue';
import UiIconButton from './ui/UiIconButton.vue';
import { t } from '../i18n';
import { faceFamily, useFaceLoader } from '../preview/faceLoader';
import { useActions } from '../stores/commands';
import { usePalettesStore } from '../stores/palettes';
import { ALL_FONTS_VIEW } from '../stores/preview';
import { useSamplesStore } from '../stores/samples';
import { specimenOf } from '../preview/tofu';
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
  /** The parent already names the face and explains the gestures: no name line, the lock floats
   *  on the page and the hint is for screen readers only. */
  bare?: boolean;
  /** Each block's role name (E45.2), shown in the margin on hover; null for a block without. */
  roleMarks?: readonly (string | null)[];
  /** The selected block (E45.2): its mark stays and it has the accent ring. */
  selected?: number | null;
}>();
defineSlots<{
  /** Controls for one block, beside its size badge. */
  'block-tools'?: (props: { index: number; block: TextBlock; label: string }) => unknown;
}>();
const slots = useSlots();
const palettes = usePalettesStore();

const emit = defineEmits<{
  'edit-text': [index: number, text: string];
  /** The face has loaded or failed, so the text has its final height. */
  settled: [];
  /** Block `index` was pressed or got the focus (with `roleMarks`). */
  select: [index: number];
}>();

type LoadState = 'loading' | 'ready' | 'failed';

const loader = useFaceLoader();
const state = ref<LoadState>('loading');
let held: FaceId | null = null;

function settle(face: FaceId, result: LoadState): void {
  if (held !== face) return;
  state.value = result;
  // After the render of the new state, when the text has its height.
  void nextTick(() => emit('settled'));
}

function hold(face: FaceId | null): void {
  if (held === face) return;
  if (held !== null) loader.release(held);
  held = face;
  if (face === null) return;
  state.value = 'loading';
  loader.acquire(face).then(
    () => settle(face, 'ready'),
    () => settle(face, 'failed'),
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
const hasTools = computed(() => Boolean(slots['block-tools'] || props.roleMarks));

/** Block `i` is selected, when the parent shows roles. */
function selectBlock(i: number): void {
  if (props.roleMarks) emit('select', i);
}
// The gesture sentence sits behind the info button (Studio has its own).
const helpOpen = ref(false);
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
/** The page the layout is set on, if it has one. */
const sheet = computed(() => props.layout?.page ?? null);
/** The sheet at its real size: CSS pixels at 100 %. */
const SCALE = 100;
const sheetStyle = computed(() => {
  const p = sheet.value;
  if (!p) return undefined;
  return {
    width: `${p.widthPx}px`,
    minHeight: `${p.heightPx}px`,
    padding: `${p.marginPx ?? 0}px`,
    '--sheet-margin': `${p.marginPx ?? 0}px`,
  };
});
/** One dashed frame per safe area, inset from every edge by its share of the page. */
const safeAreas = computed(() => {
  const p = sheet.value;
  if (!p) return [];
  return (p.safeAreas ?? []).map((share) => ({
    inset: `${(share ?? 0) * p.heightPx}px ${(share ?? 0) * p.widthPx}px`,
  }));
});
const locked = computed(() => sheet.value !== null || props.layout?.widthLocked !== false);
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

/** The face block `i` is set in: its own, or the layout's. */
const blockFace = (i: number): FaceSummary | null => props.blockFaces?.[i] ?? props.face;

const samples = useSamplesStore();
watchEffect(() => {
  props.layout?.blocks.forEach((b, i) => {
    const face = blockFace(i);
    if (face) samples.want(face.id, b.text);
  });
});
/** The characters of block `i`'s text its face lacks: drawn as tofu (E43.4). */
function blockTofu(i: number, text: string): string[] {
  const face = blockFace(i);
  return face ? specimenOf(text, samples.get(face.id, text), true).tofu : [];
}

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
    :class="{ paged: sheet !== null }"
    :aria-label="
      layout ? t('textLayout.regionNamed', { name: layout.name }) : t('textLayout.region')
    "
    :data-preset="layout?.preset ?? undefined"
    :data-state="face ? state : undefined"
    :data-family="face?.family"
  >
    <template v-if="face && layout">
      <header
        v-if="!(sheet && bare) || state === 'failed'"
        class="head"
        :class="{ bare, locked: pageStyle !== undefined, 'with-tools': hasTools }"
        :style="pageStyle"
      >
        <template v-if="!bare">
          <span class="name">{{ face.familyName }}</span>
          <span class="style">{{ face.style }}</span>
        </template>
        <span v-if="state === 'failed'" class="failed" role="status">
          {{ t('textLayout.loadFailed') }}
        </span>
        <span v-if="!bare" class="spacer" />
        <UiIconButton
          v-if="hasEditor && !bare"
          icon="info"
          :label="t('textLayout.help')"
          class="fm-no-print"
          :aria-expanded="helpOpen"
          :aria-controls="hintId"
          @click="helpOpen = !helpOpen"
        />
        <UiIconButton
          v-if="setWidthLock && !sheet"
          icon="width"
          :label="t('textLayout.lockWidth')"
          class="fm-no-print"
          data-testid="layout-width-lock"
          :aria-pressed="locked"
          :title="locked ? t('textLayout.lockedTitle') : t('textLayout.unlockedTitle')"
          @click="toggleWidthLock"
        />
      </header>
      <p
        v-if="hasEditor"
        :id="hintId"
        class="hint fm-no-print"
        :class="{ 'sr-only': bare || !helpOpen }"
      >
        {{ t('textLayout.hint') }}
        <template v-if="editable">{{ t('textLayout.hintEdit') }}</template>
      </p>
      <p
        v-if="sheet"
        class="sheet-label fm-no-print"
        data-testid="layout-sheet-label"
        :style="{ width: `${sheet.widthPx}px` }"
      >
        {{ t('studio.pageScale', { page: sheet.name, scale: String(SCALE) }) }}
      </p>
      <div
        ref="page"
        class="page"
        :class="{ locked: pageStyle !== undefined, 'with-tools': hasTools, sheet: sheet !== null }"
        data-testid="layout-page"
        :data-page-width="sheet?.widthPx"
        :data-page-height="sheet?.heightPx"
        :style="[pageStyle, fontPaletteStyle, sheetStyle]"
      >
        <div
          v-for="(area, i) in safeAreas"
          :key="`safe-${i}`"
          class="safe-area fm-no-print"
          data-testid="layout-safe-area"
          aria-hidden="true"
          :style="area"
        />
        <div
          v-for="(block, i) in layout.blocks"
          :key="`${layout.preset ?? layout.name}-${i}`"
          class="row"
          :class="{
            'with-badge': hasEditor || hasTools,
            'has-role': Boolean(roleMarks?.[i]),
            selected: selected === i,
          }"
          :data-selected="selected === i ? '' : undefined"
          @pointerdown="selectBlock(i)"
          @focusin="selectBlock(i)"
        >
          <div v-if="hasEditor || hasTools" class="tools fm-no-print">
            <span
              v-if="roleMarks?.[i]"
              class="role-mark"
              data-testid="block-role-mark"
              aria-hidden="true"
            >
              {{ roleMarks[i] }}
            </span>
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
              :missing="blockTofu(i, block.text)"
              :editable="editable"
              :label="
                roleMarks?.[i]
                  ? t('studio.roleBlock', { role: roleMarks[i]!, label: blockLabel(i) })
                  : blockLabel(i)
              "
              :bounds="hasEditor ? bounds : null"
              :describedby="hasEditor ? hintId : undefined"
              @edit="(text) => emit('edit-text', i, text)"
              @change="(change) => resize(i, change)"
            />
          </div>
        </div>
      </div>
      <p
        v-if="sheet && bare"
        class="sheet-hint fm-no-print"
        data-testid="layout-sheet-hint"
        :style="{ width: `${sheet.widthPx}px` }"
      >
        {{ t('studio.hint') }}
      </p>
    </template>
    <p v-else-if="!face" class="empty">{{ t('textLayout.empty') }}</p>
  </section>
</template>

<style scoped>
.layout {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: auto;
  background-color: var(--fm-bg);
  color: var(--fm-fg);
}
.layout:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: -2px;
}
.head {
  display: flex;
  align-items: baseline;
  gap: var(--fm-space-2);
  padding: var(--fm-space-2) var(--fm-space-4);
  border-bottom: 1px solid var(--fm-border);
  font-size: var(--fm-text-sm);
}
.head.bare {
  position: sticky;
  inset-block-start: 0;
  z-index: var(--fm-z-sticky);
  justify-content: flex-end;
  height: 0;
  padding: var(--fm-space-2) var(--fm-space-4) 0;
  border-bottom: 0;
  overflow: visible;
}
/* Locked in the Studio: the icon sits at the right edge of the centred page, not the window. */
/* Same tracks as `.page.locked .row.with-badge` below; the end pad skips the right tools
   column, so the icon ends where the text column does. */
.head.bare.locked {
  max-width: calc(
    var(--fm-layout-width) + 2 * var(--fm-tools-width) + 2 * var(--fm-space-2) + 2 *
      var(--fm-space-8)
  );
  margin-inline: auto;
  padding-inline: var(--fm-space-8)
    calc(var(--fm-space-8) + var(--fm-tools-width) + var(--fm-space-2));
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
.page,
.head.bare.locked {
  --fm-tools-width: 5em;
}
.page {
  padding: var(--fm-space-6) var(--fm-space-8);
}
/* Room for the parent's block controls (a font picker) beside the badge. */
.page.with-tools,
.head.bare.locked.with-tools {
  --fm-tools-width: 11em;
}
.row.with-badge {
  display: grid;
  /* An empty column opposite the badges keeps the text centered and gives the locked and
     unlocked grids the same tracks, so locking keeps the width the text has on screen. */
  grid-template-columns: var(--fm-tools-width) minmax(0, 1fr) var(--fm-tools-width);
  gap: var(--fm-space-2);
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
.spacer {
  flex: 1;
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
  border: 1px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-sm);
  background-color: var(--fm-bg);
  color: var(--fm-muted);
  font: var(--fm-text-xs)/1.4 var(--fm-font-ui);
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
  padding: var(--fm-space-2) var(--fm-space-4) 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
@media print {
  .layout,
  .layout.paged {
    overflow: visible;
    padding: 0;
    background: none;
  }
  .page.sheet {
    box-shadow: none;
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
/* A page (E45.1): a sheet at its real size on the neutral ground. The ground keeps room for the
   block controls left of the sheet; a sheet wider than the window scrolls, never clips. */
.layout.paged {
  padding: var(--fm-space-6) calc(var(--fm-tools-width-sheet) + var(--fm-space-6)) var(--fm-space-8);
  background-color: var(--fm-studio-ground);
  --fm-tools-width-sheet: 11em;
}
.sheet-label,
.sheet-hint {
  max-width: none;
  margin: 0 auto;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.sheet-label {
  padding-bottom: var(--fm-space-2);
  text-align: end;
  font-variant-numeric: tabular-nums;
}
.sheet-hint {
  padding-top: var(--fm-space-4);
  text-align: center;
}
.page.sheet {
  position: relative;
  box-sizing: border-box;
  margin-inline: auto;
  background-color: var(--fm-bg);
  box-shadow: var(--fm-shadow-2);
}
/* The text keeps the page's inner width; no tool columns inside the page. */
.page.sheet .row,
.page.sheet .row.with-badge {
  position: relative;
  display: block;
  max-width: none;
  margin-inline: 0;
}
/* The block's controls hang outside the sheet's left edge and show on hover or focus. */
.page.sheet .tools {
  position: absolute;
  inset-block-start: 0;
  inset-inline-end: 100%;
  width: var(--fm-tools-width-sheet);
  margin-inline-end: calc(var(--sheet-margin, 0px) + var(--fm-space-4));
  opacity: 0;
  transition: opacity var(--fm-motion-fast) var(--fm-ease);
}
.page.sheet .row:hover .tools,
.page.sheet .row:focus-within .tools,
.page.sheet .tools:has([aria-expanded='true']) {
  opacity: 1;
}
/* Roles (E45.2): no mark at rest; hover shows the mark and a hairline ring, selection the
   accent ring, and the mark stays. */
.role-mark {
  color: var(--fm-muted);
  font: 600 var(--fm-text-xs)/1.4 var(--fm-font-ui);
  white-space: nowrap;
}
.row.selected .role-mark {
  color: var(--fm-fg);
}
.page.sheet .row.has-role :deep(.block) {
  outline: 1px solid transparent;
  outline-offset: 4px;
  transition: outline-color var(--fm-motion-fast) var(--fm-ease);
}
.page.sheet .row.has-role:hover :deep(.block) {
  outline-color: var(--fm-border-strong);
}
.page.sheet .row.selected :deep(.block),
.page.sheet .row.selected:hover :deep(.block) {
  outline: 2px solid var(--fm-accent);
  outline-offset: 4px;
}
.page.sheet .row.selected .tools {
  opacity: 1;
}
.safe-area {
  position: absolute;
  border: 1px dashed var(--fm-border-strong);
  pointer-events: none;
}
.empty {
  display: grid;
  height: 100%;
  margin: 0;
  place-items: center;
  color: var(--fm-muted);
}
</style>
