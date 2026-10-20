<script setup lang="ts">
// The glyph compare overlay: one character drawn by the webview in the inspected
// face and in a second face, at one em size on one shared baseline, each in its
// own color at 50% opacity. The x-height and cap-height lines of each face come
// from its `FaceDetail.metrics`; whether the second face has the character comes
// from fm-core's `check_coverage` (the store asks; nothing is computed here).
//
// A modal dialog. The picker (a combobox over the library search, then a style
// select) stays in it, so the second face can be swapped and the size changed
// without closing. Tab cycles inside; Escape or Close returns focus to where it
// was (the "Compare with…" button).
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { FaceDetail, FaceId, FamilyHit } from '../api/types';
import { COMPARE_SIZE, compareLayout } from '../glyphs/compare';
import { codePointLabel } from '../glyphs/layout';
import { useFaceLoader } from '../preview/faceLoader';
import { useGlyphCompareStore } from '../stores/glyphCompare';

const compare = useGlyphCompareStore();
const loader = useFaceLoader();

const dialog = ref<HTMLElement | null>(null);
const input = ref<HTMLInputElement | null>(null);
let returnFocus: HTMLElement | null = null;

// The two faces' fonts, registered with the webview while the overlay shows them.
const families = ref<(string | null)[]>([null, null]);
const held: (FaceId | null)[] = [null, null];
function hold(slot: 0 | 1, id: FaceId | null): void {
  if (held[slot] === id) return;
  if (held[slot] !== null) loader.release(held[slot]);
  held[slot] = id;
  families.value[slot] = null;
  if (id === null) return;
  loader.acquire(id).then(
    (name) => {
      if (held[slot] === id) families.value[slot] = name;
    },
    () => undefined,
  );
}
watch(
  () => [compare.base?.face.id ?? null, compare.other?.face.id ?? null] as const,
  ([a, b]) => {
    hold(0, a);
    hold(1, b);
  },
  { immediate: true },
);

onMounted(async () => {
  returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  await nextTick();
  input.value?.focus();
});
onBeforeUnmount(() => {
  hold(0, null);
  hold(1, null);
  if (returnFocus?.isConnected) returnFocus.focus();
});

const char = computed(() =>
  compare.codePoint === null ? '' : String.fromCodePoint(compare.codePoint),
);
const cpLabel = computed(() =>
  compare.codePoint === null ? '' : codePointLabel(compare.codePoint),
);
const faceName = (f: FaceDetail) => `${f.face.familyName} ${f.face.style}`;

/** The second face lacks the character, as `check_coverage` says. */
const lacks = computed(
  () =>
    !!compare.other &&
    compare.coverage?.face === compare.other.face.id &&
    !compare.coverage.complete,
);

/** The faces drawn, in order: the inspected one, then the second when it has the character. */
const drawn = computed(() => {
  const out: { face: FaceDetail; family: string | null; slot: number }[] = [];
  if (compare.base) out.push({ face: compare.base, family: families.value[0] ?? null, slot: 0 });
  if (compare.other && !lacks.value)
    out.push({ face: compare.other, family: families.value[1] ?? null, slot: 1 });
  return out;
});
const layout = computed(() =>
  compareLayout(
    compare.size,
    drawn.value.map((d) => d.face.metrics),
  ),
);
const lineName = { xHeight: 'x-height', capHeight: 'cap height' } as const;
const drawingLabel = computed(() => {
  const names = drawn.value.map((d) => faceName(d.face));
  return `${cpLabel.value} in ${names.join(' and ')}, ${compare.size} px`;
});
/** Each drawn face's lines, for the legend. */
const metricText = (slot: number) =>
  layout.value.lines
    .filter((l) => drawn.value[l.font]?.slot === slot)
    .map((l) => `${lineName[l.kind]} ${l.units.toLocaleString('en-US')}`)
    .join(', ') || 'no x-height or cap height declared';

// The picker: a combobox over the search results.
const active = ref(0);
const optionId = (i: number) => `fm-compare-option-${i}`;
const listOpen = ref(true);
watch(
  () => compare.results,
  () => (active.value = 0),
);
function onSearch(e: Event): void {
  listOpen.value = true;
  void compare.search((e.target as HTMLInputElement).value);
}
async function choose(hit: FamilyHit | undefined): Promise<void> {
  if (!hit) return;
  listOpen.value = false;
  await compare.pickFamily(hit);
}
function onStyle(e: Event): void {
  void compare.pickFace(Number((e.target as HTMLSelectElement).value));
}
function onSize(e: Event): void {
  compare.setSize(Number((e.target as HTMLInputElement).value));
}
function onComboKey(e: KeyboardEvent): void {
  const n = compare.results.length;
  switch (e.key) {
    case 'ArrowDown':
      listOpen.value = true;
      if (n) active.value = (active.value + 1) % n;
      break;
    case 'ArrowUp':
      listOpen.value = true;
      if (n) active.value = (active.value - 1 + n) % n;
      break;
    case 'Enter':
      if (!listOpen.value) return;
      void choose(compare.results[active.value]);
      break;
    default:
      return;
  }
  e.preventDefault();
}

