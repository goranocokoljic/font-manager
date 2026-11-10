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
// outline or tracking is on (E16.1), every card of a wrapped preset (E41.2: a thumbnail is
// one line), and a card waiting for the thumbnail of a text just typed, so a key repaints
// the wall on the next frame. A card whose face lacks characters of the text shows live
// text too, with tofu for them (E43.4): a thumbnail is a mask and cannot draw the box. Its
// meta line says "N glyphs missing · š đ č". The grid shares the list's cursor, selection
// and keyboard model; Up/Down move by a line, Left/Right by a card, Enter expands a
// family or opens a style. While the grid moves fast (E35.8, `list/settle.ts`) cards
// ask for no thumbnails or faces and show their empty frame; once it settles, the
// cards on screen ask.
//
// Selection (E44.1, board `4-drag.png`): a click selects one card, Shift-click a range,
// Ctrl/Cmd-click toggles one. A drag on the wall's empty ground draws a box; every card it
// touches is selected (with Shift or Ctrl/Cmd, added to the selection). The box is checked
// against the layout (`list/marquee.ts`), so it selects cards scrolled out of the render too.
// A click on the ground without a drag clears the selection. A selected card has the accent
// ring and a 6 % accent tint.
//
// Paragraph mode (E43.1, board `3-wall-text.png`): the Paragraph preset, or your own text
// past 80 characters, lays the wall out as reading rows: at most two per line, the family
// name and its meta in a 12 px line on top, the text under it at the body size of the
// Preview options (17 px / 1.45 by default), 18 px padding, the cards' chrome. A line's
// rows share a height, so the list stays virtualized.
import UiBadge from './ui/UiBadge.vue';
import UiChevron from './ui/UiChevron.vue';
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, watchEffect } from 'vue';
import type { FaceId, FaceSummary } from '../api/types';
import { useScrollAnchor } from '../list/anchor';
import { SETTLE_MS, useScrollSettle } from '../list/settle';
import { rowActivation } from '../list/activation';
import { dragKeys } from '../list/drag';
import { boxBetween, cardsInBox, isClick, type Box } from '../list/marquee';
import {
  gridRange,
  lineCount,
  rowsCardHeight,
  rowsColumns,
  rowsLines,
  specimenBox,
  specimenRenderPx,
  specimenScale,
  specimenText,
  WALL,
  WALL_PRESETS,
  wallCardHeight,
  wallCardWidth,
  wallColumns,
  wallLayout,
  wallSize,
  wallSpecimenPx,
  wrappedLines,
} from '../list/grid';
import { handleListKey, modeOf } from '../list/keys';
import { useRowPreviews } from '../list/previews';
import { rowLabel, styleCount, styleName, type ListRow, type RowKey } from '../list/rows';
import { poolSize, scrollToShow } from '../list/virtual';
import { useCompareStore } from '../stores/compare';
import { useFontListStore } from '../stores/fontList';
import { useFontDragStore } from '../stores/fontDrag';
import { useFontSwitchStore } from '../stores/fontSwitch';
import { useLibraryStore } from '../stores/library';
import { usePalettesStore } from '../stores/palettes';
import {
  ALL_FONTS_VIEW,
  DEFAULT_PREVIEW,
  usePreviewStore,
  type PreviewSettings,
} from '../stores/preview';
import { sampleTextFor } from '../preview/sampleText';
import { deviceScale, useThumbnailsStore, type Thumbnail } from '../stores/thumbnails';
import { t } from '../i18n';
import { weightRangeText } from '../list/columns';
import ActivationDot from './ActivationDot.vue';
import CardLiveText from './CardLiveText.vue';
import CompareToggle from './CompareToggle.vue';
import MissingGlyphs from './MissingGlyphs.vue';
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
  /** Enter on a row: open its family page. */
  open: [row: ListRow];
}>();

/** Viewport size before the first measurement (and where there is no layout). */
const FALLBACK_HEIGHT = 600;
const FALLBACK_WIDTH = 800;

