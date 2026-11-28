<script setup lang="ts">
// The Characters section of the family page (E42.5): every character the face maps, drawn by the
// webview from the face's own file. The cells scale (E50.7): a row holds as many cells of at
// least the slider's size as fit, they stretch to fill it, and the glyph is half its cell; a
// hairline grid runs between them. Above the grid, a specimen line whose box comes from the
// face's ascender and descender, with rulers. fm-core lists the characters, runs the
// search and answers for the detail; this section lays them out. The grid has no scrollbar of its
// own: it is as tall as its rows and the page scrolls it, so only the rows in the page's window
// exist, and a CJK face with 30k+ characters scrolls like a Latin one.
//
// Keyboard (WAI-ARIA grid): the grid holds focus and points at the cursor cell with
// aria-activedescendant; arrow keys move by cell and row and Home/End to the ends.
//
// The glyph detail is a popover anchored to its cell (E50.9): a click or Enter opens it, and it
// follows the cursor while open. Escape, a press outside it and its close button close it, and
// focus goes back to the grid, on the cell. 'Details on click' (a view preference) turns that off:
// a click then only selects the cell, and the detail is in the cell's context menu (right click,
// Shift+F10 or the Menu key). While 'Compare with…' has a face, the popover adds that face's
// metrics next to this one's and ghosts its glyph under this one; with no comparison no metric is
// shown. Below the grid, the coverage list (GlyphCoverage): a block chosen there shows that block
// in the grid and focuses it. Under the grid, the face's detected letter shapes (FamilyTraits,
// E49.10).
import { computed, nextTick, onBeforeUnmount, ref, shallowRef, watch, watchEffect } from 'vue';
import type { FaceDetail, FaceId, FeatureInfo, GlyphInfo } from '../api/types';
import { glyphAlternates, type GlyphAlternate } from '../glyphs/alternates';
import { glyphMetricRow } from '../glyphs/metrics';
import {
  CELL,
  cellTemplate,
  codePointLabel,
  glyphColumns,
  glyphFontSize,
  glyphRowCount,
  glyphRowHeight,
  moveCursor,
  SPECIMEN_FIT,
  fitSpecimenSize,
  rowOf,
} from '../glyphs/layout';
import { useFaceLoader } from '../preview/faceLoader';
import { metricsSpecimen, specimenRulers } from '../preview/metricsSpecimen';
import { errorMessage, inlineKey, t } from '../i18n';
import { windowRange } from '../list/virtual';
import { useAxesStore } from '../stores/axes';
import { useCoverageStore } from '../stores/coverage';
import { useFamilyPageStore } from '../stores/familyPage';
import { useGlyphCompareStore } from '../stores/glyphCompare';
import { useGlyphGridStore } from '../stores/glyphGrid';
import { useGlyphsStore } from '../stores/glyphs';
import CopyValue from './CopyValue.vue';
import GlyphCompare from './GlyphCompare.vue';
import FamilyTraits from './FamilyTraits.vue';
import GlyphCoverage from './GlyphCoverage.vue';
import UiChevron from './ui/UiChevron.vue';
import UiIcon from './ui/UiIcon.vue';
import UiIconButton from './ui/UiIconButton.vue';
import UiPopover from './ui/UiPopover.vue';
import UiSwitch from './ui/UiSwitch.vue';
import { useCopy } from './useCopy';
import { likeThisLabel } from '../search/like';
import { localWidth, useInterfaceFactor } from '../preview/trueSize';
import type { TraitTag } from '../search/traits';

const props = withDefaults(
  defineProps<{
    /** The detail of the hero's face, once it has loaded. */
    detail: FaceDetail | null;
    /** The hero's font: family, features and palette. Undefined until the face has loaded. */
    faceStyle: Record<string, string | undefined> | undefined;
    /** Rows rendered beyond each edge of the window. */
    overscan?: number;
  }>(),
  { overscan: 3 },
);
const emit = defineEmits<{
  /** 'find fonts with this' on a trait tag (E49.10). */
  findTrait: [tag: TraitTag];
  /** 'Fonts with an R like this' on the selected glyph (E49.9). */
  findLike: [like: { face: FaceId; glyph: string; family: string }];
}>();

/** Window size before the first measurement (and where there is no layout). */
const FALLBACK_HEIGHT = 384;
const FALLBACK_WIDTH = 720;
/** Room left between a revealed row and the edge of the window. */
const REVEAL_MARGIN = 8;
/** The popover's width and its distance from its cell, in CSS px. */
const POPOVER_WIDTH = 360;
const POPOVER_GAP = 8;
/** The view whose cell size and grid switch this section remembers. */
const GRID_VIEW = 'family.characters';

const page = useFamilyPageStore();
const axesSet = useAxesStore();
const glyphs = useGlyphsStore();
const compare = useGlyphCompareStore();
const coverage = useCoverageStore();
const gridSetup = useGlyphGridStore();
const { status: copyStatus, copy } = useCopy();

const faceId = computed(() => props.detail?.face.id ?? null);
watch(
  faceId,
  (id) => {
    if (id !== null) {
      void glyphs.open(id);
      void coverage.open(id);
    }
  },
  { immediate: true },
);

const grid = ref<HTMLElement | null>(null);
/** What scrolls the grid: the page, found once the grid is in the document. */
const scroller = shallowRef<HTMLElement | null>(null);
const scrollTop = ref(0);
const height = ref(FALLBACK_HEIGHT);
const width = ref(FALLBACK_WIDTH);
let observer: ResizeObserver | undefined;

function findScroller(el: HTMLElement): HTMLElement | null {
  for (let p = el.parentElement; p && p !== document.documentElement; p = p.parentElement) {
    const overflow = getComputedStyle(p).overflowY;
    if (overflow === 'auto' || overflow === 'scroll') return p;
  }
  return null;
}

function measure(): void {
  const el = grid.value;
  if (!el) return;
  // The fractional width, as the CSS grid's auto-fill counts its tracks from it.
  const w = el.getBoundingClientRect().width || el.clientWidth;
  if (w > 0) width.value = w;
  const s = scroller.value;
  if (!s || s.clientHeight <= 0) return;
  height.value = s.clientHeight;
  scrollTop.value = Math.max(0, s.getBoundingClientRect().top - el.getBoundingClientRect().top);
}
function detach(): void {
  observer?.disconnect();
  scroller.value?.removeEventListener('scroll', measure);
  scroller.value = null;
}
watch(
  grid,
  (el) => {
    detach();
    if (!el) return;
    scroller.value = findScroller(el);
    scroller.value?.addEventListener('scroll', measure, { passive: true });
    if (typeof ResizeObserver !== 'undefined') {
      observer ??= new ResizeObserver(measure);
      observer.observe(el);
      if (scroller.value) observer.observe(scroller.value);
    }
    measure();
  },
  { flush: 'post' },
);
onBeforeUnmount(detach);