/** Tab cycles inside the dialog; Escape closes it. */
function onKeyDown(e: KeyboardEvent): void {
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    compare.close();
    return;
  }
  if (e.key !== 'Tab' || !dialog.value) return;
  const stops = [
    ...dialog.value.querySelectorAll<HTMLElement>('input, select, button, [tabindex="0"]'),
  ].filter((el) => !el.hasAttribute('disabled'));
  const first = stops[0];
  const last = stops[stops.length - 1];
  if (!first || !last) return;
  if (e.shiftKey && document.activeElement === first) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && document.activeElement === last) {
    e.preventDefault();
    first.focus();
  }
}
</script>

<template>
  <div class="backdrop" @mousedown.self="compare.close">
    <div
      ref="dialog"
      class="compare"
      role="dialog"
      aria-modal="true"
      aria-labelledby="fm-compare-title"
      @keydown="onKeyDown"
    >
      <header class="head">
        <h2 id="fm-compare-title">Compare {{ cpLabel }}</h2>
        <button
          type="button"
          class="close"
          aria-label="Close compare"
          title="Close (Esc)"
          @click="compare.close"
        >
          ×
        </button>
      </header>
      <div class="controls">
        <div class="picker">
          <input
            ref="input"
            :value="compare.query"
            class="field"
            type="text"
            role="combobox"
            aria-label="Compare with font"
            aria-autocomplete="list"
            :aria-expanded="listOpen"
            aria-controls="fm-compare-list"
            :aria-activedescendant="
              listOpen && compare.results.length ? optionId(active) : undefined
            "
            placeholder="Search fonts by name"
            autocomplete="off"
            spellcheck="false"
            @input="onSearch"
            @keydown="onComboKey"
            @focus="listOpen = true"
          />
          <!-- mousedown.prevent keeps focus in the input while an option is clicked. -->
          <ul
            v-show="listOpen"
            id="fm-compare-list"
            class="list"
            role="listbox"
            aria-label="Fonts"
            @mousedown.prevent
          >
            <li
              v-for="(hit, i) in compare.results"
              :id="optionId(i)"
              :key="hit.family.id"
              role="option"
              class="option"
              :class="{ active: i === active }"
              :aria-selected="i === active"
              @mousemove="active = i"
              @click="choose(hit)"
            >
              {{ hit.family.name }}
              <span class="muted">
                {{ hit.family.styleCount }} {{ hit.family.styleCount === 1 ? 'style' : 'styles' }}
              </span>
            </li>
            <li v-if="!compare.results.length" class="muted none" role="presentation">
              No font matches the search.
            </li>
          </ul>
        </div>
        <select
          v-if="compare.other && compare.styles.length"
          class="field"
          aria-label="Style"
          :value="compare.other.face.id"
          @change="onStyle"
        >
          <option v-for="f in compare.styles" :key="f.id" :value="f.id">{{ f.style }}</option>
        </select>
        <div class="size">
          <span aria-hidden="true">Size</span>
          <input
            type="range"
            aria-label="Size"
            :min="COMPARE_SIZE.min"
            :max="COMPARE_SIZE.max"
            :step="COMPARE_SIZE.step"
            :value="compare.size"
            @input="onSize"
          />
          <input
            class="field number"
            type="number"
            aria-label="Size in pixels"
            :min="COMPARE_SIZE.min"
            :max="COMPARE_SIZE.max"
            :value="compare.size"
            @change="onSize"
          />
          <span class="muted">px</span>
        </div>
      </div>
      <p v-if="compare.error" class="muted" role="alert">{{ compare.error.message }}</p>
      <p v-if="!compare.other" class="muted" role="status">
        Pick a font to compare {{ cpLabel }} with.
      </p>
      <p v-else-if="lacks" class="missing" role="status">
        {{ faceName(compare.other) }} has no glyph for {{ cpLabel }}.
      </p>
      <div class="stage">
        <svg
          class="drawing"
          role="img"
          :aria-label="drawingLabel"
          :width="layout.width"
          :height="layout.height"
          :viewBox="`0 0 ${layout.width} ${layout.height}`"
        >
          <line
            class="baseline"
            data-line="baseline"
            x1="0"
            :x2="layout.width"
            :y1="layout.baseline"
            :y2="layout.baseline"
          />
          <line
            v-for="l in layout.lines"
            :key="`${l.font}-${l.kind}`"
            :class="['guide', `font-${drawn[l.font]?.slot}`]"
            :data-line="`${l.kind}-${drawn[l.font]?.slot}`"
            x1="0"
            :x2="layout.width"
            :y1="l.y"
            :y2="l.y"
          />
          <template v-for="d in drawn" :key="d.slot">
            <text
              v-if="d.family"
              :class="['glyph', `font-${d.slot}`]"
              :data-face="d.face.face.id"
              :x="layout.origin"
              :y="layout.baseline"
              :font-size="compare.size"
              :style="{ fontFamily: `&quot;${d.family}&quot;` }"
            >
              {{ char }}
            </text>
          </template>
        </svg>
      </div>
      <ul class="legend" aria-label="Legend">
        <li v-if="compare.base">
          <span class="swatch font-0" aria-hidden="true"></span>
          <span>{{ faceName(compare.base) }}</span>
          <span class="muted">— solid lines: {{ metricText(0) }}</span>
        </li>
        <li v-if="compare.other">
          <span class="swatch font-1" aria-hidden="true"></span>
          <span>{{ faceName(compare.other) }}</span>
          <span v-if="lacks" class="muted">— not drawn</span>
          <span v-else class="muted">— dashed lines: {{ metricText(1) }}</span>
        </li>
        <li>
          <span class="swatch base" aria-hidden="true"></span>
          <span>Shared baseline</span>
        </li>
      </ul>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 10;
  display: flex;
  justify-content: center;
  align-items: flex-start;
  padding-top: 6vh;
  background: rgb(0 0 0 / 20%);
}
.compare {
  /* Two colors that stay apart in both themes and for common color blindness. */
  --fm-compare-0: #1d6fe0;
  --fm-compare-1: #e0451d;
  display: flex;
  flex-direction: column;
  gap: 10px;
  width: min(720px, 92vw);
  max-height: 88vh;
  padding: 16px;
  border: 1px solid var(--fm-border);
  border-radius: 8px;
  background: var(--fm-bg);
  color: var(--fm-fg);
  box-shadow: 0 8px 32px rgb(0 0 0 / 24%);
  overflow: auto;
}
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
h2 {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
}
.controls {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: flex-start;
}
.picker {
  position: relative;
  flex: 1 1 240px;
  display: flex;
  flex-direction: column;
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
.field:focus-visible,
.close:focus-visible,
input[type='range']:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
.list {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  z-index: 1;
  max-height: 200px;
  margin: 2px 0 0;
  padding: 4px 0;
  overflow: auto;
  border: 1px solid var(--fm-border);
  border-radius: 6px;
  background: var(--fm-bg);
  box-shadow: 0 4px 16px rgb(0 0 0 / 16%);
  list-style: none;
}
.option {
  display: flex;
  gap: 8px;
  align-items: baseline;
  padding: 4px 8px;
  cursor: default;
}
.option.active {
  background: var(--fm-surface);
  box-shadow: inset 2px 0 0 var(--fm-accent);
}
.none {
  padding: 4px 8px;
}
.size {
  display: flex;
  gap: 6px;
  align-items: center;
}
.number {
  width: 72px;
}
.muted {
  margin: 0;
  color: var(--fm-muted);
}
.missing {
  margin: 0;
  font-weight: 600;
}
.close {
  padding: 0 6px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: var(--fm-bg);
  color: var(--fm-fg);
  font: inherit;
  font-size: 16px;
  line-height: 1.4;
}
.stage {
  overflow: auto;
  border: 1px solid var(--fm-border);
  border-radius: 6px;
}
.drawing {
  display: block;
  margin: 0 auto;
}
.baseline {
  stroke: var(--fm-fg);
  stroke-width: 1;
}
.guide {
  /* Two pixels: a 1 px line on a whole-pixel y is smeared over two device rows at
     half strength (about 2:1); two full rows keep the line at its 3:1+ color. */
  stroke-width: 2;
}
.guide.font-0 {
  stroke: var(--fm-compare-0);
}
.guide.font-1 {
  stroke: var(--fm-compare-1);
  stroke-dasharray: 6 4;
}
.glyph {
  fill-opacity: 0.5;
}
.glyph.font-0 {
  fill: var(--fm-compare-0);
}
.glyph.font-1 {
  fill: var(--fm-compare-1);
}
.legend {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 0;
  padding: 0;
  list-style: none;
}
.legend li {
  display: flex;
  gap: 8px;
  align-items: center;
}
.swatch {
  flex: none;
  width: 14px;
  height: 14px;
  border-radius: 3px;
  opacity: 0.5;
}
.swatch.font-0 {
  background: var(--fm-compare-0);
}
.swatch.font-1 {
  background: var(--fm-compare-1);
}
.swatch.base {
  height: 2px;
  background: var(--fm-fg);
  opacity: 1;
}
</style>