const list = useFontListStore();
const fontSwitch = useFontSwitchStore();
const fontDrag = useFontDragStore();
const library = useLibraryStore();
const thumbnails = useThumbnailsStore();
const palettes = usePalettesStore();
const previewStore = usePreviewStore();
const compare = useCompareStore();

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

/** The layout: the preset's, or the reading rows for your own long text (E43.1). */
const preset = computed(() =>
  wallLayout(props.preview.preset, props.preview.sample, props.preview.customText),
);
/** Paragraph mode: the reading rows. */
const rows = computed(() => preset.value === 'paragraph');
/** The specimen's size now: the slider, scaled by the preset; the body size in the rows. */
const specimenPx = computed(() =>
  rows.value ? props.preview.bodySizePx : wallSpecimenPx(sizePx.value, preset.value),
);
const wrapped = computed(() => WALL_PRESETS[preset.value].wrapped);

const columns = computed(() =>
  rows.value ? rowsColumns(width.value) : wallColumns(width.value, layoutPx.value, preset.value),
);
const cardWidth = computed(() => wallCardWidth(width.value, columns.value));
/** The text of the reading rows, for the lines they give it. */
const rowsText = computed(() => sampleTextFor(props.preview, '', previewStore.pangramText));
const rowsLineCount = computed(() =>
  rowsLines(rowsText.value, Math.max(0, cardWidth.value - 2 * WALL.paddingPx), specimenPx.value),
);
const cardHeight = computed(() =>
  rows.value
    ? rowsCardHeight(rowsLineCount.value, specimenPx.value, props.preview.bodyLineHeight)
    : wallCardHeight(layoutPx.value, preset.value),
);
const linePx = computed(() => cardHeight.value + WALL.gap);
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

/** The layout's key for the place it was left at: the wall, or its reading rows. */
const layoutKey = computed(() => (rows.value ? 'rows' : 'wall'));

/**
 * The pixel this layout came back to, until the user scrolls: the first layout after the
 * mount (the real width replacing the fallback) scrolls to the anchor's line, and must
 * land here again.
 */
let layoutTop: number | null = null;

/** Back from the other layout without a scroll there: the very pixel this one was left at. */
function restoreLayout(): void {
  const el = viewport.value;
  if (!el) return;
  layoutTop = list.layoutScrollTop(layoutKey.value, linePx.value, columns.value);
  if (layoutTop === null || el.scrollTop === layoutTop) return;
  el.scrollTop = layoutTop;
  motion.jumped(el.scrollTop);
}

onMounted(() => {
  measure();
  anchor.restore();
  measure();
  restoreLayout();
  measure();
  measured.value = true;
  if (typeof ResizeObserver !== 'undefined' && viewport.value) {
    observer = new ResizeObserver(measure);
    observer.observe(viewport.value);
  }
});
onBeforeUnmount(() => {
  observer?.disconnect();
  const el = viewport.value;
  if (el) list.layoutLeft(layoutKey.value, el.scrollTop, linePx.value, columns.value);
});

// The slider or a resize moves every card: keep the anchor family on top. The render needs
// the window of the new scroll position; the viewport can only scroll there once the
// spacer has its new height. The post watcher is queued when the size changes, so it runs
// before the FlipGroup measures where the cards went: a card then glides only by its
// distance from the anchor, not by the whole scroll.
// Only the slider or a resize glides the cards (motion table "Size slider"). A search or a
// preview text that reorders them is a cut (E46.4: nothing animates in search results), so
// the move class has no transition unless the size just changed.
const gliding = ref(false);
let glideTimer: ReturnType<typeof setTimeout> | undefined;
watch([linePx, columns], () => {
  gliding.value = true;
  clearTimeout(glideTimer);
  // After the move has started (the group reads the class once, as it starts it).
  glideTimer = setTimeout(() => (gliding.value = false));
});
onBeforeUnmount(() => clearTimeout(glideTimer));
watch([linePx, columns], () => {
  const i = list.anchorIndex;
  if (i >= 0) scrollTop.value = Math.floor(i / columns.value) * linePx.value;
});
watch(
  [linePx, columns],
  () => {
    anchor.restore();
    if (layoutTop !== null) restoreLayout();
    measure();
  },
  { flush: 'post' },
);