const cell = computed(() => gridSetup.settingsFor(GRID_VIEW).cell);
const lines = computed(() => gridSetup.settingsFor(GRID_VIEW).lines);
const detailsOnClick = computed(() => gridSetup.settingsFor(GRID_VIEW).details);
// The interface size (E58.7): the grid's rows (`.sizer`) take it back, so the cell slider
// is true pixels. The grid and the page around it are measured in interface pixels; the
// rows are laid out in true ones, `factor` times as many.
const factor = useInterfaceFactor();
const trueWidth = computed(() => width.value * factor.value);
const columns = computed(() => glyphColumns(trueWidth.value, cell.value));
const rowHeight = computed(() => glyphRowHeight(trueWidth.value, columns.value));
const rowCount = computed(() => glyphRowCount(glyphs.total, columns.value));
const rowStyle = computed(() => ({
  gridTemplateColumns: cellTemplate(cell.value),
  fontSize: `${glyphFontSize(rowHeight.value)}px`,
}));
function slideCell(e: Event): void {
  gridSetup.update(GRID_VIEW, { cell: (e.target as HTMLInputElement).valueAsNumber });
}
function toggleLines(): void {
  gridSetup.update(GRID_VIEW, { lines: !lines.value });
}
function toggleDetails(): void {
  const on = !detailsOnClick.value;
  gridSetup.update(GRID_VIEW, { details: on });
  if (!on) popoverOpen.value = false;
}

// The specimen: the largest size up to 96 px at which the whole line fits its pane, measured in
// the face (again when the pane resizes or a font loads). Its box and rulers come from the face's
// metrics (`metricsSpecimen`, shared with the Info section): ascender to descender, so g j p q y
// stay inside it.
const specimenLine = ref<HTMLElement | null>(null);
const specimenText = ref<HTMLElement | null>(null);
const specimenPx = ref<number>(SPECIMEN_FIT.max);
const specimen = computed(() => {
  const m = props.detail?.metrics;
  return m ? metricsSpecimen(m, specimenPx.value) : null;
});
/** A length in CSS px, to the hundredth. */
const px = (v: number) => `${Math.round(v * 100) / 100}px`;
const rulers = computed(() => (specimen.value ? specimenRulers(specimen.value) : []));
function fitSpecimen(): void {
  const line = specimenLine.value;
  const text = specimenText.value;
  if (!line || !text) return;
  // Both in the specimen's own (true) pixels: it takes the interface size back.
  const drawn = localWidth(text, line, factor.value);
  specimenPx.value = fitSpecimenSize(drawn / specimenPx.value, line.clientWidth);
}
let specimenObserver: ResizeObserver | undefined;
watch(
  [specimenLine, () => props.faceStyle],
  ([el]) => {
    specimenObserver?.disconnect();
    if (!el) return;
    if (typeof ResizeObserver !== 'undefined') {
      specimenObserver ??= new ResizeObserver(fitSpecimen);
      specimenObserver.observe(el);
    }
    fitSpecimen();
  },
  { flush: 'post' },
);
const fontSet = typeof document !== 'undefined' ? document.fonts : undefined;
fontSet?.addEventListener?.('loadingdone', fitSpecimen);
onBeforeUnmount(() => {
  specimenObserver?.disconnect();
  fontSet?.removeEventListener?.('loadingdone', fitSpecimen);
});
const range = computed(() =>
  windowRange(
    scrollTop.value * factor.value,
    height.value * factor.value,
    rowHeight.value,
    props.overscan,
    rowCount.value,
  ),
);

/** The cursor: a list index, or -1 before the user moves into the grid. */
const cursor = ref(-1);
const cellId = (i: number) => `fm-glyph-${i}`;

const visible = computed(() => {
  const rows = [];
  for (let r = range.value.start; r < range.value.end; r++) {
    const first = r * columns.value;
    const count = Math.min(columns.value, glyphs.total - first);
    rows.push({
      r,
      cells: Array.from({ length: count }, (_, c) => ({
        index: first + c,
        glyph: glyphs.item(first + c),
      })),
    });
  }
  return rows;
});

// Load the pages of the rows on screen.
watchEffect(() => {
  void glyphs.ensure(
    range.value.start * columns.value,
    Math.min(range.value.end * columns.value, glyphs.total),
  );
});

/** "U+0416 uni0416": code point and name, for screen readers and tooltips. */
function cellLabel(g: GlyphInfo | undefined, index: number): string {
  if (!g) return t('glyphs.cellLoading', { number: index + 1 });
  return `${codePointLabel(g.codePoint)} ${g.name ?? t('glyphs.noGlyphName')}`;
}
const charOf = (g: GlyphInfo) => String.fromCodePoint(g.codePoint);

const activeId = computed(() => {
  const i = cursor.value;
  if (i < 0) return undefined;
  const r = rowOf(i, columns.value);
  return r >= range.value.start && r < range.value.end ? cellId(i) : undefined;
});

/** Scrolls the page so list index `i` is on screen, and so rendered for aria-activedescendant. */
async function reveal(i: number): Promise<void> {
  const el = grid.value;
  const s = scroller.value;
  // In the page's pixels: the rows are in true ones.
  const row = (rowOf(i, columns.value) * rowHeight.value) / factor.value;
  const tall = rowHeight.value / factor.value;
  if (el && s && s.clientHeight > 0) {
    const top = s.scrollTop + el.getBoundingClientRect().top - s.getBoundingClientRect().top + row;
    if (top < s.scrollTop) s.scrollTop = top - REVEAL_MARGIN;
    else if (top + tall > s.scrollTop + s.clientHeight)
      s.scrollTop = top + tall - s.clientHeight + REVEAL_MARGIN;
    measure();
  } else {
    scrollTop.value = row;
  }
  await nextTick();
}

/** The glyph popover is showing (anchored to the cursor cell). */
const popoverOpen = ref(false);

function choose(index: number): void {
  cursor.value = index;
  const g = glyphs.item(index);
  if (g) void glyphs.select(g);
}

/** Selects list index `index` and opens its popover. */
function openDetail(index: number): void {
  choose(index);
  popoverOpen.value = true;
}

