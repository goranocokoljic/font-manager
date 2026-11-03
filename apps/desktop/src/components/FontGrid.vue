<script setup lang="ts">
// The grid view: the font list's rows (families, and the styles of an expanded
// family) as cards, `columns` per line. Each card shows a thumbnail rendered by
// the API (`render_preview`, cached by E4.3's preview cache) of the row's face
// and its preview text. Thumbnails are black on transparent; the card paints
// them as a CSS mask in the text color, so themes and surfaces need no new render.
//
// Lines are virtualized like the list's rows (`list/grid.ts`), cards keyed by
// slot. A face whose family has a color palette chosen (E16.3) shows live text in
// that palette instead, as a mask cannot show colors; so does every card while
// outline or tracking is on (E16.1). The grid shares the list's cursor, selection
// and keyboard model; Up/Down move by a line, Left/Right by a card, Enter expands a
// family or opens a style. While the grid moves fast (E35.8, `list/settle.ts`) cards
// ask for no thumbnails or faces and show their empty frame; once it settles, the
// cards on screen ask.
import UiBadge from './ui/UiBadge.vue';
import UiChevron from './ui/UiChevron.vue';
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, watchEffect } from 'vue';
import type { FaceId } from '../api/types';
import { useScrollAnchor } from '../list/anchor';
import { useScrollSettle } from '../list/settle';
import { rowActivation } from '../list/activation';
import { dragKeys } from '../list/drag';
import { CARD, cardChars, cardText, gridColumns, gridRange, lineCount } from '../list/grid';
import { handleListKey, modeOf } from '../list/keys';
import { useRowPreviews } from '../list/previews';
import { rowLabel, styleCount, styleName, type ListRow } from '../list/rows';
import { poolSize, scrollToShow } from '../list/virtual';
import { useFontListStore } from '../stores/fontList';
import { useFontSwitchStore } from '../stores/fontSwitch';
import { useLibraryStore } from '../stores/library';
import { usePalettesStore } from '../stores/palettes';
import { ALL_FONTS_VIEW, DEFAULT_PREVIEW, type PreviewSettings } from '../stores/preview';
import { useThumbnailsStore, type Thumbnail } from '../stores/thumbnails';
import { direction, t } from '../i18n';
import { weightRangeText } from '../list/columns';
import ActivationDot from './ActivationDot.vue';
import CardLiveText from './CardLiveText.vue';

const props = withDefaults(
  defineProps<{
    preview?: PreviewSettings;
    /** Lines rendered beyond each edge of the viewport. */
    overscan?: number;
  }>(),
  { preview: () => ({ ...DEFAULT_PREVIEW }), overscan: 2 },
);

const emit = defineEmits<{
  /** The face the cursor shows changed (a style, or a family's first style). */
  cursorFace: [face: FaceId];
  /** Enter on a style: show it in the inspector. */
  open: [face: FaceId];
}>();

/** Text size of a thumbnail, in CSS px. */
const THUMB_PX = 36;
/** Viewport size before the first measurement (and where there is no layout). */
const FALLBACK_HEIGHT = 600;
const FALLBACK_WIDTH = 800;

const list = useFontListStore();
const fontSwitch = useFontSwitchStore();
const library = useLibraryStore();
const thumbnails = useThumbnailsStore();
const palettes = usePalettesStore();

const surfaceClass = computed(() =>
  props.preview.surface === 'auto' ? undefined : `surface-${props.preview.surface}`,
);
const viewportStyle = computed(() => ({
  backgroundColor: props.preview.backgroundColor ?? undefined,
}));

const viewport = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const height = ref(FALLBACK_HEIGHT);
const width = ref(FALLBACK_WIDTH);
let observer: ResizeObserver | undefined;

function measure(): void {
  const el = viewport.value;
  if (!el) return;
  scrollTop.value = el.scrollTop;
  if (el.clientHeight > 0) height.value = el.clientHeight;
  if (el.clientWidth > 0) width.value = el.clientWidth;
}

const columns = computed(() => gridColumns(width.value));
const count = computed(() => list.rows.length);
const range = computed(() =>
  gridRange(scrollTop.value, height.value, CARD.linePx, props.overscan, count.value, columns.value),
);
const pool = computed(() => poolSize(height.value, CARD.linePx, props.overscan) * columns.value);

const motion = useScrollSettle(() => CARD.linePx);
const anchor = useScrollAnchor(
  viewport,
  () => CARD.linePx,
  () => columns.value,
  motion.jumped,
);

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

// A resize that changes the cards per line moves every card: keep the anchor family on top.
watch(columns, async () => {
  await nextTick();
  anchor.restore();
  measure();
});

function onScroll(): void {
  measure();
  motion.scrolled(scrollTop.value);
  anchor.track(scrollTop.value);
}

