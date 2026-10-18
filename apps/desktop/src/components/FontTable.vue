<script setup lang="ts">
// The detail list (table mode): one row per family, expandable to its styles,
// with the spec's columns. It shares the font list's rows, cursor, selection
// and keyboard model (the fontList store, `list/keys.ts`) and its row
// virtualization (`list/virtual.ts`: fixed-height rows keyed by slot).
//
// Headers sort (ascending, descending, default) and can be dragged to reorder;
// the Columns menu (TableColumnsMenu) hides and moves them by keyboard. The
// order of the families comes from the API (`ListFamiliesRequest.order`).
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, watchEffect } from 'vue';
import type { FaceId } from '../api/types';
import {
  ACTIVATION_LABEL,
  ariaSort,
  dateText,
  formatBadges,
  licenseBadge,
  visibleColumns,
  weightPosition,
  weightRangeText,
  type Column,
  type ColumnId,
} from '../list/columns';
import { dragKeys, startFontDrag } from '../list/drag';
import { handleListKey, modeOf } from '../list/keys';
import { rowLabel, type ListRow } from '../list/rows';
import { poolSize, scrollToShow, windowRange } from '../list/virtual';
import { useFontListStore } from '../stores/fontList';
import { useLibraryStore } from '../stores/library';
import { useViewsStore } from '../stores/views';

const props = withDefaults(
  defineProps<{
    /** The view whose table setup (columns, sort) this table shows. */
    view: string;
    /** Rows rendered beyond each edge of the viewport. */
    overscan?: number;
  }>(),
  { overscan: 6 },
);

const emit = defineEmits<{
  /** The face the cursor shows changed (a style, or a family's first style). */
  cursorFace: [face: FaceId];
  /** Enter on a style: show it in the inspector. */
  open: [face: FaceId];
}>();

/** Row and header height in CSS px. */
const ROW_PX = 32;
const HEADER_PX = 32;
/** Viewport height before the first measurement (and where there is no layout). */
const FALLBACK_HEIGHT = 600;

const list = useFontListStore();
const library = useLibraryStore();
const views = useViewsStore();

const config = computed(() => views.tableFor(props.view));
const columns = computed(() => visibleColumns(config.value));
const gridTemplate = computed(() =>
  columns.value
    .map((c) => (c.id === 'name' ? `minmax(${c.width}px, 1fr)` : `${c.width}px`))
    .join(' '),
);
const minWidth = computed(() => columns.value.reduce((n, c) => n + c.width, 0));

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

/** The rows' part of the viewport: the sticky header covers the top. */
const bodyHeight = computed(() => Math.max(ROW_PX, height.value - HEADER_PX));
const count = computed(() => list.rows.length);
const range = computed(() =>
  windowRange(scrollTop.value, bodyHeight.value, ROW_PX, props.overscan, count.value),
);
const pool = computed(() => poolSize(bodyHeight.value, ROW_PX, props.overscan));

const rowId = (row: ListRow) => `fm-trow-${row.key.replace(':', '-')}`;

const visible = computed(() =>
  list.rows.slice(range.value.start, range.value.end).map((row, i) => {
    const index = range.value.start + i;
    return { row, index, slot: index % pool.value, id: rowId(row) };
  }),
);

