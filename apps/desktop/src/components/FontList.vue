<script setup lang="ts">
// The font list: families, each expandable to its styles inline, as a
// virtualized tree. Only the rows in the viewport (plus `overscan` above and
// below) exist, and they are keyed by slot, so scrolling reuses the same row
// elements (row recycling) and only the faces on screen are loaded.
//
// Focus stays on the tree; the cursor row is its `aria-activedescendant`, so
// the list is one Tab stop. Keys: arrows, Home/End, PageUp/PageDown move;
// Right/Left expand/collapse; Enter toggles a family or opens a style in the
// inspector; Shift extends the selection, Ctrl/Cmd moves without selecting,
// Ctrl/Cmd+Space toggles a row, Ctrl/Cmd+A selects all. Plain Space is the
// app's quick-look key, so the list leaves it alone.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, watchEffect } from 'vue';
import type { FaceId } from '../api/types';
import { poolSize, scrollToShow, windowRange } from '../list/virtual';
import type { ListRow } from '../list/rows';
import type { SelectMode } from '../list/selection';
import { sampleText } from '../preview/sampleText';
import { useFontListStore } from '../stores/fontList';
import { useLibraryStore } from '../stores/library';
import { DEFAULT_PREVIEW, previewRowHeight, type PreviewSettings } from '../stores/preview';
import FontPreviewRow from './FontPreviewRow.vue';

const props = withDefaults(
  defineProps<{
    preview?: PreviewSettings;
    /** Fixed row height; derived from `preview` when absent. */
    rowHeight?: number;
    /** Rows rendered beyond each edge of the viewport. */
    overscan?: number;
  }>(),
  { preview: () => ({ ...DEFAULT_PREVIEW }), rowHeight: undefined, overscan: 4 },
);

const emit = defineEmits<{
  /** The face the cursor shows changed (a style, or a family's first style). */
  cursorFace: [face: FaceId];
  /** Enter on a style: show it in the inspector. */
  open: [face: FaceId];
}>();

const list = useFontListStore();
const library = useLibraryStore();

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

const count = computed(() => list.rows.length);
const range = computed(() =>
  windowRange(scrollTop.value, height.value, rowPx.value, props.overscan, count.value),
);
const pool = computed(() => poolSize(height.value, rowPx.value, props.overscan));

const rowId = (row: ListRow) => `fm-row-${row.key.replace(':', '-')}`;

const visible = computed(() =>
  list.rows.slice(range.value.start, range.value.end).map((row, i) => {
    const index = range.value.start + i;
    const face = row.kind === 'face' ? row.face : library.faces.get(row.family.id)?.[0];
    return {
      row,
      index,
      slot: index % pool.value,
      id: rowId(row),
      face,
      text: face ? sampleText(props.preview, face) : '',
    };
  }),
);

// Family rows preview their first face: fetch the faces of the rendered rows and
// of one pool beyond each edge, so rows scrolled in already have them and their
// preview row is patched in place, not swapped for a placeholder.
watchEffect(() => {
  const { start, end } = range.value;
  const ahead = pool.value;
  for (const row of list.rows.slice(Math.max(0, start - ahead), end + ahead)) {
    const id = row.family.id;
    if (row.kind === 'family' && !library.faces.has(id) && !library.failed.has(id))
      void library.facesOf(id).catch(() => undefined);
  }
});

const activeId = computed(() => {
  const row = list.cursorRow;
  const i = list.cursorIndex;
  return row && i >= range.value.start && i < range.value.end ? rowId(row) : undefined;
});

watch(
  () => list.cursorFace?.id,
  (face) => {
    if (face !== undefined) emit('cursorFace', face);
  },
);

/** Scrolls so the cursor row is on screen (and so rendered for aria-activedescendant). */
async function revealCursor(): Promise<void> {
  const el = viewport.value;
  const i = list.cursorIndex;
  if (!el || i < 0) return;
  const top = scrollToShow(i, rowPx.value, el.scrollTop, height.value);
  if (top !== el.scrollTop) el.scrollTop = top;
  measure();
  await nextTick();
}

function modeOf(e: KeyboardEvent | MouseEvent): SelectMode {
  if (e.shiftKey) return 'extend';
  if (e.ctrlKey || e.metaKey) return e instanceof KeyboardEvent ? 'keep' : 'toggle';
  return 'replace';
}