function onScroll(): void {
  measure();
  if (boxFrom) drawMarquee();
  if (layoutTop !== null && Math.abs(scrollTop.value - layoutTop) >= 1) layoutTop = null;
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
const liveText = computed(
  () => props.preview.outline || props.preview.trackingEm !== 0 || wrapped.value,
);

/**
 * True once a text was typed or chosen here: from then on a card with no thumbnail for its
 * text draws it live at once. `typing` holds back the thumbnails until the keys rest, as
 * each key would ask for a render of every card.
 */
const textChanged = ref(false);
const typing = ref(false);
let typingTimer: ReturnType<typeof setTimeout> | undefined;
watch(
  [() => props.preview.sample, () => props.preview.customText, () => props.preview.language],
  () => {
    textChanged.value = true;
    typing.value = true;
    clearTimeout(typingTimer);
    typingTimer = setTimeout(() => (typing.value = false), SETTLE_MS);
  },
  { flush: 'sync' },
);
onBeforeUnmount(() => clearTimeout(typingTimer));

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
    specimenBox(
      wallCardWidth(width.value, wallColumns(width.value, textPx.value, preset.value)),
      textPx.value,
      preset.value,
    ).width,
);
/** The line height and the lines of a wrapped specimen. */
const wrapLineHeight = computed(() =>
  rows.value
    ? props.preview.bodyLineHeight
    : (WALL_PRESETS[preset.value].lineHeight ?? props.preview.lineHeight),
);
/** The room a specimen has right now. */
const box = computed(() =>
  rows.value
    ? {
        width: Math.max(0, cardWidth.value - 2 * WALL.paddingPx),
        height: rowsLineCount.value * specimenPx.value * wrapLineHeight.value,
      }
    : specimenBox(cardWidth.value, layoutPx.value, preset.value),
);
const wrapLines = computed(() =>
  rows.value
    ? rowsLineCount.value
    : wrappedLines(box.value.height, specimenPx.value, wrapLineHeight.value),
);

/** A thumbnail, the size it was rendered at, and the sample it was cut from. */
type HeldThumbnail = Thumbnail & { renderPx: number; sample: string };

/** A card's last thumbnail: it stays up while the thumbnail of a new text is on its way. */
let lastThumbs = new Map<string, HeldThumbnail>();

const visible = computed(() => {
  const thumbs = new Map<string, HeldThumbnail>();
  const items = rendered.value.map((row, i) => {
    const p = previewOf(row);
    const text = wrapped.value
      ? p.text.trim()
      : specimenText(p.text, textWidth.value, wallSpecimenPx(textPx.value, preset.value));
    const face = p.face;
    const palette = face ? palettes.fontPaletteOf(ALL_FONTS_VIEW, face) : undefined;
    const usesThumb = !!face && !palette && !liveText.value && !p.tofu.length;
    const renderPx = specimenRenderPx(text, deviceScale());
    const fresh = face && usesThumb ? thumbnails.get(face.id, text, renderPx) : undefined;
    // A held thumbnail shows the same sample at another cut; another sample is a stale text.
    const last = usesThumb ? lastThumbs.get(row.key) : undefined;
    const thumb = fresh
      ? { ...fresh, renderPx, sample: p.sample }
      : last?.sample === p.sample
        ? last
        : undefined;
    if (thumb) thumbs.set(row.key, thumb);
    const failed =
      face && usesThumb && !thumb ? thumbnails.hasFailed(face.id, text, renderPx) : false;
    const live = !usesThumb || (!thumb && !failed && textChanged.value);
    return {
      row,
      index: range.value.start + i,
      id: rowId(row),
      face,
      text,
      fallback: p.fallback,
      tofu: p.tofu,
      palette,
      usesThumb,
      live,
      thumb,
      renderPx,
      failed,
      name: row.kind === 'face' ? styleName(row) : row.family.name,
    };
  });
  lastThumbs = thumbs;
  return items;
});

