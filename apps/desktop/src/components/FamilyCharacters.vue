<script setup lang="ts">
// The Characters section of the family page (E42.5): every character the face maps, drawn by the
// webview from the face's own file, in twelve columns. fm-core lists the characters, runs the
// search and answers for the detail; this section lays them out. The grid has no scrollbar of its
// own: it is as tall as its rows and the page scrolls it, so only the rows in the page's window
// exist, and a CJK face with 30k+ characters scrolls like a Latin one.
//
// Keyboard (WAI-ARIA grid): the grid holds focus and points at the cursor cell with
// aria-activedescendant; arrow keys move by cell and row and Home/End to the ends, and the detail
// card follows. Enter or a click selects the cursor's glyph again after Escape closed the card.
// Tab reaches the card; Escape closes it and returns to the grid. Below the grid, the coverage
// list (GlyphCoverage): a block chosen there shows that block in the grid and focuses it.
// Under the grid, the face's detected letter shapes (FamilyTraits, E49.10).
import { computed, nextTick, onBeforeUnmount, ref, shallowRef, watch, watchEffect } from 'vue';
import type { FaceDetail, FeatureInfo, GlyphInfo } from '../api/types';
import { glyphAlternates, type GlyphAlternate } from '../glyphs/alternates';
import {
  GLYPH_COLUMNS,
  codePointLabel,
  glyphRowCount,
  glyphRowHeight,
  moveCursor,
  rowOf,
} from '../glyphs/layout';
import { errorMessage, inlineKey, t } from '../i18n';
import { windowRange } from '../list/virtual';
import { useAxesStore } from '../stores/axes';
import { useCoverageStore } from '../stores/coverage';
import { useFamilyPageStore } from '../stores/familyPage';
import { useGlyphCompareStore } from '../stores/glyphCompare';
import { useGlyphsStore } from '../stores/glyphs';
import CopyValue from './CopyValue.vue';
import GlyphCompare from './GlyphCompare.vue';
import FamilyTraits from './FamilyTraits.vue';
import GlyphCoverage from './GlyphCoverage.vue';
import UiChevron from './ui/UiChevron.vue';
import UiIcon from './ui/UiIcon.vue';
import { useCopy } from './useCopy';
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
}>();

/** Window size before the first measurement (and where there is no layout). */
const FALLBACK_HEIGHT = 384;
const FALLBACK_WIDTH = 720;
/** Room left between a revealed row and the edge of the window. */
const REVEAL_MARGIN = 8;

const page = useFamilyPageStore();
const axesSet = useAxesStore();
const glyphs = useGlyphsStore();
const compare = useGlyphCompareStore();
const coverage = useCoverageStore();
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
  if (el.clientWidth > 0) width.value = el.clientWidth;
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

const rowHeight = computed(() => glyphRowHeight(width.value));
const rowCount = computed(() => glyphRowCount(glyphs.total));
const range = computed(() =>
  windowRange(scrollTop.value, height.value, rowHeight.value, props.overscan, rowCount.value),
);

/** The cursor: a list index, or -1 before the user moves into the grid. */
const cursor = ref(-1);
const cellId = (i: number) => `fm-glyph-${i}`;