/** The character 'Compare with…' starts on: the selected one, else the cursor's or the first. */
function compareCodePoint(): number | null {
  return glyphs.selected?.codePoint ?? glyphs.item(Math.max(cursor.value, 0))?.codePoint ?? null;
}

/** Opens the compare overlay, to pick (or change) the face to compare with. */
function openCompare(): void {
  const cp = compareCodePoint();
  if (cp !== null && faceId.value !== null) void compare.start(faceId.value, cp);
}

/** Sorts the wall by the selected glyph of this face (E49.9). */
function findLike(): void {
  const g = glyphs.selected;
  const face = props.detail?.face;
  if (g && face) emit('findLike', { face: face.id, glyph: charOf(g), family: face.familyName });
}

/** Closes the glyph popover and puts focus back on the grid, on the cell. */
function closeDetail(): void {
  popoverOpen.value = false;
  grid.value?.focus({ preventScroll: true });
}

async function onKeyDown(e: KeyboardEvent): Promise<void> {
  if (e.key === 'Escape' && popoverOpen.value) {
    e.preventDefault();
    closeDetail();
    return;
  }
  if (e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey)) {
    if (cursor.value >= 0) {
      e.preventDefault();
      openMenu(cursor.value);
    }
    return;
  }
  if (e.key === 'Enter' || e.key === ' ') {
    if (cursor.value >= 0) {
      e.preventDefault();
      if (detailsOnClick.value) openDetail(cursor.value);
      else choose(cursor.value);
    }
    return;
  }
  // Left/Right follow what the user sees: the grid runs from the right in RTL chrome.
  const next = moveCursor(glyphs.total, Math.max(cursor.value, 0), inlineKey(e.key), columns.value);
  if (next === null) return;
  e.preventDefault();
  const to = cursor.value < 0 && e.key !== 'Home' && e.key !== 'End' ? 0 : next;
  cursor.value = to;
  await reveal(to);
  // Home or End can land on a page that has not loaded yet.
  await glyphs.ensure(to, to + 1);
  if (cursor.value === to) choose(to);
}

function onClick(index: number): void {
  if (detailsOnClick.value) openDetail(index);
  else choose(index);
  grid.value?.focus({ preventScroll: true });
}

// The popover sits beside its cell, on the inline-end side when there is room, else on the other
// side, within the section; its top is the cell's top. Only where it is drawn is computed here.
const mapEl = ref<HTMLElement | null>(null);
const popover = ref<HTMLElement | null>(null);
const popoverPos = ref<{ left: number; top: number; width: number }>({
  left: 0,
  top: 0,
  width: POPOVER_WIDTH,
});
function placePopover(): void {
  const map = mapEl.value;
  const cellEl = cursor.value >= 0 ? document.getElementById(cellId(cursor.value)) : null;
  if (!map || !cellEl) return;
  const m = map.getBoundingClientRect();
  const c = cellEl.getBoundingClientRect();
  const w = Math.min(POPOVER_WIDTH, m.width || POPOVER_WIDTH);
  const rtl = getComputedStyle(map).direction === 'rtl';
  const after = rtl ? c.left - m.left - POPOVER_GAP - w : c.right - m.left + POPOVER_GAP;
  const before = rtl ? c.right - m.left + POPOVER_GAP : c.left - m.left - POPOVER_GAP - w;
  const fits = (x: number) => x >= 0 && x + w <= m.width;
  const left = fits(after)
    ? after
    : fits(before)
      ? before
      : Math.max(0, Math.min(c.left - m.left, m.width - w));
  popoverPos.value = { left, top: c.top - m.top, width: w };
}
const popoverStyle = computed(() => ({
  left: px(popoverPos.value.left),
  top: px(popoverPos.value.top),
  width: px(popoverPos.value.width),
}));
watch(
  [popoverOpen, cursor, width, cell],
  async ([open]) => {
    if (!open) return;
    await nextTick();
    placePopover();
  },
  { flush: 'post' },
);

// A press outside the popover closes it: on a cell it moves to that cell instead (the click does),
// inside the context menu or a dialog (the compare overlay) it stays. Focus goes back to the grid
// unless the press lands on a control, which takes the focus itself.
function onOutsidePress(e: PointerEvent): void {
  const target = e.target;
  if (!(target instanceof Element)) return;
  if (popover.value?.contains(target)) return;
  if (target.closest('[role="gridcell"]') && grid.value?.contains(target)) return;
  // The compare overlay and the context menu (and any other dialog or layer) work with it open.
  if (target.closest('[role="dialog"], [data-ui-popover]')) return;
  popoverOpen.value = false;
  // After the press has moved the focus (or not): a press on nothing focusable leaves it on the
  // grid, on the cell.
  setTimeout(() => {
    const active = document.activeElement;
    if (!active || active === document.body) grid.value?.focus({ preventScroll: true });
  });
}
watch(popoverOpen, (open) => {
  if (open) document.addEventListener('pointerdown', onOutsidePress, true);
  else document.removeEventListener('pointerdown', onOutsidePress, true);
});
onBeforeUnmount(() => document.removeEventListener('pointerdown', onOutsidePress, true));

// The context menu of a cell: the detail (the way to it with 'Details on click' off), copy,
// 'fonts with a … like this' and compare.
const menuIndex = ref<number | null>(null);
const menuAnchor = ref<HTMLElement | null>(null);
const menuEl = ref<HTMLElement | null>(null);
const menuGlyph = computed(() => (menuIndex.value === null ? null : glyphs.item(menuIndex.value)));
function openMenu(index: number, e?: MouseEvent): void {
  const g = glyphs.item(index);
  if (!g) return;
  cursor.value = index;
  void glyphs.select(g);
  menuAnchor.value =
    (e?.currentTarget as HTMLElement | undefined) ?? document.getElementById(cellId(index));
  menuIndex.value = index;
}
function closeMenu(focusGrid = true): void {
  menuIndex.value = null;
  if (focusGrid) grid.value?.focus({ preventScroll: true });
}
function focusMenu(): void {
  menuEl.value?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
}
function onMenuKey(e: KeyboardEvent): void {
  e.stopPropagation();
  if (e.key === 'Escape' || e.key === 'Tab') {
    e.preventDefault();
    closeMenu();
    return;
  }
  const items = [...(menuEl.value?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])];
  const at = items.indexOf(document.activeElement as HTMLElement);
  let to: number | null = null;
  if (e.key === 'ArrowDown') to = (at + 1) % items.length;
  else if (e.key === 'ArrowUp') to = (at - 1 + items.length) % items.length;
  else if (e.key === 'Home') to = 0;
  else if (e.key === 'End') to = items.length - 1;
  if (to === null) return;
  e.preventDefault();
  items[to]?.focus();
}
function menuDetails(): void {
  const i = menuIndex.value;
  closeMenu();
  if (i !== null) openDetail(i);
}
function menuCopy(): void {
  const g = menuGlyph.value;
  closeMenu();
  if (g) void copy(charOf(g));
}
function menuLike(): void {
  closeMenu();
  findLike();
}
function menuCompare(): void {
  closeMenu(false);
  openCompare();
}