const rendered = computed(() => list.rows.slice(range.value.start, range.value.end));
const { previewOf } = useRowPreviews(
  () => props.preview,
  () => rendered.value,
  () => list.rows.slice(Math.max(0, range.value.start - pool.value), range.value.end + pool.value),
  false,
  false,
  () => motion.settled.value,
);

/**
 * Cards draw live webview text instead of the thumbnail while outline or tracking is
 * on (E16.1): the thumbnail is a one-color mask rendered without either.
 */
const liveText = computed(() => props.preview.outline || props.preview.trackingEm !== 0);

const rowId = (row: ListRow) => `fm-card-${row.key.replace(':', '-')}`;

const textMax = computed(() => cardChars(width.value / columns.value, THUMB_PX));

const visible = computed(() =>
  rendered.value.map((row, i) => {
    const index = range.value.start + i;
    const p = previewOf(row);
    const text = cardText(p.text, textMax.value);
    const face = p.face;
    const palette = face ? palettes.fontPaletteOf(ALL_FONTS_VIEW, face) : undefined;
    const live = !!palette || liveText.value;
    return {
      row,
      index,
      slot: index % pool.value,
      id: rowId(row),
      face,
      text,
      fallback: p.fallback,
      palette,
      live,
      thumb: face && !live ? thumbnails.get(face.id, text, THUMB_PX) : undefined,
      failed: face && !live ? thumbnails.hasFailed(face.id, text, THUMB_PX) : false,
      name: row.kind === 'face' ? styleName(row) : row.family.name,
    };
  }),
);

// Ask for the thumbnails of the cards on screen, once the grid has settled.
watchEffect(() => {
  if (!motion.settled.value) return;
  for (const item of visible.value)
    if (item.face && item.text && !item.live) thumbnails.want(item.face.id, item.text, THUMB_PX);
});

function cardStyle(index: number) {
  const cols = columns.value;
  const line = Math.floor(index / cols);
  // Slots start at the inline start; in right-to-left chrome that is the right
  // edge, so the next card is a step to the left.
  const step = direction() === 'rtl' ? -100 : 100;
  return {
    width: `${100 / cols}%`,
    height: `${CARD.linePx}px`,
    transform: `translate(${(index % cols) * step}%, ${line * CARD.linePx}px)`,
  };
}

function thumbStyle(thumb: Thumbnail | undefined) {
  const color = props.preview.textColor ?? undefined;
  if (!thumb) return { color };
  const image = `url("${thumb.url}")`;
  const size = `${thumb.width}px ${thumb.height}px`;
  return {
    color,
    maskImage: image,
    WebkitMaskImage: image,
    maskSize: size,
    WebkitMaskSize: size,
  };
}

const thumbState = (item: { thumb?: Thumbnail; failed: boolean }) =>
  item.thumb ? 'ready' : item.failed ? 'failed' : motion.settled.value ? 'loading' : 'deferred';

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

/** Scrolls so the cursor's line is on screen (and so rendered for aria-activedescendant). */
async function revealCursor(): Promise<void> {
  const el = viewport.value;
  const i = list.cursorIndex;
  if (!el || i < 0) return;
  const line = Math.floor(i / columns.value);
  const top = scrollToShow(line, CARD.linePx, el.scrollTop, height.value);
  if (top !== el.scrollTop) {
    el.scrollTop = top;
    motion.jumped(el.scrollTop);
  }
  measure();
  await nextTick();
}

function onKeyDown(e: KeyboardEvent): void {
  const page = Math.max(1, Math.floor(height.value / CARD.linePx)) * columns.value;
  const opts = {
    page,
    count: count.value,
    columns: columns.value,
    open: (f: FaceId) => emit('open', f),
  };
  if (!handleListKey(e, list, opts)) return;
  e.preventDefault();
  void revealCursor();
}

/** Dragging a card onto a collection adds it, or the whole selection when the card is in it. */
function onCardDrag(row: ListRow, e: DragEvent): void {
  fontSwitch.startDrag(e, dragKeys(list.selection.selected, row.key), row);
}