const visible = computed(() => {
  const rows = [];
  for (let r = range.value.start; r < range.value.end; r++) {
    const first = r * GLYPH_COLUMNS;
    const count = Math.min(GLYPH_COLUMNS, glyphs.total - first);
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
    range.value.start * GLYPH_COLUMNS,
    Math.min(range.value.end * GLYPH_COLUMNS, glyphs.total),
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
  const r = rowOf(i);
  return r >= range.value.start && r < range.value.end ? cellId(i) : undefined;
});

/** Scrolls the page so list index `i` is on screen, and so rendered for aria-activedescendant. */
async function reveal(i: number): Promise<void> {
  const el = grid.value;
  const s = scroller.value;
  const row = rowOf(i) * rowHeight.value;
  if (el && s && s.clientHeight > 0) {
    const top = s.scrollTop + el.getBoundingClientRect().top - s.getBoundingClientRect().top + row;
    if (top < s.scrollTop) s.scrollTop = top - REVEAL_MARGIN;
    else if (top + rowHeight.value > s.scrollTop + s.clientHeight)
      s.scrollTop = top + rowHeight.value - s.clientHeight + REVEAL_MARGIN;
    measure();
  } else {
    scrollTop.value = row;
  }
  await nextTick();
}

function choose(index: number): void {
  cursor.value = index;
  const g = glyphs.item(index);
  if (g) void glyphs.select(g);
}

/** Opens the compare overlay on the selected character. */
function openCompare(): void {
  const g = glyphs.selected;
  if (g && faceId.value !== null) void compare.start(faceId.value, g.codePoint);
}

/** Closes the glyph detail and puts focus back on the grid. */
function closeDetail(): void {
  void glyphs.select(null);
  grid.value?.focus({ preventScroll: true });
}

async function onKeyDown(e: KeyboardEvent): Promise<void> {
  if (e.key === 'Escape' && glyphs.selected) {
    e.preventDefault();
    closeDetail();
    return;
  }
  if (e.key === 'Enter' || e.key === ' ') {
    if (cursor.value >= 0) {
      e.preventDefault();
      choose(cursor.value);
    }
    return;
  }
  // Left/Right follow what the user sees: the grid runs from the right in RTL chrome.
  const next = moveCursor(glyphs.total, Math.max(cursor.value, 0), inlineKey(e.key));
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
  choose(index);
  grid.value?.focus({ preventScroll: true });
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
const advance = computed(() => {
  const d = selDetail.value;
  if (!d) return glyphs.detailError ? t('glyphs.advanceUnknown') : t('glyphs.advanceLoading');
  return d.advanceWidth === null ? t('glyphs.none') : t('glyphs.units', { count: d.advanceWidth });
});
/** The feature lists of the detail, each with its heading and empty text. */
const featureLists = computed(() => {
  const d = selDetail.value;
  if (!d) return [];
  return [
    {
      id: 'fm-glyph-produced',
      title: t('glyphs.producedBy'),
      empty: t('glyphs.producedByNone'),
      items: d.producedBy,
    },
    {
      id: 'fm-glyph-replaced',
      title: t('glyphs.replacedBy'),
      empty: t('glyphs.replacedByNone'),
      items: d.replacedBy,
    },
  ];
});
const featureName = (f: FeatureInfo) => f.name ?? t('glyphs.unregisteredFeature');
const broken = computed(() => (glyphs.health.kind === 'broken' ? glyphs.health.reason : null));
const noResults = computed(
  () => !glyphs.loading && !broken.value && glyphs.total === 0 && glyphs.face !== null,
);
</script>

<template>
  <p v-if="!detail" class="placeholder">{{ t('glyphs.noFace') }}</p>
  <div v-else class="characters" data-testid="family-characters">
    <div class="map">
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
        role="grid"
        data-keeps-arrows
        :aria-label="t('glyphs.grid')"
        :aria-rowcount="rowCount"
        :aria-colcount="GLYPH_COLUMNS"
        :aria-activedescendant="activeId"
        tabindex="0"
        @keydown="onKeyDown"
      >
        <div class="sizer" :style="{ height: `${rowCount * rowHeight}px` }">
          <div
            v-for="v in visible"
            :key="v.r"
            role="row"
            class="row"
            :aria-rowindex="v.r + 1"
            :style="{ transform: `translateY(${v.r * rowHeight}px)`, height: `${rowHeight}px` }"
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
              :title="cellLabel(c.glyph, c.index)"
              :data-code-point="c.glyph?.codePoint"
              @click="onClick(c.index)"
            >
              <span
                v-if="c.glyph"
                class="char"
                :class="{ loading: !faceStyle }"
                :style="faceStyle"
                aria-hidden="true"
                >{{ charOf(c.glyph) }}</span
              >
            </div>
          </div>
        </div>
      </div>
      <FamilyTraits :traits="detail.glyphTraits ?? []" @find="emit('findTrait', $event)" />
      <GlyphCoverage @block="showBlock" />
      <p v-if="glyphs.unicodeVersion" class="muted note">
        {{ t('glyphs.unicodeVersion', { version: glyphs.unicodeVersion }) }}
      </p>
    </div>
    <section
      v-if="sel"
      class="card"
      data-testid="glyph-card"
      :aria-label="t('glyphs.selected')"
      @keydown.esc.stop.prevent="closeDetail"
    >
      <p
        class="big"
        data-testid="glyph-big"
        :class="{ loading: !faceStyle }"
        :style="faceStyle"
        aria-hidden="true"
      >
        {{ charOf(sel) }}
      </p>
      <dl>
        <dt id="fm-glyph-name">{{ t('glyphs.glyphName') }}</dt>
        <dd aria-labelledby="fm-glyph-name">{{ sel.name ?? t('glyphs.none') }}</dd>
        <dt id="fm-glyph-cp">{{ t('glyphs.codePoint') }}</dt>
        <dd class="mono" aria-labelledby="fm-glyph-cp">{{ codePointLabel(sel.codePoint) }}</dd>
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
      </dl>
      <div class="actions">
        <button type="button" class="action" @click="copy(charOf(sel))">
          {{ t('glyphs.copyGlyph') }}
        </button>
        <button
          type="button"
          class="action"
          aria-haspopup="dialog"
          :title="t('glyphs.compareHint')"
          @click="openCompare"
        >
          {{ t('glyphs.compareWith') }}
        </button>
        <span class="muted copied" role="status">
          <template v-if="copyStatus === 'copied'">{{ t('common.copied') }}</template>
          <template v-else-if="copyStatus === 'failed'">{{ t('copy.failed') }}</template>
        </span>
      </div>
      <p v-if="glyphs.detailError" class="muted" role="alert">
        {{ errorMessage(glyphs.detailError) }}
      </p>
      <p v-else-if="detailBroken" class="muted" role="status">
        {{ t('glyphs.detailBroken', { reason: detailBroken }) }}
      </p>
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
        <dl>
          <dt id="fm-glyph-id">{{ t('glyphs.glyphId') }}</dt>
          <dd aria-labelledby="fm-glyph-id">{{ sel.glyphId }}</dd>
          <dt id="fm-glyph-advance">{{ t('glyphs.advance') }}</dt>
          <dd aria-labelledby="fm-glyph-advance">{{ advance }}</dd>
          <dt id="fm-glyph-block">{{ t('glyphs.block') }}</dt>
          <dd aria-labelledby="fm-glyph-block">{{ sel.block }}</dd>
        </dl>
        <div v-for="list in featureLists" :key="list.id" class="features">
          <h3 :id="list.id">{{ list.title }}</h3>
          <ul v-if="list.items.length" :aria-labelledby="list.id">
            <li v-for="f in list.items" :key="f.tag">
              <CopyValue :value="f.tag" :label="t('glyphs.featureTag')" />
              <span class="muted">{{ featureName(f) }}</span>
            </li>
          </ul>
          <p v-else class="muted">{{ list.empty }}</p>
        </div>
      </div>
    </section>
    <div v-else class="card empty" data-testid="glyph-card-empty">
      <p class="muted">{{ t('glyphs.pickHint') }}</p>
    </div>
    <GlyphCompare v-if="compare.open" />
  </div>
</template>

<style scoped>
/* The card's column stays reserved, so selecting a glyph never moves the grid. */
.characters {
  display: grid;
  grid-template-columns: minmax(0, 1fr) clamp(280px, 36%, 340px);
  align-items: start;
  gap: var(--fm-space-5);
}
.map {
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
  grid-template-columns: repeat(12, minmax(0, 1fr));
}
.cell {
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  box-sizing: border-box;
  border: 2px solid transparent;
  background-clip: padding-box;
  border-radius: var(--fm-radius-md);
  font-size: 26px;
  cursor: default;
  transition: background-color var(--fm-motion-fast) var(--fm-ease);
}
.cell:hover {
  background-color: var(--fm-hover);
}
.cell[aria-selected='true'] {
  border-color: var(--fm-accent);
  background-color: color-mix(in srgb, var(--fm-accent) 12%, transparent);
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
/* Sticks while the window is tall enough to show the card and the page above it. */
.card {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-3);
  box-sizing: border-box;
  min-width: 0;
  padding: var(--fm-space-4);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-surface);
}
@media (min-height: 720px) {
  .card {
    position: sticky;
    top: var(--fm-space-3);
  }
}
.card.empty {
  min-height: 200px;
  align-items: center;
  justify-content: center;
  text-align: center;
}
.big {
  overflow: hidden;
  margin: 0;
  font-size: 160px;
  line-height: 1.2;
  text-align: center;
  white-space: nowrap;
}
/* Never a fallback font: nothing shows until the face has loaded. */
.loading {
  visibility: hidden;
}
dl {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: var(--fm-space-1) var(--fm-space-3);
  margin: 0;
  font-size: var(--fm-text-sm);
}
dt {
  color: var(--fm-muted);
}
dd {
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
.features h3 {
  margin: 0 0 2px;
  font-size: var(--fm-text-sm);
  font-weight: 600;
}
.features ul {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}
.features li {
  display: flex;
  gap: var(--fm-space-2);
  align-items: baseline;
}
.features li :deep(.copy) {
  font-family: var(--fm-font-mono);
}
.features p {
  font-size: var(--fm-text-sm);
}
.note {
  font-size: var(--fm-text-xs);
}
</style>