// The cursor family's faces show in the preview strip and the inspector.
watchEffect(() => {
  const row = list.cursorRow;
  const id = row?.family.id;
  if (
    row?.kind === 'family' &&
    id !== undefined &&
    !library.faces.has(id) &&
    !library.failed.has(id)
  )
    void library.facesOf(id).catch(() => undefined);
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
  { immediate: true },
);

/** Scrolls so the cursor row is on screen (and so rendered for aria-activedescendant). */
async function revealCursor(): Promise<void> {
  const el = viewport.value;
  const i = list.cursorIndex;
  if (!el || i < 0) return;
  const top = scrollToShow(i, ROW_PX, el.scrollTop, bodyHeight.value);
  if (top !== el.scrollTop) el.scrollTop = top;
  measure();
  await nextTick();
}

function onKeyDown(e: KeyboardEvent): void {
  if (e.target !== viewport.value) return; // keys on a header button are the button's
  const page = Math.max(1, Math.floor(bodyHeight.value / ROW_PX));
  if (!handleListKey(e, list, { page, count: count.value, open: (face) => emit('open', face) }))
    return;
  e.preventDefault();
  void revealCursor();
}

/** Dragging a row onto a collection adds it, or the whole selection when the row is in it. */
function onRowDrag(row: ListRow, e: DragEvent): void {
  startFontDrag(e, dragKeys(list.selection.selected, row.key));
}

function onRowClick(row: ListRow, e: MouseEvent): void {
  list.click(row.key, modeOf(e));
  viewport.value?.focus({ preventScroll: true });
}

function onToggle(row: ListRow): void {
  if (row.kind === 'family') list.toggleExpanded(row.family.id);
  viewport.value?.focus({ preventScroll: true });
}

// ---- Header: sort and drag to reorder. ----

const dragging = ref<ColumnId | null>(null);
const dropTarget = ref<ColumnId | null>(null);

function onDragStart(col: Column, e: DragEvent): void {
  dragging.value = col.id;
  e.dataTransfer?.setData('text/plain', col.id);
  if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
}

function onDragOver(col: Column, e: DragEvent): void {
  if (!dragging.value || dragging.value === col.id) return;
  e.preventDefault();
  dropTarget.value = col.id;
}

function onDrop(col: Column, e: DragEvent): void {
  e.preventDefault();
  const from = dragging.value;
  if (from && from !== col.id)
    views.moveColumn(props.view, from, config.value.order.indexOf(col.id));
  onDragEnd();
}

function onDragEnd(): void {
  dragging.value = null;
  dropTarget.value = null;
}

const SORT_ARROW = { ascending: '▲', descending: '▼', none: '' } as const;

// ---- Cells. ----

interface Cell {
  /** Plain text of the cell (also its accessible text). */
  text: string;
  badges?: string[];
  /** Weight line dots: position in percent, italic dots are hollow. */
  dots?: { left: number; italic: boolean }[];
  /** Weight line span of a variable family, in percent. */
  span?: { left: number; width: number };
}

function cellOf(row: ListRow, id: ColumnId): Cell {
  if (row.kind === 'face') {
    const f = row.face;
    switch (id) {
      case 'activation':
        return { text: f.active ? 'Active' : 'Inactive' };
      case 'name':
        return { text: f.style };
      case 'weightLine':
        return {
          text: String(f.weight),
          dots: [{ left: weightPosition(f.weight), italic: f.slant !== 'upright' }],
        };
      case 'weightRange':
        return { text: String(f.weight) };
      case 'format':
        return { text: '', badges: formatBadges([f.format], f.isVariable) };
      default:
        return { text: '' };
    }
  }
  const fam = row.family;
  switch (id) {
    case 'activation':
      return { text: ACTIVATION_LABEL[fam.activation] };
    case 'name':
      return { text: fam.name };
    case 'styleCount':
      return { text: String(fam.styleCount) };
    case 'weightLine': {
      const r = fam.weightRange;
      return {
        text: weightRangeText(r),
        dots: (fam.styles ?? []).map((s) => ({ left: weightPosition(s.weight), italic: s.italic })),
        span:
          fam.isVariable && r && r.max > r.min
            ? { left: weightPosition(r.min), width: weightPosition(r.max) - weightPosition(r.min) }
            : undefined,
      };
    }
    case 'weightRange':
      return { text: weightRangeText(fam.weightRange) };
    case 'format':
      return { text: '', badges: formatBadges(fam.formats, fam.isVariable, fam.isColor ?? false) };
    case 'version':
      return { text: fam.version ?? '' };
    case 'manufacturer':
      return { text: fam.foundry ?? '' };
    case 'location':
      return { text: fam.location ?? '' };
    case 'license': {
      const badge = licenseBadge(fam);
      return { text: '', badges: badge ? [badge] : [] };
    }
    case 'added':
      return { text: dateText(fam.added) };
    case 'tags':
      return { text: (fam.tags ?? []).join(', ') };
  }
}

const dotState = (row: ListRow): 'active' | 'partial' | 'inactive' =>
  row.kind === 'face' ? (row.face.active ? 'active' : 'inactive') : row.family.activation;
</script>

<template>
  <div
    ref="viewport"
    class="viewport"
    role="treegrid"
    tabindex="0"
    aria-label="Font table"
    aria-multiselectable="true"
    :aria-rowcount="count + 1"
    :aria-colcount="columns.length"
    :aria-activedescendant="activeId"
    @scroll="measure"
    @keydown="onKeyDown"
  >
    <div class="grid" :style="{ minWidth: `${minWidth}px` }">
      <div
        class="header"
        role="row"
        aria-rowindex="1"
        :style="{ gridTemplateColumns: gridTemplate, height: `${HEADER_PX}px` }"
      >
        <div
          v-for="(col, c) in columns"
          :key="col.id"
          class="th"
          :class="{
            dragging: dragging === col.id,
            'drop-target': dropTarget === col.id,
            [`col-${col.id}`]: true,
          }"
          role="columnheader"
          :aria-colindex="c + 1"
          :aria-sort="ariaSort(config, col.id)"
          :data-column="col.id"
          draggable="true"
          @dragstart="onDragStart(col, $event)"
          @dragover="onDragOver(col, $event)"
          @dragleave="dropTarget = dropTarget === col.id ? null : dropTarget"
          @drop="onDrop(col, $event)"
          @dragend="onDragEnd"
        >
          <button
            type="button"
            class="sort"
            :title="`Sort by ${col.label}`"
            @click="views.sortBy(view, col.id)"
          >
            <span :class="{ 'visually-hidden': col.id === 'activation' }">{{ col.label }}</span>
            <span v-if="col.id === 'activation'" class="dot" aria-hidden="true" />
            <span class="arrow" aria-hidden="true">{{ SORT_ARROW[ariaSort(config, col.id)] }}</span>
          </button>
        </div>
      </div>

      <div class="spacer" role="none" :style="{ height: `${count * ROW_PX}px` }">
        <div
          v-for="item in visible"
          :id="item.id"
          :key="item.slot"
          class="tr"
          :class="{
            face: item.row.kind === 'face',
            selected: list.isSelected(item.row.key),
            cursor: item.id === activeId,
          }"
          role="row"
          :data-key="item.row.key"
          :aria-rowindex="item.index + 2"
          :aria-level="item.row.kind === 'face' ? 2 : 1"
          :aria-expanded="item.row.kind === 'family' ? item.row.expanded : undefined"
          :aria-selected="list.isSelected(item.row.key)"
          :aria-setsize="item.row.kind === 'face' ? item.row.count : list.families.length"
          :aria-posinset="item.row.position"
          :aria-label="rowLabel(item.row)"
          :style="{
            gridTemplateColumns: gridTemplate,
            height: `${ROW_PX}px`,
            transform: `translateY(${item.index * ROW_PX}px)`,
          }"
          draggable="true"
          @click="onRowClick(item.row, $event)"
          @dragstart="onRowDrag(item.row, $event)"
        >
          <div
            v-for="(col, c) in columns"
            :key="col.id"
            class="td"
            :class="`col-${col.id}`"
            role="gridcell"
            :aria-colindex="c + 1"
            :data-column="col.id"
          >
            <template v-if="col.id === 'activation'">
              <span class="dot" :class="dotState(item.row)" aria-hidden="true" />
              <span class="visually-hidden">{{ cellOf(item.row, col.id).text }}</span>
            </template>
            <template v-else-if="col.id === 'name'">
              <span
                v-if="item.row.kind === 'family'"
                class="twisty"
                aria-hidden="true"
                @click.stop="onToggle(item.row)"
                >{{ item.row.expanded ? '▾' : '▸' }}</span
              >
              <span class="text" :title="cellOf(item.row, col.id).text">{{
                cellOf(item.row, col.id).text
              }}</span>
            </template>
            <template v-else-if="col.id === 'weightLine'">
              <span class="line" aria-hidden="true">
                <span
                  v-if="cellOf(item.row, col.id).span"
                  class="span"
                  :style="{
                    left: `${cellOf(item.row, col.id).span!.left}%`,
                    width: `${cellOf(item.row, col.id).span!.width}%`,
                  }"
                />
                <span
                  v-for="(d, k) in cellOf(item.row, col.id).dots"
                  :key="k"
                  class="mark"
                  :class="{ italic: d.italic }"
                  :style="{ left: `${d.left}%` }"
                />
              </span>
              <span class="visually-hidden">{{ cellOf(item.row, col.id).text }}</span>
            </template>
            <template v-else-if="cellOf(item.row, col.id).badges">
              <span v-for="b in cellOf(item.row, col.id).badges" :key="b" class="badge">{{
                b
              }}</span>
            </template>
            <span v-else class="text" :title="cellOf(item.row, col.id).text">{{
              cellOf(item.row, col.id).text
            }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.viewport {
  flex: 1;
  min-height: 0;
  overflow: auto;
  background: var(--fm-bg);
  color: var(--fm-fg);
  font-size: 12px;
}
.viewport:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: -2px;
}
.grid {
  position: relative;
}
.header {
  position: sticky;
  z-index: 1;
  top: 0;
  display: grid;
  box-sizing: border-box;
  border-bottom: 1px solid var(--fm-border);
  background: var(--fm-bg);
}
.th {
  display: flex;
  min-width: 0;
  align-items: stretch;
}
.th.dragging {
  opacity: 0.5;
}
.th.drop-target {
  box-shadow: inset 2px 0 0 var(--fm-accent);
}
.sort {
  display: flex;
  flex: 1;
  min-width: 0;
  align-items: center;
  gap: 4px;
  padding: 0 8px;
  border: 0;
  background: transparent;
  color: var(--fm-muted);
  font: inherit;
  font-weight: 600;
  text-align: start;
  cursor: pointer;
}
.sort:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: -2px;
}
.th[aria-sort='ascending'] .sort,
.th[aria-sort='descending'] .sort {
  color: var(--fm-fg);
}
.arrow {
  font-size: 9px;
}
.spacer {
  position: relative;
}
.tr {
  position: absolute;
  top: 0;
  right: 0;
  left: 0;
  display: grid;
  box-sizing: border-box;
  align-items: center;
  border-bottom: 1px solid var(--fm-border);
  user-select: none;
  contain: layout paint;
}
.tr.selected {
  background: color-mix(in srgb, var(--fm-accent) 16%, transparent);
}
/* The tint alone is under 3:1; a solid accent bar marks every selected row (WCAG 1.4.11). */
.tr.selected::before {
  position: absolute;
  top: 0;
  bottom: 0;
  left: 0;
  width: 4px;
  background: var(--fm-accent);
  content: '';
}
.viewport:focus .tr.cursor {
  box-shadow: inset 0 0 0 2px var(--fm-accent);
}
.td {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 4px;
  padding: 0 8px;
  overflow: hidden;
}
.text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tr.face .col-name {
  padding-left: 32px;
}
.twisty {
  display: grid;
  flex: none;
  place-items: center;
  width: 16px;
  color: var(--fm-muted);
  cursor: pointer;
}
.col-activation {
  justify-content: center;
}
.dot {
  display: inline-block;
  flex: none;
  width: 8px;
  height: 8px;
  box-sizing: border-box;
  border: 1.5px solid var(--fm-muted);
  border-radius: 50%;
}
.dot.active {
  border-color: var(--fm-accent);
  background: var(--fm-accent);
}
.dot.partial {
  border-color: var(--fm-accent);
  background: linear-gradient(90deg, var(--fm-accent) 50%, transparent 50%);
}
.line {
  position: relative;
  width: 100%;
  height: 10px;
}
.line::before {
  position: absolute;
  top: 50%;
  right: 0;
  left: 0;
  height: 1px;
  background: var(--fm-border);
  content: '';
}
.span {
  position: absolute;
  top: 3px;
  height: 4px;
  border-radius: 2px;
  background: color-mix(in srgb, var(--fm-fg) 35%, transparent);
}
.mark {
  position: absolute;
  top: 1px;
  width: 8px;
  height: 8px;
  box-sizing: border-box;
  margin-left: -4px;
  border: 1.5px solid var(--fm-fg);
  border-radius: 50%;
  background: var(--fm-fg);
}
.mark.italic {
  background: var(--fm-bg);
}
.badge {
  flex: none;
  padding: 1px 5px;
  border: 1px solid var(--fm-border);
  border-radius: 4px;
  color: var(--fm-muted);
  font-size: 10.5px;
  font-weight: 600;
}
.visually-hidden {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
}
</style>
