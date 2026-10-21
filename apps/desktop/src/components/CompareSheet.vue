<script setup lang="ts">
// Compare: the pinned faces (2–4) in the list's current sample text at one size,
// as a sheet over the list. Side by side and Stacked show one block per face;
// Overlay draws two of them on one shared baseline, each in its own color at 50%
// opacity. Every face is live text in the webview, registered through the shared
// face loader while the sheet shows it.
//
// A modal dialog: Tab cycles inside, plain keys do not reach the list's
// shortcuts, and Escape or Close returns focus to where it was (the list row).
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue';
import type { FaceId, FaceSummary } from '../api/types';
import { fontNameSample, sampleText } from '../preview/sampleText';
import { useFaceLoader } from '../preview/faceLoader';
import { COMPARE_MODES, MAX_PINS, useCompareStore, type CompareMode } from '../stores/compare';
import { ALL_FONTS_VIEW, usePreviewStore } from '../stores/preview';

const compare = useCompareStore();
const preview = usePreviewStore();
const loader = useFaceLoader();

const dialog = ref<HTMLElement | null>(null);
let returnFocus: HTMLElement | null = null;

const SLOTS = [0, 1] as const;

const settings = computed(() => preview.settingsFor(ALL_FONTS_VIEW));
const textOf = (face: FaceSummary) => sampleText(settings.value, face);

// The pinned faces' fonts, registered with the webview while the sheet is open.
const families = reactive(new Map<FaceId, string>());
const held = new Set<FaceId>();
watch(
  () => compare.pins.map((p) => p.id),
  (ids) => {
    for (const id of [...held]) {
      if (ids.includes(id)) continue;
      held.delete(id);
      families.delete(id);
      loader.release(id);
    }
    for (const id of ids) {
      if (held.has(id)) continue;
      held.add(id);
      loader.acquire(id).then(
        (name) => {
          if (held.has(id)) families.set(id, name);
        },
        () => undefined,
      );
    }
  },
  { immediate: true },
);

/** The face's font as a CSS `font-family`, or the UI font until it has loaded. */
const fontOf = (face: FaceSummary) => {
  const name = families.get(face.id);
  return name ? `"${name}", var(--fm-preview-fallback, system-ui), sans-serif` : undefined;
};

onMounted(async () => {
  const was = document.activeElement;
  // Opened from the command palette, the focus is in the palette (or already nowhere,
  // the palette gone): the list column takes it back instead.
  returnFocus =
    was instanceof HTMLElement && was !== document.body && !was.closest('[role="dialog"]')
      ? was
      : document.querySelector('main');
  await nextTick();
  dialog.value?.querySelector<HTMLInputElement>('input[name="fm-compare-mode"]:checked')?.focus();
});
onBeforeUnmount(() => {
  for (const id of held) loader.release(id);
  held.clear();
  if (returnFocus?.isConnected) returnFocus.focus();
});

/** Removes a pin and keeps the focus in the sheet: on the next Remove button, else the mode. */
async function remove(face: FaceId): Promise<void> {
  const buttons = [...(dialog.value?.querySelectorAll<HTMLElement>('[data-remove]') ?? [])];
  const at = buttons.findIndex((b) => b.dataset.remove === String(face));
  compare.unpin(face);
  await nextTick();
  if (!dialog.value) return;
  const left = [...dialog.value.querySelectorAll<HTMLElement>('[data-remove]')];
  const next =
    left[Math.min(at, left.length - 1)] ??
    dialog.value.querySelector<HTMLElement>('input[name="fm-compare-mode"]:checked');
  next?.focus();
}

const size = computed(() => settings.value.sizePx);
// Overlay: one SVG line box; both texts sit on the same baseline (one `y`).
const overlayHeight = computed(() => Math.ceil(size.value * 1.5));
const overlayBaseline = computed(() => Math.round(size.value * 1.1));
const overlayDrawn = computed(() =>
  compare.overlayPair.filter((f): f is FaceSummary => f !== null),
);
const overlayLabel = computed(
  () => `Overlay of ${overlayDrawn.value.map(fontNameSample).join(' and ')}, ${size.value} px`,
);

function onMode(e: Event): void {
  compare.setMode((e.target as HTMLInputElement).value as CompareMode);
}
function onOverlay(slot: 0 | 1, e: Event): void {
  compare.setOverlay(slot, Number((e.target as HTMLSelectElement).value));
}