function onKeyDown(e: KeyboardEvent): void {
  if (e.altKey) return;
  const mod = e.ctrlKey || e.metaKey;
  const page = Math.max(1, Math.floor(height.value / rowPx.value));
  const row = list.cursorRow;
  switch (e.key) {
    case 'ArrowDown':
      list.moveBy(1, modeOf(e));
      break;
    case 'ArrowUp':
      list.moveBy(-1, modeOf(e));
      break;
    case 'PageDown':
      list.moveBy(page, modeOf(e));
      break;
    case 'PageUp':
      list.moveBy(-page, modeOf(e));
      break;
    case 'Home':
      list.moveCursor(0, modeOf(e));
      break;
    case 'End':
      list.moveCursor(count.value - 1, modeOf(e));
      break;
    case 'ArrowRight':
      if (mod || e.shiftKey) return;
      list.expandOrEnter();
      break;
    case 'ArrowLeft':
      if (mod || e.shiftKey) return;
      list.collapseOrLeave();
      break;
    case 'Enter':
      if (mod || e.shiftKey || !row) return;
      if (row.kind === 'family') list.toggleExpanded(row.family.id);
      else emit('open', row.face.id);
      break;
    case ' ':
      if (!mod) return;
      list.toggleCursor();
      break;
    default:
      if (mod && !e.shiftKey && e.key.toLowerCase() === 'a') list.selectAll();
      else return;
  }
  e.preventDefault();
  void revealCursor();
}

function onRowClick(row: ListRow, e: MouseEvent): void {
  list.click(row.key, modeOf(e));
  viewport.value?.focus({ preventScroll: true });
}

function onToggle(row: ListRow): void {
  if (row.kind === 'family') list.toggleExpanded(row.family.id);
  viewport.value?.focus({ preventScroll: true });
}

const styleCount = (n: number) => (n === 1 ? '1 style' : `${n} styles`);
</script>

<template>
  <div
    ref="viewport"
    class="viewport"
    :class="surfaceClass"
    :style="viewportStyle"
    :data-surface="preview.surface"
    role="tree"
    tabindex="0"
    aria-label="Font list"
    aria-multiselectable="true"
    :aria-activedescendant="activeId"
    @scroll="measure"
    @keydown="onKeyDown"
  >
    <div class="spacer" role="none" :style="{ height: `${count * rowPx}px` }">
      <div
        v-for="item in visible"
        :id="item.id"
        :key="item.slot"
        class="slot"
        :class="{
          face: item.row.kind === 'face',
          selected: list.isSelected(item.row.key),
          cursor: item.id === activeId,
        }"
        role="treeitem"
        :data-key="item.row.key"
        :aria-level="item.row.kind === 'face' ? 2 : 1"
        :aria-expanded="item.row.kind === 'family' ? item.row.expanded : undefined"
        :aria-selected="list.isSelected(item.row.key)"
        :aria-setsize="item.row.kind === 'face' ? item.row.count : library.families.length"
        :aria-posinset="item.row.position"
        :aria-label="
          item.row.kind === 'face'
            ? item.row.face.style
            : `${item.row.family.name}, ${styleCount(item.row.family.styleCount)}`
        "
        :style="{ height: `${rowPx}px`, transform: `translateY(${item.index * rowPx}px)` }"
        @click="onRowClick(item.row, $event)"
      >
        <span
          v-if="item.row.kind === 'family'"
          class="twisty"
          aria-hidden="true"
          @click.stop="onToggle(item.row)"
          >{{ item.row.expanded ? '▾' : '▸' }}</span
        >
        <FontPreviewRow
          v-if="item.face"
          class="preview"
          :face="item.face"
          :label="item.row.kind === 'face' ? item.row.face.style : item.row.family.name"
          :text="item.text"
          :size-px="preview.sizePx"
          :line-height="preview.lineHeight"
          :tracking-em="preview.trackingEm"
          :align="preview.align"
          :color="preview.textColor"
        >
          <template v-if="item.row.kind === 'family'" #detail>
            <span class="count">{{ styleCount(item.row.family.styleCount) }}</span>
          </template>
        </FontPreviewRow>
        <div v-else class="preview placeholder">
          <span class="label">{{ item.row.family.name }}</span>
          <span v-if="library.failed.has(item.row.family.id)" class="note">
            Styles not available
          </span>
        </div>
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
  display: flex;
  align-items: stretch;
  box-sizing: border-box;
  border-bottom: 1px solid var(--fm-border);
  user-select: none;
  contain: layout paint;
}
.slot.face {
  padding-left: 24px;
}
.slot.selected {
  background: color-mix(in srgb, var(--fm-accent) 16%, transparent);
}
/* The tint alone is under 3:1; a solid accent bar marks every selected row (WCAG 1.4.11). */
.slot.selected::before {
  position: absolute;
  top: 0;
  bottom: 0;
  left: 0;
  width: 4px;
  background: var(--fm-accent);
  content: '';
}
.viewport:focus .slot.cursor {
  box-shadow: inset 0 0 0 2px var(--fm-accent);
}
.twisty {
  display: grid;
  flex: none;
  place-items: center;
  width: 24px;
  color: var(--fm-muted);
  font-size: 13px;
  cursor: pointer;
}
.preview {
  flex: 1;
  min-width: 0;
}
/* The slot draws the row line, full width; the preview row sits inside it. */
.slot .preview {
  border-bottom: 0;
}
.slot:not(.face) .preview {
  padding-left: 0;
}
.placeholder {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 2px;
  box-sizing: border-box;
  padding-right: 16px;
  color: var(--fm-muted);
  font-size: 11.5px;
}
.count {
  color: var(--fm-muted);
}
.note {
  color: var(--fm-warning);
}
</style>
