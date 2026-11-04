<script setup lang="ts">
// The wall (E41.1), the grid view: the font list's rows (families, and the styles of an
// expanded family) as uniform cards, `columns` per line. Each card centres a specimen,
// a thumbnail rendered by the API (`render_preview`, cached by E4.3's preview cache) of
// the row's face and its preview text. Thumbnails are black on transparent; the card
// paints them as a CSS mask in the text color, so themes and surfaces need no new
// render. They render once at the largest size: the size slider only scales the mask,
// and a specimen wider than its card shrinks to fit, so the text is never cut.
//
// Lines are virtualized like the list's rows (`list/grid.ts`), cards keyed by row: a
// card keeps its element while the slider or a resize moves it, `<FmFlipGroup>` glides
// it there, and a card that was off screen just appears at its place.
// A face whose family has a color palette chosen (E16.3) shows live text in
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
import { SETTLE_MS, useScrollSettle } from '../list/settle';
import { rowActivation } from '../list/activation';
import { dragKeys } from '../list/drag';
import {
  gridRange,
  lineCount,
  specimenBox,
  specimenScale,
  specimenText,
  WALL,
  wallCardHeight,
  wallCardWidth,
  wallColumns,
  wallLinePx,
  wallSize,
} from '../list/grid';
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
import { t } from '../i18n';
import { weightRangeText } from '../list/columns';
import ActivationDot from './ActivationDot.vue';
import CardLiveText from './CardLiveText.vue';
import FmFlipGroup from '../motion/FmFlipGroup.vue';
import { useReducedMotion } from '../motion/useReducedMotion';

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
// A chosen background is the ground and the cards alike, as the board's white on white.
const viewportStyle = computed(() =>
  props.preview.backgroundColor
    ? {
        backgroundColor: props.preview.backgroundColor,
        '--fm-card-bg': props.preview.backgroundColor,
      }
    : {},
);

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

/** The slider's size: the specimens follow it on every frame. */
const sizePx = computed(() => wallSize(props.preview.sizePx));
/**
 * The size the cards are laid out for. Restarting the glide of every card is what a drag
 * pays for, so it happens at most every `WALL.reflowMs`; the 200 ms glide runs on between
 * two. Under reduced motion nothing glides, so the cards follow at once.
 */
const layoutPx = ref(sizePx.value);
const reducedMotion = useReducedMotion();
let reflowedAt = -Infinity;
let reflowTimer: ReturnType<typeof setTimeout> | undefined;
function reflow(px: number): void {
  reflowedAt = performance.now();
  layoutPx.value = px;
}
watch(
  sizePx,
  (px) => {
    clearTimeout(reflowTimer);
    const wait = reflowedAt + WALL.reflowMs - performance.now();
    if (wait <= 0 || reducedMotion.value) reflow(px);
    else reflowTimer = setTimeout(() => reflow(sizePx.value), wait);
  },
  { flush: 'sync' },
);
onBeforeUnmount(() => clearTimeout(reflowTimer));

const linePx = computed(() => wallLinePx(layoutPx.value));
const columns = computed(() => wallColumns(width.value, layoutPx.value));
const cardWidth = computed(() => wallCardWidth(width.value, columns.value));
const count = computed(() => list.rows.length);
const range = computed(() =>
  gridRange(
    scrollTop.value,
    height.value,
    linePx.value,
    props.overscan,
    count.value,
    columns.value,
  ),
);
const pool = computed(() => poolSize(height.value, linePx.value, props.overscan) * columns.value);

const motion = useScrollSettle(() => linePx.value);
const anchor = useScrollAnchor(
  viewport,
  () => linePx.value,
  () => columns.value,
  motion.jumped,
);

/** No cards before the first measure: they would glide from the fallback width's places. */
const measured = ref(false);

onMounted(() => {
  measure();
  anchor.restore();
  measure();
  measured.value = true;
  if (typeof ResizeObserver !== 'undefined' && viewport.value) {
    observer = new ResizeObserver(measure);
    observer.observe(viewport.value);
  }
});
onBeforeUnmount(() => observer?.disconnect());

// The slider or a resize moves every card: keep the anchor family on top. The render needs
// the window of the new scroll position; the viewport can only scroll there once the
// spacer has its new height. The post watcher is queued when the size changes, so it runs
// before the FlipGroup measures where the cards went: a card then glides only by its
// distance from the anchor, not by the whole scroll.
watch([linePx, columns], () => {
  const i = list.anchorIndex;
  if (i >= 0) scrollTop.value = Math.floor(i / columns.value) * linePx.value;
});
watch(
  [linePx, columns],
  () => {
    anchor.restore();
    measure();
  },
  { flush: 'post' },
);