function onCardClick(row: ListRow, e: MouseEvent): void {
  list.click(row.key, modeOf(e));
  viewport.value?.focus({ preventScroll: true });
}

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
    :data-columns="columns"
    role="listbox"
    tabindex="0"
    :aria-label="t('grid.label')"
    aria-multiselectable="true"
    :aria-activedescendant="activeId"
    @scroll="onScroll"
    @keydown="onKeyDown"
  >
    <div
      class="spacer"
      role="none"
      :style="{ height: `${lineCount(count, columns) * CARD.linePx}px` }"
    >
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
        role="option"
        :data-key="item.row.key"
        :aria-selected="list.isSelected(item.row.key)"
        :aria-label="rowLabel(item.row)"
        :style="cardStyle(item.index)"
        draggable="true"
        @click="onCardClick(item.row, $event)"
        @pointerdown="fontSwitch.prime(item.row)"
        @dragstart="onCardDrag(item.row, $event)"
      >
        <div class="card">
          <CardLiveText
            v-if="item.face && item.live"
            :face="item.face.id"
            :text="item.text"
            :palette="item.palette"
            :size-px="THUMB_PX"
            :tracking-em="preview.trackingEm"
            :outline="preview.outline"
            :color="preview.textColor"
            :defer="!motion.settled.value"
            aria-hidden="true"
          />
          <div
            v-else
            class="thumb"
            data-testid="thumbnail"
            :data-state="thumbState(item)"
            :style="thumbStyle(item.thumb)"
            aria-hidden="true"
          />
          <span v-if="item.failed" class="note">{{ t('list.previewUnavailable') }}</span>
          <span v-else-if="!item.face && library.failed.has(item.row.family.id)" class="note">
            {{ t('list.stylesUnavailable') }}
          </span>
          <div class="meta">
            <span class="lead">
              <span
                v-if="item.row.kind === 'family'"
                class="twisty fm-directional"
                aria-hidden="true"
                @click.stop="onToggle(item.row)"
                ><UiChevron v-if="!item.row.single" :open="item.row.expanded"
              /></span>
              <ActivationDot :state="rowActivation(item.row)" />
              <bdi class="name">{{ item.name }}</bdi>
            </span>
            <span v-if="item.row.kind === 'family'" class="detail">
              <span class="count">{{ styleCount(item.row.family.styleCount) }}</span>
              <span
                v-if="item.row.family.weightRange"
                class="range"
                data-testid="weight-range"
                aria-hidden="true"
                >{{ weightRangeText(item.row.family.weightRange) }}</span
              >
            </span>
            <UiBadge v-if="item.face?.isVariable">{{ t('list.variable') }}</UiBadge>
            <UiBadge v-if="item.fallback" data-testid="fallback-hint">{{
              t('list.fallback')
            }}</UiBadge>
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
  overflow-x: hidden;
  overflow-y: auto;
  contain: strict;
  background-color: var(--fm-bg);
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
  inset-inline-start: 0;
  box-sizing: border-box;
  padding: 6px;
  user-select: none;
  contain: layout paint;
}
.card {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 4px;
  box-sizing: border-box;
  height: 100%;
  padding: 8px 10px;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-md);
  overflow: hidden;
}
.slot.face .card {
  border-color: transparent;
  background: var(--fm-surface);
}
.slot .card {
  transition: background-color var(--fm-motion-fast) var(--fm-ease);
}
.slot:hover .card {
  background: var(--fm-hover);
}
.slot.selected .card {
  background: color-mix(in srgb, var(--fm-accent) 16%, transparent);
}
/* The tint alone is under 3:1; a solid accent bar marks every selected card (WCAG 1.4.11). */
.slot.selected .card::before {
  position: absolute;
  top: 0;
  bottom: 0;
  inset-inline-start: 0;
  width: 4px;
  background: var(--fm-accent);
  content: '';
}
.viewport:focus .slot.cursor .card {
  box-shadow: inset 0 0 0 2px var(--fm-accent);
}
.thumb {
  flex: 1;
  min-height: 0;
  background-color: currentColor;
  mask-repeat: no-repeat;
  mask-position: left center;
  -webkit-mask-repeat: no-repeat;
  -webkit-mask-position: left center;
}
/* No thumbnail yet: nothing to paint. */
.thumb:not([data-state='ready']) {
  background-color: transparent;
}
/* One line, name first: what does not fit beside the name wraps out of sight whole, so the
   name is never cut to "Sam…" by the count or the badge. */
.meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-1) var(--fm-space-2);
  block-size: 16px;
  overflow: hidden;
  min-width: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
}
.lead {
  display: flex;
  flex: 0 1 auto;
  align-items: center;
  gap: var(--fm-space-2);
  min-width: 0;
  max-width: 100%;
}
.name {
  min-width: 0;
  overflow: hidden;
  color: var(--fm-fg);
  white-space: nowrap;
  text-overflow: ellipsis;
}
/* The weight span takes the style count's place on hover or at the cursor: no extra width. */
.detail {
  display: inline-grid;
  flex: none;
}
.count,
.range {
  grid-area: 1 / 1;
  transition: opacity var(--fm-motion-fast) var(--fm-ease);
}
.range {
  font-variant-numeric: tabular-nums;
  opacity: 0;
}
.slot:hover .range,
.viewport:focus .slot.cursor .range {
  opacity: 1;
}
.slot:hover .count,
.viewport:focus .slot.cursor .count {
  opacity: 0;
}
.twisty {
  flex: none;
  cursor: pointer;
}
.note {
  color: var(--fm-warning);
  font-size: var(--fm-text-xs);
}
</style>