/** Tab cycles inside the sheet; Escape closes it; plain keys stay out of the list's shortcuts. */
function onKeyDown(e: KeyboardEvent): void {
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    compare.close();
    return;
  }
  if (!e.ctrlKey && !e.metaKey) e.stopPropagation();
  if (e.key !== 'Tab' || !dialog.value) return;
  // One stop per radio group: the checked radio.
  const stops = [
    ...dialog.value.querySelectorAll<HTMLElement>('input, select, button, [tabindex="0"]'),
  ].filter(
    (el) =>
      !el.hasAttribute('disabled') &&
      !(el instanceof HTMLInputElement && el.type === 'radio' && !el.checked),
  );
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
      class="sheet"
      role="dialog"
      aria-modal="true"
      aria-labelledby="fm-compare-sheet-title"
      @keydown="onKeyDown"
    >
      <header class="head">
        <h2 id="fm-compare-sheet-title">Compare</h2>
        <span class="muted">{{ compare.pins.length }} of {{ MAX_PINS }} pinned</span>
        <div class="modes" role="radiogroup" aria-labelledby="fm-compare-mode-label">
          <span id="fm-compare-mode-label" class="muted">Mode</span>
          <label v-for="m in COMPARE_MODES" :key="m.value" class="mode">
            <input
              type="radio"
              name="fm-compare-mode"
              :value="m.value"
              :checked="compare.mode === m.value"
              @change="onMode"
            />
            {{ m.label }}
          </label>
        </div>
        <button
          type="button"
          class="button close"
          aria-label="Close compare"
          title="Close (Esc)"
          @click="compare.close"
        >
          ×
        </button>
      </header>

      <p v-if="compare.pins.length < 2" class="muted" role="status">
        Pin at least two fonts to compare them: select a font in the list and press C.
      </p>

      <div
        v-if="compare.mode === 'overlay'"
        class="overlay"
        role="group"
        aria-label="Overlay"
        data-testid="compare-overlay"
      >
        <div v-if="compare.pins.length > 2" class="picks">
          <label v-for="slot in SLOTS" :key="slot" class="pick">
            <span class="swatch" :class="`color-${slot}`" aria-hidden="true"></span>
            {{ slot === 0 ? 'First font' : 'Second font' }}
            <select
              class="field"
              :aria-label="slot === 0 ? 'First font' : 'Second font'"
              :value="compare.overlayPair[slot]?.id"
              @change="onOverlay(slot, $event)"
            >
              <option v-for="p in compare.pins" :key="p.id" :value="p.id">
                {{ fontNameSample(p) }}
              </option>
            </select>
          </label>
        </div>
        <div class="stage">
          <svg
            class="drawing"
            role="img"
            :aria-label="overlayLabel"
            width="100%"
            :height="overlayHeight"
          >
            <line class="baseline" x1="0" x2="100%" :y1="overlayBaseline" :y2="overlayBaseline" />
            <text
              v-for="(f, slot) in overlayDrawn"
              :key="f.id"
              :class="['overlay-text', `color-${slot}`]"
              :data-face="f.id"
              x="8"
              :y="overlayBaseline"
              :font-size="size"
              :style="{
                fontFamily: fontOf(f),
                fill: `var(--fm-compare-${slot})`,
                fillOpacity: 0.5,
              }"
            >
              {{ textOf(f) }}
            </text>
          </svg>
        </div>
        <ul class="legend" aria-label="Legend">
          <li v-for="(f, slot) in overlayDrawn" :key="f.id">
            <span class="swatch" :class="`color-${slot}`" aria-hidden="true"></span>
            {{ fontNameSample(f) }}
          </li>
        </ul>
      </div>

      <ul
        v-else
        class="faces"
        :class="compare.mode"
        :aria-label="compare.mode === 'stacked' ? 'Stacked' : 'Side by side'"
      >
        <li v-for="f in compare.pins" :key="f.id" class="face" :data-face="f.id">
          <div class="label">
            <span class="name">{{ fontNameSample(f) }}</span>
            <button
              type="button"
              class="button"
              :data-remove="f.id"
              :aria-label="`Remove ${fontNameSample(f)} from Compare`"
              @click="remove(f.id)"
            >
              Remove
            </button>
          </div>
          <p
            class="sample"
            :style="{
              fontFamily: fontOf(f),
              fontSize: `${size}px`,
              lineHeight: settings.lineHeight,
              letterSpacing: `${settings.trackingEm}em`,
            }"
          >
            {{ textOf(f) }}
          </p>
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
  background: rgb(0 0 0 / 20%);
}
.sheet {
  /* Two colors that stay apart in both themes and for common color blindness. */
  --fm-compare-0: #1d6fe0;
  --fm-compare-1: #e0451d;
  display: flex;
  flex-direction: column;
  gap: 12px;
  width: min(1100px, 96vw);
  max-height: 90vh;
  padding: 16px;
  border: 1px solid var(--fm-border);
  border-top: 0;
  border-radius: 0 0 8px 8px;
  background: var(--fm-bg);
  color: var(--fm-fg);
  box-shadow: 0 8px 32px rgb(0 0 0 / 24%);
  overflow: auto;
}
.head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
}
h2 {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
}
.modes {
  display: flex;
  flex: 1;
  justify-content: center;
  align-items: center;
  gap: 12px;
}
.mode {
  display: flex;
  align-items: center;
  gap: 4px;
}
.muted {
  margin: 0;
  color: var(--fm-muted);
}
.button,
.field {
  padding: 2px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: var(--fm-bg);
  color: var(--fm-fg);
  font: inherit;
}
.close {
  font-size: 16px;
  line-height: 1.4;
}
.button:focus-visible,
.field:focus-visible,
.mode input:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
.faces {
  display: grid;
  gap: 12px;
  margin: 0;
  padding: 0;
  list-style: none;
}
.faces.sideBySide {
  grid-auto-flow: column;
  grid-auto-columns: minmax(0, 1fr);
}
.face {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
  padding: 8px;
  border: 1px solid var(--fm-border);
  border-radius: 6px;
}
.label {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  color: var(--fm-muted);
}
.name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sample {
  margin: 0;
  overflow-wrap: anywhere;
}
.overlay {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.picks {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
}
.pick {
  display: flex;
  align-items: center;
  gap: 6px;
}
.stage {
  overflow: hidden;
  border: 1px solid var(--fm-border);
  border-radius: 6px;
}
.drawing {
  display: block;
}
.baseline {
  stroke: var(--fm-fg);
  stroke-width: 1;
}
.overlay-text {
  white-space: pre;
}
.legend {
  display: flex;
  gap: 16px;
  margin: 0;
  padding: 0;
  list-style: none;
}
.legend li {
  display: flex;
  align-items: center;
  gap: 6px;
}
.swatch {
  flex: none;
  width: 14px;
  height: 14px;
  border-radius: 3px;
  opacity: 0.5;
}
.swatch.color-0 {
  background: var(--fm-compare-0);
}
.swatch.color-1 {
  background: var(--fm-compare-1);
}
</style>