// A new list (a search, a block, a face): the top of the grid, or the glyph a search selected.
watch(
  () => glyphs.generation,
  async () => {
    cursor.value = -1;
    const el = grid.value;
    const s = scroller.value;
    if (el && s) {
      const above = el.getBoundingClientRect().top - s.getBoundingClientRect().top;
      if (above < 0) s.scrollTop += above - REVEAL_MARGIN;
    }
    measure();
    await nextTick();
    const i = glyphs.query.trim() ? glyphs.selectedIndex : -1;
    if (i >= 0) {
      cursor.value = i;
      await reveal(i);
      // A search that finds a character shows its detail, as a click would.
      if (detailsOnClick.value) popoverOpen.value = true;
    } else {
      popoverOpen.value = false;
    }
  },
);

const searchText = ref('');
watch(
  () => glyphs.query,
  (q) => {
    if (q !== searchText.value) searchText.value = q;
  },
);
function onSearch(): void {
  void glyphs.search(searchText.value);
}
/** A block chosen in the coverage list: that block in the grid, focus on the grid. */
function showBlock(name: string): void {
  void glyphs.showBlock(name);
  grid.value?.focus({ preventScroll: true });
}
/** A block chip: that block only, or every block again when it is already on. */
function toggleBlock(name: string): void {
  void glyphs.chooseBlock(glyphs.block === name ? null : name);
}

const sel = computed(() => glyphs.selected);
const selDetail = computed(() =>
  glyphs.detail && glyphs.detail.codePoint === sel.value?.codePoint ? glyphs.detail : null,
);
const detailBroken = computed(() =>
  selDetail.value?.health.kind === 'broken' ? selDetail.value.health.reason : null,
);

// 'In N of M styles' asks every style of the family, and asks again when the family changes.
watch([() => sel.value?.codePoint ?? null, () => page.faces], ([cp]) => glyphs.countStyles(cp), {
  immediate: true,
});
const stylesText = computed(() => {
  if (glyphs.styleCountFailed) return t('glyphs.advanceUnknown');
  const count = glyphs.styleCount;
  if (!count) return t('glyphs.advanceLoading');
  return count.have === count.total && count.total > 1
    ? t('glyphs.inAllStyles', { count: count.total })
    : t('glyphs.inSomeStyles', { have: count.have, total: count.total });
});

const moved = computed(() => (faceId.value !== null && axesSet.values(faceId.value)) || []);
const alternates = computed(() =>
  glyphAlternates(selDetail.value, props.detail?.axes ?? [], moved.value),
);
function alternateStyle(a: GlyphAlternate) {
  return {
    ...props.faceStyle,
    ...(a.fontFeatureSettings ? { fontFeatureSettings: a.fontFeatureSettings } : {}),
    ...(a.fontVariationSettings ? { fontVariationSettings: a.fontVariationSettings } : {}),
  };
}
const alternateLabel = (a: GlyphAlternate) =>
  a.value === undefined ? a.name : `${a.name} ${a.value}`;

const moreOpen = ref(false);
/** The features that produce or replace the glyph, as chips: each tag once. */
const featureChips = computed(() => {
  const d = selDetail.value;
  if (!d) return [];
  const seen = new Set<string>();
  return [...d.producedBy, ...d.replacedBy].filter((f) => !seen.has(f.tag) && seen.add(f.tag));
});
/** "LATIN SMALL LETTER G · glyph g": what Unicode and the font call the character. */
const namesLine = computed(() => {
  const g = sel.value;
  if (!g) return '';
  const parts = [
    selDetail.value?.unicodeName,
    g.name ? t('glyphs.glyphNamed', { name: g.name }) : null,
  ];
  return parts.filter(Boolean).join(' · ');
});

// The comparison (E50.9): the compared face's glyph, its metrics beside this face's, and its
// font for the ghost under the glyph.
const comparedFamily = computed(() => compare.other?.face.familyName ?? '');
/**
 * Puts focus on the compare entry ('Compare with…' or 'change') that replaced a button the
 * comparison removed: in the popover, else in the control row. Only when focus was lost.
 */
async function refocusCompareEntry(prefer: 'popover' | 'chip' = 'popover'): Promise<void> {
  await nextTick();
  const active = document.activeElement;
  if (active && active !== document.body) return;
  const inPopover = popover.value?.querySelector<HTMLElement>('[data-compare-entry]');
  const inChip = mapEl.value?.querySelector<HTMLElement>('.controls [data-compare-entry]');
  const to = prefer === 'chip' ? (inChip ?? inPopover) : (inPopover ?? inChip);
  to?.focus();
}
// The overlay gives focus back to what opened it; a pick swaps 'Compare with…' for 'change', so
// when that button is gone the focus goes to its successor.
watch(
  () => compare.open,
  (open) => {
    if (!open) void refocusCompareEntry();
  },
);
/** Ends the comparison from the popover's 'stop' or the chip's ×, which both disappear with it. */
function stopCompare(from: 'popover' | 'chip'): void {
  compare.stop();
  void refocusCompareEntry(from);
}
watch(
  [() => sel.value?.codePoint ?? null, () => compare.other?.face.id ?? null],
  ([cp]) => {
    if (compare.active) void compare.loadOtherGlyph(cp);
  },
  { immediate: true },
);
const metricRow = computed(() =>
  compare.active
    ? glyphMetricRow(
        { glyph: selDetail.value, metrics: props.detail?.metrics },
        { glyph: compare.otherGlyph, metrics: compare.other?.metrics },
      )
    : [],
);
const metricText = (v: number | null) => (v === null ? t('glyphs.metricNone') : String(v));
const loader = useFaceLoader();
const ghostFamily = ref<string | null>(null);
let ghostHeld: FaceId | null = null;
function holdGhost(id: FaceId | null): void {
  if (ghostHeld === id) return;
  if (ghostHeld !== null) loader.release(ghostHeld);
  ghostHeld = id;
  ghostFamily.value = null;
  if (id === null) return;
  loader.acquire(id).then(
    (name) => {
      if (ghostHeld === id) ghostFamily.value = name;
    },
    () => undefined,
  );
}
watch(
  () => (popoverOpen.value && compare.active ? (compare.other?.face.id ?? null) : null),
  holdGhost,
  { immediate: true },
);
onBeforeUnmount(() => holdGhost(null));
/** The ghost draws only where the compared face has the character. */
const ghostShown = computed(
  () => ghostFamily.value !== null && compare.otherGlyph?.codePoint === sel.value?.codePoint,
);
const featureName = (f: FeatureInfo) => f.name ?? t('glyphs.unregisteredFeature');
const broken = computed(() => (glyphs.health.kind === 'broken' ? glyphs.health.reason : null));
const noResults = computed(
  () => !glyphs.loading && !broken.value && glyphs.total === 0 && glyphs.face !== null,
);
</script>