function onScroll(): void {
  measure();
  motion.scrolled(scrollTop.value);
  anchor.track(scrollTop.value);
}

const rendered = computed(() =>
  measured.value ? list.rows.slice(range.value.start, range.value.end) : [],
);
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

/**
 * The size the specimen's text was chosen for. It follows the slider once it rests: while
 * it moves, every card keeps its text and only scales it, so a drag starts no render.
 */
const textPx = ref(sizePx.value);
let textTimer: ReturnType<typeof setTimeout> | undefined;
watch(sizePx, (px) => {
  clearTimeout(textTimer);
  textTimer = setTimeout(() => (textPx.value = px), SETTLE_MS);
});
onBeforeUnmount(() => clearTimeout(textTimer));

/** The room of the specimen's text, at the size it was chosen for. */
const textWidth = computed(
  () =>
    specimenBox(wallCardWidth(width.value, wallColumns(width.value, textPx.value)), textPx.value)
      .width,
);
/** The room a specimen has right now. */
const box = computed(() => specimenBox(cardWidth.value, layoutPx.value));

/** A card's last thumbnail: it stays up while the thumbnail of a new text is on its way. */
let lastThumbs = new Map<string, Thumbnail>();

const visible = computed(() => {
  const thumbs = new Map<string, Thumbnail>();
  const items = rendered.value.map((row, i) => {
    const p = previewOf(row);
    const text = specimenText(p.text, textWidth.value, textPx.value);
    const face = p.face;
    const palette = face ? palettes.fontPaletteOf(ALL_FONTS_VIEW, face) : undefined;
    const live = !!palette || liveText.value;
    const fresh = face && !live ? thumbnails.get(face.id, text, WALL.renderPx) : undefined;
    const thumb = fresh ?? (face && !live ? lastThumbs.get(row.key) : undefined);
    if (thumb) thumbs.set(row.key, thumb);
    return {
      row,
      index: range.value.start + i,
      id: rowId(row),
      face,
      text,
      fallback: p.fallback,
      palette,
      live,
      thumb,
      failed: face && !live && !thumb ? thumbnails.hasFailed(face.id, text, WALL.renderPx) : false,
      name: row.kind === 'face' ? styleName(row) : row.family.name,
    };
  });
  lastThumbs = thumbs;
  return items;
});

// Ask for the thumbnails of the cards on screen, once the grid has settled.
watchEffect(() => {
  if (!motion.settled.value) return;
  for (const item of visible.value)
    if (item.face && item.text && !item.live)
      thumbnails.want(item.face.id, item.text, WALL.renderPx);
});

/** A card's place. Not a transform: the FlipGroup owns that while a card moves. */
function cardStyle(index: number) {
  const cols = columns.value;
  const line = Math.floor(index / cols);
  return {
    width: `${cardWidth.value}px`,
    height: `${wallCardHeight(layoutPx.value)}px`,
    top: `${WALL.gap + line * linePx.value}px`,
    insetInlineStart: `${WALL.inset + (index % cols) * (cardWidth.value + WALL.gap)}px`,
  };
}

function thumbStyle(thumb: Thumbnail | undefined) {
  const color = props.preview.textColor ?? undefined;
  if (!thumb) return { color };
  const image = `url("${thumb.url}")`;
  const scale = sizePx.value / WALL.renderPx;
  const w = thumb.width * scale;
  const h = thumb.height * scale;
  const fit = specimenScale(w, h, box.value);
  const size = `${w * fit}px ${h * fit}px`;
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
  const top = scrollToShow(line, linePx.value, el.scrollTop, height.value);
  if (top !== el.scrollTop) {
    el.scrollTop = top;
    motion.jumped(el.scrollTop);
  }
  measure();
  await nextTick();
}