// Ask for the thumbnails of the cards on screen, once the grid has settled.
watchEffect(() => {
  if (!motion.settled.value || typing.value) return;
  for (const item of visible.value)
    if (item.face && item.text && item.usesThumb)
      thumbnails.want(item.face.id, item.text, item.renderPx);
});

/** A card's place. Not a transform: the FlipGroup owns that while a card moves. */
function cardStyle(index: number) {
  const cols = columns.value;
  const line = Math.floor(index / cols);
  return {
    width: `${cardWidth.value}px`,
    height: `${cardHeight.value}px`,
    top: `${WALL.gap + line * linePx.value}px`,
    insetInlineStart: `${WALL.inset + (index % cols) * (cardWidth.value + WALL.gap)}px`,
  };
}

function thumbStyle(thumb: HeldThumbnail | undefined) {
  const color = props.preview.textColor ?? undefined;
  if (!thumb) return { color };
  const image = `url("${thumb.url}")`;
  const scale = specimenPx.value / thumb.renderPx;
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
    open: (row: ListRow) => emit('open', row),
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

/** The selection box: where the drag started, where the pointer is, what it adds to. */
const marquee = ref<Box | null>(null);
let boxFrom: { inline: number; block: number } | null = null;
let boxPointer = { x: 0, y: 0 };
let boxAdds = false;
let boxBase: ReadonlySet<RowKey> = new Set();

/** A point of the window in the wall's content: inline from its start edge, block from its top. */
function contentPoint(x: number, y: number): { inline: number; block: number } {
  const el = viewport.value!;
  const r = el.getBoundingClientRect();
  const fromLeft = x - r.left - el.clientLeft;
  const rtl = getComputedStyle(el).direction === 'rtl';
  return {
    inline: rtl ? el.clientWidth - fromLeft : fromLeft,
    block: y - r.top - el.clientTop + el.scrollTop,
  };
}

/** The wall's layout, as `cardStyle` places the cards. */
function geometry() {
  return {
    count: count.value,
    columns: columns.value,
    cardWidth: cardWidth.value,
    cardHeight: cardHeight.value,
    linePx: linePx.value,
    gap: WALL.gap,
    inset: WALL.inset,
  };
}

function drawMarquee(): void {
  if (!boxFrom) return;
  const b = boxBetween(boxFrom, contentPoint(boxPointer.x, boxPointer.y));
  // Until the pointer has moved a few px it is a click, not a box.
  if (!marquee.value && isClick(b)) return;
  marquee.value = b;
  list.boxSelect(cardsInBox(b, geometry()), boxBase);
}

/** A press on the wall's ground (not on a card, not on the scrollbar) starts a box. */
function onGroundDown(e: PointerEvent): void {
  const el = viewport.value;
  if (!el || e.button !== 0) return;
  if (e.target instanceof Element && e.target.closest('.slot, .card-compare')) return;
  const x = e.clientX - el.getBoundingClientRect().left - el.clientLeft;
  if (x < 0 || x > el.clientWidth) return;
  boxAdds = e.shiftKey || e.ctrlKey || e.metaKey;
  boxBase = boxAdds ? new Set(list.selection.selected) : new Set();
  boxFrom = contentPoint(e.clientX, e.clientY);
  boxPointer = { x: e.clientX, y: e.clientY };
  el.setPointerCapture?.(e.pointerId);
  el.focus({ preventScroll: true });
  e.preventDefault();
}

function onGroundMove(e: PointerEvent): void {
  if (!boxFrom) return;
  boxPointer = { x: e.clientX, y: e.clientY };
  drawMarquee();
}

function onGroundUp(e: PointerEvent): void {
  if (!boxFrom) return;
  const drew = marquee.value !== null;
  boxFrom = null;
  marquee.value = null;
  viewport.value?.releasePointerCapture?.(e.pointerId);
  // A plain click on the ground: nothing selected.
  if (!drew && !boxAdds && e.type === 'pointerup') list.clearSelection();
}

/** The box, in the wall's content: it scrolls with the cards. */
const marqueeStyle = computed(() => {
  const b = marquee.value;
  if (!b) return {};
  return {
    insetInlineStart: `${b.inline0}px`,
    top: `${b.block0}px`,
    width: `${b.inline1 - b.inline0}px`,
    height: `${b.block1 - b.block0}px`,
  };
});

/** A card's accessible name: its row's, and ", in Compare" while it is in the set (E48.4). */
function cardLabel(row: ListRow, face: FaceSummary | undefined): string {
  const scope = row.kind === 'family' ? 'family' : 'face';
  return rowLabel(row) + (face && compare.inSet(face, scope) ? t('grid.inCompare') : '');
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
    :class="[surfaceClass, { rows }]"
    :style="viewportStyle"
    :data-surface="preview.surface"
    :data-columns="columns"
    :data-preset="preset"
    :data-layout="layoutKey"
    role="listbox"
    tabindex="0"
    :aria-label="t('grid.label')"
    aria-multiselectable="true"
    :aria-activedescendant="activeId"
    @scroll="onScroll"
    @keydown="onKeyDown"
    @pointerdown="onGroundDown"
    @pointermove="onGroundMove"
    @pointerup="onGroundUp"
    @pointercancel="onGroundUp"
  >
    <FmFlipGroup
      tag="div"
      :move-class="gliding ? undefined : 'fm-flip-still'"
      :css="false"
      class="spacer"
      role="none"
      :style="{ height: `${WALL.gap + lineCount(count, columns) * linePx}px` }"
    >
      <template v-for="item in visible" :key="item.row.key">
        <div
          :id="item.id"
          class="slot"
          :class="{
            face: item.row.kind === 'face',
            selected: list.isSelected(item.row.key),
            origin: fontDrag.isOrigin(item.row.key),
            cursor: item.id === activeId,
          }"
          role="option"
          :data-key="item.row.key"
          :aria-setsize="count"
          :aria-posinset="item.index + 1"
          :aria-selected="list.isSelected(item.row.key)"
          :aria-label="cardLabel(item.row, item.face)"
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
              :missing="item.tofu"
              :palette="item.palette"
              :size-px="specimenPx"
              :fit-width="wrapped ? undefined : box.width"
              :wrap-lines="wrapped ? wrapLines : undefined"
              :line-height="wrapped ? wrapLineHeight : undefined"
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
              data-ghost="hide"
              :class="{ open: item.row.expanded, beside: !!item.face }"
              aria-hidden="true"
              @click.stop="onToggle(item.row)"
              ><UiChevron :open="item.row.expanded"
            /></span>
            <div class="meta">
              <span class="lead">
                <ActivationDot :state="rowActivation(item.row)" />
                <bdi class="name" data-family-name>{{ item.name }}</bdi>
              </span>
              <span
                v-if="item.face?.isVariable"
                class="detail"
                data-testid="card-variable"
                data-ghost="hide"
              >
                <span class="count">{{ t('list.variable') }}</span>
              </span>
              <span v-else-if="item.row.kind === 'family'" class="detail" data-ghost="hide">
                <span class="count">{{ styleCount(item.row.family.styleCount) }}</span>
                <span
                  v-if="item.row.family.weightRange"
                  class="range"
                  data-testid="weight-range"
                  aria-hidden="true"
                  >{{ weightRangeText(item.row.family.weightRange) }}</span
                >
              </span>
              <MissingGlyphs v-if="item.tofu.length" :missing="item.tofu" />
              <UiBadge v-if="item.fallback" data-testid="fallback-hint">{{
                t('list.fallback')
              }}</UiBadge>
            </div>
          </div>
        </div>
        <!-- The compare icon (E48.4) at the card's top-right. Beside the card, not in it: an
             option holds no button and a listbox holds only options, so the icon is hidden from
             assistive technology like the chevron. The keyboard compares with C; the card's name
             says ", in Compare". The same box as the card, so it glides with it; only the icon
             takes the pointer. -->
        <div
          v-if="item.face"
          :key="`compare:${item.row.key}`"
          class="corner"
          :class="{ cursor: item.id === activeId }"
          aria-hidden="true"
          :data-for="item.row.key"
          :style="cardStyle(item.index)"
        >
          <CompareToggle
            class="card-compare"
            data-testid="card-compare"
            tabindex="-1"
            :face="item.face"
            :scope="item.row.kind === 'family' ? 'family' : 'face'"
            @toggle="viewport?.focus({ preventScroll: true })"
          />
        </div>
      </template>
    </FmFlipGroup>
    <div
      v-if="marquee"
      class="marquee"
      data-testid="selection-box"
      aria-hidden="true"
      :style="marqueeStyle"
    />
  </div>
</template>

<style scoped>
.viewport {
  position: relative;
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
/* A selected card: the ring below and a 6 % accent tint over its ground. */
.slot.selected .card {
  background-color: color-mix(in srgb, var(--fm-accent) 6%, var(--fm-card-bg, var(--fm-bg)));
}
/* The selection box: the accent's hairline over a 6 % tint; it never takes a click. */
.marquee {
  position: absolute;
  z-index: 1;
  box-sizing: border-box;
  border: 1px solid var(--fm-accent);
  border-radius: var(--fm-radius-sm);
  background-color: color-mix(in srgb, var(--fm-accent) 6%, transparent);
  pointer-events: none;
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
/* With the compare icon in the corner, the chevron sits just before it. */
.twisty.beside {
  inset-inline-end: calc(var(--fm-space-1) + 28px + var(--fm-space-1));
}
.slot:hover .twisty,
.viewport:focus .slot.cursor .twisty,
.twisty.open {
  opacity: 1;
}
/* The compare icon's box: the card's box over it, the icon in its top-right corner. The box
   lets the pointer through to the card; the icon takes it. */
.corner {
  position: absolute;
  box-sizing: border-box;
  pointer-events: none;
}
.card-compare {
  position: absolute;
  top: var(--fm-space-1);
  inset-inline-end: var(--fm-space-1);
  opacity: 0;
  pointer-events: auto;
  transition: opacity var(--fm-motion-fast) var(--fm-ease);
}
/* Hidden at rest; shown on the card's hover or focus, at the cursor, and always in the set. */
.slot:hover + .corner .card-compare,
.slot:focus-within + .corner .card-compare,
.viewport:focus .corner.cursor .card-compare,
.card-compare:hover,
.card-compare:focus-visible,
.card-compare.in-set {
  opacity: 1;
}
/* The card keeps its hover shadow while the pointer is on its icon. */
.slot:has(+ .corner .card-compare:hover) .card {
  box-shadow: var(--fm-shadow-1);
}
/* Paragraph mode (E43.1): the meta line on top in 12 px, the text under it from the start. */
.rows .card {
  /* ROWS.paddingPx in list/grid.ts. */
  padding: 18px;
}
/* The missing line takes the rest of the line, at its end, and is cut rather than wrapped. */
.meta > .missing-glyphs {
  flex: 1 1 0;
  text-align: end;
}
.rows .meta {
  order: -1;
  /* ROWS.metaLinePx and ROWS.metaGapPx. */
  block-size: 16px;
  margin-block: 0 8px;
  font-size: 12px;
  line-height: 16px;
}
.rows .detail {
  margin-inline-start: 0;
}
.rows :deep(.live) {
  flex: none;
  align-items: flex-start;
  height: auto;
}
.note {
  color: var(--fm-warning);
  font-size: var(--fm-text-xs);
}
/* A dragged row stays in place, faded, while its ghost travels (E44.2). */
.slot.origin {
  opacity: 0.35;
}
</style>