<template>
  <p v-if="!detail" class="placeholder">{{ t('glyphs.noFace') }}</p>
  <div v-else class="characters" data-testid="family-characters">
    <div ref="mapEl" class="map">
      <div class="controls">
        <span class="search">
          <UiIcon name="search" class="search-icon" />
          <input
            v-model="searchText"
            class="field"
            type="search"
            :aria-label="t('glyphs.search')"
            :placeholder="t('glyphs.searchPlaceholder')"
            @input="onSearch"
          />
        </span>
        <div class="blocks" role="group" :aria-label="t('glyphs.blocks')">
          <button
            v-for="b in glyphs.faceBlocks"
            :key="b.name"
            type="button"
            class="chip"
            :aria-pressed="glyphs.block === b.name"
            @click="toggleBlock(b.name)"
          >
            {{ b.name }}
            <span class="chip-count">{{ b.count }}</span>
          </button>
        </div>
        <span class="grid-controls">
          <span class="cell-size">
            <span class="small" aria-hidden="true">{{ t('preview.sizeGlyph') }}</span>
            <input
              type="range"
              data-testid="glyph-cell-size"
              :aria-label="t('glyphs.cellSize')"
              :min="CELL.min"
              :max="CELL.max"
              :step="CELL.step"
              :value="cell"
              @input="slideCell"
            />
            <span class="large" aria-hidden="true">{{ t('preview.sizeGlyph') }}</span>
          </span>
          <span v-if="compare.active" class="compare-chip on" data-testid="glyph-compare-chip">
            <button
              type="button"
              class="compare-open"
              data-compare-entry
              aria-haspopup="dialog"
              :aria-label="t('glyphs.compareChange', { family: comparedFamily })"
              @click="openCompare"
            >
              {{ t('glyphs.compareOn', { family: comparedFamily }) }}
            </button>
            <UiIconButton
              icon="close"
              size="sm"
              data-testid="glyph-compare-stop"
              :label="t('glyphs.compareStop', { family: comparedFamily })"
              @click="stopCompare('chip')"
            />
          </span>
          <button
            v-else
            type="button"
            class="compare-chip"
            data-testid="glyph-compare-chip"
            data-compare-entry
            aria-haspopup="dialog"
            :title="t('glyphs.compareHint')"
            @click="openCompare"
          >
            {{ t('glyphs.compareWith') }}
            <UiIcon name="plus" />
          </button>
          <span class="switch">
            <span id="fm-glyph-lines" aria-hidden="true">{{ t('glyphs.gridLines') }}</span>
            <UiSwitch
              :checked="lines"
              aria-labelledby="fm-glyph-lines"
              data-testid="glyph-grid-lines"
              @click="toggleLines"
            />
          </span>
          <span class="switch">
            <span id="fm-glyph-details" aria-hidden="true">{{ t('glyphs.detailsOnClick') }}</span>
            <UiSwitch
              :checked="detailsOnClick"
              aria-labelledby="fm-glyph-details"
              data-testid="glyph-details-on-click"
              @click="toggleDetails"
            />
          </span>
        </span>
      </div>
      <div
        v-if="specimen"
        class="specimen fm-true-size"
        data-testid="glyph-specimen"
        role="img"
        :aria-label="t('glyphs.specimenLabel')"
        :style="{ height: px(specimen.height) }"
      >
        <span
          v-for="r in rulers"
          :key="r.id"
          class="ruler"
          :class="r.id"
          :data-ruler="r.id"
          :style="{ top: px(r.top) }"
        >
          <span class="ruler-label">{{ t(`glyphs.rulers.${r.id}`) }}</span>
        </span>
        <div ref="specimenLine" class="specimen-line" :style="{ fontSize: `${specimenPx}px` }">
          <span class="strut" :style="{ height: px(specimen.baseline) }" /><span
            ref="specimenText"
            class="specimen-text"
            :class="{ loading: !faceStyle }"
            :style="faceStyle"
            >{{ t('glyphs.specimen') }}</span
          >
        </div>
      </div>
      <p v-if="broken" class="placeholder" role="status">
        {{ t('glyphs.broken', { reason: broken }) }}
      </p>
      <p v-else-if="glyphs.error" class="placeholder" role="alert">
        {{ errorMessage(glyphs.error) }}
      </p>
      <p v-else-if="noResults" class="placeholder" role="status">
        {{ glyphs.query.trim() ? t('glyphs.noMatch') : t('glyphs.empty') }}
      </p>
      <p class="sr-only" aria-live="polite">
        {{ t('glyphs.characters', { count: glyphs.total }) }}
      </p>
      <div
        ref="grid"
        class="grid"
        :class="{ lines }"
        role="grid"
        data-keeps-arrows
        :aria-label="t('glyphs.grid')"
        :aria-rowcount="rowCount"
        :aria-colcount="columns"
        :aria-activedescendant="activeId"
        tabindex="0"
        @keydown="onKeyDown"
      >
        <div class="sizer fm-true-size" :style="{ height: `${rowCount * rowHeight}px` }">
          <div
            v-for="v in visible"
            :key="v.r"
            role="row"
            class="row"
            :aria-rowindex="v.r + 1"
            :style="{
              ...rowStyle,
              transform: `translateY(${v.r * rowHeight}px)`,
              height: `${rowHeight}px`,
            }"
          >
            <div
              v-for="c in v.cells"
              :id="cellId(c.index)"
              :key="c.index"
              role="gridcell"
              class="cell"
              :class="{ cursor: c.index === cursor }"
              :aria-label="cellLabel(c.glyph, c.index)"
              :aria-selected="!!c.glyph && sel?.codePoint === c.glyph.codePoint"
              :aria-haspopup="detailsOnClick ? 'dialog' : undefined"
              :title="cellLabel(c.glyph, c.index)"
              :data-code-point="c.glyph?.codePoint"
              @click="onClick(c.index)"
              @contextmenu.prevent="openMenu(c.index, $event)"
            >
              <span
                v-if="c.glyph"
                class="char"
                :class="{ loading: !faceStyle }"
                :style="faceStyle"
                aria-hidden="true"
                >{{ charOf(c.glyph) }}</span
              >
              <span v-if="c.glyph" class="cp" aria-hidden="true">{{
                codePointLabel(c.glyph.codePoint)
              }}</span>
            </div>
          </div>
        </div>
      </div>
      <section
        v-if="sel && popoverOpen"
        ref="popover"
        class="popover"
        role="dialog"
        data-testid="glyph-card"
        :aria-label="t('glyphs.selected')"
        :style="popoverStyle"
        @keydown.esc.stop.prevent="closeDetail"
      >
        <UiIconButton
          class="popover-close"
          icon="close"
          size="sm"
          data-testid="glyph-popover-close"
          :label="t('glyphs.closeDetails')"
          @click="closeDetail"
        />
        <div class="popover-head">
          <div class="paper" aria-hidden="true">
            <span
              v-if="compare.active && ghostShown"
              class="ghost"
              data-testid="glyph-ghost"
              :style="{ fontFamily: `'${ghostFamily}'` }"
              >{{ charOf(sel) }}</span
            >
            <span
              class="big"
              data-testid="glyph-big"
              :class="{ loading: !faceStyle }"
              :style="faceStyle"
              >{{ charOf(sel) }}</span
            >
          </div>
          <div class="ident">
            <p class="ident-line">
              <span class="ident-char" :class="{ loading: !faceStyle }" :style="faceStyle">{{
                charOf(sel)
              }}</span>
              <span class="mono" data-testid="glyph-code">{{ codePointLabel(sel.codePoint) }}</span>
            </p>
            <p class="muted names" data-testid="glyph-names">{{ namesLine }}</p>
            <ul
              v-if="featureChips.length"
              class="feature-chips"
              :aria-label="t('glyphs.features')"
              data-testid="glyph-features"
            >
              <li
                v-for="f in featureChips"
                :key="f.tag"
                class="feature-chip"
                :title="featureName(f)"
              >
                <CopyValue :value="f.tag" :label="t('glyphs.featureTag')" />
              </li>
            </ul>
          </div>
        </div>
        <p v-if="glyphs.detailError" class="muted" role="alert">
          {{ errorMessage(glyphs.detailError) }}
        </p>
        <p v-else-if="detailBroken" class="muted" role="status">
          {{ t('glyphs.detailBroken', { reason: detailBroken }) }}
        </p>
        <p class="compare-line">
          <UiIcon name="compare" />
          <template v-if="compare.active">
            <span>{{ t('glyphs.comparing') }}</span>
            <strong>{{ comparedFamily }}</strong>
            <span aria-hidden="true">·</span>
            <button
              type="button"
              class="link"
              data-compare-entry
              aria-haspopup="dialog"
              :aria-label="t('glyphs.compareChange', { family: comparedFamily })"
              @click="openCompare"
            >
              {{ t('glyphs.change') }}
            </button>
            <span aria-hidden="true">·</span>
            <button
              type="button"
              class="link muted"
              :aria-label="t('glyphs.compareStop', { family: comparedFamily })"
              @click="stopCompare('popover')"
            >
              {{ t('glyphs.stop') }}
            </button>
          </template>
          <template v-else>
            <button
              type="button"
              class="link"
              data-compare-entry
              aria-haspopup="dialog"
              :title="t('glyphs.compareHint')"
              @click="openCompare"
            >
              {{ t('glyphs.compareWith') }}
            </button>
            <span class="muted">{{ t('glyphs.compareInvite') }}</span>
          </template>
        </p>
        <div v-if="compare.active" class="metrics" data-testid="glyph-metrics">
          <p class="metrics-head">
            <span class="metrics-title">{{ t('glyphs.metricsTitle') }}</span>
            <span class="muted"
              >{{ detail.face.familyName }} · <span class="accent">{{ comparedFamily }}</span></span
            >
          </p>
          <dl class="metrics-row">
            <div v-for="m in metricRow" :key="m.id" class="metric" :data-metric="m.id">
              <dt>{{ t(`glyphs.metric.${m.id}`) }}</dt>
              <dd
                :aria-label="
                  t('glyphs.metricValues', {
                    first: detail.face.familyName,
                    mine: metricText(m.mine),
                    second: comparedFamily,
                    theirs: metricText(m.theirs),
                  })
                "
              >
                <span class="mine">{{ metricText(m.mine) }}</span>
                <span class="theirs accent">{{ metricText(m.theirs) }}</span>
              </dd>
            </div>
          </dl>
        </div>
        <button
          type="button"
          class="more fm-directional"
          aria-controls="fm-glyph-more"
          :aria-expanded="moreOpen"
          @click="moreOpen = !moreOpen"
        >
          <UiChevron :open="moreOpen" />
          {{ t('glyphs.moreDetails') }}
        </button>
        <div v-if="moreOpen" id="fm-glyph-more" class="more-body">
          <dl class="facts">
            <dt id="fm-glyph-styles">{{ t('glyphs.inStyles') }}</dt>
            <dd aria-labelledby="fm-glyph-styles">{{ stylesText }}</dd>
            <dt id="fm-glyph-alternates">{{ t('glyphs.alternates') }}</dt>
            <dd>
              <ul v-if="alternates.length" class="alternates" aria-labelledby="fm-glyph-alternates">
                <li
                  v-for="a in alternates"
                  :key="a.key"
                  class="alternate"
                  :title="alternateLabel(a)"
                  data-testid="glyph-alternate"
                >
                  <span
                    class="alternate-char"
                    :class="{ loading: !faceStyle }"
                    :style="alternateStyle(a)"
                    aria-hidden="true"
                    >{{ charOf(sel) }}</span
                  >
                  <span class="sr-only">{{ alternateLabel(a) }}</span>
                </li>
              </ul>
              <span v-else class="muted">{{ t('glyphs.none') }}</span>
            </dd>
            <dt id="fm-glyph-id">{{ t('glyphs.glyphId') }}</dt>
            <dd aria-labelledby="fm-glyph-id">{{ sel.glyphId }}</dd>
            <dt id="fm-glyph-block">{{ t('glyphs.block') }}</dt>
            <dd aria-labelledby="fm-glyph-block">{{ sel.block }}</dd>
          </dl>
          <div class="actions">
            <button type="button" class="action" @click="copy(charOf(sel))">
              {{ t('glyphs.copyGlyph') }}
            </button>
            <button type="button" class="action" data-testid="glyph-like" @click="findLike">
              {{ likeThisLabel(charOf(sel)) }}
            </button>
            <span class="muted copied" role="status">
              <template v-if="copyStatus === 'copied'">{{ t('common.copied') }}</template>
              <template v-else-if="copyStatus === 'failed'">{{ t('copy.failed') }}</template>
            </span>
          </div>
        </div>
      </section>
      <FamilyTraits :traits="detail.glyphTraits ?? []" @find="emit('findTrait', $event)" />
      <GlyphCoverage @block="showBlock" />
      <p v-if="glyphs.unicodeVersion" class="muted note">
        {{ t('glyphs.unicodeVersion', { version: glyphs.unicodeVersion }) }}
      </p>
    </div>
    <UiPopover
      :open="menuGlyph !== null"
      :anchor="menuAnchor"
      @close="closeMenu(false)"
      @placed="focusMenu"
    >
      <div
        v-if="menuGlyph"
        ref="menuEl"
        class="menu"
        role="menu"
        data-testid="glyph-menu"
        :aria-label="t('glyphs.menu', { codePoint: codePointLabel(menuGlyph.codePoint) })"
        @keydown="onMenuKey"
      >
        <button type="button" role="menuitem" tabindex="-1" @click="menuDetails">
          {{ t('glyphs.menuDetails') }}
        </button>
        <button type="button" role="menuitem" tabindex="-1" @click="menuCopy">
          {{ t('glyphs.copyGlyph') }}
        </button>
        <button type="button" role="menuitem" tabindex="-1" @click="menuLike">
          {{ likeThisLabel(charOf(menuGlyph)) }}
        </button>
        <button
          type="button"
          role="menuitem"
          tabindex="-1"
          aria-haspopup="dialog"
          @click="menuCompare"
        >
          {{ t('glyphs.compareWith') }}
        </button>
      </div>
    </UiPopover>
    <GlyphCompare v-if="compare.open" />
  </div>