function onKeyDown(e: KeyboardEvent): void {
  const page = Math.max(1, Math.floor(height.value / linePx.value)) * columns.value;
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
    <FmFlipGroup
      tag="div"
      :css="false"
      class="spacer"
      role="none"
      :style="{ height: `${WALL.gap + lineCount(count, columns) * linePx}px` }"
    >
      <div
        v-for="item in visible"
        :id="item.id"
        :key="item.row.key"
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
            :size-px="sizePx"
            :fit-width="box.width"
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
          <span
            v-if="item.row.kind === 'family' && !item.row.single"
            class="twisty fm-directional"
            :class="{ open: item.row.expanded }"
            aria-hidden="true"
            @click.stop="onToggle(item.row)"
            ><UiChevron :open="item.row.expanded"
          /></span>
          <div class="meta">
            <span class="lead">
              <ActivationDot :state="rowActivation(item.row)" />
              <bdi class="name">{{ item.name }}</bdi>
            </span>
            <span v-if="item.face?.isVariable" class="detail" data-testid="card-variable">
              <span class="count">{{ t('list.variable') }}</span>
            </span>
            <span v-else-if="item.row.kind === 'family'" class="detail">
              <span class="count">{{ styleCount(item.row.family.styleCount) }}</span>
              <span
                v-if="item.row.family.weightRange"
                class="range"
                data-testid="weight-range"
                aria-hidden="true"
                >{{ weightRangeText(item.row.family.weightRange) }}</span
              >
            </span>
            <UiBadge v-if="item.fallback" data-testid="fallback-hint">{{
              t('list.fallback')
            }}</UiBadge>
          </div>
        </div>
      </div>
    </FmFlipGroup>
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
/* Not contain: paint, which would clip the hover shadow. */
.slot {
  position: absolute;
  box-sizing: border-box;
  user-select: none;
  contain: layout style;
}
.card {
  position: relative;
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  height: 100%;
  /* With the hairline and the footer: WALL.paddingPx and WALL.chromePx in list/grid.ts. */
  padding: 13px 18px;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-card-bg, var(--fm-bg));
  transition: box-shadow var(--fm-motion-fast) var(--fm-ease);
}
.slot.face .card {
  border-color: transparent;
  background-color: var(--fm-surface);
}
.slot:hover .card {
  box-shadow: var(--fm-shadow-1);
}
/* The ring draws in over the hairline: 2 px of solid accent (WCAG 1.4.11), ring opacity per the motion table. */
.card::after {
  position: absolute;
  inset: -1px;
  border: 2px solid var(--fm-accent);
  border-radius: inherit;
  content: '';
  opacity: 0;
  pointer-events: none;
  transition: opacity var(--fm-motion-fast) var(--fm-ease);
}
.slot.selected .card::after {
  opacity: 1;
}
.viewport:focus .slot.cursor .card {
  box-shadow: inset 0 0 0 2px var(--fm-accent);
}
.thumb {
  flex: 1;
  min-height: 0;
  background-color: currentColor;
  mask-repeat: no-repeat;
  mask-position: center;
  -webkit-mask-repeat: no-repeat;
  -webkit-mask-position: center;
}
/* No thumbnail yet: nothing to paint. */
.thumb:not([data-state='ready']) {
  background-color: transparent;
}
/* One line, name first: what does not fit beside the name wraps out of sight whole, so the
   name is never cut to "Sam…" by the count or the badge. */
.meta {
  display: flex;
  flex: none;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-1) var(--fm-space-2);
  block-size: 18px;
  margin-block-start: 7px;
  overflow: hidden;
  min-width: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
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
  font-weight: 500;
  white-space: nowrap;
  text-overflow: ellipsis;
}
/* The weight span takes the style count's place on hover or at the cursor: no extra width. */
.detail {
  display: inline-grid;
  flex: none;
  margin-inline-start: auto;
  font-variant-numeric: tabular-nums;
}
.count,
.range {
  grid-area: 1 / 1;
  transition: opacity var(--fm-motion-fast) var(--fm-ease);
}
.range {
  opacity: 0;
}
.slot:hover .range,
.viewport:focus .slot.cursor .range {
  opacity: 1;
}
.slot:hover .count:has(+ .range),
.viewport:focus .slot.cursor .count:has(+ .range) {
  opacity: 0;
}
/* The board's footer has no chevron: it comes up on the card that can use it. */
.twisty {
  position: absolute;
  top: var(--fm-space-2);
  inset-inline-end: var(--fm-space-2);
  cursor: pointer;
  opacity: 0;
  transition: opacity var(--fm-motion-fast) var(--fm-ease);
}
.slot:hover .twisty,
.viewport:focus .slot.cursor .twisty,
.twisty.open {
  opacity: 1;
}
.note {
  color: var(--fm-warning);
  font-size: var(--fm-text-xs);
}
</style>
