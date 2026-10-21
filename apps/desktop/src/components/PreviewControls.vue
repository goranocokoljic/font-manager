<script setup lang="ts">
// The preview controls above a font list: sample text, size, auto size, line
// height, tracking, alignment, colors and the preview surface. Every change goes to
// the preview store for `view`, which every row of that list reads.
import { computed, useId } from 'vue';
import { PANGRAM_LANGUAGES, PANGRAMS, SAMPLE_KINDS } from '../preview/sampleText';
import {
  LINE_HEIGHT,
  SIZE,
  TRACKING,
  usePreviewStore,
  type Alignment,
  type AutoSize,
  type PreviewSettings,
  type Surface,
} from '../stores/preview';

const props = defineProps<{ view: string }>();

const store = usePreviewStore();
// A select is labelled by `for`: wrapped in its label, its option texts would join the name.
const uid = useId();
const s = computed(() => store.settingsFor(props.view));

function set(patch: Partial<PreviewSettings>): void {
  store.update(props.view, patch);
}

type NumberKey = 'sizePx' | 'lineHeight' | 'trackingEm';
const BOUNDS = { sizePx: SIZE, lineHeight: LINE_HEIGHT, trackingEm: TRACKING } as const;

/**
 * While typing, only a value inside the bounds applies (so "1" on the way to "12" does
 * not jump to the minimum). On change, the store clamps and the field shows the result.
 */
function typed(key: NumberKey, e: Event, commit: boolean): void {
  const input = e.target as HTMLInputElement;
  const value = input.valueAsNumber;
  const b = BOUNDS[key];
  if (commit || (Number.isFinite(value) && value >= b.min && value <= b.max)) {
    set({ [key]: value });
  }
  if (commit) input.value = String(store.settingsFor(props.view)[key]);
}

const alignments: { value: Alignment; label: string }[] = [
  { value: 'start', label: 'Start' },
  { value: 'center', label: 'Center' },
  { value: 'end', label: 'End' },
];
const autoSizes: { value: AutoSize; label: string }[] = [
  { value: 'off', label: 'Off' },
  { value: 'xHeight', label: 'x-height' },
  { value: 'capHeight', label: 'Cap height' },
];
const surfaces: { value: Surface; label: string }[] = [
  { value: 'auto', label: 'Follow theme' },
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
];

/** What a color well shows while its setting is `null` (the surface's own color). */
function isDarkSurface(): boolean {
  if (s.value.surface !== 'auto') return s.value.surface === 'dark';
  const theme = document.documentElement.dataset.theme;
  if (theme === 'light' || theme === 'dark') return theme === 'dark';
  return globalThis.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false;
}
const textWell = computed(() => s.value.textColor ?? (isDarkSurface() ? '#ececec' : '#1a1a1a'));
const backgroundWell = computed(
  () => s.value.backgroundColor ?? (isDarkSurface() ? '#1c1c1e' : '#ffffff'),
);

function colorFrom(e: Event): string {
  return (e.target as HTMLInputElement).value;
}
</script>

