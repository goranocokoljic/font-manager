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
//
// Each row has a "Similar" action (E23.3), shown on the cursor row and under the
// pointer; it is out of the Tab order, the S key runs it for the keyboard.
// A family downloaded from a catalog that the last update check found a newer
// version of shows an "Update available" badge (E22.4).
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { FaceId, FaceSummary } from '../api/types';
import { poolSize, scrollToShow, windowRange } from '../list/virtual';
import { useScrollAnchor } from '../list/anchor';
import { keepRowInView } from '../list/density';
import { dragKeys, startFontDrag } from '../list/drag';
import { handleListKey, modeOf } from '../list/keys';
import { ACTIONS, ariaShortcut, chordOf, formatChord } from '../keyboard/keymap';
import { rowActivation } from '../list/activation';
import { openLicenseBadge, openLicenseName } from '../list/columns';
import { useRowPreviews } from '../list/previews';
import { rowLabel, styleCount, styleName, type ListRow } from '../list/rows';
import { useCatalogsStore } from '../stores/catalogs';
import { useFontListStore } from '../stores/fontList';
import { useLibraryStore } from '../stores/library';
import { useQuickCollectionStore } from '../stores/quickCollection';
import { useSettingsStore } from '../stores/settings';
import { useSimilarStore } from '../stores/similar';
import { DEFAULT_PREVIEW, previewRowHeight, type PreviewSettings } from '../stores/preview';
import { t } from '../i18n';
import ActivationDot from './ActivationDot.vue';
import FontPreviewRow from './FontPreviewRow.vue';