</template>

<style scoped>
/* The grid takes the whole width; the glyph detail floats over it (E50.9). */
.characters {
  min-width: 0;
}
.map {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-3);
  min-width: 0;
}
.placeholder,
.muted {
  margin: 0;
  color: var(--fm-muted);
}
.controls {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-2);
}
.search {
  position: relative;
  display: flex;
  flex: 0 1 240px;
  min-width: 160px;
}
.search-icon {
  position: absolute;
  inset-block-start: 50%;
  inset-inline-start: var(--fm-space-2);
  color: var(--fm-muted);
  transform: translateY(-50%);
  pointer-events: none;
}
.field {
  flex: 1;
  min-width: 0;
  height: 32px;
  padding-block: 0;
  padding-inline: calc(var(--fm-space-2) + 22px) var(--fm-space-2);
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-sm);
  background-color: var(--fm-bg);
  color: var(--fm-fg);
  font: inherit;
}
/* The chips flow on from the search field, as one wrapping line. */
.blocks {
  display: contents;
}
.chip {
  display: inline-flex;
  align-items: baseline;
  gap: var(--fm-space-1);
  padding: var(--fm-space-1) var(--fm-space-2);
  border: 1px solid transparent;
  border-radius: 999px;
  background-color: var(--fm-surface);
  color: var(--fm-fg);
  font: inherit;
  font-size: var(--fm-text-sm);
  cursor: pointer;
}
/* A solid edge: a tint alone is not the 3:1 a selected state needs. */
.chip[aria-pressed='true'] {
  border-color: var(--fm-fg);
  background-color: var(--fm-hover);
}
.chip-count {
  color: var(--fm-muted);
  font-variant-numeric: tabular-nums;
}
.grid {
  position: relative;
  border-radius: var(--fm-radius-sm);
}
.grid:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
.sizer {
  position: relative;
}
.row {
  position: absolute;
  top: 0;
  inset-inline: 0;
  display: grid;
  /* grid-template-columns and font-size come from the cell slider (rowStyle). */
}
.cell {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  box-sizing: border-box;
  border-radius: var(--fm-radius-md);
  cursor: default;
  transition: background-color var(--fm-motion-fast) var(--fm-ease);
}
/* The hairline grid: 0.5 px at 7 % ink between cells, square so the lines meet. */
.grid.lines .cell {
  border-inline-end: 0.5px solid color-mix(in srgb, var(--fm-fg) 7%, transparent);
  border-block-end: 0.5px solid color-mix(in srgb, var(--fm-fg) 7%, transparent);
  border-radius: 0;
}
.cell:hover {
  background-color: var(--fm-hover);
}
/* An inset ring, so the selection does not move the hairlines. */
.cell[aria-selected='true'] {
  box-shadow: inset 0 0 0 2px var(--fm-accent);
  background-color: color-mix(in srgb, var(--fm-accent) 12%, transparent);
}
/* The code point at the corner of the hovered cell. */
.cp {
  position: absolute;
  inset-block-start: 3px;
  inset-inline-start: 4px;
  color: var(--fm-muted);
  font-family: var(--fm-font-mono);
  font-size: 9px;
  line-height: 1;
  opacity: 0;
  pointer-events: none;
}
.cell:hover .cp {
  opacity: 1;
}
.grid:focus-visible .cell.cursor {
  outline: 2px solid var(--fm-accent);
  outline-offset: -2px;
}
.grid:focus-visible .cell.cursor[aria-selected='true'] {
  outline-offset: -6px;
}
.char {
  line-height: 1;
}
.grid-controls {
  display: flex;
  align-items: center;
  gap: var(--fm-space-4);
  margin-inline-start: auto;
}
.cell-size,
.switch {
  display: flex;
  align-items: center;
  gap: var(--fm-space-2);
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.cell-size .small {
  font-size: var(--fm-text-xs);
}
.cell-size .large {
  font-size: var(--fm-text-lg);
}
.cell-size input {
  width: 96px;
  accent-color: var(--fm-accent);
}
.cell-size input:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
/* The specimen: its box is the face's ascender to its descender, so descenders stay in it. */
.specimen {
  position: relative;
  margin-block: var(--fm-space-3);
}
.ruler {
  position: absolute;
  inset-inline: 0;
  height: 0;
  border-block-start: 1px dashed color-mix(in srgb, var(--fm-fg) 12%, transparent);
  pointer-events: none;
}
.ruler.baseline {
  border-block-start-style: solid;
  border-block-start-color: color-mix(in srgb, var(--fm-fg) 20%, transparent);
}
.ruler-label {
  position: absolute;
  inset-inline-end: 0;
  inset-block-end: 2px;
  color: var(--fm-muted);
  font-size: 10px;
  line-height: 1;
}
/* line-height 0: the strut alone sets the line's top, so the baseline lands on its bottom. */
.specimen-line {
  position: relative;
  overflow-x: clip;
  margin-inline-end: 72px;
  line-height: 0;
  white-space: nowrap;
}
.strut {
  display: inline-block;
  width: 0;
  vertical-align: baseline;
}
/* The compare chip of the control row: an outline, the accent while comparing. */
.compare-chip {
  display: inline-flex;
  align-items: center;
  gap: var(--fm-space-1);
  height: 32px;
  padding-inline: var(--fm-space-3) var(--fm-space-2);
  border: 1px solid var(--fm-control-border);
  border-radius: 999px;
  background-color: var(--fm-bg);
  color: var(--fm-fg);
  font: inherit;
  font-size: var(--fm-text-sm);
  white-space: nowrap;
  cursor: pointer;
}
.compare-chip.on {
  border-color: var(--fm-accent);
  cursor: default;
}
.compare-open {
  padding: 0;
  border: 0;
  background: none;
  color: inherit;
  font: inherit;
  cursor: pointer;
}
.compare-chip:focus-visible,
.compare-open:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
/* The glyph popover: 360 px, beside its cell (placePopover), over the grid. */
.popover {
  position: absolute;
  z-index: var(--fm-z-popover);
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-3);
  box-sizing: border-box;
  padding: var(--fm-space-4);
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-bg);
  box-shadow: var(--fm-shadow-2);
}
.popover-close {
  position: absolute;
  inset-block-start: var(--fm-space-2);
  inset-inline-end: var(--fm-space-2);
}
.popover-head {
  display: flex;
  gap: var(--fm-space-4);
  align-items: center;
  padding-inline-end: var(--fm-space-5);
}
/* The glyph on paper; the compared glyph ghosted under it on the same baseline. */
.paper {
  display: grid;
  flex: none;
  grid-template-areas: 'glyph';
  align-items: baseline;
  justify-items: center;
  width: 120px;
  height: 120px;
  overflow: hidden;
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-surface);
  color: var(--fm-fg);
  font-size: 92px;
  line-height: 120px;
}
.paper > span {
  grid-area: glyph;
}
.ghost {
  color: color-mix(in srgb, var(--fm-accent) 40%, transparent);
}
.big {
  position: relative;
  white-space: nowrap;
}
.ident {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-1);
  min-width: 0;
}
.ident p {
  margin: 0;
}
.ident-line {
  display: flex;
  align-items: baseline;
  gap: var(--fm-space-2);
}
.ident-char {
  font-size: var(--fm-text-lg);
}
.names {
  font-size: var(--fm-text-sm);
  overflow-wrap: anywhere;
}
.feature-chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-1);
  margin: var(--fm-space-1) 0 0;
  padding: 0;
  list-style: none;
}
.feature-chip {
  padding: 2px var(--fm-space-2);
  border-radius: 999px;
  background-color: var(--fm-surface);
  font-size: var(--fm-text-xs);
}
.compare-line {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-1);
  margin: 0;
  padding-block-start: var(--fm-space-3);
  border-block-start: 1px solid var(--fm-border);
  font-size: var(--fm-text-sm);
}
.link {
  padding: 0;
  border: 0;
  background: none;
  color: var(--fm-fg);
  font: inherit;
  text-decoration: underline;
  text-underline-offset: 3px;
  cursor: pointer;
}
.link.muted {
  color: var(--fm-muted);
}
.link:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
/* The compared face's words in the accent's hue, mixed toward the text colour so they reach
   4.5:1 (the accent alone is not a text colour), as the missing-glyph line does. */