<template>
  <div class="controls" role="group" aria-label="Preview settings">
    <div class="field">
      <label :for="`${uid}-sample`">Sample</label>
      <select
        :id="`${uid}-sample`"
        :value="s.sample"
        @change="
          set({ sample: ($event.target as HTMLSelectElement).value as PreviewSettings['sample'] })
        "
      >
        <option v-for="kind in SAMPLE_KINDS" :key="kind.value" :value="kind.value">
          {{ kind.label }}
        </option>
      </select>
    </div>
    <div v-if="s.sample === 'pangram'" class="field">
      <label :for="`${uid}-language`">Language</label>
      <select
        :id="`${uid}-language`"
        :value="s.language"
        @change="
          set({
            language: ($event.target as HTMLSelectElement).value as PreviewSettings['language'],
          })
        "
      >
        <option v-for="lang in PANGRAM_LANGUAGES" :key="lang" :value="lang">
          {{ PANGRAMS[lang].label }}
        </option>
      </select>
    </div>
    <label v-if="s.sample === 'custom'" class="field grow">
      <span>Custom text</span>
      <input
        type="text"
        :value="s.customText"
        placeholder="Type a sample"
        @input="set({ customText: ($event.target as HTMLInputElement).value })"
      />
    </label>

    <div class="field" role="group" aria-labelledby="fm-preview-size">
      <span id="fm-preview-size">Size</span>
      <input
        type="range"
        aria-label="Size slider"
        :min="SIZE.min"
        :max="SIZE.max"
        :step="SIZE.step"
        :value="s.sizePx"
        @input="typed('sizePx', $event, false)"
      />
      <input
        type="number"
        class="number"
        aria-label="Size in pixels"
        :min="SIZE.min"
        :max="SIZE.max"
        :step="SIZE.step"
        :value="s.sizePx"
        @input="typed('sizePx', $event, false)"
        @change="typed('sizePx', $event, true)"
      />
    </div>
    <div class="field">
      <label
        :for="`${uid}-auto-size`"
        title="Scale each row so every font's x-height or cap height matches at this size"
        >Auto size</label
      >
      <select
        :id="`${uid}-auto-size`"
        :value="s.autoSize"
        @change="set({ autoSize: ($event.target as HTMLSelectElement).value as AutoSize })"
      >
        <option v-for="o in autoSizes" :key="o.value" :value="o.value">{{ o.label }}</option>
      </select>
    </div>
    <label class="field">
      <span>Line height</span>
      <input
        type="number"
        class="number"
        :min="LINE_HEIGHT.min"
        :max="LINE_HEIGHT.max"
        :step="LINE_HEIGHT.step"
        :value="s.lineHeight"
        @input="typed('lineHeight', $event, false)"
        @change="typed('lineHeight', $event, true)"
      />
    </label>
    <label class="field">
      <span>Tracking</span>
      <input
        type="number"
        class="number"
        :min="TRACKING.min"
        :max="TRACKING.max"
        :step="TRACKING.step"
        :value="s.trackingEm"
        @input="typed('trackingEm', $event, false)"
        @change="typed('trackingEm', $event, true)"
      />
    </label>

    <div class="field" role="group" aria-label="Alignment">
      <button
        v-for="a in alignments"
        :key="a.value"
        type="button"
        class="button"
        :aria-pressed="s.align === a.value"
        @click="set({ align: a.value })"
      >
        {{ a.label }}
      </button>
    </div>

    <label class="field">
      <span>Text</span>
      <input
        type="color"
        aria-label="Text color"
        :value="textWell"
        @input="set({ textColor: colorFrom($event) })"
      />
    </label>
    <label class="field">
      <span>Background</span>
      <input
        type="color"
        aria-label="Background color"
        :value="backgroundWell"
        @input="set({ backgroundColor: colorFrom($event) })"
      />
    </label>
    <div class="field">
      <label :for="`${uid}-surface`">Surface</label>
      <select
        :id="`${uid}-surface`"
        :value="s.surface"
        @change="set({ surface: ($event.target as HTMLSelectElement).value as Surface })"
      >
        <option v-for="o in surfaces" :key="o.value" :value="o.value">{{ o.label }}</option>
      </select>
    </div>
    <button type="button" class="button" @click="store.reset(view)">Reset</button>
  </div>
</template>

<style scoped>
.controls {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-gap) 16px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--fm-border);
  font-size: 12px;
}
.field {
  display: flex;
  align-items: center;
  gap: 4px;
}
.field > span,
.field > label {
  color: var(--fm-muted);
}
.grow {
  flex: 1 1 160px;
}
.grow input {
  flex: 1;
  min-width: 0;
}
select,
input[type='text'],
.number {
  padding: 2px 4px;
  border: 1px solid var(--fm-control-border);
  border-radius: 4px;
  background: var(--fm-bg);
  color: inherit;
  font: inherit;
}
.number {
  width: 4.5em;
}
input[type='range'] {
  width: 96px;
}
input[type='color'] {
  width: 28px;
  height: 22px;
  padding: 0;
  border: 1px solid var(--fm-control-border);
  border-radius: 4px;
  background: transparent;
}
.button {
  padding: 2px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: 4px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.button[aria-pressed='true'] {
  border-color: var(--fm-accent);
  color: var(--fm-accent);
}
:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
</style>
