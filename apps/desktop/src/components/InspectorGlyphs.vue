<script setup lang="ts">
// The Glyphs tab: every character the selected face maps, drawn by the webview
// from the face's own file (the font asset protocol), under Unicode block
// headings. fm-core lists the characters and runs the search; this tab only lays
// them out. The grid is virtualized: only the rows on screen exist, so a CJK face
// with 30k+ characters scrolls like a Latin one, and pages load as rows appear.
//
// Keyboard (WAI-ARIA grid): the grid holds focus and points at the cursor cell
// with aria-activedescendant; arrow keys move by cell and row, Home/End to the
// ends, Enter or a click selects the glyph and opens its detail below the grid
// (Tab reaches it); Escape or its Close button closes it and returns to the grid.
// Its "Compare with…" button opens the compare overlay (GlyphCompare) on the glyph.
// Below, the coverage list (GlyphCoverage): a block chosen there shows that block
// in the grid and puts focus on it.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, watchEffect } from 'vue';
import type { FeatureInfo, GlyphInfo } from '../api/types';
import { errorMessage, t } from '../i18n';
import {
  GLYPH_CELL,
  codePointLabel,
  glyphColumns,
  glyphLayout,
  moveCursor,
  rowOf,
} from '../glyphs/layout';
import { scrollToShow, windowRange } from '../list/virtual';
import { useFaceLoader } from '../preview/faceLoader';
import CopyValue from './CopyValue.vue';
import GlyphCompare from './GlyphCompare.vue';
import GlyphCoverage from './GlyphCoverage.vue';
import { useCoverageStore } from '../stores/coverage';
import { useGlyphCompareStore } from '../stores/glyphCompare';
import { useGlyphsStore } from '../stores/glyphs';
import { useSelectionStore } from '../stores/selection';

const props = withDefaults(
  defineProps<{
    /** Rows rendered beyond each edge of the viewport. */
    overscan?: number;
  }>(),
  { overscan: 3 },
);

/** Viewport size before the first measurement (and where there is no layout). */
const FALLBACK_HEIGHT = 384;
const FALLBACK_WIDTH = 288;

const selection = useSelectionStore();
const glyphs = useGlyphsStore();
const compare = useGlyphCompareStore();
const coverage = useCoverageStore();
const loader = useFaceLoader();

const faceId = computed(() => selection.detail?.face.id ?? null);

// The face's font, registered with the webview while the tab shows it.
const family = ref<string | null>(null);
let held: number | null = null;
function hold(id: number | null): void {
  if (held !== null) loader.release(held);
  held = id;
  family.value = null;
  if (id === null) return;
  loader.acquire(id).then(
    (name) => {
      if (held === id) family.value = name;
    },
    () => undefined,
  );
}
watch(
  faceId,
  (id) => {
    hold(id);
    if (id !== null) {
      void glyphs.open(id);
      void coverage.open(id);
    }
  },
  { immediate: true },
);
onBeforeUnmount(() => hold(null));

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
onMounted(() => {
  measure();
  if (typeof ResizeObserver !== 'undefined' && viewport.value) {
    observer = new ResizeObserver(measure);
    observer.observe(viewport.value);
  }
});
onBeforeUnmount(() => observer?.disconnect());

const columns = computed(() => glyphColumns(width.value));
const layout = computed(() => glyphLayout(glyphs.blocks, columns.value));
const range = computed(() =>
  windowRange(scrollTop.value, height.value, GLYPH_CELL, props.overscan, layout.value.rows.length),
);

/** The cursor: a list index, or -1 before the user moves into the grid. */
const cursor = ref(-1);
const cellId = (i: number) => `fm-glyph-${i}`;

const visible = computed(() =>
  layout.value.rows.slice(range.value.start, range.value.end).map((row, k) => {
    const r = range.value.start + k;
    const cells =
      row.kind === 'cells'
        ? Array.from({ length: row.count }, (_, c) => {
            const index = row.start + c;
            return { index, glyph: glyphs.item(index) };
          })
        : [];
    return { r, row, cells };
  }),
);

// Load the pages of the rows on screen.
watchEffect(() => {
  const rows = layout.value.rows.slice(range.value.start, range.value.end);
  const first = rows[0];
  const last = rows[rows.length - 1];
  if (!first || !last) return;
  const end = last.kind === 'cells' ? last.start + last.count : last.start + 1;
  void glyphs.ensure(first.start, end);
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
  const r = rowOf(layout.value, i);
  return r >= range.value.start && r < range.value.end ? cellId(i) : undefined;
});

