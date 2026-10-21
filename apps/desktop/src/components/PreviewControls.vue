<script setup lang="ts">
// The preview controls above a font list: sample text, size, line height,
// tracking, alignment, colors and the preview surface. Every change goes to
// the preview store for `view`, which every row of that list reads.
import { computed, useId } from 'vue';
import { t } from '../i18n';
import { SAMPLE_KINDS } from '../preview/sampleText';
import {
  LINE_HEIGHT,
  SIZE,
  TRACKING,
  usePreviewStore,
  type Alignment,
  type PreviewSettings,
  type Surface,
} from '../stores/preview';

const props = defineProps<{ view: string }>();

const store = usePreviewStore();
void store.loadPangrams();
// A select is labelled by `for`: wrapped in its label, its option texts would join the name.
const uid = useId();
const s = computed(() => store.settingsFor(props.view));

function set(patch: Partial<PreviewSettings>): void {
  store.update(props.view, patch);
}

type NumberKey = 'sizePx' | 'lineHeight' | 'trackingEm';
const BOUNDS = { sizePx: SIZE, lineHeight: LINE_HEIGHT, trackingEm: TRACKING } as const;
/** The number fields' setting keys, so the template holds no key literals. */
const KEY = { size: 'sizePx', lineHeight: 'lineHeight', tracking: 'trackingEm' } as const;

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

// The labels are read where they are shown (`preview.aligns.*`, `preview.surfaces.*`).
const alignments: Alignment[] = ['start', 'center', 'end'];
const surfaces: Surface[] = ['auto', 'light', 'dark'];

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
  <div class="controls" role="group" :aria-label="t('preview.settings')">
    <div class="field">
      <label :for="`${uid}-sample`">{{ t('preview.sample') }}</label>
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
      <label :for="`${uid}-language`">{{ t('preview.language') }}</label>
      <select
        :id="`${uid}-language`"
        :value="s.language"
        @change="
          set({
            language: ($event.target as HTMLSelectElement).value as PreviewSettings['language'],
          })
        "
      >
        <option v-for="lang in store.pangramLanguages" :key="lang.code" :value="lang.code">
          {{ lang.name }}
        </option>
      </select>
    </div>
    <label v-if="s.sample === 'custom'" class="field grow">
      <span>{{ t('preview.customText') }}</span>
      <input
        type="text"
        :value="s.customText"
        :placeholder="t('preview.customPlaceholder')"
        @input="set({ customText: ($event.target as HTMLInputElement).value })"
      />
    </label>

    <div class="field" role="group" aria-labelledby="fm-preview-size">
      <span id="fm-preview-size">{{ t('preview.size') }}</span>
      <input
        type="range"
        :aria-label="t('preview.sizeSlider')"
        :min="SIZE.min"
        :max="SIZE.max"
        :step="SIZE.step"
        :value="s.sizePx"
        @input="typed(KEY.size, $event, false)"
      />
      <input
        type="number"
        class="number"
        :aria-label="t('preview.sizeInPixels')"
        :min="SIZE.min"
        :max="SIZE.max"
        :step="SIZE.step"
        :value="s.sizePx"
        @input="typed(KEY.size, $event, false)"
        @change="typed(KEY.size, $event, true)"
      />
    </div>
    <label class="field">
      <span>{{ t('preview.lineHeight') }}</span>
      <input
        type="number"
        class="number"
        :min="LINE_HEIGHT.min"
        :max="LINE_HEIGHT.max"
        :step="LINE_HEIGHT.step"
        :value="s.lineHeight"
        @input="typed(KEY.lineHeight, $event, false)"
        @change="typed(KEY.lineHeight, $event, true)"
      />
    </label>
    <label class="field">
      <span>{{ t('preview.tracking') }}</span>
      <input
        type="number"
        class="number"
        :min="TRACKING.min"
        :max="TRACKING.max"
        :step="TRACKING.step"
        :value="s.trackingEm"
        @input="typed(KEY.tracking, $event, false)"
        @change="typed(KEY.tracking, $event, true)"
      />
    </label>

    <div class="field" role="group" :aria-label="t('preview.alignment')">
      <button
        v-for="a in alignments"
        :key="a"
        type="button"
        class="button"
        :aria-pressed="s.align === a"
        @click="set({ align: a })"
      >
        {{ t(`preview.aligns.${a}`) }}
      </button>
    </div>

    <label class="field">
      <span>{{ t('preview.text') }}</span>
      <input
        type="color"
        :aria-label="t('preview.textColor')"
        :value="textWell"
        @input="set({ textColor: colorFrom($event) })"
      />
    </label>
    <label class="field">
      <span>{{ t('preview.background') }}</span>
      <input
        type="color"
        :aria-label="t('preview.backgroundColor')"
        :value="backgroundWell"
        @input="set({ backgroundColor: colorFrom($event) })"
      />
    </label>
    <div class="field">
      <label :for="`${uid}-surface`">{{ t('preview.surface') }}</label>
      <select
        :id="`${uid}-surface`"
        :value="s.surface"
        @change="set({ surface: ($event.target as HTMLSelectElement).value as Surface })"
      >
        <option v-for="o in surfaces" :key="o" :value="o">{{ t(`preview.surfaces.${o}`) }}</option>
      </select>
    </div>
    <button type="button" class="button" @click="store.reset(view)">{{ t('common.reset') }}</button>
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