const props = withDefaults(
  defineProps<{
    preview?: PreviewSettings;
    /** Fixed row height; derived from `preview` and the row density when absent. */
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
const catalogs = useCatalogsStore();

/** The "Update available" tooltip of a library family the last update check found (E22.4). */
function updateTitle(family: string): string | null {
  const update = catalogs.libraryUpdate(family);
  if (!update) return null;
  return t('catalogs.updates.badgeTitle', {
    catalog: catalogs.catalogs.find((c) => c.id === update.catalog)?.name ?? update.catalog,
    version: update.availableVersion ?? '',
  });
}
const quick = useQuickCollectionStore();
const settings = useSettingsStore();
const similar = useSimilarStore();
const similarChord = chordOf(ACTIONS.similar)!;

const rowPx = computed(() => props.rowHeight ?? previewRowHeight(props.preview, settings.density));

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

const anchor = useScrollAnchor(viewport, () => rowPx.value);

onMounted(() => {
  measure();
  anchor.restore();
  measure();
  if (typeof ResizeObserver !== 'undefined' && viewport.value) {
    observer = new ResizeObserver(measure);
    observer.observe(viewport.value);
  }
});
onBeforeUnmount(() => observer?.disconnect());

function onScroll(): void {
  measure();
  anchor.track(scrollTop.value);
}

const count = computed(() => list.rows.length);
const range = computed(() =>
  windowRange(scrollTop.value, height.value, rowPx.value, props.overscan, count.value),
);
const pool = computed(() => poolSize(height.value, rowPx.value, props.overscan));

const rowId = (row: ListRow) => `fm-row-${row.key.replace(':', '-')}`;

const rendered = computed(() => list.rows.slice(range.value.start, range.value.end));
// Family rows preview their first face: the faces of the rendered rows and of one
// pool beyond each edge are fetched, so rows scrolled in already have them and their
// preview row is patched in place, not swapped for a placeholder.
const { previewOf } = useRowPreviews(
  () => props.preview,
  () => rendered.value,
  () => list.rows.slice(Math.max(0, range.value.start - pool.value), range.value.end + pool.value),
);

const visible = computed(() =>
  rendered.value.map((row, i) => {
    const index = range.value.start + i;
    return { row, index, slot: index % pool.value, id: rowId(row), ...previewOf(row) };
  }),
);

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

// A new row height (the density, the preview size) keeps the cursor row on screen;
// without a cursor, the first visible family stays at the top. After the render,
// so the spacer already has its new height.
watch(
  rowPx,
  (after, before) => {
    const el = viewport.value;
    if (!el) return;
    const i = list.cursorIndex;
    if (i < 0) {
      anchor.restore();
    } else {
      const top = keepRowInView(i, before, after, el.scrollTop, height.value);
      if (top !== el.scrollTop) el.scrollTop = top;
    }
    measure();
  },
  { flush: 'post' },
);

function onKeyDown(e: KeyboardEvent): void {
  const page = Math.max(1, Math.floor(height.value / rowPx.value));
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

const similarLabel = (face: FaceSummary) =>
  t('list.findSimilarLabel', { name: `${face.familyName} ${face.style}` });

/** The row's "Similar" action: selects the row and finds the faces like its face. */
function findSimilar(row: ListRow, face: FaceSummary): void {
  list.click(row.key);
  void similar.openFor(face);
}

// A face picked elsewhere ("Find similar") becomes the cursor: scroll to it.
watch(
  () => list.revealed,
  () => void revealCursor(),
  { flush: 'post' },
);

function onToggle(row: ListRow): void {
  if (row.kind === 'family') list.toggleExpanded(row.family.id);
  viewport.value?.focus({ preventScroll: true });
}
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
    :aria-label="t('list.label')"
    aria-multiselectable="true"
    :aria-activedescendant="activeId"
    @scroll="onScroll"
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
        :aria-setsize="item.row.kind === 'face' ? item.row.count : list.families.length"
        :aria-posinset="item.row.position"
        :aria-label="
          rowLabel(item.row) + (quick.holds(item.row) ? t('list.inQuickCollection') : '')
        "
        :style="{ height: `${rowPx}px`, transform: `translateY(${item.index * rowPx}px)` }"
        draggable="true"
        @click="onRowClick(item.row, $event)"
        @dragstart="onRowDrag(item.row, $event)"
      >
        <span
          v-if="item.row.kind === 'family'"
          class="twisty fm-directional"
          aria-hidden="true"
          @click.stop="onToggle(item.row)"
          >{{ item.row.expanded ? '▾' : '▸' }}</span
        >
        <FontPreviewRow
          v-if="item.face"
          class="preview"
          :face="item.face"
          :label="item.row.kind === 'face' ? styleName(item.row) : item.row.family.name"
          :text="item.text"
          :size-px="preview.sizePx"
          :line-height="preview.lineHeight"
          :tracking-em="preview.trackingEm"
          :align="preview.align"
          :color="preview.textColor"
          :fallback="item.fallback"
          :missing="item.missing"
          :variations="item.variations"
        >
          <template #lead>
            <ActivationDot :state="rowActivation(item.row)" />
            <span
              v-if="quick.holds(item.row)"
              class="quick-mark"
              data-testid="quick-mark"
              :title="t('list.inQuickCollectionTitle')"
              aria-hidden="true"
              >●</span
            >
          </template>
          <template v-if="item.row.kind === 'family'" #detail>
            <span class="count">{{ styleCount(item.row.family.styleCount) }}</span>
            <span
              v-if="item.row.family.openLicense"
              class="open-license"
              data-testid="open-license"
              :title="openLicenseName(item.row.family.openLicense)"
              >{{ openLicenseBadge() }}</span
            >
            <span
              v-if="updateTitle(item.row.family.name)"
              class="update"
              data-testid="update-available"
              :title="updateTitle(item.row.family.name) ?? undefined"
              >{{ t('catalogs.updates.badge') }}</span
            >
          </template>
          <template v-else-if="item.row.virtual" #detail>
            <span class="count">{{ t('list.pinned') }}</span>
          </template>
        </FontPreviewRow>
        <button
          v-if="item.face"
          type="button"
          class="row-action"
          tabindex="-1"
          data-testid="row-similar"
          :aria-label="similarLabel(item.face)"
          :aria-keyshortcuts="ariaShortcut(similarChord)"
          :title="`${similarLabel(item.face)} (${formatChord(similarChord)})`"
          @click.stop="findSimilar(item.row, item.face)"
        >
          {{ t('list.findSimilar') }}
        </button>
        <div v-else class="preview placeholder">
          <span class="label"
            ><ActivationDot :state="rowActivation(item.row)" /><bdi>{{
              item.row.family.name
            }}</bdi></span
          >
          <span v-if="library.failed.has(item.row.family.id)" class="note">
            {{ t('list.stylesUnavailable') }}
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
  inset-inline-end: 0;
  inset-inline-start: 0;
  display: flex;
  align-items: stretch;
  box-sizing: border-box;
  border-bottom: 1px solid var(--fm-border);
  user-select: none;
  contain: layout paint;
}
.slot.face {
  padding-inline-start: 24px;
}
.slot.selected {
  background: color-mix(in srgb, var(--fm-accent) 16%, transparent);
}
/* The tint alone is under 3:1; a solid accent bar marks every selected row (WCAG 1.4.11). */
.slot.selected::before {
  position: absolute;
  top: 0;
  bottom: 0;
  inset-inline-start: 0;
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
/* The slot draws the row line, full width; the preview row sits inside it, padded
   by the density (16px comfortable, 8px compact). */
.slot .preview {
  padding-inline: calc(2 * var(--fm-gap));
  border-bottom: 0;
}
.slot:not(.face) .preview {
  padding-inline-start: 0;
}
.placeholder {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 2px;
  box-sizing: border-box;
  padding-inline-end: 16px;
  color: var(--fm-muted);
  font-size: 11.5px;
}
.placeholder .label {
  display: flex;
  align-items: center;
  gap: 8px;
}
.count {
  color: var(--fm-muted);
}
.open-license {
  padding: 0 4px;
  border: 1px solid var(--fm-success);
  border-radius: 3px;
  color: var(--fm-success);
  font-size: 10.5px;
  line-height: 14px;
}
.update {
  padding: 0 4px;
  border: 1px solid var(--fm-accent);
  border-radius: 3px;
  color: var(--fm-accent);
  font-size: 10.5px;
  line-height: 14px;
}
.note {
  color: var(--fm-warning);
}
.row-action {
  flex: none;
  align-self: center;
  margin-inline-end: 8px;
  padding: 2px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: var(--fm-bg);
  color: inherit;
  font: inherit;
  font-size: 11.5px;
  visibility: hidden;
}
.slot:hover .row-action,
.slot.cursor .row-action {
  visibility: visible;
}
.quick-mark {
  color: var(--fm-accent);
  font-size: 10px;
}
</style>