.accent {
  color: color-mix(in srgb, var(--fm-accent) 65%, var(--fm-fg));
}
.metrics {
  padding-block-start: var(--fm-space-3);
  border-block-start: 1px solid var(--fm-border);
}
.metrics-head {
  display: flex;
  justify-content: space-between;
  gap: var(--fm-space-2);
  margin: 0 0 var(--fm-space-2);
  font-size: var(--fm-text-xs);
}
.metrics-title {
  color: var(--fm-muted);
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}
.metrics-row {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--fm-space-2);
  margin: 0;
}
.metric dt {
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
}
.metric dd {
  display: flex;
  gap: var(--fm-space-2);
  margin: 0;
  font-variant-numeric: tabular-nums;
}
/* Never a fallback font: nothing shows until the face has loaded. */
.loading {
  visibility: hidden;
}
.facts {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: var(--fm-space-1) var(--fm-space-3);
  margin: 0;
  font-size: var(--fm-text-sm);
}
.facts dt {
  color: var(--fm-muted);
}
.facts dd {
  margin: 0;
  overflow-wrap: anywhere;
}
.mono {
  font-family: var(--fm-font-mono);
}
.alternates {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-1) var(--fm-space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}
.alternate-char {
  font-size: 26px;
  line-height: 1.1;
}
.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-2);
}
.action {
  padding: var(--fm-space-1) var(--fm-space-2);
  border: 1px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-sm);
  background-color: var(--fm-bg);
  color: var(--fm-fg);
  font: inherit;
  font-size: var(--fm-text-sm);
  white-space: nowrap;
}
.action:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
.copied {
  font-size: var(--fm-text-xs);
}
.more {
  display: flex;
  align-items: center;
  gap: var(--fm-space-1);
  padding: 0;
  border: 0;
  background: none;
  color: var(--fm-muted);
  font: inherit;
  font-size: var(--fm-text-sm);
  cursor: pointer;
}
.more-body {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
}
.menu {
  display: flex;
  flex-direction: column;
}
.menu button {
  display: block;
  width: 100%;
  height: 28px;
  padding: 0 var(--fm-space-2);
  border: 0;
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: var(--fm-fg);
  font: inherit;
  font-size: var(--fm-text-sm);
  text-align: start;
  white-space: nowrap;
}
.menu button:hover,
.menu button:focus-visible {
  background-color: var(--fm-hover);
}
/* The global 2 px accent ring marks the focused item, drawn inside so the layer does not clip it. */
.menu button:focus-visible {
  outline-offset: -2px;
}
.note {
  font-size: var(--fm-text-xs);
}
</style>