/** Scrolls so list index `i` is on screen, and so rendered for aria-activedescendant. */
async function reveal(i: number): Promise<void> {
  const el = viewport.value;
  const r = rowOf(layout.value, i);
  if (!el || r < 0) return;
  // Show a block's first row with its heading.
  const row = layout.value.rows[r - 1]?.kind === 'heading' ? r - 1 : r;
  const top = scrollToShow(row, GLYPH_CELL, el.scrollTop, height.value);
  const bottom = scrollToShow(r, GLYPH_CELL, top, height.value);
  if (bottom !== el.scrollTop) el.scrollTop = bottom;
  measure();
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
  viewport.value?.focus();
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
  const next = moveCursor(layout.value, Math.max(cursor.value, 0), e.key);
  if (next === null) return;
  e.preventDefault();
  cursor.value = cursor.value < 0 && e.key !== 'Home' && e.key !== 'End' ? 0 : next;
  await reveal(cursor.value);
}

async function onClick(index: number): Promise<void> {
  choose(index);
  viewport.value?.focus();
}

// A new list (a search, a block, a face): back to the top, or to the glyph a
// search selected.
watch(
  () => glyphs.generation,
  async () => {
    cursor.value = -1;
    const el = viewport.value;
    if (el) el.scrollTop = 0;
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
  const el = viewport.value;
  el?.scrollIntoView?.({ block: 'nearest' });
  el?.focus();
}
function onBlock(e: Event): void {
  const value = (e.target as HTMLSelectElement).value;
  void glyphs.chooseBlock(value || null);
}

const sel = computed(() => glyphs.selected);
const detail = computed(() =>
  glyphs.detail && glyphs.detail.codePoint === sel.value?.codePoint ? glyphs.detail : null,
);
const detailBroken = computed(() =>
  detail.value?.health.kind === 'broken' ? detail.value.health.reason : null,
);
const advance = computed(() => {
  const d = detail.value;
  if (!d) return glyphs.detailError ? t('glyphs.advanceUnknown') : t('glyphs.advanceLoading');
  return d.advanceWidth === null ? t('glyphs.none') : t('glyphs.units', { count: d.advanceWidth });
});
/** The feature lists of the detail, each with its heading and empty text. */
const featureLists = computed(() => {
  const d = detail.value;
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
const fontStyle = computed(() => ({
  fontFamily: family.value ? `"${family.value}", sans-serif` : undefined,
}));
const broken = computed(() => (glyphs.health.kind === 'broken' ? glyphs.health.reason : null));
const noResults = computed(
  () => !glyphs.loading && !broken.value && glyphs.total === 0 && glyphs.face !== null,
);
</script>

<template>
  <div class="glyphs">
    <p v-if="!selection.detail" class="placeholder">{{ t('glyphs.noFace') }}</p>
    <template v-else>
      <div class="controls">
        <input
          v-model="searchText"
          class="field search"
          type="search"
          :aria-label="t('glyphs.search')"
          :placeholder="t('glyphs.searchPlaceholder')"
          @input="onSearch"
        />
        <select
          class="field"
          :aria-label="t('glyphs.block')"
          :value="glyphs.block ?? ''"
          @change="onBlock"
        >
          <option value="">{{ t('glyphs.allBlocks') }}</option>
          <option v-for="b in glyphs.faceBlocks" :key="b.name" :value="b.name">
            {{ b.name }} ({{ b.count }})
          </option>
        </select>
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
      <p class="count" aria-live="polite">
        {{ t('glyphs.characters', { count: glyphs.total }) }}
      </p>
      <div
        ref="viewport"
        class="viewport"
        role="grid"
        :aria-label="t('glyphs.grid')"
        :aria-rowcount="layout.rows.length"
        :aria-colcount="columns"
        :aria-activedescendant="activeId"
        tabindex="0"
        @scroll="measure"
        @keydown="onKeyDown"
      >
        <div class="sizer" :style="{ height: `${layout.rows.length * GLYPH_CELL}px` }">
          <div
            v-for="v in visible"
            :key="v.r"
            role="row"
            class="row"
            :aria-rowindex="v.r + 1"
            :style="{ transform: `translateY(${v.r * GLYPH_CELL}px)`, height: `${GLYPH_CELL}px` }"
          >
            <div
              v-if="v.row.kind === 'heading'"
              role="rowheader"
              class="heading"
              :aria-colspan="columns"
            >
              {{ v.row.block.name }}
              <span class="muted">{{ v.row.block.count }}</span>
            </div>
            <div
              v-for="c in v.cells"
              v-else
              :id="cellId(c.index)"
              :key="c.index"
              role="gridcell"
              class="cell"
              :class="{ cursor: c.index === cursor }"
              :aria-label="cellLabel(c.glyph, c.index)"
              :aria-selected="!!c.glyph && sel?.codePoint === c.glyph.codePoint"
              :title="cellLabel(c.glyph, c.index)"
              :data-code-point="c.glyph?.codePoint"
              :style="{ width: `${100 / columns}%` }"
              @click="onClick(c.index)"
            >
              <span v-if="c.glyph" class="char" :style="fontStyle" aria-hidden="true">{{
                charOf(c.glyph)
              }}</span>
            </div>
          </div>
        </div>
      </div>
      <section
        v-if="sel"
        class="detail"
        :aria-label="t('glyphs.selected')"
        @keydown.esc.stop.prevent="closeDetail"
      >
        <div class="detail-head">
          <span class="big" :style="fontStyle" aria-hidden="true">{{ charOf(sel) }}</span>
          <dl>
            <dt id="fm-glyph-cp">{{ t('glyphs.codePoint') }}</dt>
            <dd aria-labelledby="fm-glyph-cp">{{ codePointLabel(sel.codePoint) }}</dd>
            <dt id="fm-glyph-name">{{ t('glyphs.glyphName') }}</dt>
            <dd aria-labelledby="fm-glyph-name">{{ sel.name ?? t('glyphs.none') }}</dd>
            <dt id="fm-glyph-id">{{ t('glyphs.glyphId') }}</dt>
            <dd aria-labelledby="fm-glyph-id">{{ sel.glyphId }}</dd>
            <dt id="fm-glyph-advance">{{ t('glyphs.advance') }}</dt>
            <dd aria-labelledby="fm-glyph-advance">{{ advance }}</dd>
            <dt id="fm-glyph-block">{{ t('glyphs.block') }}</dt>
            <dd aria-labelledby="fm-glyph-block">{{ sel.block }}</dd>
          </dl>
          <button
            type="button"
            class="close"
            :aria-label="t('glyphs.closeDetail')"
            :title="t('glyphs.closeEsc')"
            @click="closeDetail"
          >
            ×
          </button>
        </div>
        <div class="detail-actions">
          <button
            type="button"
            class="action"
            aria-haspopup="dialog"
            :title="t('glyphs.compareHint')"
            @click="openCompare"
          >
            {{ t('glyphs.compareWith') }}
          </button>
        </div>
        <p v-if="glyphs.detailError" class="muted" role="alert">
          {{ errorMessage(glyphs.detailError) }}
        </p>
        <p v-else-if="detailBroken" class="muted" role="status">
          {{ t('glyphs.detailBroken', { reason: detailBroken }) }}
        </p>
        <template v-else>
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
        </template>
      </section>
      <GlyphCompare v-if="compare.open" />
      <GlyphCoverage @block="showBlock" />
      <p v-if="glyphs.unicodeVersion" class="muted note">
        {{ t('glyphs.unicodeVersion', { version: glyphs.unicodeVersion }) }}
      </p>
    </template>
  </div>
</template>

<style scoped>
.glyphs {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 16px;
}
.placeholder,
.muted,
.count {
  margin: 0;
  color: var(--fm-muted);
}
.controls {
  display: flex;
  gap: 8px;
}
.field {
  min-width: 0;
  padding: 4px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: var(--fm-bg);
  color: var(--fm-fg);
  font: inherit;
}
.search {
  flex: 1;
}
select.field {
  flex: 0 1 45%;
}
.viewport {
  position: relative;
  height: 384px;
  overflow-y: auto;
  border: 1px solid var(--fm-border);
  border-radius: 6px;
}
.viewport:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
.sizer {
  position: relative;
}
.row {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  display: flex;
}
.heading {
  display: flex;
  align-items: flex-end;
  gap: 6px;
  width: 100%;
  padding: 0 8px 6px;
  font-weight: 600;
}
.cell {
  display: flex;
  flex: none;
  align-items: center;
  justify-content: center;
  border-right: 1px solid var(--fm-border);
  border-bottom: 1px solid var(--fm-border);
  font-size: 24px;
  cursor: default;
}
.cell[aria-selected='true'] {
  background: var(--fm-accent);
  color: var(--fm-bg);
}
.viewport:focus-visible .cell.cursor {
  outline: 2px solid var(--fm-accent);
  outline-offset: -3px;
}
.viewport:focus-visible .cell.cursor[aria-selected='true'] {
  outline-color: var(--fm-bg);
}
.char {
  line-height: 1;
}
.detail {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.detail-head {
  display: flex;
  gap: 16px;
  align-items: center;
}
.detail-head dl {
  flex: 1;
}
.detail-actions {
  display: flex;
  gap: 8px;
}
.action {
  padding: 2px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: var(--fm-bg);
  color: var(--fm-fg);
  font: inherit;
  white-space: nowrap;
}
.action:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
.close {
  align-self: flex-start;
  padding: 0 6px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: var(--fm-bg);
  color: var(--fm-fg);
  font: inherit;
  font-size: 16px;
  line-height: 1.4;
}
.close:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
.features h3 {
  margin: 0 0 2px;
  font-size: 12px;
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
  gap: 8px;
  align-items: baseline;
}
.features li :deep(.copy) {
  font-family: ui-monospace, monospace;
}
.big {
  flex: none;
  min-width: 64px;
  font-size: 56px;
  line-height: 1;
  text-align: center;
}
dl {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 2px 8px;
  margin: 0;
}
dt {
  color: var(--fm-muted);
}
dd {
  margin: 0;
  overflow-wrap: anywhere;
}
.note {
  font-size: 11.5px;
}
</style>
